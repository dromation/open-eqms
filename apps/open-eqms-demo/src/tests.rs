use crate::asset_model::{
    asset_calibration_accepted_event_type_ref, asset_calibration_performed_event_type_ref,
    asset_event_type_definitions, asset_object_type_definition, asset_object_type_ref,
    asset_registered_event_type_ref, register_event_metadata, register_object_metadata,
    ASSET_CALIBRATION_ACCEPTED_EVENT_TYPE_NAME, ASSET_CALIBRATION_PERFORMED_EVENT_TYPE_NAME,
    ASSET_OBJECT_TYPE_NAME, ASSET_REGISTERED_EVENT_TYPE_NAME,
};
use crate::clock::DeterministicClock;
use crate::crypto::DemoCryptographicProvider;
use crate::ids::{
    DeterministicAppendSequences, DeterministicEventIds, DeterministicObjectIds,
    DeterministicTransactionIds,
};
use crate::outcome::{ConsistencyStatus, StepId};
use crate::scenario::{DemoApp, RecordCalibrationInput, RegisterAssetInput};
use crate::storage::{DemoEventStore, DemoObjectAndTransactionStore};
use open_eqms_event_engine::errors::{
    EventEngineError, EventEngineResult, MetadataError as EventMetadataError,
};
use open_eqms_event_engine::storage::StorageProvider as EventStorageProvider;
use open_eqms_event_engine::storage::{AppendSequenceGenerator, EventIdGenerator};
use open_eqms_event_engine::types::{
    AppendSequence, EventId, EventRecord, EventTypeDefinition, EventTypeRef,
};
use open_eqms_event_engine::EventEngine;
use open_eqms_object_runtime::errors::{
    MetadataError as ObjectMetadataError, ObjectRuntimeError, ObjectRuntimeResult,
};
use open_eqms_object_runtime::storage::ObjectIdGenerator;
use open_eqms_object_runtime::storage::StorageProvider as ObjectStorageProvider;
use open_eqms_object_runtime::types::{
    LifecycleState, ObjectRecord, ObjectTypeDefinition, ObjectTypeRef, OwnershipInfo,
    PermissionScopeRef, PropertyChange, Version,
};
use open_eqms_object_runtime::{CreateObjectRequest, ObjectChanges, ObjectRuntime};
use open_eqms_runtime_contracts::{ObjectId, PropertyValue, UnitOfWork};
use open_eqms_transaction_engine::crypto::CryptographicProvider;
use open_eqms_transaction_engine::storage::TransactionIdGenerator;
use open_eqms_transaction_engine::types::{
    ActorRef, DeviceRef, OperationDescriptor, PriorReference, SiteRef, TransactionId,
    TransactionLevel, TransactionSchemaVersion, TransactionTimestamp,
};
use open_eqms_transaction_engine::{
    AppendTransactionRequest, AppendTransactionResult, TransactionEngine,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct ObjectMetadataStore {
    types: BTreeMap<ObjectTypeRef, ObjectTypeDefinition>,
}

impl ObjectStorageProvider for ObjectMetadataStore {
    fn get_type(
        &self,
        object_type: &ObjectTypeRef,
    ) -> ObjectRuntimeResult<Option<ObjectTypeDefinition>> {
        Ok(self.types.get(object_type).cloned())
    }

    fn put_type(&mut self, definition: ObjectTypeDefinition) -> ObjectRuntimeResult<()> {
        self.types.insert(definition.type_ref.clone(), definition);
        Ok(())
    }

    fn get_object(&self, _object_id: &ObjectId) -> ObjectRuntimeResult<Option<ObjectRecord>> {
        Ok(None)
    }

    fn insert_object(&mut self, _record: ObjectRecord) -> ObjectRuntimeResult<()> {
        Ok(())
    }

    fn replace_object(
        &mut self,
        _record: ObjectRecord,
        _expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        Ok(())
    }

    fn object_exists(&self, _object_id: &ObjectId) -> ObjectRuntimeResult<bool> {
        Ok(false)
    }

    fn visit_objects(
        &self,
        _visitor: &mut dyn FnMut(&ObjectRecord) -> ObjectRuntimeResult<()>,
    ) -> ObjectRuntimeResult<()> {
        Ok(())
    }
}

#[derive(Default)]
struct EventMetadataStore {
    types: BTreeMap<EventTypeRef, EventTypeDefinition>,
}

impl EventStorageProvider for EventMetadataStore {
    fn get_type(
        &self,
        event_type: &EventTypeRef,
    ) -> EventEngineResult<Option<EventTypeDefinition>> {
        Ok(self.types.get(event_type).cloned())
    }

    fn insert_type(&mut self, definition: EventTypeDefinition) -> EventEngineResult<()> {
        self.types.insert(definition.type_ref.clone(), definition);
        Ok(())
    }

    fn get_event(&self, _event_id: &EventId) -> EventEngineResult<Option<EventRecord>> {
        Ok(None)
    }

    fn event_exists(&self, _event_id: &EventId) -> EventEngineResult<bool> {
        Ok(false)
    }

    fn append_event(&mut self, _record: EventRecord) -> EventEngineResult<()> {
        Ok(())
    }

    fn read_sequence_range(
        &self,
        _start: AppendSequence,
        _max: usize,
    ) -> EventEngineResult<Vec<EventRecord>> {
        Ok(Vec::new())
    }
}

fn test_asset_create_request() -> CreateObjectRequest {
    CreateObjectRequest {
        object_type: asset_object_type_ref(),
        properties: BTreeMap::from([
            text_property("identity_label", "Scale A-100"),
            enum_property("asset_class", "measuring_equipment"),
            text_property("organization", "Open-EQMS Demo"),
            text_property("location", "Calibration Lab"),
            text_property("manufacturer", "Demo Instruments"),
            text_property("model", "DI-100"),
            text_property("serial_number", "SN-100"),
            enum_property("status", "registered"),
            text_property("responsible_reference", "demo-operator"),
            datetime_property("registered_at", "2026-07-15T10:00:00Z"),
            (
                "calibration_required".to_owned(),
                PropertyValue::Boolean(true),
            ),
            (
                "calibration_interval_days".to_owned(),
                PropertyValue::Number(365.0),
            ),
            datetime_property("next_calibration_due", "2027-07-15T10:00:00Z"),
        ]),
        relations: BTreeSet::new(),
        lifecycle_state: LifecycleState::new("active"),
        ownership: OwnershipInfo::new(ObjectId::new("demo-operator"), BTreeSet::new()),
        permission_scope: PermissionScopeRef::new("demo-local-scope"),
        retention_rule: None,
        external_references: BTreeSet::new(),
        comments_ref: None,
        attachments_ref: None,
    }
}

fn calibration_acceptance_changes() -> ObjectChanges {
    let mut changes = ObjectChanges::default();
    changes.property_changes.insert(
        "status".to_owned(),
        PropertyChange::Set(PropertyValue::EnumValue("in_service".to_owned())),
    );
    changes.property_changes.insert(
        "next_calibration_due".to_owned(),
        PropertyChange::Set(PropertyValue::DateTime("2027-07-15T10:00:00Z".to_owned())),
    );
    changes
}

fn level2_calibration_transaction(
    asset_id: ObjectId,
    registration_transaction_id: TransactionId,
    base_version: Version,
    resulting_version: Version,
) -> AppendTransactionRequest {
    AppendTransactionRequest {
        schema_version: TransactionSchemaVersion::new(1),
        level: TransactionLevel::Level2,
        object_id: asset_id,
        operation: OperationDescriptor::new("asset.calibration.accepted"),
        old_value: PropertyValue::Text {
            value: "status=registered,next_calibration_due=2027-07-15T10:00:00Z".to_owned(),
            language: Some("en".to_owned()),
        },
        new_value: PropertyValue::Text {
            value: "status=in_service,next_calibration_due=2027-07-15T10:00:00Z".to_owned(),
            language: Some("en".to_owned()),
        },
        actor: ActorRef::new("demo-operator"),
        device: DeviceRef::new("demo-cli"),
        site: Some(SiteRef::new("demo-site")),
        edit_timestamp: TransactionTimestamp::new("2026-07-15T10:00:04Z"),
        base_version,
        resulting_version,
        server_receipt_time: Some(TransactionTimestamp::new("2026-07-15T10:00:05Z")),
        prior_reference: Some(PriorReference::Transaction(registration_transaction_id)),
        prior_transaction_hash: None,
        rule_evaluation: None,
        signer: None,
        signer_role: None,
        signature_meaning: None,
        authentication_evidence: None,
        signed_revision: None,
        reason: None,
        signing_timestamp: None,
        signature: None,
    }
}

fn text_property(name: &str, value: &str) -> (String, PropertyValue) {
    (
        name.to_owned(),
        PropertyValue::Text {
            value: value.to_owned(),
            language: Some("en".to_owned()),
        },
    )
}

fn enum_property(name: &str, value: &str) -> (String, PropertyValue) {
    (name.to_owned(), PropertyValue::EnumValue(value.to_owned()))
}

fn datetime_property(name: &str, value: &str) -> (String, PropertyValue) {
    (name.to_owned(), PropertyValue::DateTime(value.to_owned()))
}

fn text_property_value(record: &ObjectRecord, name: &str) -> String {
    match record.properties.get(name).unwrap() {
        PropertyValue::Text { value, .. }
        | PropertyValue::DateTime(value)
        | PropertyValue::EnumValue(value) => value.clone(),
        value => panic!("unexpected property kind for {name}: {value:?}"),
    }
}

fn register_demo_asset(app: &mut DemoApp) -> (ObjectId, TransactionId) {
    let registration = app.register_asset(RegisterAssetInput::demo());
    assert_eq!(registration.consistency_status, ConsistencyStatus::Complete);
    (
        registration.created_object_id.unwrap(),
        registration.created_transaction_ids[0].clone(),
    )
}

#[test]
fn deterministic_generators_replay_identical_sequences() {
    fn scenario() -> Vec<String> {
        let mut object_ids = DeterministicObjectIds::new("asset");
        let mut event_ids = DeterministicEventIds::new("event");
        let mut transaction_ids = DeterministicTransactionIds::new("txn");
        let mut append_sequences = DeterministicAppendSequences::new();

        vec![
            object_ids.next_id().to_string(),
            object_ids.next_id().to_string(),
            event_ids.next_id().to_string(),
            event_ids.next_id().to_string(),
            transaction_ids.next_id().to_string(),
            transaction_ids.next_id().to_string(),
            append_sequences.next_sequence().value().to_string(),
            append_sequences.next_sequence().value().to_string(),
        ]
    }

    assert_eq!(scenario(), scenario());
}

#[test]
fn deterministic_clock_replays_fixed_timestamps() {
    fn scenario() -> Vec<String> {
        let mut clock = DeterministicClock::new(&["2026-07-15T10:00:00Z", "2026-07-15T10:00:01Z"]);

        vec![
            clock.next_timestamp(),
            clock.next_event_timestamp().as_str().to_owned(),
            clock.next_transaction_timestamp().as_str().to_owned(),
        ]
    }

    assert_eq!(scenario(), scenario());
    assert_eq!(
        scenario(),
        vec![
            "2026-07-15T10:00:00Z".to_owned(),
            "2026-07-15T10:00:01Z".to_owned(),
            "2026-07-15T10:00:00Z".to_owned()
        ]
    );
}

#[test]
fn demo_crypto_hash_is_deterministic_and_demo_only() {
    let mut first = DemoCryptographicProvider::default();
    let mut second = DemoCryptographicProvider::default();

    let first_hash = first.hash(b"same canonical content").unwrap();
    let second_hash = second.hash(b"same canonical content").unwrap();

    assert_eq!(first_hash, second_hash);
    assert!(first_hash.as_str().starts_with("demo-noncrypto-"));
}

#[test]
fn asset_and_event_metadata_registration_succeeds() {
    let object_runtime = ObjectRuntime::new(
        ObjectMetadataStore::default(),
        DeterministicObjectIds::new("asset"),
    );
    let event_engine = EventEngine::new(
        EventMetadataStore::default(),
        DeterministicEventIds::new("event"),
        DeterministicAppendSequences::new(),
    );

    register_object_metadata(&object_runtime).unwrap();
    register_event_metadata(&event_engine).unwrap();

    let object_type = object_runtime
        .get_object_type(&asset_object_type_ref())
        .unwrap();
    assert_eq!(object_type.type_ref.name, ASSET_OBJECT_TYPE_NAME);
    assert_eq!(object_type.properties.len(), 13);

    let event_types = [
        event_engine
            .get_event_type(&asset_registered_event_type_ref())
            .unwrap(),
        event_engine
            .get_event_type(&asset_event_type_definitions()[1].type_ref)
            .unwrap(),
        event_engine
            .get_event_type(&asset_event_type_definitions()[2].type_ref)
            .unwrap(),
    ];
    let event_names = event_types
        .iter()
        .map(|definition| definition.type_ref.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        event_names,
        [
            ASSET_REGISTERED_EVENT_TYPE_NAME,
            ASSET_CALIBRATION_PERFORMED_EVENT_TYPE_NAME,
            ASSET_CALIBRATION_ACCEPTED_EVENT_TYPE_NAME
        ]
    );
}

#[test]
fn malformed_asset_metadata_is_rejected_by_engine_validators() {
    let object_runtime = ObjectRuntime::new(
        ObjectMetadataStore::default(),
        DeterministicObjectIds::new("asset"),
    );
    let mut malformed_object = asset_object_type_definition();
    let definition = malformed_object
        .properties
        .remove("identity_label")
        .unwrap();
    malformed_object
        .properties
        .insert("wrong_key".to_owned(), definition);

    let object_error = object_runtime
        .register_object_type(malformed_object)
        .unwrap_err();
    assert!(matches!(
        object_error,
        ObjectRuntimeError::MalformedMetadata {
            failure: ObjectMetadataError::PropertyKeyMismatch { .. }
        }
    ));

    let event_engine = EventEngine::new(
        EventMetadataStore::default(),
        DeterministicEventIds::new("event"),
        DeterministicAppendSequences::new(),
    );
    let mut malformed_event = asset_event_type_definitions().remove(0);
    malformed_event.type_ref = EventTypeRef::new("", 1);

    let event_error = event_engine
        .register_event_type(malformed_event)
        .unwrap_err();
    assert!(matches!(
        event_error,
        EventEngineError::MalformedMetadata {
            failure: EventMetadataError::EmptyTypeName
        }
    ));
}

#[test]
fn combined_store_commits_object_update_and_level2_transaction_together() {
    let store = DemoObjectAndTransactionStore::default();
    let object_runtime = ObjectRuntime::new(store.clone(), DeterministicObjectIds::new("asset"));
    let transaction_engine = TransactionEngine::new(
        store.clone(),
        DeterministicTransactionIds::new("txn"),
        DemoCryptographicProvider::default(),
    );
    register_object_metadata(&object_runtime).unwrap();
    let created = object_runtime
        .create_object(test_asset_create_request())
        .unwrap();
    let registration_transaction_id = TransactionId::new("txn-registration");
    let unit_of_work = UnitOfWork::new("uow-calibration-commit");

    store.begin_shared_unit_of_work(&unit_of_work).unwrap();
    let updated = object_runtime
        .update_object_in_unit_of_work(
            open_eqms_object_runtime::UpdateObjectRequest {
                object_id: created.object_id.clone(),
                base_version: created.version,
                changes: calibration_acceptance_changes(),
            },
            &unit_of_work,
        )
        .unwrap();
    let staged_transaction = match transaction_engine
        .append_transaction_in_unit_of_work(
            level2_calibration_transaction(
                created.object_id.clone(),
                registration_transaction_id.clone(),
                created.version,
                updated.version,
            ),
            &unit_of_work,
        )
        .unwrap()
    {
        AppendTransactionResult::Staged(staged) => staged,
        AppendTransactionResult::Committed(_) => panic!("expected staged transaction append"),
    };

    let before_commit = object_runtime.read_object(&created.object_id).unwrap();
    assert_eq!(before_commit.version, Version::initial());
    assert_eq!(text_property_value(&before_commit, "status"), "registered");

    store
        .commit_shared_unit_of_work(&unit_of_work, &mut DemoCryptographicProvider::default())
        .unwrap();

    let after_commit = object_runtime.read_object(&created.object_id).unwrap();
    assert_eq!(after_commit.version, Version::new(2));
    assert_eq!(text_property_value(&after_commit, "status"), "in_service");

    let transaction = transaction_engine
        .read_transaction(&staged_transaction.transaction_id)
        .unwrap();
    assert_eq!(transaction.level, TransactionLevel::Level2);
    assert_eq!(
        transaction.prior_reference,
        Some(PriorReference::Transaction(registration_transaction_id))
    );
    assert_eq!(transaction.prior_transaction_hash, None);
    assert!(transaction.transaction_hash.is_some());
}

#[test]
fn transaction_commit_failure_rolls_back_paired_object_update() {
    let store = DemoObjectAndTransactionStore::default();
    let object_runtime = ObjectRuntime::new(store.clone(), DeterministicObjectIds::new("asset"));
    let transaction_engine = TransactionEngine::new(
        store.clone(),
        DeterministicTransactionIds::new("txn"),
        DemoCryptographicProvider::default(),
    );
    register_object_metadata(&object_runtime).unwrap();
    let created = object_runtime
        .create_object(test_asset_create_request())
        .unwrap();
    let unit_of_work = UnitOfWork::new("uow-calibration-fails");

    store.begin_shared_unit_of_work(&unit_of_work).unwrap();
    let updated = object_runtime
        .update_object_in_unit_of_work(
            open_eqms_object_runtime::UpdateObjectRequest {
                object_id: created.object_id.clone(),
                base_version: created.version,
                changes: calibration_acceptance_changes(),
            },
            &unit_of_work,
        )
        .unwrap();
    let staged_transaction = match transaction_engine
        .append_transaction_in_unit_of_work(
            level2_calibration_transaction(
                created.object_id.clone(),
                TransactionId::new("txn-registration"),
                created.version,
                updated.version,
            ),
            &unit_of_work,
        )
        .unwrap()
    {
        AppendTransactionResult::Staged(staged) => staged,
        AppendTransactionResult::Committed(_) => panic!("expected staged transaction append"),
    };
    store.fail_next_transaction_commit();

    assert!(store
        .commit_shared_unit_of_work(&unit_of_work, &mut DemoCryptographicProvider::default())
        .is_err());

    let after_failure = object_runtime.read_object(&created.object_id).unwrap();
    assert_eq!(after_failure.version, Version::initial());
    assert_eq!(text_property_value(&after_failure, "status"), "registered");
    assert!(transaction_engine
        .read_transaction(&staged_transaction.transaction_id)
        .is_err());
    assert_eq!(store.transaction_count(), 0);
}

#[test]
fn independent_event_store_has_no_unit_of_work_trait_surface() {
    fn accepts_event_storage_provider<S: EventStorageProvider>(_store: S) {}

    accepts_event_storage_provider(DemoEventStore::default());

    let source = include_str!("storage.rs");
    let event_store_source = source
        .split("pub struct DemoEventStore")
        .nth(1)
        .expect("DemoEventStore source section");
    assert!(!event_store_source.contains("UnitOfWork"));
    assert!(!event_store_source.contains("stage_event"));
    assert!(!event_store_source.contains("commit_unit_of_work"));
    assert!(!event_store_source.contains("rollback_unit_of_work"));
}

#[test]
fn register_asset_command_creates_readable_object_event_and_level1_transaction() {
    let mut app = DemoApp::new();

    let outcome = app.register_asset(RegisterAssetInput::demo());

    assert_eq!(outcome.consistency_status, ConsistencyStatus::Complete);
    assert!(outcome.is_complete());
    assert_eq!(outcome.failed_step, None);
    let object_id = outcome.created_object_id.clone().unwrap();
    assert_eq!(object_id.as_str(), "asset-0001");

    let asset = app.read_asset(&object_id).unwrap();
    assert_eq!(asset.id, object_id);
    assert_eq!(asset.version, Version::initial());
    assert_eq!(text_property_value(&asset, "identity_label"), "Scale A-100");

    assert_eq!(outcome.created_event_ids.len(), 1);
    let event = app.read_event(&outcome.created_event_ids[0]).unwrap();
    assert_eq!(event.event_type, asset_registered_event_type_ref());
    assert!(event.object_refs.contains(&asset.id));

    assert_eq!(outcome.created_transaction_ids.len(), 1);
    let transaction = app
        .read_transaction(&outcome.created_transaction_ids[0])
        .unwrap();
    assert_eq!(transaction.level, TransactionLevel::Level1);
    assert_eq!(transaction.object_id, asset.id);
    assert_eq!(transaction.prior_reference, None);
    assert_eq!(transaction.prior_transaction_hash, None);
    assert_eq!(transaction.transaction_hash, None);
}

#[test]
fn invalid_register_asset_input_is_rejected_before_object_runtime_call() {
    let mut app = DemoApp::new();
    let mut input = RegisterAssetInput::demo();
    input.identity_label.clear();

    let outcome = app.register_asset(input);

    assert_eq!(
        outcome.consistency_status,
        ConsistencyStatus::FailedBeforeMutation
    );
    assert_eq!(outcome.failed_step, Some(StepId::ValidateAssetInput));
    assert_eq!(outcome.created_object_id, None);
    assert!(outcome.created_event_ids.is_empty());
    assert!(outcome.created_transaction_ids.is_empty());
    assert_eq!(app.object_count(), 0);
    assert_eq!(app.event_count(), 0);
    assert_eq!(app.transaction_count(), 0);
}

#[test]
fn registration_transaction_failure_returns_partial_outcome_with_existing_records() {
    let mut app = DemoApp::new();
    app.fail_next_registration_transaction_append();

    let outcome = app.register_asset(RegisterAssetInput::demo());

    assert_eq!(
        outcome.consistency_status,
        ConsistencyStatus::PartiallyCompleted
    );
    assert!(!outcome.is_complete());
    assert_eq!(
        outcome.failed_step,
        Some(StepId::AppendRegistrationTransaction)
    );
    assert_eq!(
        outcome.created_object_id.as_ref().unwrap().as_str(),
        "asset-0001"
    );
    assert_eq!(outcome.created_event_ids.len(), 1);
    assert!(outcome.created_transaction_ids.is_empty());
    assert!(!outcome.recovery_guidance.is_empty());
    assert_eq!(app.object_count(), 1);
    assert_eq!(app.event_count(), 1);
    assert_eq!(app.transaction_count(), 0);

    let report = outcome.to_cli_report();
    assert!(report.contains("consistency_status: PartiallyCompleted"));
    assert!(report.contains("created_object_id: asset-0001"));
    assert!(report.contains("created_event_ids: event-0001"));
    assert!(report.contains("created_transaction_ids: none"));
}

#[test]
fn record_calibration_accepted_appends_events_level2_transaction_and_updates_asset() {
    let mut app = DemoApp::new();
    let (asset_id, registration_transaction_id) = register_demo_asset(&mut app);

    let outcome = app.record_calibration(RecordCalibrationInput::accepted(asset_id.clone()));

    assert_eq!(outcome.consistency_status, ConsistencyStatus::Complete);
    assert_eq!(outcome.created_event_ids.len(), 2);
    assert_eq!(outcome.created_transaction_ids.len(), 1);

    let performed = app.read_event(&outcome.created_event_ids[0]).unwrap();
    let accepted = app.read_event(&outcome.created_event_ids[1]).unwrap();
    assert_eq!(
        performed.event_type,
        asset_calibration_performed_event_type_ref()
    );
    assert_eq!(
        accepted.event_type,
        asset_calibration_accepted_event_type_ref()
    );
    assert!(performed.append_sequence < accepted.append_sequence);

    let calibration_transaction = app
        .read_transaction(&outcome.created_transaction_ids[0])
        .unwrap();
    assert_eq!(calibration_transaction.level, TransactionLevel::Level2);
    assert_eq!(
        calibration_transaction.prior_reference,
        Some(PriorReference::Transaction(registration_transaction_id))
    );
    assert_eq!(calibration_transaction.prior_transaction_hash, None);
    assert!(calibration_transaction.transaction_hash.is_some());

    let asset = app.read_asset(&asset_id).unwrap();
    assert_eq!(asset.version, Version::new(2));
    assert_eq!(text_property_value(&asset, "status"), "in_service");
    assert_eq!(
        text_property_value(&asset, "next_calibration_due"),
        "2027-07-15T10:00:00Z"
    );
}

#[test]
fn record_calibration_rejected_appends_only_performed_event() {
    let mut app = DemoApp::new();
    let (asset_id, _) = register_demo_asset(&mut app);

    let outcome = app.record_calibration(RecordCalibrationInput::rejected(asset_id.clone()));

    assert_eq!(outcome.consistency_status, ConsistencyStatus::Complete);
    assert_eq!(outcome.created_event_ids.len(), 1);
    assert!(outcome.created_transaction_ids.is_empty());

    let performed = app.read_event(&outcome.created_event_ids[0]).unwrap();
    assert_eq!(
        performed.event_type,
        asset_calibration_performed_event_type_ref()
    );
    assert_eq!(
        performed.payload.get("outcome"),
        Some(&PropertyValue::EnumValue("rejected".to_owned()))
    );

    let asset = app.read_asset(&asset_id).unwrap();
    assert_eq!(asset.version, Version::initial());
    assert_eq!(text_property_value(&asset, "status"), "registered");
    assert_eq!(app.transaction_count(), 1);
}

#[test]
fn record_calibration_before_registration_is_rejected_before_event_or_transaction_append() {
    let mut app = DemoApp::new();

    let outcome = app.record_calibration(RecordCalibrationInput::accepted(ObjectId::new(
        "asset-0001",
    )));

    assert_eq!(
        outcome.consistency_status,
        ConsistencyStatus::FailedBeforeMutation
    );
    assert_eq!(outcome.failed_step, Some(StepId::ValidateCalibrationInput));
    assert!(outcome.created_event_ids.is_empty());
    assert!(outcome.created_transaction_ids.is_empty());
    assert_eq!(app.object_count(), 0);
    assert_eq!(app.event_count(), 0);
    assert_eq!(app.transaction_count(), 0);
}

#[test]
fn calibration_transaction_commit_failure_rolls_back_object_update_in_command() {
    let mut app = DemoApp::new();
    let (asset_id, _) = register_demo_asset(&mut app);
    app.fail_next_calibration_transaction_commit();

    let outcome = app.record_calibration(RecordCalibrationInput::accepted(asset_id.clone()));

    assert_eq!(
        outcome.consistency_status,
        ConsistencyStatus::PartiallyCompleted
    );
    assert_eq!(
        outcome.failed_step,
        Some(StepId::CommitCalibrationUnitOfWork)
    );
    assert_eq!(outcome.created_event_ids.len(), 2);
    assert!(outcome.created_transaction_ids.is_empty());

    let asset = app.read_asset(&asset_id).unwrap();
    assert_eq!(asset.version, Version::initial());
    assert_eq!(text_property_value(&asset, "status"), "registered");
    assert_eq!(app.transaction_count(), 1);
    assert!(outcome
        .recovery_guidance
        .contains("Object update is not durably visible"));
}

#[test]
fn show_asset_renders_current_asset_state() {
    let mut app = DemoApp::new();
    let (asset_id, _) = register_demo_asset(&mut app);
    let calibration = app.record_calibration(RecordCalibrationInput::accepted(asset_id.clone()));
    assert_eq!(calibration.consistency_status, ConsistencyStatus::Complete);

    let report = app.show_asset(&asset_id).unwrap();

    assert!(report.contains("Asset asset-0001"));
    assert!(report.contains("object_type: asset.equipment@1"));
    assert!(report.contains("version: 2"));
    assert!(report.contains("status: in_service"));
    assert!(report.contains("next_calibration_due: 2027-07-15T10:00:00Z"));
}

#[test]
fn show_timeline_ordering_is_deterministic() {
    fn timeline() -> String {
        let mut app = DemoApp::new();
        let (asset_id, _) = register_demo_asset(&mut app);
        let calibration =
            app.record_calibration(RecordCalibrationInput::accepted(asset_id.clone()));
        assert_eq!(calibration.consistency_status, ConsistencyStatus::Complete);
        app.show_timeline(&asset_id).unwrap()
    }

    let first = timeline();
    let second = timeline();

    assert_eq!(first, second);
    assert!(first.contains("Events (3; bounded Event range read, app-layer filter):"));
    assert!(first.contains("Transactions (2; bounded Transaction range read, app-layer filter):"));

    let registered_event = first.find("event 1 event-0001 asset.registered@1").unwrap();
    let performed_event = first
        .find("event 2 event-0002 asset.calibration_performed@1")
        .unwrap();
    let accepted_event = first
        .find("event 3 event-0003 asset.calibration_accepted@1")
        .unwrap();
    assert!(registered_event < performed_event);
    assert!(performed_event < accepted_event);

    let registration_transaction = first.find("transaction 1 txn-0001 level=level-1").unwrap();
    let calibration_transaction = first.find("transaction 2 txn-0002 level=level-2").unwrap();
    assert!(registration_transaction < calibration_transaction);
    assert!(first.contains("prior_reference: transaction:txn-0001"));
    assert!(first.contains("transaction_hash: demo-noncrypto-"));
}

#[test]
fn show_timeline_filters_out_unrelated_asset_records() {
    let mut app = DemoApp::new();
    let (first_asset_id, _) = register_demo_asset(&mut app);
    let calibration =
        app.record_calibration(RecordCalibrationInput::accepted(first_asset_id.clone()));
    assert_eq!(calibration.consistency_status, ConsistencyStatus::Complete);

    let mut second_input = RegisterAssetInput::demo();
    second_input.identity_label = "Scale B-200".to_owned();
    second_input.serial_number = "SN-200".to_owned();
    let second_registration = app.register_asset(second_input);
    assert_eq!(
        second_registration.consistency_status,
        ConsistencyStatus::Complete
    );
    assert_eq!(
        second_registration
            .created_object_id
            .as_ref()
            .unwrap()
            .as_str(),
        "asset-0002"
    );

    let timeline = app.show_timeline(&first_asset_id).unwrap();

    assert!(timeline.contains("event-0001"));
    assert!(timeline.contains("event-0002"));
    assert!(timeline.contains("event-0003"));
    assert!(timeline.contains("txn-0001"));
    assert!(timeline.contains("txn-0002"));
    assert!(!timeline.contains("asset-0002"));
    assert!(!timeline.contains("event-0004"));
    assert!(!timeline.contains("txn-0003"));
}

#[test]
fn run_demo_output_is_byte_identical_for_fixed_inputs() {
    fn run() -> String {
        DemoApp::new().run_demo()
    }

    let first = run();
    let second = run();

    assert_eq!(first, second);
    assert!(first.contains("VS-001 traceable asset registration demo"));
    assert!(first.contains("register-asset outcome"));
    assert!(first.contains("record-calibration outcome"));
    assert!(first.contains("show-asset"));
    assert!(first.contains("show-timeline"));
    assert!(first.contains("consistency_status: Complete"));
    assert!(first.contains("event 3 event-0003 asset.calibration_accepted@1"));
    assert!(first.contains("transaction 2 txn-0002 level=level-2"));
}

#[test]
fn application_layers_do_not_bypass_runtime_engine_apis() {
    let app_layer_sources = [
        ("asset_model.rs", include_str!("asset_model.rs")),
        ("clock.rs", include_str!("clock.rs")),
        ("crypto.rs", include_str!("crypto.rs")),
        ("ids.rs", include_str!("ids.rs")),
        ("main.rs", include_str!("main.rs")),
        ("outcome.rs", include_str!("outcome.rs")),
        ("presentation.rs", include_str!("presentation.rs")),
        ("scenario.rs", include_str!("scenario.rs")),
    ];

    for (file_name, source) in app_layer_sources {
        for forbidden in [
            "ObjectRecord::new",
            "EventRecord::new",
            "TransactionRecord::new",
            "inner.",
            "lock_object",
            "lock_event",
            "lock_transaction",
            ".objects",
            "open_eqms_query_engine",
            "SecurityEngine",
            "ConsistencyBoundary",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file_name} bypasses an engine boundary with {forbidden}"
            );
        }
    }

    let storage_source = include_str!("storage.rs");
    assert!(storage_source.contains("impl ObjectStorageProvider"));
    assert!(storage_source.contains("impl EventStorageProvider"));
    assert!(storage_source.contains("impl TransactionStore"));

    let manifest_source = include_str!("../Cargo.toml");
    assert!(!manifest_source.contains("open-eqms-query-engine"));
}
