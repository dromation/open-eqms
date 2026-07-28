use super::*;
use crate::crypto::CryptographicProvider;
use crate::errors::{TransactionEngineError, TransactionEngineResult, ValidationError};
use crate::storage::{
    finalize_prepared_transaction, PreparedTransaction, TransactionIdGenerator, TransactionStore,
};
use crate::types::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

#[derive(Clone, Default)]
struct InMemoryTransactionStore {
    inner: Arc<Mutex<InMemoryTransactionStoreInner>>,
}

#[derive(Clone)]
struct InMemoryTransactionStoreInner {
    transactions: BTreeMap<TransactionId, TransactionRecord>,
    append_order: BTreeMap<AppendIndex, TransactionId>,
    next_append_index: u64,
    next_transaction_sequence: u64,
    active_units: BTreeSet<UnitOfWork>,
    staged_transactions: BTreeMap<UnitOfWork, Vec<PreparedTransaction>>,
    staged_object_writes: BTreeMap<UnitOfWork, Vec<String>>,
    committed_object_writes: BTreeSet<String>,
    fail_append: bool,
    fail_stage: bool,
    fail_append_index: bool,
    fail_sequence: bool,
}

impl Default for InMemoryTransactionStoreInner {
    fn default() -> Self {
        Self {
            transactions: BTreeMap::new(),
            append_order: BTreeMap::new(),
            next_append_index: 1,
            next_transaction_sequence: 1,
            active_units: BTreeSet::new(),
            staged_transactions: BTreeMap::new(),
            staged_object_writes: BTreeMap::new(),
            committed_object_writes: BTreeSet::new(),
            fail_append: false,
            fail_stage: false,
            fail_append_index: false,
            fail_sequence: false,
        }
    }
}

impl InMemoryTransactionStore {
    fn with_append_failure() -> Self {
        let store = Self::default();
        store.inner.lock().unwrap().fail_append = true;
        store
    }

    fn with_stage_failure() -> Self {
        let store = Self::default();
        store.inner.lock().unwrap().fail_stage = true;
        store
    }

    fn with_append_index_failure() -> Self {
        let store = Self::default();
        store.inner.lock().unwrap().fail_append_index = true;
        store
    }

    fn with_sequence_failure() -> Self {
        let store = Self::default();
        store.inner.lock().unwrap().fail_sequence = true;
        store
    }

    fn begin_unit_of_work(&self, unit_of_work: &UnitOfWork) -> TransactionEngineResult<()> {
        self.inner
            .lock()
            .unwrap()
            .active_units
            .insert(unit_of_work.clone());
        Ok(())
    }

    fn commit_unit_of_work<C: CryptographicProvider>(
        &self,
        unit_of_work: &UnitOfWork,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<()> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.active_units.contains(unit_of_work) {
            return Err(TransactionEngineError::StorageProviderFailure {
                message: format!("unit of work is not active: {unit_of_work}"),
            });
        }

        let backup = inner.clone();
        let staged_transactions = inner
            .staged_transactions
            .remove(unit_of_work)
            .unwrap_or_default();
        let staged_object_writes = inner
            .staged_object_writes
            .remove(unit_of_work)
            .unwrap_or_default();
        inner.active_units.remove(unit_of_work);

        for prepared in staged_transactions {
            if let Err(error) = inner.finalize_and_insert(prepared, cryptographic_provider) {
                *inner = backup;
                return Err(error);
            }
        }

        for object_write in staged_object_writes {
            inner.committed_object_writes.insert(object_write);
        }

        Ok(())
    }

    fn rollback_unit_of_work(&self, unit_of_work: &UnitOfWork) -> TransactionEngineResult<()> {
        let mut inner = self.inner.lock().unwrap();
        inner.staged_transactions.remove(unit_of_work);
        inner.staged_object_writes.remove(unit_of_work);
        inner.active_units.remove(unit_of_work);
        Ok(())
    }

    fn stage_object_write(&self, unit_of_work: &UnitOfWork, value: &str) -> Result<(), String> {
        if value == "fail" {
            return Err("forced object write failure".to_owned());
        }

        let mut inner = self.inner.lock().unwrap();
        if !inner.active_units.contains(unit_of_work) {
            return Err(format!("unit of work is not active: {unit_of_work}"));
        }
        inner
            .staged_object_writes
            .entry(unit_of_work.clone())
            .or_default()
            .push(value.to_owned());
        Ok(())
    }

    fn object_write_committed(&self, value: &str) -> bool {
        self.inner
            .lock()
            .unwrap()
            .committed_object_writes
            .contains(value)
    }

    fn transaction_count(&self) -> usize {
        self.inner.lock().unwrap().transactions.len()
    }

    fn append_counters(&self) -> (u64, u64) {
        let inner = self.inner.lock().unwrap();
        (inner.next_append_index, inner.next_transaction_sequence)
    }
}

impl InMemoryTransactionStoreInner {
    fn identity_exists(&self, transaction_id: &TransactionId) -> bool {
        self.transactions.contains_key(transaction_id)
            || self
                .staged_transactions
                .values()
                .any(|staged| staged.iter().any(|prepared| &prepared.id == transaction_id))
    }

    fn finalize_and_insert<C: CryptographicProvider>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord> {
        let backup = self.clone();
        let result = self.finalize_and_insert_inner(prepared, cryptographic_provider);
        if result.is_err() {
            *self = backup;
        }
        result
    }

    fn finalize_and_insert_inner<C: CryptographicProvider>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord> {
        if self.fail_append {
            return Err(TransactionEngineError::StorageProviderFailure {
                message: "forced append failure".to_owned(),
            });
        }
        if self.identity_exists(&prepared.id) {
            return Err(TransactionEngineError::DuplicateIdentity {
                transaction_id: prepared.id,
            });
        }

        let append_index = self.allocate_append_index()?;
        let transaction_sequence = if prepared.level.requires_level2_fields() {
            Some(self.allocate_transaction_sequence()?)
        } else {
            None
        };

        let record = finalize_prepared_transaction(
            prepared,
            append_index,
            transaction_sequence,
            cryptographic_provider,
        )?;
        self.append_order
            .insert(record.append_index, record.id.clone());
        self.transactions.insert(record.id.clone(), record.clone());
        Ok(record)
    }

    fn allocate_append_index(&mut self) -> TransactionEngineResult<AppendIndex> {
        if self.fail_append_index {
            return Err(TransactionEngineError::AppendIndexAllocationFailure {
                message: "forced append index allocation failure".to_owned(),
            });
        }
        let append_index = AppendIndex::new(self.next_append_index);
        self.next_append_index += 1;
        Ok(append_index)
    }

    fn allocate_transaction_sequence(&mut self) -> TransactionEngineResult<TransactionSequence> {
        if self.fail_sequence {
            return Err(
                TransactionEngineError::TransactionSequenceAllocationFailure {
                    message: "forced transaction sequence allocation failure".to_owned(),
                },
            );
        }
        let transaction_sequence = TransactionSequence::new(self.next_transaction_sequence);
        self.next_transaction_sequence += 1;
        Ok(transaction_sequence)
    }
}

impl TransactionStore for InMemoryTransactionStore {
    fn transaction_exists(&self, transaction_id: &TransactionId) -> TransactionEngineResult<bool> {
        Ok(self.inner.lock().unwrap().identity_exists(transaction_id))
    }

    fn get_transaction(
        &self,
        transaction_id: &TransactionId,
    ) -> TransactionEngineResult<Option<TransactionRecord>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .transactions
            .get(transaction_id)
            .cloned())
    }

    fn append_transaction<C: CryptographicProvider>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord> {
        self.inner
            .lock()
            .unwrap()
            .finalize_and_insert(prepared, cryptographic_provider)
    }

    fn stage_transaction(
        &mut self,
        unit_of_work: &UnitOfWork,
        prepared: PreparedTransaction,
    ) -> TransactionEngineResult<()> {
        let mut inner = self.inner.lock().unwrap();
        if !inner.active_units.contains(unit_of_work) {
            return Err(TransactionEngineError::StorageProviderFailure {
                message: format!("unit of work is not active: {unit_of_work}"),
            });
        }
        if inner.fail_stage {
            return Err(TransactionEngineError::StorageProviderFailure {
                message: "forced stage failure".to_owned(),
            });
        }
        if inner.identity_exists(&prepared.id) {
            return Err(TransactionEngineError::DuplicateIdentity {
                transaction_id: prepared.id,
            });
        }
        inner
            .staged_transactions
            .entry(unit_of_work.clone())
            .or_default()
            .push(prepared);
        Ok(())
    }

    fn read_append_index_range(
        &self,
        start: AppendIndex,
        max: usize,
    ) -> TransactionEngineResult<Vec<TransactionRecord>> {
        let inner = self.inner.lock().unwrap();
        Ok(inner
            .append_order
            .range(start..)
            .take(max)
            .filter_map(|(_, transaction_id)| inner.transactions.get(transaction_id))
            .cloned()
            .collect())
    }
}

#[derive(Clone, Default)]
struct AlternateMemoryTransactionStore(InMemoryTransactionStore);

impl TransactionStore for AlternateMemoryTransactionStore {
    fn transaction_exists(&self, transaction_id: &TransactionId) -> TransactionEngineResult<bool> {
        self.0.transaction_exists(transaction_id)
    }

    fn get_transaction(
        &self,
        transaction_id: &TransactionId,
    ) -> TransactionEngineResult<Option<TransactionRecord>> {
        self.0.get_transaction(transaction_id)
    }

    fn append_transaction<C: CryptographicProvider>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord> {
        self.0.append_transaction(prepared, cryptographic_provider)
    }

    fn stage_transaction(
        &mut self,
        unit_of_work: &UnitOfWork,
        prepared: PreparedTransaction,
    ) -> TransactionEngineResult<()> {
        self.0.stage_transaction(unit_of_work, prepared)
    }

    fn read_append_index_range(
        &self,
        start: AppendIndex,
        max: usize,
    ) -> TransactionEngineResult<Vec<TransactionRecord>> {
        self.0.read_append_index_range(start, max)
    }
}

#[derive(Clone)]
struct FixedTransactionIds {
    ids: Arc<Mutex<VecDeque<TransactionId>>>,
    fallback: Arc<Mutex<u64>>,
}

impl FixedTransactionIds {
    fn new(ids: &[&str]) -> Self {
        Self {
            ids: Arc::new(Mutex::new(
                ids.iter()
                    .map(|value| TransactionId::new(*value))
                    .collect::<VecDeque<_>>(),
            )),
            fallback: Arc::new(Mutex::new(0)),
        }
    }
}

impl TransactionIdGenerator for FixedTransactionIds {
    fn next_id(&mut self) -> TransactionId {
        if let Some(id) = self.ids.lock().unwrap().pop_front() {
            return id;
        }

        let mut fallback = self.fallback.lock().unwrap();
        *fallback += 1;
        TransactionId::new(format!("txn-fallback-{fallback}"))
    }
}

#[derive(Clone, Default)]
struct DeterministicCrypto {
    fail_hash: bool,
    fail_verify: bool,
}

impl DeterministicCrypto {
    fn with_hash_failure() -> Self {
        Self {
            fail_hash: true,
            fail_verify: false,
        }
    }

    fn with_verify_failure() -> Self {
        Self {
            fail_hash: false,
            fail_verify: true,
        }
    }
}

impl CryptographicProvider for DeterministicCrypto {
    fn hash(&mut self, canonical_content: &[u8]) -> Result<TransactionHash, String> {
        if self.fail_hash {
            return Err("forced hash failure".to_owned());
        }

        let mut hash = 0xcbf29ce484222325_u64;
        for byte in canonical_content {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        Ok(TransactionHash::new(format!(
            "hash-{hash:016x}-{}",
            canonical_content.len()
        )))
    }

    fn verify(
        &mut self,
        signed_revision: &SignedRevision,
        signer: &SignerRef,
        signature: &CryptographicSignature,
    ) -> Result<bool, String> {
        if self.fail_verify {
            return Err("forced verify failure".to_owned());
        }
        Ok(signature.as_str() == signature_token(signed_revision.version(), signer))
    }
}

fn signature_token(version: Version, signer: &SignerRef) -> String {
    format!("signature:{}:{}", version.value(), signer.as_str())
}

fn signature_for(version: Version, signer: &SignerRef) -> CryptographicSignature {
    CryptographicSignature::new(signature_token(version, signer))
}

fn engine(
    store: InMemoryTransactionStore,
    ids: &[&str],
) -> TransactionEngine<InMemoryTransactionStore, FixedTransactionIds, DeterministicCrypto> {
    TransactionEngine::new(
        store,
        FixedTransactionIds::new(ids),
        DeterministicCrypto::default(),
    )
}

fn engine_with_crypto(
    store: InMemoryTransactionStore,
    ids: &[&str],
    crypto: DeterministicCrypto,
) -> TransactionEngine<InMemoryTransactionStore, FixedTransactionIds, DeterministicCrypto> {
    TransactionEngine::new(store, FixedTransactionIds::new(ids), crypto)
}

fn transaction_request(level: TransactionLevel) -> AppendTransactionRequest {
    let resulting_version = Version::new(2);
    let signer = SignerRef::new("signer-001");
    let mut request = AppendTransactionRequest {
        schema_version: TransactionSchemaVersion::new(1),
        level,
        object_id: ObjectId::new("object-001"),
        operation: OperationDescriptor::new("update-name"),
        old_value: text("old"),
        new_value: text("new"),
        actor: ActorRef::new("actor-001"),
        device: DeviceRef::new("device-001"),
        site: Some(SiteRef::new("site-001")),
        edit_timestamp: timestamp("2026-07-15T10:00:00Z"),
        base_version: Version::initial(),
        resulting_version,
        server_receipt_time: level
            .requires_level2_fields()
            .then(|| timestamp("2026-07-15T10:00:01Z")),
        prior_reference: None,
        prior_transaction_hash: None,
        rule_evaluation: Some(rule_metadata()),
        signer: None,
        signer_role: None,
        signature_meaning: None,
        authentication_evidence: None,
        signed_revision: None,
        reason: None,
        signing_timestamp: None,
        signature: None,
    };

    if level.requires_level3_fields() {
        request.signer = Some(signer.clone());
        request.signer_role = Some(SignerRole::new("qa-approver"));
        request.signature_meaning = Some(SignatureMeaning::new("approved"));
        request.authentication_evidence = Some(AuthenticationEvidenceRef::new("auth-001"));
        request.signed_revision = Some(SignedRevision::new(resulting_version));
        request.reason = Some(Reason::new("regulated approval"));
        request.signing_timestamp = Some(timestamp("2026-07-15T10:00:02Z"));
        request.signature = Some(signature_for(resulting_version, &signer));
    }

    request
}

fn text(value: &str) -> PropertyValue {
    PropertyValue::Text {
        value: value.to_owned(),
        language: Some("en".to_owned()),
    }
}

fn timestamp(value: &str) -> TransactionTimestamp {
    TransactionTimestamp::new(value)
}

fn rule_metadata() -> RuleEvaluationMetadata {
    let mut execution_metadata = BTreeMap::new();
    execution_metadata.insert("duration_ms".to_owned(), "4".to_owned());

    RuleEvaluationMetadata {
        triggering_event_ref: Some("event-001".to_owned()),
        rule_version: Some("rule@1".to_owned()),
        evaluation_result: Some("passed".to_owned()),
        generated_actions: vec!["record-transaction".to_owned()],
        execution_metadata,
    }
}

fn committed(result: AppendTransactionResult) -> CommittedTransactionAppend {
    match result {
        AppendTransactionResult::Committed(committed) => *committed,
        AppendTransactionResult::Staged(_) => panic!("expected committed append result"),
    }
}

fn staged_append(result: AppendTransactionResult) -> StagedTransactionAppend {
    match result {
        AppendTransactionResult::Staged(staged) => staged,
        AppendTransactionResult::Committed(_) => panic!("expected staged append result"),
    }
}

#[test]
fn level1_missing_required_field_is_rejected_before_store() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store.clone(), &["txn-001"]);
    let mut request = transaction_request(TransactionLevel::Level1);
    request.actor = ActorRef::new("");

    let error = runtime.append_transaction(request).unwrap_err();

    assert!(matches!(
        error,
        TransactionEngineError::ValidationFailed {
            failure: ValidationError::EmptyActor
        }
    ));
    assert_eq!(store.transaction_count(), 0);
    assert_eq!(store.append_counters(), (1, 1));
}

#[test]
fn level2_missing_server_receipt_time_is_rejected_without_allocating_values() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store.clone(), &["txn-001"]);
    let mut request = transaction_request(TransactionLevel::Level2);
    request.server_receipt_time = None;

    let error = runtime.append_transaction(request).unwrap_err();

    assert!(matches!(
        error,
        TransactionEngineError::ValidationFailed {
            failure: ValidationError::MissingServerReceiptTime {
                level: TransactionLevel::Level2
            }
        }
    ));
    assert_eq!(store.transaction_count(), 0);
    assert_eq!(store.append_counters(), (1, 1));
}

#[test]
fn level3_signature_failure_is_rejected_without_partial_record() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store.clone(), &["txn-001"]);
    let mut request = transaction_request(TransactionLevel::Level3);
    request.signature = Some(CryptographicSignature::new("invalid"));

    let error = runtime.append_transaction(request).unwrap_err();

    assert!(matches!(
        error,
        TransactionEngineError::SignatureVerificationFailure { .. }
    ));
    assert_eq!(store.transaction_count(), 0);
    assert_eq!(store.append_counters(), (1, 1));
}

#[test]
fn concurrent_level2_appends_receive_distinct_ordered_sequences() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store, &["txn-001", "txn-002"]);
    let barrier = Arc::new(Barrier::new(3));

    let first_runtime = runtime.clone();
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        committed(
            first_runtime
                .append_transaction(transaction_request(TransactionLevel::Level2))
                .unwrap(),
        )
    });

    let second_runtime = runtime.clone();
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        committed(
            second_runtime
                .append_transaction(transaction_request(TransactionLevel::Level2))
                .unwrap(),
        )
    });

    barrier.wait();
    let first = first.join().unwrap();
    let second = second.join().unwrap();
    let sequences = BTreeSet::from([
        first.transaction_sequence.unwrap().value(),
        second.transaction_sequence.unwrap().value(),
    ]);
    let append_indices = BTreeSet::from([first.append_index.value(), second.append_index.value()]);

    assert_eq!(sequences, BTreeSet::from([1, 2]));
    assert_eq!(append_indices, BTreeSet::from([1, 2]));
}

#[test]
fn public_api_has_no_mutation_query_reservation_or_status_operations() {
    let source = include_str!("lib.rs");
    for forbidden in [
        "pub fn update",
        "pub fn delete",
        "pub fn filter",
        "pub fn search",
        "pub fn sort",
        "pub fn aggregate",
        "pub fn reserve",
        "pub fn poll",
        "pub fn status",
    ] {
        assert!(
            !source.contains(forbidden),
            "unexpected public operation found: {forbidden}"
        );
    }
}

#[test]
fn unit_of_work_append_returns_staged_then_read_after_commit_or_rollback() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store.clone(), &["txn-001", "txn-002"]);
    let unit_of_work = UnitOfWork::new("uow-commit");

    store.begin_unit_of_work(&unit_of_work).unwrap();
    let staged = staged_append(
        runtime
            .append_transaction_in_unit_of_work(
                transaction_request(TransactionLevel::Level2),
                &unit_of_work,
            )
            .unwrap(),
    );

    assert_eq!(staged.transaction_id.as_str(), "txn-001");
    assert!(matches!(
        runtime.read_transaction(&staged.transaction_id),
        Err(TransactionEngineError::TransactionNotFound { .. })
    ));

    store
        .commit_unit_of_work(&unit_of_work, &mut DeterministicCrypto::default())
        .unwrap();
    let committed = runtime.read_transaction(&staged.transaction_id).unwrap();
    assert_eq!(committed.append_index, AppendIndex::first());
    assert_eq!(
        committed.transaction_sequence,
        Some(TransactionSequence::first())
    );
    assert!(committed.transaction_hash.is_some());

    let rolled_back_unit = UnitOfWork::new("uow-rollback");
    store.begin_unit_of_work(&rolled_back_unit).unwrap();
    let rolled_back = staged_append(
        runtime
            .append_transaction_in_unit_of_work(
                transaction_request(TransactionLevel::Level2),
                &rolled_back_unit,
            )
            .unwrap(),
    );
    store.rollback_unit_of_work(&rolled_back_unit).unwrap();

    assert!(matches!(
        runtime.read_transaction(&rolled_back.transaction_id),
        Err(TransactionEngineError::TransactionNotFound { .. })
    ));
}

#[test]
fn shared_unit_of_work_test_double_commits_or_rolls_back_atomically() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store.clone(), &["txn-001"]);
    let failed_object_unit = UnitOfWork::new("uow-object-fails");

    store.begin_unit_of_work(&failed_object_unit).unwrap();
    assert!(store
        .stage_object_write(&failed_object_unit, "fail")
        .is_err());
    let staged = staged_append(
        runtime
            .append_transaction_in_unit_of_work(
                transaction_request(TransactionLevel::Level2),
                &failed_object_unit,
            )
            .unwrap(),
    );
    store.rollback_unit_of_work(&failed_object_unit).unwrap();
    assert!(matches!(
        runtime.read_transaction(&staged.transaction_id),
        Err(TransactionEngineError::TransactionNotFound { .. })
    ));

    let failing_store = InMemoryTransactionStore::with_stage_failure();
    let failing_runtime = engine(failing_store.clone(), &["txn-002"]);
    let failed_transaction_unit = UnitOfWork::new("uow-transaction-fails");
    failing_store
        .begin_unit_of_work(&failed_transaction_unit)
        .unwrap();
    failing_store
        .stage_object_write(&failed_transaction_unit, "object-change")
        .unwrap();
    assert!(matches!(
        failing_runtime.append_transaction_in_unit_of_work(
            transaction_request(TransactionLevel::Level2),
            &failed_transaction_unit
        ),
        Err(TransactionEngineError::StorageProviderFailure { .. })
    ));
    failing_store
        .rollback_unit_of_work(&failed_transaction_unit)
        .unwrap();
    assert!(!failing_store.object_write_committed("object-change"));
}

#[test]
fn independent_append_commits_without_unit_of_work() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store, &["txn-001"]);

    let result = committed(
        runtime
            .append_transaction(transaction_request(TransactionLevel::Level2))
            .unwrap(),
    );
    let reread = runtime.read_transaction(&result.transaction_id).unwrap();

    assert_eq!(result.append_index, AppendIndex::first());
    assert_eq!(
        result.transaction_sequence,
        Some(TransactionSequence::first())
    );
    assert_eq!(result.transaction_hash, reread.transaction_hash);
    assert!(result.transaction_hash.is_some());
}

#[test]
fn resulting_version_must_increment_base_version() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store.clone(), &["txn-001"]);
    let mut request = transaction_request(TransactionLevel::Level1);
    request.resulting_version = Version::new(4);

    let error = runtime.append_transaction(request).unwrap_err();

    assert!(matches!(
        error,
        TransactionEngineError::ValidationFailed {
            failure: ValidationError::ResultingVersionMismatch { .. }
        }
    ));
    assert_eq!(store.transaction_count(), 0);
}

#[test]
fn deterministic_replay_produces_identical_records() {
    fn scenario() -> String {
        let store = InMemoryTransactionStore::default();
        let runtime = engine(store, &["txn-001", "txn-002"]);
        runtime
            .append_transaction(transaction_request(TransactionLevel::Level1))
            .unwrap();
        runtime
            .append_transaction(transaction_request(TransactionLevel::Level2))
            .unwrap();
        format!(
            "{:?}",
            runtime
                .read_append_index_range(AppendIndex::first(), 10)
                .unwrap()
        )
    }

    assert_eq!(scenario(), scenario());
}

#[test]
fn storage_provider_can_be_swapped_without_changing_callers() {
    fn conformance<S: TransactionStore>(storage: S) -> TransactionEngineResult<TransactionRecord> {
        let runtime = TransactionEngine::new(
            storage,
            FixedTransactionIds::new(&["txn-001"]),
            DeterministicCrypto::default(),
        );
        let appended =
            committed(runtime.append_transaction(transaction_request(TransactionLevel::Level2))?);
        runtime.read_transaction(&appended.transaction_id)
    }

    let first = conformance(InMemoryTransactionStore::default()).unwrap();
    let second = conformance(AlternateMemoryTransactionStore::default()).unwrap();

    assert_eq!(first.level, TransactionLevel::Level2);
    assert_eq!(second.level, TransactionLevel::Level2);
}

#[test]
fn level1_transactions_are_retrievable_by_append_index_range() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store, &["txn-001", "txn-002"]);

    let level1 = committed(
        runtime
            .append_transaction(transaction_request(TransactionLevel::Level1))
            .unwrap(),
    );
    let level2 = committed(
        runtime
            .append_transaction(transaction_request(TransactionLevel::Level2))
            .unwrap(),
    );
    let range = runtime
        .read_append_index_range(AppendIndex::first(), 10)
        .unwrap();

    assert_eq!(range.transactions.len(), 2);
    assert_eq!(range.transactions[0].id, level1.transaction_id);
    assert_eq!(range.transactions[1].id, level2.transaction_id);
    assert_eq!(range.transactions[0].transaction_sequence, None);
    assert_eq!(range.transactions[0].transaction_hash, None);
}

#[test]
fn failed_validation_signature_or_rollback_do_not_allocate_numbering() {
    let validation_store = InMemoryTransactionStore::default();
    let validation_runtime = engine(validation_store.clone(), &["txn-001"]);
    let mut invalid = transaction_request(TransactionLevel::Level2);
    invalid.server_receipt_time = None;
    assert!(validation_runtime.append_transaction(invalid).is_err());
    assert_eq!(validation_store.append_counters(), (1, 1));

    let signature_store = InMemoryTransactionStore::default();
    let signature_runtime = engine(signature_store.clone(), &["txn-002"]);
    let mut bad_signature = transaction_request(TransactionLevel::Level3);
    bad_signature.signature = Some(CryptographicSignature::new("bad"));
    assert!(signature_runtime.append_transaction(bad_signature).is_err());
    assert_eq!(signature_store.append_counters(), (1, 1));

    let rollback_store = InMemoryTransactionStore::default();
    let rollback_runtime = engine(rollback_store.clone(), &["txn-003"]);
    let unit_of_work = UnitOfWork::new("uow-no-numbering");
    rollback_store.begin_unit_of_work(&unit_of_work).unwrap();
    rollback_runtime
        .append_transaction_in_unit_of_work(
            transaction_request(TransactionLevel::Level2),
            &unit_of_work,
        )
        .unwrap();
    rollback_store.rollback_unit_of_work(&unit_of_work).unwrap();
    assert_eq!(rollback_store.append_counters(), (1, 1));
}

#[test]
fn level3_signed_revision_mismatch_is_validation_failure() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store, &["txn-001"]);
    let mut request = transaction_request(TransactionLevel::Level3);
    request.signed_revision = Some(SignedRevision::new(Version::new(9)));

    let error = runtime.append_transaction(request).unwrap_err();

    assert!(matches!(
        error,
        TransactionEngineError::ValidationFailed {
            failure: ValidationError::SignedRevisionMismatch { .. }
        }
    ));
}

#[test]
fn prior_hash_without_transaction_reference_is_rejected() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store, &["txn-001"]);
    let mut request = transaction_request(TransactionLevel::Level2);
    request.prior_reference = Some(PriorReference::Event("event-001".to_owned()));
    request.prior_transaction_hash = Some(PriorTransactionHash::new("hash-prior"));

    let error = runtime.append_transaction(request).unwrap_err();

    assert!(matches!(
        error,
        TransactionEngineError::ValidationFailed {
            failure: ValidationError::PriorTransactionHashRequiresTransactionReference
        }
    ));
}

#[test]
fn hash_output_is_stable_for_identical_canonical_content() {
    let mut crypto = DeterministicCrypto::default();
    let first = crypto.hash(b"identical canonical bytes").unwrap();
    let second = crypto.hash(b"identical canonical bytes").unwrap();

    assert_eq!(first, second);
}

#[test]
fn hash_covers_assigned_append_index_and_transaction_sequence() {
    let prepared = PreparedTransaction::from_request(
        TransactionId::new("txn-stable"),
        transaction_request(TransactionLevel::Level2),
    );
    let mut crypto = DeterministicCrypto::default();

    let first = finalize_prepared_transaction(
        prepared.clone(),
        AppendIndex::new(1),
        Some(TransactionSequence::new(1)),
        &mut crypto,
    )
    .unwrap();
    let second = finalize_prepared_transaction(
        prepared,
        AppendIndex::new(2),
        Some(TransactionSequence::new(2)),
        &mut crypto,
    )
    .unwrap();

    assert_ne!(first.transaction_hash, second.transaction_hash);
}

#[test]
fn append_index_range_is_validated_and_ordered() {
    let store = InMemoryTransactionStore::default();
    let runtime = engine(store, &["txn-001", "txn-002"]);

    runtime
        .append_transaction(transaction_request(TransactionLevel::Level1))
        .unwrap();
    runtime
        .append_transaction(transaction_request(TransactionLevel::Level1))
        .unwrap();
    let range = runtime
        .read_append_index_range(AppendIndex::first(), 1)
        .unwrap();

    assert_eq!(range.transactions.len(), 1);
    assert_eq!(range.transactions[0].append_index, AppendIndex::first());
    assert_eq!(range.next_start, Some(AppendIndex::new(2)));
    assert!(matches!(
        runtime.read_append_index_range(AppendIndex::new(0), 1),
        Err(TransactionEngineError::InvalidAppendIndexRange { .. })
    ));
    assert!(matches!(
        runtime.read_append_index_range(AppendIndex::first(), 0),
        Err(TransactionEngineError::InvalidAppendIndexRange { .. })
    ));
}

#[test]
fn negative_paths_cover_distinct_error_kinds() {
    let duplicate_store = InMemoryTransactionStore::default();
    let duplicate_runtime = engine(duplicate_store, &["txn-dup", "txn-dup"]);
    duplicate_runtime
        .append_transaction(transaction_request(TransactionLevel::Level1))
        .unwrap();
    assert!(matches!(
        duplicate_runtime.append_transaction(transaction_request(TransactionLevel::Level1)),
        Err(TransactionEngineError::DuplicateIdentity { .. })
    ));

    let append_index_runtime = engine(
        InMemoryTransactionStore::with_append_index_failure(),
        &["txn-idx"],
    );
    assert!(matches!(
        append_index_runtime.append_transaction(transaction_request(TransactionLevel::Level1)),
        Err(TransactionEngineError::AppendIndexAllocationFailure { .. })
    ));

    let sequence_runtime = engine(
        InMemoryTransactionStore::with_sequence_failure(),
        &["txn-seq"],
    );
    assert!(matches!(
        sequence_runtime.append_transaction(transaction_request(TransactionLevel::Level2)),
        Err(TransactionEngineError::TransactionSequenceAllocationFailure { .. })
    ));

    let hash_runtime = engine_with_crypto(
        InMemoryTransactionStore::default(),
        &["txn-hash"],
        DeterministicCrypto::with_hash_failure(),
    );
    assert!(matches!(
        hash_runtime.append_transaction(transaction_request(TransactionLevel::Level2)),
        Err(TransactionEngineError::CryptographicProviderFailure { .. })
    ));

    let verify_runtime = engine_with_crypto(
        InMemoryTransactionStore::default(),
        &["txn-verify"],
        DeterministicCrypto::with_verify_failure(),
    );
    assert!(matches!(
        verify_runtime.append_transaction(transaction_request(TransactionLevel::Level3)),
        Err(TransactionEngineError::CryptographicProviderFailure { .. })
    ));

    let storage_runtime = engine(
        InMemoryTransactionStore::with_append_failure(),
        &["txn-store"],
    );
    assert!(matches!(
        storage_runtime.append_transaction(transaction_request(TransactionLevel::Level1)),
        Err(TransactionEngineError::StorageProviderFailure { .. })
    ));

    let unsupported_runtime = engine(InMemoryTransactionStore::default(), &["txn-unsupported"]);
    let mut unsupported = transaction_request(TransactionLevel::Level1);
    unsupported.level = TransactionLevel::Unsupported;
    assert!(matches!(
        unsupported_runtime.append_transaction(unsupported),
        Err(TransactionEngineError::UnsupportedTransactionLevel { .. })
    ));
}

#[test]
fn boundary_has_no_forbidden_engine_dependencies_or_calls() {
    let manifests = [include_str!("../Cargo.toml")];
    let source_files = [
        include_str!("lib.rs"),
        include_str!("types.rs"),
        include_str!("errors.rs"),
        include_str!("storage.rs"),
        include_str!("crypto.rs"),
        include_str!("canonical.rs"),
        include_str!("validation.rs"),
    ];
    let forbidden = [
        "open-eqms-object-runtime",
        "open_eqms_object_runtime",
        "open-eqms-event-engine",
        "open_eqms_event_engine",
        "object_runtime::",
        "event_engine::",
        "rule_engine::",
        "process_engine::",
        "query_engine::",
        "security::",
        "content_package::",
    ];

    for manifest in manifests {
        for needle in forbidden {
            assert!(
                !manifest.contains(needle),
                "forbidden dependency found in manifest: {needle}"
            );
        }
    }

    for source in source_files {
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "forbidden source reference found: {needle}"
            );
        }
    }
}

#[test]
fn cryptographic_provider_has_hash_and_verify_only_no_signing_api() {
    let crypto_source = include_str!("crypto.rs");

    assert!(crypto_source.contains("fn hash("));
    assert!(crypto_source.contains("fn verify("));
    assert!(!crypto_source.contains("fn sign("));
}
