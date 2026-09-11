use super::*;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};

use open_eqms_event_engine::errors::{EventEngineError, EventEngineResult, MetadataError};
use open_eqms_event_engine::storage::{
    AppendSequenceGenerator, EventIdGenerator, StorageProvider as EventStorageProvider,
};
use open_eqms_event_engine::types::{
    AppendSequence, EventId, EventRecord, EventTypeDefinition, EventTypeRef,
};
use open_eqms_object_runtime::errors::{ObjectRuntimeError, ObjectRuntimeResult};
use open_eqms_object_runtime::storage::{
    ObjectIdGenerator, StorageProvider as ObjectStorageProvider,
};
use open_eqms_object_runtime::types::{
    LifecycleState, ObjectId, ObjectRecord, ObjectTypeDefinition, ObjectTypeRef, OwnershipInfo,
    PermissionScopeRef, PropertyDefinition, PropertyValue, PropertyValueKind, Relation,
    StructuralConstraints, Version,
};
use open_eqms_runtime_contracts::UnitOfWork;
use open_eqms_transaction_engine::crypto::CryptographicProvider;
use open_eqms_transaction_engine::errors::{TransactionEngineError, TransactionEngineResult};
use open_eqms_transaction_engine::storage::{
    PreparedTransaction, TransactionIdGenerator, TransactionStore,
};
use open_eqms_transaction_engine::types::{
    AppendIndex, PriorReference, TransactionHash, TransactionId, TransactionLevel,
    TransactionRecord, TransactionSequence, TransactionTimestamp,
};

type TestEngine = ProcessEngine<
    ObjectTransactionStore,
    FixedObjectIds,
    EventStore,
    FixedEventIds,
    FixedSequences,
    FixedTransactionIds,
    DeterministicCrypto,
>;

#[derive(Clone, Default)]
struct ObjectTransactionStore {
    inner: Arc<Mutex<ObjectTransactionState>>,
}

#[derive(Clone)]
struct ObjectTransactionState {
    object_types: BTreeMap<ObjectTypeRef, ObjectTypeDefinition>,
    objects: BTreeMap<ObjectId, ObjectRecord>,
    pending_units: BTreeMap<UnitOfWork, PendingUnit>,
    transactions: BTreeMap<TransactionId, TransactionRecord>,
    transaction_order: BTreeMap<AppendIndex, TransactionId>,
    next_append_index: u64,
    next_transaction_sequence: u64,
    fail_next_commit: bool,
}

impl Default for ObjectTransactionState {
    fn default() -> Self {
        Self {
            object_types: BTreeMap::new(),
            objects: BTreeMap::new(),
            pending_units: BTreeMap::new(),
            transactions: BTreeMap::new(),
            transaction_order: BTreeMap::new(),
            next_append_index: 1,
            next_transaction_sequence: 1,
            fail_next_commit: false,
        }
    }
}

#[derive(Clone, Default)]
struct PendingUnit {
    object_replacements: Vec<(ObjectRecord, Version)>,
    transactions: Vec<PreparedTransaction>,
}

impl ObjectTransactionStore {
    fn fail_next_commit(&self) {
        self.inner
            .lock()
            .expect("object/transaction store lock poisoned")
            .fail_next_commit = true;
    }

    fn transaction_count(&self) -> usize {
        self.inner
            .lock()
            .expect("object/transaction store lock poisoned")
            .transactions
            .len()
    }

    fn lock_object(&self) -> ObjectRuntimeResult<MutexGuard<'_, ObjectTransactionState>> {
        self.inner
            .lock()
            .map_err(|_| object_storage_failure("object/transaction store lock poisoned"))
    }

    fn lock_transaction(&self) -> TransactionEngineResult<MutexGuard<'_, ObjectTransactionState>> {
        self.inner
            .lock()
            .map_err(|_| transaction_storage_failure("object/transaction store lock poisoned"))
    }

    fn lock_process(&self) -> ProcessEngineResult<MutexGuard<'_, ObjectTransactionState>> {
        self.inner
            .lock()
            .map_err(|_| ProcessEngineError::UnitOfWorkFailure {
                message: "object/transaction store lock poisoned".to_owned(),
            })
    }
}

impl SharedTransitionStore for ObjectTransactionStore {
    fn begin_shared_transition(&self, unit_of_work: &UnitOfWork) -> ProcessEngineResult<()> {
        let mut inner = self.lock_process()?;
        inner.pending_units.entry(unit_of_work.clone()).or_default();
        Ok(())
    }

    fn commit_shared_transition<C>(
        &self,
        unit_of_work: &UnitOfWork,
        cryptographic_provider: &mut C,
    ) -> ProcessEngineResult<()>
    where
        C: CryptographicProvider,
    {
        let mut inner = self.lock_process()?;
        let backup = inner.clone();
        let result = inner.commit_shared_transition(unit_of_work, cryptographic_provider);
        if result.is_err() {
            *inner = backup;
        }
        result
    }

    fn rollback_shared_transition(&self, unit_of_work: &UnitOfWork) -> ProcessEngineResult<()> {
        let mut inner = self.lock_process()?;
        inner.pending_units.remove(unit_of_work).ok_or_else(|| {
            ProcessEngineError::UnitOfWorkFailure {
                message: format!("unit of work is not active: {unit_of_work}"),
            }
        })?;
        Ok(())
    }
}

impl ObjectTransactionState {
    fn commit_shared_transition<C>(
        &mut self,
        unit_of_work: &UnitOfWork,
        cryptographic_provider: &mut C,
    ) -> ProcessEngineResult<()>
    where
        C: CryptographicProvider,
    {
        let pending = self.pending_units.remove(unit_of_work).ok_or_else(|| {
            ProcessEngineError::UnitOfWorkFailure {
                message: format!("unit of work is not active: {unit_of_work}"),
            }
        })?;

        if self.fail_next_commit {
            self.fail_next_commit = false;
            return Err(ProcessEngineError::UnitOfWorkCommitFailure {
                message: "injected transaction commit failure".to_owned(),
            });
        }

        for (record, expected_version) in &pending.object_replacements {
            let current =
                self.objects
                    .get(&record.id)
                    .ok_or_else(|| ProcessEngineError::ObjectNotFound {
                        object_id: record.id.clone(),
                    })?;
            if current.version != *expected_version {
                return Err(ProcessEngineError::UnitOfWorkCommitFailure {
                    message: format!(
                        "object version conflict on {}: expected {}, actual {}",
                        record.id,
                        expected_version.value(),
                        current.version.value()
                    ),
                });
            }
        }

        for prepared in pending.transactions {
            let record = self.finalize_transaction(prepared, cryptographic_provider)?;
            self.transaction_order
                .insert(record.append_index, record.id.clone());
            self.transactions.insert(record.id.clone(), record);
        }
        for (record, _) in pending.object_replacements {
            self.objects.insert(record.id.clone(), record);
        }

        Ok(())
    }

    fn transaction_exists_anywhere(&self, transaction_id: &TransactionId) -> bool {
        self.transactions.contains_key(transaction_id)
            || self.pending_units.values().any(|pending| {
                pending
                    .transactions
                    .iter()
                    .any(|prepared| &prepared.id == transaction_id)
            })
    }

    fn finalize_transaction<C>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> ProcessEngineResult<TransactionRecord>
    where
        C: CryptographicProvider,
    {
        if self.transaction_exists_anywhere(&prepared.id) {
            return Err(ProcessEngineError::TransactionEngineFailure {
                source: TransactionEngineError::DuplicateIdentity {
                    transaction_id: prepared.id,
                },
            });
        }

        let append_index = AppendIndex::new(self.next_append_index);
        self.next_append_index += 1;
        let transaction_sequence = if prepared.level.requires_level2_fields() {
            let sequence = TransactionSequence::new(self.next_transaction_sequence);
            self.next_transaction_sequence += 1;
            Some(sequence)
        } else {
            None
        };

        let mut record = TransactionRecord::new(
            prepared.id,
            prepared.schema_version,
            prepared.level,
            prepared.object_id,
            prepared.operation,
            prepared.old_value,
            prepared.new_value,
            prepared.actor,
            prepared.device,
            prepared.site,
            prepared.edit_timestamp,
            prepared.base_version,
            prepared.resulting_version,
            append_index,
            transaction_sequence,
            prepared.server_receipt_time,
            prepared.prior_reference,
            prepared.prior_transaction_hash,
            None,
            prepared.rule_evaluation,
            prepared.signer,
            prepared.signer_role,
            prepared.signature_meaning,
            prepared.authentication_evidence,
            prepared.signed_revision,
            prepared.reason,
            prepared.signing_timestamp,
            prepared.signature,
        );

        if record.level.requires_level2_fields() {
            let canonical = format!(
                "{:?}|append={}|sequence={:?}",
                record,
                record.append_index.value(),
                record.transaction_sequence.map(TransactionSequence::value)
            );
            let hash = cryptographic_provider
                .hash(canonical.as_bytes())
                .map_err(|message| ProcessEngineError::TransactionEngineFailure {
                    source: TransactionEngineError::CryptographicProviderFailure { message },
                })?;
            record.transaction_hash = Some(hash);
        }

        Ok(record)
    }
}

impl ObjectStorageProvider for ObjectTransactionStore {
    fn get_type(
        &self,
        object_type: &ObjectTypeRef,
    ) -> ObjectRuntimeResult<Option<ObjectTypeDefinition>> {
        Ok(self.lock_object()?.object_types.get(object_type).cloned())
    }

    fn put_type(&mut self, definition: ObjectTypeDefinition) -> ObjectRuntimeResult<()> {
        self.lock_object()?
            .object_types
            .insert(definition.type_ref.clone(), definition);
        Ok(())
    }

    fn get_object(&self, object_id: &ObjectId) -> ObjectRuntimeResult<Option<ObjectRecord>> {
        Ok(self.lock_object()?.objects.get(object_id).cloned())
    }

    fn insert_object(&mut self, record: ObjectRecord) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_object()?;
        if inner.objects.contains_key(&record.id) {
            return Err(ObjectRuntimeError::DuplicateIdentity {
                object_id: record.id,
            });
        }
        inner.objects.insert(record.id.clone(), record);
        Ok(())
    }

    fn replace_object(
        &mut self,
        record: ObjectRecord,
        expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_object()?;
        replace_committed_object(&mut inner, record, expected_version)
    }

    fn replace_object_in_unit_of_work(
        &mut self,
        unit_of_work: &UnitOfWork,
        record: ObjectRecord,
        expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_object()?;
        let current =
            inner
                .objects
                .get(&record.id)
                .ok_or_else(|| ObjectRuntimeError::ObjectNotFound {
                    object_id: record.id.clone(),
                })?;
        if current.version != expected_version {
            return Err(ObjectRuntimeError::VersionConflict {
                object_id: record.id,
                expected: expected_version,
                actual: current.version,
            });
        }

        let pending = inner.pending_units.get_mut(unit_of_work).ok_or_else(|| {
            object_storage_failure(format!("unit of work is not active: {unit_of_work}"))
        })?;
        pending.object_replacements.push((record, expected_version));
        Ok(())
    }

    fn object_exists(&self, object_id: &ObjectId) -> ObjectRuntimeResult<bool> {
        Ok(self.lock_object()?.objects.contains_key(object_id))
    }

    fn visit_objects(
        &self,
        visitor: &mut dyn FnMut(&ObjectRecord) -> ObjectRuntimeResult<()>,
    ) -> ObjectRuntimeResult<()> {
        for record in self.lock_object()?.objects.values() {
            visitor(record)?;
        }
        Ok(())
    }
}

impl TransactionStore for ObjectTransactionStore {
    fn transaction_exists(&self, transaction_id: &TransactionId) -> TransactionEngineResult<bool> {
        Ok(self
            .lock_transaction()?
            .transaction_exists_anywhere(transaction_id))
    }

    fn get_transaction(
        &self,
        transaction_id: &TransactionId,
    ) -> TransactionEngineResult<Option<TransactionRecord>> {
        Ok(self
            .lock_transaction()?
            .transactions
            .get(transaction_id)
            .cloned())
    }

    fn append_transaction<C>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord>
    where
        C: CryptographicProvider,
    {
        let mut inner = self.lock_transaction()?;
        let mut process_inner = inner.clone();
        let record = process_inner
            .finalize_transaction(prepared, cryptographic_provider)
            .map_err(|error| match error {
                ProcessEngineError::TransactionEngineFailure { source } => source,
                other => transaction_storage_failure(other.to_string()),
            })?;
        process_inner
            .transaction_order
            .insert(record.append_index, record.id.clone());
        process_inner
            .transactions
            .insert(record.id.clone(), record.clone());
        *inner = process_inner;
        Ok(record)
    }

    fn stage_transaction(
        &mut self,
        unit_of_work: &UnitOfWork,
        prepared: PreparedTransaction,
    ) -> TransactionEngineResult<()> {
        let mut inner = self.lock_transaction()?;
        if inner.transaction_exists_anywhere(&prepared.id) {
            return Err(TransactionEngineError::DuplicateIdentity {
                transaction_id: prepared.id,
            });
        }
        let pending = inner.pending_units.get_mut(unit_of_work).ok_or_else(|| {
            transaction_storage_failure(format!("unit of work is not active: {unit_of_work}"))
        })?;
        pending.transactions.push(prepared);
        Ok(())
    }

    fn read_append_index_range(
        &self,
        start: AppendIndex,
        max: usize,
    ) -> TransactionEngineResult<Vec<TransactionRecord>> {
        let inner = self.lock_transaction()?;
        Ok(inner
            .transaction_order
            .range(start..)
            .take(max)
            .filter_map(|(_, transaction_id)| inner.transactions.get(transaction_id))
            .cloned()
            .collect())
    }
}

#[derive(Clone, Default)]
struct EventStore {
    inner: Arc<Mutex<EventState>>,
}

#[derive(Clone, Default)]
struct EventState {
    types: BTreeMap<EventTypeRef, EventTypeDefinition>,
    events: BTreeMap<EventId, EventRecord>,
    sequence_order: BTreeMap<AppendSequence, EventId>,
    fail_next_append: bool,
}

impl EventStore {
    fn fail_next_append(&self) {
        self.inner
            .lock()
            .expect("event store lock poisoned")
            .fail_next_append = true;
    }

    fn event_count(&self) -> usize {
        self.inner
            .lock()
            .expect("event store lock poisoned")
            .events
            .len()
    }

    fn lock_event(&self) -> EventEngineResult<MutexGuard<'_, EventState>> {
        self.inner
            .lock()
            .map_err(|_| event_storage_failure("event store lock poisoned"))
    }
}

impl EventStorageProvider for EventStore {
    fn get_type(
        &self,
        event_type: &EventTypeRef,
    ) -> EventEngineResult<Option<EventTypeDefinition>> {
        Ok(self.lock_event()?.types.get(event_type).cloned())
    }

    fn insert_type(&mut self, definition: EventTypeDefinition) -> EventEngineResult<()> {
        let mut inner = self.lock_event()?;
        if inner.types.contains_key(&definition.type_ref) {
            return Err(EventEngineError::MalformedMetadata {
                failure: MetadataError::EventTypeAlreadyRegistered {
                    event_type: definition.type_ref,
                },
            });
        }
        inner.types.insert(definition.type_ref.clone(), definition);
        Ok(())
    }

    fn get_event(&self, event_id: &EventId) -> EventEngineResult<Option<EventRecord>> {
        Ok(self.lock_event()?.events.get(event_id).cloned())
    }

    fn event_exists(&self, event_id: &EventId) -> EventEngineResult<bool> {
        Ok(self.lock_event()?.events.contains_key(event_id))
    }

    fn append_event(&mut self, record: EventRecord) -> EventEngineResult<()> {
        let mut inner = self.lock_event()?;
        if inner.fail_next_append {
            inner.fail_next_append = false;
            return Err(event_storage_failure("injected event append failure"));
        }
        if inner.events.contains_key(&record.id) {
            return Err(EventEngineError::DuplicateIdentity {
                event_id: record.id,
            });
        }
        if inner.sequence_order.contains_key(&record.append_sequence) {
            return Err(EventEngineError::AppendSequenceConflict {
                append_sequence: record.append_sequence,
            });
        }

        inner
            .sequence_order
            .insert(record.append_sequence, record.id.clone());
        inner.events.insert(record.id.clone(), record);
        Ok(())
    }

    fn read_sequence_range(
        &self,
        start: AppendSequence,
        max: usize,
    ) -> EventEngineResult<Vec<EventRecord>> {
        let inner = self.lock_event()?;
        Ok(inner
            .sequence_order
            .range(start..)
            .take(max)
            .filter_map(|(_, event_id)| inner.events.get(event_id))
            .cloned()
            .collect())
    }
}

#[derive(Clone)]
struct FixedObjectIds {
    ids: Vec<ObjectId>,
    index: usize,
}

impl FixedObjectIds {
    fn new(ids: &[&str]) -> Self {
        Self {
            ids: ids.iter().copied().map(ObjectId::new).collect(),
            index: 0,
        }
    }
}

impl ObjectIdGenerator for FixedObjectIds {
    fn next_id(&mut self) -> ObjectId {
        let id = self
            .ids
            .get(self.index)
            .cloned()
            .unwrap_or_else(|| ObjectId::new(format!("object-extra-{}", self.index + 1)));
        self.index += 1;
        id
    }
}

#[derive(Clone)]
struct FixedEventIds {
    ids: Vec<EventId>,
    index: usize,
}

impl FixedEventIds {
    fn new(ids: &[&str]) -> Self {
        Self {
            ids: ids.iter().copied().map(EventId::new).collect(),
            index: 0,
        }
    }
}

impl EventIdGenerator for FixedEventIds {
    fn next_id(&mut self) -> EventId {
        let id = self
            .ids
            .get(self.index)
            .cloned()
            .unwrap_or_else(|| EventId::new(format!("event-extra-{}", self.index + 1)));
        self.index += 1;
        id
    }
}

#[derive(Clone)]
struct FixedSequences {
    next: u64,
}

impl FixedSequences {
    fn new() -> Self {
        Self { next: 1 }
    }
}

impl AppendSequenceGenerator for FixedSequences {
    fn next_sequence(&mut self) -> AppendSequence {
        let sequence = AppendSequence::new(self.next);
        self.next += 1;
        sequence
    }
}

#[derive(Clone)]
struct FixedTransactionIds {
    ids: Vec<TransactionId>,
    index: usize,
}

impl FixedTransactionIds {
    fn new(ids: &[&str]) -> Self {
        Self {
            ids: ids.iter().copied().map(TransactionId::new).collect(),
            index: 0,
        }
    }
}

impl TransactionIdGenerator for FixedTransactionIds {
    fn next_id(&mut self) -> TransactionId {
        let id = self
            .ids
            .get(self.index)
            .cloned()
            .unwrap_or_else(|| TransactionId::new(format!("tx-extra-{}", self.index + 1)));
        self.index += 1;
        id
    }
}

#[derive(Clone, Default)]
struct DeterministicCrypto;

impl CryptographicProvider for DeterministicCrypto {
    fn hash(&mut self, canonical_content: &[u8]) -> Result<TransactionHash, String> {
        Ok(TransactionHash::new(format!(
            "test-hash:{}",
            canonical_content.len()
        )))
    }

    fn verify(
        &mut self,
        _signed_revision: &open_eqms_transaction_engine::types::SignedRevision,
        _signer: &open_eqms_transaction_engine::types::SignerRef,
        _signature: &open_eqms_transaction_engine::types::CryptographicSignature,
    ) -> Result<bool, String> {
        Ok(true)
    }
}

#[test]
fn process_definition_registration_validates_graph() {
    let (engine, _, _) = test_engine();

    engine.register_process_definition(definition()).unwrap();
    assert_eq!(
        "asset-calibration",
        engine
            .get_process_definition(&definition_id(), Version::new(1))
            .unwrap()
            .id
            .as_str()
    );

    assert!(matches!(
        engine.register_process_definition(definition()),
        Err(ProcessEngineError::DuplicateDefinition { .. })
    ));
    assert!(matches!(
        engine.register_process_definition(ProcessDefinition::new(
            ProcessDefinitionId::new("bad-empty"),
            Version::new(1),
            BTreeSet::new(),
            Vec::new(),
            BTreeSet::new(),
        )),
        Err(ProcessEngineError::MalformedDefinition {
            failure: ProcessDefinitionValidationError::EmptyNodeSet
        })
    ));
    assert!(matches!(
        engine.register_process_definition(ProcessDefinition::new(
            ProcessDefinitionId::new("bad-dangling"),
            Version::new(1),
            BTreeSet::from([node("draft")]),
            vec![ProcessTransition::new(
                node("draft"),
                node("approved"),
                "approve"
            )],
            BTreeSet::from([node("draft")]),
        )),
        Err(ProcessEngineError::MalformedDefinition {
            failure: ProcessDefinitionValidationError::TransitionTargetNotDeclared { .. }
        })
    ));
    assert!(matches!(
        engine.register_process_definition(ProcessDefinition::new(
            ProcessDefinitionId::new("bad-duplicate"),
            Version::new(1),
            BTreeSet::from([node("draft"), node("approved")]),
            vec![
                ProcessTransition::new(node("draft"), node("approved"), "approve"),
                ProcessTransition::new(node("draft"), node("approved"), "approve-again"),
            ],
            BTreeSet::from([node("draft")]),
        )),
        Err(ProcessEngineError::MalformedDefinition {
            failure: ProcessDefinitionValidationError::DuplicateTransitionEdge { .. }
        })
    ));
}

#[test]
fn reserved_process_property_keys_are_deterministic() {
    let id = ProcessDefinitionId::new("asset-calibration");

    assert_eq!(
        "process.asset-calibration.current_node",
        current_node_property_key(&id)
    );
    assert_eq!(
        "process.asset-calibration.last_transaction_id",
        last_transaction_property_key(&id)
    );
}

#[test]
fn legal_entry_transition_updates_object_commits_level2_transaction_and_appends_event() {
    let (engine, object_transaction_store, event_store) = prepared_engine(true, true);
    let object_id = create_asset(&engine, BTreeMap::new());

    let outcome = engine
        .record_transition(transition_request(&object_id, "draft", "uow-001"))
        .unwrap();

    assert_eq!(None, outcome.prior_node);
    assert_eq!(node("draft"), outcome.new_node);
    assert_eq!("tx-001", outcome.transaction_id.as_str());
    assert_eq!(Some(EventId::new("event-001")), outcome.event_id);
    assert_eq!(
        TransitionConsistencyStatus::Complete,
        outcome.consistency_status
    );
    assert_eq!(1, object_transaction_store.transaction_count());
    assert_eq!(1, event_store.event_count());

    let object = engine.read_object(&object_id).unwrap();
    assert_eq!(Version::new(2), object.version);
    assert_eq!(
        Some(&PropertyValue::EnumValue("draft".to_owned())),
        object
            .properties
            .get(&current_node_property_key(&definition_id()))
    );
    assert_eq!(
        Some(&PropertyValue::Text {
            value: "tx-001".to_owned(),
            language: None,
        }),
        object
            .properties
            .get(&last_transaction_property_key(&definition_id()))
    );

    let transaction = engine
        .read_transaction_range(AppendIndex::first(), 10)
        .unwrap()
        .transactions
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(TransactionLevel::Level2, transaction.level);
    assert_eq!(
        PropertyValue::Text {
            value: "not-started".to_owned(),
            language: None,
        },
        transaction.old_value
    );
    assert_eq!(
        PropertyValue::EnumValue("draft".to_owned()),
        transaction.new_value
    );
    assert_eq!(Version::new(1), transaction.base_version);
    assert_eq!(Version::new(2), transaction.resulting_version);
    assert!(transaction.transaction_hash.is_some());
    assert_eq!(None, transaction.prior_reference);

    let event = engine.read_event(&EventId::new("event-001")).unwrap();
    assert_eq!(process_transition_event_type_ref(), event.event_type);
    assert_eq!(
        Some(&PropertyValue::Text {
            value: "asset-calibration".to_owned(),
            language: None,
        }),
        event.payload.get("definition_id")
    );
    assert_eq!(
        Some(&PropertyValue::Text {
            value: "not-started".to_owned(),
            language: None,
        }),
        event.payload.get("from_node")
    );
    assert_eq!(
        Some(&PropertyValue::Text {
            value: "draft".to_owned(),
            language: None,
        }),
        event.payload.get("to_node")
    );
}

#[test]
fn legal_transition_chains_to_prior_process_transaction() {
    let (engine, _, _) = prepared_engine(true, true);
    let object_id = create_asset(&engine, BTreeMap::new());

    engine
        .record_transition(transition_request(&object_id, "draft", "uow-001"))
        .unwrap();
    let outcome = engine
        .record_transition(transition_request(&object_id, "approved", "uow-002"))
        .unwrap();

    assert_eq!(Some(node("draft")), outcome.prior_node);
    assert_eq!("tx-002", outcome.transaction_id.as_str());

    let transactions = engine
        .read_transaction_range(AppendIndex::first(), 10)
        .unwrap()
        .transactions;
    assert_eq!(2, transactions.len());
    assert_eq!(None, transactions[0].prior_reference);
    assert_eq!(
        Some(PriorReference::Transaction(TransactionId::new("tx-001"))),
        transactions[1].prior_reference
    );
    assert_eq!(
        Some(&PropertyValue::Text {
            value: "tx-002".to_owned(),
            language: None,
        }),
        engine
            .read_object(&object_id)
            .unwrap()
            .properties
            .get(&last_transaction_property_key(&definition_id()))
    );
}

#[test]
fn illegal_transition_is_rejected_before_mutation_or_audit_append() {
    let (engine, object_transaction_store, event_store) = prepared_engine(true, true);
    let object_id = create_asset(&engine, BTreeMap::new());

    let result = engine.record_transition(transition_request(&object_id, "approved", "uow-001"));

    assert!(matches!(
        result,
        Err(ProcessEngineError::NoSuchTransitionEdge {
            from: None,
            to
        }) if to == node("approved")
    ));
    assert_eq!(
        Version::initial(),
        engine.read_object(&object_id).unwrap().version
    );
    assert_eq!(0, object_transaction_store.transaction_count());
    assert_eq!(0, event_store.event_count());
}

#[test]
fn unknown_current_state_is_rejected_before_mutation() {
    let (engine, object_transaction_store, event_store) = prepared_engine(false, true);
    let object_id = create_asset(
        &engine,
        BTreeMap::from([(
            current_node_property_key(&definition_id()),
            PropertyValue::EnumValue("orphan".to_owned()),
        )]),
    );

    let result = engine.record_transition(transition_request(&object_id, "approved", "uow-001"));

    assert!(matches!(
        result,
        Err(ProcessEngineError::UnknownCurrentState { recorded_value })
            if recorded_value == "orphan"
    ));
    assert_eq!(
        Version::initial(),
        engine.read_object(&object_id).unwrap().version
    );
    assert_eq!(0, object_transaction_store.transaction_count());
    assert_eq!(0, event_store.event_count());
}

#[test]
fn object_active_in_another_process_is_rejected_before_mutation() {
    let (engine, object_transaction_store, event_store) = test_engine();
    engine.register_process_definition(definition()).unwrap();
    engine
        .register_object_type(object_type(true, true, true))
        .unwrap();
    engine
        .register_event_type(process_transition_event_type_definition())
        .unwrap();
    let other_definition_id = ProcessDefinitionId::new("other-process");
    let object_id = create_asset(
        &engine,
        BTreeMap::from([(
            current_node_property_key(&other_definition_id),
            PropertyValue::EnumValue("active".to_owned()),
        )]),
    );

    let result = engine.record_transition(transition_request(&object_id, "draft", "uow-001"));

    assert!(matches!(
        result,
        Err(ProcessEngineError::ObjectAlreadyInDifferentProcess {
            active_definition_id
        }) if active_definition_id == other_definition_id
    ));
    assert_eq!(
        Version::initial(),
        engine.read_object(&object_id).unwrap().version
    );
    assert_eq!(0, object_transaction_store.transaction_count());
    assert_eq!(0, event_store.event_count());
}

#[test]
fn missing_process_property_declaration_is_rejected_before_mutation() {
    let (engine, object_transaction_store, event_store) = test_engine();
    engine.register_process_definition(definition()).unwrap();
    engine
        .register_object_type(object_type(true, false, false))
        .unwrap();
    engine
        .register_event_type(process_transition_event_type_definition())
        .unwrap();
    let object_id = create_asset(&engine, BTreeMap::new());

    let result = engine.record_transition(transition_request(&object_id, "draft", "uow-001"));

    assert!(matches!(
        result,
        Err(ProcessEngineError::ObjectTypeMissingProcessPropertyDeclaration {
            property_key,
            ..
        }) if property_key == last_transaction_property_key(&definition_id())
    ));
    assert_eq!(
        Version::initial(),
        engine.read_object(&object_id).unwrap().version
    );
    assert_eq!(0, object_transaction_store.transaction_count());
    assert_eq!(0, event_store.event_count());
}

#[test]
fn commit_failure_rolls_back_object_update_transaction_and_event() {
    let (engine, object_transaction_store, event_store) = prepared_engine(true, true);
    let object_id = create_asset(&engine, BTreeMap::new());
    object_transaction_store.fail_next_commit();

    let result = engine.record_transition(transition_request(&object_id, "draft", "uow-001"));

    assert!(matches!(
        result,
        Err(ProcessEngineError::UnitOfWorkCommitFailure { .. })
    ));
    let object = engine.read_object(&object_id).unwrap();
    assert_eq!(Version::initial(), object.version);
    assert!(!object
        .properties
        .contains_key(&current_node_property_key(&definition_id())));
    assert_eq!(0, object_transaction_store.transaction_count());
    assert_eq!(0, event_store.event_count());
}

#[test]
fn event_append_failure_reports_partial_completion_after_state_and_transaction_commit() {
    let (engine, object_transaction_store, event_store) = prepared_engine(true, true);
    let object_id = create_asset(&engine, BTreeMap::new());
    event_store.fail_next_append();

    let outcome = engine
        .record_transition(transition_request(&object_id, "draft", "uow-001"))
        .unwrap();

    assert_eq!(
        TransitionConsistencyStatus::PartiallyCompleted,
        outcome.consistency_status
    );
    assert_eq!(None, outcome.event_id);
    assert!(outcome
        .recovery_guidance
        .as_ref()
        .is_some_and(|guidance| guidance.contains("process.transitioned")));
    assert_eq!(1, object_transaction_store.transaction_count());
    assert_eq!(0, event_store.event_count());
    assert_eq!(
        Version::new(2),
        engine.read_object(&object_id).unwrap().version
    );
}

#[test]
fn deterministic_replay_produces_identical_outcomes_and_records() {
    let first = run_deterministic_transition();
    let second = run_deterministic_transition();

    assert_eq!(first, second);
}

#[test]
fn public_boundary_has_no_deferred_engine_or_service_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    for required in [
        "open-eqms-runtime-contracts",
        "open-eqms-object-runtime",
        "open-eqms-event-engine",
        "open-eqms-transaction-engine",
    ] {
        assert!(manifest.contains(required));
    }
    for forbidden in [
        "open-eqms-query-engine",
        "open-eqms-security",
        "open-eqms-demo",
        "tokio",
        "async-std",
        "rusqlite",
        "postgres",
        "content",
        "plugins",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "forbidden dependency found: {forbidden}"
        );
    }

    for (file, source) in [
        ("lib.rs", include_str!("lib.rs")),
        ("types.rs", include_str!("types.rs")),
        ("validation.rs", include_str!("validation.rs")),
        ("storage.rs", include_str!("storage.rs")),
        ("errors.rs", include_str!("errors.rs")),
    ] {
        for forbidden in [
            "SystemTime",
            "Instant",
            "chrono",
            "std::thread",
            "tokio",
            "async fn",
            "open_eqms_query_engine",
            "open_eqms_security",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} contains deferred or nondeterministic token {forbidden}"
            );
        }
    }
}

fn prepared_engine(
    restrict_current_nodes: bool,
    include_event_type: bool,
) -> (TestEngine, ObjectTransactionStore, EventStore) {
    let (engine, object_transaction_store, event_store) = test_engine();
    engine.register_process_definition(definition()).unwrap();
    engine
        .register_object_type(object_type(true, true, restrict_current_nodes))
        .unwrap();
    if include_event_type {
        engine
            .register_event_type(process_transition_event_type_definition())
            .unwrap();
    }
    (engine, object_transaction_store, event_store)
}

fn test_engine() -> (TestEngine, ObjectTransactionStore, EventStore) {
    let object_transaction_store = ObjectTransactionStore::default();
    let event_store = EventStore::default();
    let engine = ProcessEngine::new(
        object_transaction_store.clone(),
        FixedObjectIds::new(&["object-001", "object-002"]),
        event_store.clone(),
        FixedEventIds::new(&["event-001", "event-002"]),
        FixedSequences::new(),
        FixedTransactionIds::new(&["tx-001", "tx-002", "tx-003"]),
        DeterministicCrypto,
    );
    (engine, object_transaction_store, event_store)
}

fn run_deterministic_transition() -> (
    TransitionOutcome,
    ObjectRecord,
    Vec<TransactionRecord>,
    EventRecord,
) {
    let (engine, _, _) = prepared_engine(true, true);
    let object_id = create_asset(&engine, BTreeMap::new());
    let outcome = engine
        .record_transition(transition_request(&object_id, "draft", "uow-001"))
        .unwrap();
    let object = engine.read_object(&object_id).unwrap();
    let transactions = engine
        .read_transaction_range(AppendIndex::first(), 10)
        .unwrap()
        .transactions;
    let event = engine.read_event(&EventId::new("event-001")).unwrap();

    (outcome, object, transactions, event)
}

fn definition() -> ProcessDefinition {
    ProcessDefinition::new(
        definition_id(),
        Version::new(1),
        BTreeSet::from([node("draft"), node("approved"), node("closed")]),
        vec![
            ProcessTransition::new(node("draft"), node("approved"), "approve"),
            ProcessTransition::new(node("approved"), node("closed"), "close"),
        ],
        BTreeSet::from([node("draft")]),
    )
}

fn definition_id() -> ProcessDefinitionId {
    ProcessDefinitionId::new("asset-calibration")
}

fn node(value: &str) -> ProcessNodeId {
    ProcessNodeId::new(value)
}

fn object_type(
    include_own_process_keys: bool,
    include_last_transaction_key: bool,
    restrict_current_nodes: bool,
) -> ObjectTypeDefinition {
    let definition_id = definition_id();
    let mut properties = BTreeMap::from([(
        "serial".to_owned(),
        PropertyDefinition::new(
            "serial",
            PropertyValueKind::Text,
            true,
            StructuralConstraints::default(),
        ),
    )]);
    if include_own_process_keys {
        let mut current_constraints = StructuralConstraints::default();
        if restrict_current_nodes {
            current_constraints.allowed_enum_values = BTreeSet::from([
                "draft".to_owned(),
                "approved".to_owned(),
                "closed".to_owned(),
            ]);
        }
        let current_key = current_node_property_key(&definition_id);
        properties.insert(
            current_key.clone(),
            PropertyDefinition::new(
                current_key,
                PropertyValueKind::EnumValue,
                false,
                current_constraints,
            ),
        );
    }
    if include_last_transaction_key {
        let last_key = last_transaction_property_key(&definition_id);
        properties.insert(
            last_key.clone(),
            PropertyDefinition::new(
                last_key,
                PropertyValueKind::Text,
                false,
                StructuralConstraints::default(),
            ),
        );
    }

    let other_definition_id = ProcessDefinitionId::new("other-process");
    let other_current_key = current_node_property_key(&other_definition_id);
    properties.insert(
        other_current_key.clone(),
        PropertyDefinition::new(
            other_current_key,
            PropertyValueKind::EnumValue,
            false,
            StructuralConstraints::default(),
        ),
    );

    ObjectTypeDefinition::new(
        ObjectTypeRef::new("asset", 1),
        properties,
        BTreeMap::new(),
        None,
    )
}

fn create_asset(
    engine: &TestEngine,
    additional_properties: BTreeMap<String, PropertyValue>,
) -> ObjectId {
    let mut properties = BTreeMap::from([(
        "serial".to_owned(),
        PropertyValue::Text {
            value: "SN-001".to_owned(),
            language: None,
        },
    )]);
    properties.extend(additional_properties);
    engine
        .create_object(open_eqms_object_runtime::CreateObjectRequest {
            object_type: ObjectTypeRef::new("asset", 1),
            properties,
            relations: BTreeSet::<Relation>::new(),
            lifecycle_state: LifecycleState::new("active"),
            ownership: OwnershipInfo::new(ObjectId::new("owner-001"), BTreeSet::new()),
            permission_scope: PermissionScopeRef::new("scope-001"),
            retention_rule: None,
            external_references: BTreeSet::new(),
            comments_ref: None,
            attachments_ref: None,
        })
        .unwrap()
        .object_id
}

fn transition_request(object_id: &ObjectId, to: &str, unit_of_work: &str) -> TransitionRequest {
    TransitionRequest {
        object_id: object_id.clone(),
        definition_id: definition_id(),
        schema_version: Version::new(1),
        to: node(to),
        actor: open_eqms_transaction_engine::types::ActorRef::new("actor-001"),
        device: open_eqms_transaction_engine::types::DeviceRef::new("device-001"),
        site: Some(open_eqms_transaction_engine::types::SiteRef::new(
            "site-001",
        )),
        edit_timestamp: TransactionTimestamp::new("2026-09-09T10:00:00Z"),
        server_receipt_time: TransactionTimestamp::new("2026-09-09T10:00:05Z"),
        unit_of_work: UnitOfWork::new(unit_of_work),
    }
}

fn replace_committed_object(
    inner: &mut ObjectTransactionState,
    record: ObjectRecord,
    expected_version: Version,
) -> ObjectRuntimeResult<()> {
    let current =
        inner
            .objects
            .get(&record.id)
            .ok_or_else(|| ObjectRuntimeError::ObjectNotFound {
                object_id: record.id.clone(),
            })?;
    if current.version != expected_version {
        return Err(ObjectRuntimeError::VersionConflict {
            object_id: record.id,
            expected: expected_version,
            actual: current.version,
        });
    }

    inner.objects.insert(record.id.clone(), record);
    Ok(())
}

fn object_storage_failure(message: impl Into<String>) -> ObjectRuntimeError {
    ObjectRuntimeError::StorageProviderFailure {
        message: message.into(),
    }
}

fn transaction_storage_failure(message: impl Into<String>) -> TransactionEngineError {
    TransactionEngineError::StorageProviderFailure {
        message: message.into(),
    }
}

fn event_storage_failure(message: impl Into<String>) -> EventEngineError {
    EventEngineError::StorageProviderFailure {
        message: message.into(),
    }
}
