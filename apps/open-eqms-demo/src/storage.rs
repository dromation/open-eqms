use open_eqms_event_engine::errors::{EventEngineError, EventEngineResult, MetadataError};
use open_eqms_event_engine::storage::StorageProvider as EventStorageProvider;
use open_eqms_event_engine::types::{
    AppendSequence, EventId, EventRecord, EventTypeDefinition, EventTypeRef,
};
use open_eqms_object_runtime::errors::{ObjectRuntimeError, ObjectRuntimeResult};
use open_eqms_object_runtime::storage::StorageProvider as ObjectStorageProvider;
use open_eqms_object_runtime::types::{
    ObjectId, ObjectRecord, ObjectTypeDefinition, ObjectTypeRef, Version,
};
use open_eqms_runtime_contracts::UnitOfWork;
use open_eqms_transaction_engine::crypto::CryptographicProvider;
use open_eqms_transaction_engine::errors::{TransactionEngineError, TransactionEngineResult};
use open_eqms_transaction_engine::storage::{PreparedTransaction, TransactionStore};
use open_eqms_transaction_engine::types::{
    AppendIndex, TransactionRecord, TransactionSequence, TransactionTimestamp,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Default)]
pub struct DemoObjectAndTransactionStore {
    inner: Arc<Mutex<DemoObjectAndTransactionState>>,
}

#[derive(Clone)]
struct DemoObjectAndTransactionState {
    object_types: BTreeMap<ObjectTypeRef, ObjectTypeDefinition>,
    objects: BTreeMap<ObjectId, ObjectRecord>,
    pending_units: BTreeMap<UnitOfWork, PendingUnit>,
    transactions: BTreeMap<open_eqms_transaction_engine::types::TransactionId, TransactionRecord>,
    transaction_order: BTreeMap<AppendIndex, open_eqms_transaction_engine::types::TransactionId>,
    next_append_index: u64,
    next_transaction_sequence: u64,
    fail_next_transaction_append: bool,
    fail_next_transaction_stage: bool,
    fail_next_transaction_commit: bool,
}

impl Default for DemoObjectAndTransactionState {
    fn default() -> Self {
        Self {
            object_types: BTreeMap::new(),
            objects: BTreeMap::new(),
            pending_units: BTreeMap::new(),
            transactions: BTreeMap::new(),
            transaction_order: BTreeMap::new(),
            next_append_index: 1,
            next_transaction_sequence: 1,
            fail_next_transaction_append: false,
            fail_next_transaction_stage: false,
            fail_next_transaction_commit: false,
        }
    }
}

#[derive(Clone, Default)]
struct PendingUnit {
    object_replacements: Vec<(ObjectRecord, Version)>,
    transactions: Vec<PreparedTransaction>,
}

impl DemoObjectAndTransactionStore {
    pub fn begin_shared_unit_of_work(&self, unit_of_work: &UnitOfWork) -> Result<(), String> {
        let mut inner = self.lock_string()?;
        inner.pending_units.entry(unit_of_work.clone()).or_default();
        Ok(())
    }

    pub fn commit_shared_unit_of_work<C>(
        &self,
        unit_of_work: &UnitOfWork,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<()>
    where
        C: CryptographicProvider,
    {
        let mut inner = self.lock_transaction()?;
        let backup = inner.clone();
        let result = inner.commit_shared_unit_of_work(unit_of_work, cryptographic_provider);
        if result.is_err() {
            *inner = backup;
        }
        result
    }

    pub fn rollback_shared_unit_of_work(&self, unit_of_work: &UnitOfWork) -> Result<(), String> {
        let mut inner = self.lock_string()?;
        inner
            .pending_units
            .remove(unit_of_work)
            .ok_or_else(|| format!("unit of work is not active: {unit_of_work}"))?;
        Ok(())
    }

    pub fn fail_next_transaction_append(&self) {
        self.inner
            .lock()
            .expect("demo store lock poisoned")
            .fail_next_transaction_append = true;
    }

    pub fn fail_next_transaction_stage(&self) {
        self.inner
            .lock()
            .expect("demo store lock poisoned")
            .fail_next_transaction_stage = true;
    }

    pub fn fail_next_transaction_commit(&self) {
        self.inner
            .lock()
            .expect("demo store lock poisoned")
            .fail_next_transaction_commit = true;
    }

    pub fn object_count(&self) -> usize {
        self.inner
            .lock()
            .expect("demo store lock poisoned")
            .objects
            .len()
    }

    pub fn transaction_count(&self) -> usize {
        self.inner
            .lock()
            .expect("demo store lock poisoned")
            .transactions
            .len()
    }

    fn lock_string(&self) -> Result<MutexGuard<'_, DemoObjectAndTransactionState>, String> {
        self.inner
            .lock()
            .map_err(|_| "demo object/transaction store lock poisoned".to_owned())
    }

    fn lock_object(&self) -> ObjectRuntimeResult<MutexGuard<'_, DemoObjectAndTransactionState>> {
        self.inner
            .lock()
            .map_err(|_| object_storage_failure("demo object/transaction store lock poisoned"))
    }

    fn lock_transaction(
        &self,
    ) -> TransactionEngineResult<MutexGuard<'_, DemoObjectAndTransactionState>> {
        self.inner
            .lock()
            .map_err(|_| transaction_storage_failure("demo object/transaction store lock poisoned"))
    }
}

impl DemoObjectAndTransactionState {
    fn commit_shared_unit_of_work<C>(
        &mut self,
        unit_of_work: &UnitOfWork,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<()>
    where
        C: CryptographicProvider,
    {
        let pending = self.pending_units.remove(unit_of_work).ok_or_else(|| {
            transaction_storage_failure(format!("unit of work is not active: {unit_of_work}"))
        })?;

        if self.fail_next_transaction_commit {
            self.fail_next_transaction_commit = false;
            return Err(transaction_storage_failure(
                "injected transaction commit failure",
            ));
        }

        self.validate_object_replacements(&pending)?;

        for prepared in pending.transactions {
            self.finalize_and_insert(prepared, cryptographic_provider)?;
        }
        for (record, _) in pending.object_replacements {
            self.objects.insert(record.id.clone(), record);
        }

        Ok(())
    }

    fn validate_object_replacements(&self, pending: &PendingUnit) -> TransactionEngineResult<()> {
        for (record, expected_version) in &pending.object_replacements {
            let current = self.objects.get(&record.id).ok_or_else(|| {
                transaction_storage_failure(format!("object not found: {}", record.id))
            })?;
            if current.version != *expected_version {
                return Err(transaction_storage_failure(format!(
                    "object version conflict on {}: expected {}, actual {}",
                    record.id,
                    expected_version.value(),
                    current.version.value()
                )));
            }
        }
        Ok(())
    }

    fn transaction_identity_exists(
        &self,
        transaction_id: &open_eqms_transaction_engine::types::TransactionId,
    ) -> bool {
        self.transactions.contains_key(transaction_id)
            || self.pending_units.values().any(|pending| {
                pending
                    .transactions
                    .iter()
                    .any(|prepared| &prepared.id == transaction_id)
            })
    }

    fn finalize_and_insert<C>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord>
    where
        C: CryptographicProvider,
    {
        let backup = self.clone();
        let result = self.finalize_and_insert_inner(prepared, cryptographic_provider);
        if result.is_err() {
            *self = backup;
        }
        result
    }

    fn finalize_and_insert_inner<C>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord>
    where
        C: CryptographicProvider,
    {
        if self.transaction_identity_exists(&prepared.id) {
            return Err(TransactionEngineError::DuplicateIdentity {
                transaction_id: prepared.id,
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
            let hash = cryptographic_provider
                .hash(demo_canonical_transaction_bytes(&record).as_bytes())
                .map_err(
                    |message| TransactionEngineError::CryptographicProviderFailure { message },
                )?;
            record.transaction_hash = Some(hash);
        }

        self.transaction_order
            .insert(record.append_index, record.id.clone());
        self.transactions.insert(record.id.clone(), record.clone());
        Ok(record)
    }
}

impl ObjectStorageProvider for DemoObjectAndTransactionStore {
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

    fn begin_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        self.begin_shared_unit_of_work(unit_of_work)
            .map_err(object_storage_failure)
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

    fn commit_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_object()?;
        let pending = inner
            .pending_units
            .get(unit_of_work)
            .cloned()
            .ok_or_else(|| {
                object_storage_failure(format!("unit of work is not active: {unit_of_work}"))
            })?;
        if !pending.transactions.is_empty() {
            return Err(object_storage_failure(
                "shared transaction staging requires commit_shared_unit_of_work",
            ));
        }
        for (record, expected_version) in pending.object_replacements {
            replace_committed_object(&mut inner, record, expected_version)?;
        }
        inner.pending_units.remove(unit_of_work);
        Ok(())
    }

    fn rollback_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        self.rollback_shared_unit_of_work(unit_of_work)
            .map_err(object_storage_failure)
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

impl TransactionStore for DemoObjectAndTransactionStore {
    fn transaction_exists(
        &self,
        transaction_id: &open_eqms_transaction_engine::types::TransactionId,
    ) -> TransactionEngineResult<bool> {
        Ok(self
            .lock_transaction()?
            .transaction_identity_exists(transaction_id))
    }

    fn get_transaction(
        &self,
        transaction_id: &open_eqms_transaction_engine::types::TransactionId,
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
        if inner.fail_next_transaction_append {
            inner.fail_next_transaction_append = false;
            return Err(transaction_storage_failure(
                "injected transaction append failure",
            ));
        }
        inner.finalize_and_insert(prepared, cryptographic_provider)
    }

    fn stage_transaction(
        &mut self,
        unit_of_work: &UnitOfWork,
        prepared: PreparedTransaction,
    ) -> TransactionEngineResult<()> {
        let mut inner = self.lock_transaction()?;
        if inner.fail_next_transaction_stage {
            inner.fail_next_transaction_stage = false;
            return Err(transaction_storage_failure(
                "injected transaction stage failure",
            ));
        }
        if inner.transaction_identity_exists(&prepared.id) {
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
pub struct DemoEventStore {
    inner: Arc<Mutex<DemoEventState>>,
}

#[derive(Clone, Default)]
struct DemoEventState {
    types: BTreeMap<EventTypeRef, EventTypeDefinition>,
    events: BTreeMap<EventId, EventRecord>,
    sequence_order: BTreeMap<AppendSequence, EventId>,
    fail_next_append: bool,
}

impl DemoEventStore {
    pub fn fail_next_append(&self) {
        self.inner
            .lock()
            .expect("demo event store lock poisoned")
            .fail_next_append = true;
    }

    pub fn event_count(&self) -> usize {
        self.inner
            .lock()
            .expect("demo event store lock poisoned")
            .events
            .len()
    }

    fn lock_event(&self) -> EventEngineResult<MutexGuard<'_, DemoEventState>> {
        self.inner
            .lock()
            .map_err(|_| event_storage_failure("demo event store lock poisoned"))
    }
}

impl EventStorageProvider for DemoEventStore {
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

fn replace_committed_object(
    inner: &mut DemoObjectAndTransactionState,
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

fn demo_canonical_transaction_bytes(record: &TransactionRecord) -> String {
    format!(
        "{:?}|append={}|sequence={:?}|server_receipt={:?}",
        record,
        record.append_index.value(),
        record.transaction_sequence.map(TransactionSequence::value),
        record
            .server_receipt_time
            .as_ref()
            .map(TransactionTimestamp::as_str)
    )
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
