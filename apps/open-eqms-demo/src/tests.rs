use crate::asset_model::{
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
