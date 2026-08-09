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
use open_eqms_object_runtime::types::{ObjectRecord, ObjectTypeDefinition, ObjectTypeRef, Version};
use open_eqms_object_runtime::ObjectRuntime;
use open_eqms_runtime_contracts::ObjectId;
use open_eqms_transaction_engine::crypto::CryptographicProvider;
use open_eqms_transaction_engine::storage::TransactionIdGenerator;
use std::collections::BTreeMap;

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
