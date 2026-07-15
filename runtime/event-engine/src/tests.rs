use super::*;
use crate::errors::{MetadataError, ValidationError};
use crate::types::{PayloadConstraints, PayloadFieldDefinition, PropertyValueKind};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier};
use std::thread;

#[derive(Default)]
struct InMemoryEventStore {
    types: BTreeMap<EventTypeRef, EventTypeDefinition>,
    events: BTreeMap<EventId, EventRecord>,
    sequence_index: BTreeMap<AppendSequence, EventId>,
    fail_next_append: bool,
}

impl StorageProvider for InMemoryEventStore {
    fn get_type(
        &self,
        event_type: &EventTypeRef,
    ) -> EventEngineResult<Option<EventTypeDefinition>> {
        Ok(self.types.get(event_type).cloned())
    }

    fn insert_type(&mut self, definition: EventTypeDefinition) -> EventEngineResult<()> {
        if self.types.contains_key(&definition.type_ref) {
            return Err(EventEngineError::MalformedMetadata {
                failure: MetadataError::EventTypeAlreadyRegistered {
                    event_type: definition.type_ref,
                },
            });
        }
        self.types.insert(definition.type_ref.clone(), definition);
        Ok(())
    }

    fn get_event(&self, event_id: &EventId) -> EventEngineResult<Option<EventRecord>> {
        Ok(self.events.get(event_id).cloned())
    }

    fn event_exists(&self, event_id: &EventId) -> EventEngineResult<bool> {
        Ok(self.events.contains_key(event_id))
    }

    fn append_event(&mut self, record: EventRecord) -> EventEngineResult<()> {
        if self.fail_next_append {
            self.fail_next_append = false;
            return Err(EventEngineError::StorageProviderFailure {
                message: "injected append failure".to_owned(),
            });
        }
        if self.events.contains_key(&record.id) {
            return Err(EventEngineError::DuplicateIdentity {
                event_id: record.id,
            });
        }
        if self.sequence_index.contains_key(&record.append_sequence) {
            return Err(EventEngineError::AppendSequenceConflict {
                append_sequence: record.append_sequence,
            });
        }

        self.sequence_index
            .insert(record.append_sequence, record.id.clone());
        self.events.insert(record.id.clone(), record);
        Ok(())
    }

    fn read_sequence_range(
        &self,
        start: AppendSequence,
        max: usize,
    ) -> EventEngineResult<Vec<EventRecord>> {
        Ok(self
            .sequence_index
            .range(start..)
            .take(max)
            .filter_map(|(_, event_id)| self.events.get(event_id).cloned())
            .collect())
    }
}

struct AlternateMemoryEventStore(InMemoryEventStore);

impl StorageProvider for AlternateMemoryEventStore {
    fn get_type(
        &self,
        event_type: &EventTypeRef,
    ) -> EventEngineResult<Option<EventTypeDefinition>> {
        self.0.get_type(event_type)
    }

    fn insert_type(&mut self, definition: EventTypeDefinition) -> EventEngineResult<()> {
        self.0.insert_type(definition)
    }

    fn get_event(&self, event_id: &EventId) -> EventEngineResult<Option<EventRecord>> {
        self.0.get_event(event_id)
    }

    fn event_exists(&self, event_id: &EventId) -> EventEngineResult<bool> {
        self.0.event_exists(event_id)
    }

    fn append_event(&mut self, record: EventRecord) -> EventEngineResult<()> {
        self.0.append_event(record)
    }

    fn read_sequence_range(
        &self,
        start: AppendSequence,
        max: usize,
    ) -> EventEngineResult<Vec<EventRecord>> {
        self.0.read_sequence_range(start, max)
    }
}

struct FixedEventIds {
    ids: Vec<EventId>,
    index: usize,
}

impl FixedEventIds {
    fn new(ids: &[&str]) -> Self {
        Self {
            ids: ids.iter().map(|id| EventId::new(*id)).collect(),
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
            .unwrap_or_else(|| EventId::new(format!("generated-event-{}", self.index)));
        self.index += 1;
        id
    }
}

struct FixedSequences {
    sequences: Vec<AppendSequence>,
    index: usize,
}

impl FixedSequences {
    fn new(sequences: &[u64]) -> Self {
        Self {
            sequences: sequences
                .iter()
                .map(|sequence| AppendSequence::new(*sequence))
                .collect(),
            index: 0,
        }
    }
}

impl AppendSequenceGenerator for FixedSequences {
    fn next_sequence(&mut self) -> AppendSequence {
        let sequence = self
            .sequences
            .get(self.index)
            .copied()
            .unwrap_or_else(|| AppendSequence::new((self.index + 1) as u64));
        self.index += 1;
        sequence
    }
}

fn type_ref() -> EventTypeRef {
    EventTypeRef::new("machine-state-changed", 1)
}

fn type_ref_v2() -> EventTypeRef {
    EventTypeRef::new("machine-state-changed", 2)
}

fn event_type(event_type: EventTypeRef) -> EventTypeDefinition {
    let mut fields = BTreeMap::new();
    fields.insert(
        "machine".to_owned(),
        PayloadFieldDefinition::new(
            "machine",
            PropertyValueKind::Reference,
            true,
            PayloadConstraints::default(),
        ),
    );
    fields.insert(
        "status".to_owned(),
        PayloadFieldDefinition::new(
            "status",
            PropertyValueKind::EnumValue,
            true,
            PayloadConstraints {
                allowed_enum_values: BTreeSet::from(["running".to_owned(), "stopped".to_owned()]),
                ..PayloadConstraints::default()
            },
        ),
    );
    fields.insert(
        "temperature".to_owned(),
        PayloadFieldDefinition::new(
            "temperature",
            PropertyValueKind::Number,
            false,
            PayloadConstraints {
                min_number: Some(0.0),
                max_number: Some(100.0),
                ..PayloadConstraints::default()
            },
        ),
    );
    fields.insert(
        "comment".to_owned(),
        PayloadFieldDefinition::new(
            "comment",
            PropertyValueKind::Text,
            false,
            PayloadConstraints {
                min_length: Some(1),
                max_length: Some(64),
                pattern: Some("^[A-Za-z0-9 -]+$".to_owned()),
                ..PayloadConstraints::default()
            },
        ),
    );

    EventTypeDefinition::new(event_type, fields)
}

fn runtime() -> EventEngine<InMemoryEventStore, FixedEventIds, FixedSequences> {
    let runtime = EventEngine::new(
        InMemoryEventStore::default(),
        FixedEventIds::new(&["event-1", "event-2", "event-3", "event-4"]),
        FixedSequences::new(&[1, 2, 3, 4]),
    );
    runtime.register_event_type(event_type(type_ref())).unwrap();
    runtime
}

fn append_request(status: &str) -> AppendEventRequest {
    let machine = ObjectId::new("missing-machine");
    let mut object_refs = BTreeSet::new();
    object_refs.insert(machine.clone());

    AppendEventRequest {
        event_type: type_ref(),
        payload: BTreeMap::from([
            ("machine".to_owned(), PropertyValue::Reference(machine)),
            (
                "status".to_owned(),
                PropertyValue::EnumValue(status.to_owned()),
            ),
        ]),
        occurred_at: EventTimestamp::new("2026-07-15T10:00:00Z"),
        recorded_at: EventTimestamp::new("2026-07-15T10:00:01Z"),
        source: EventSource::new("source-user-1"),
        object_refs,
        correlation_id: Some(CorrelationId::new("correlation-1")),
        causation_id: Some(CausationId::new("causation-1")),
    }
}

#[test]
fn append_read_and_range_are_identity_and_sequence_based() {
    let runtime = runtime();

    let first = runtime.append_event(append_request("stopped")).unwrap();
    let second = runtime.append_event(append_request("running")).unwrap();

    assert_eq!(first.event_id, EventId::new("event-1"));
    assert_eq!(first.append_sequence, AppendSequence::first());
    assert_eq!(runtime.read_event(&first.event_id).unwrap(), first.record);

    let range = runtime
        .read_sequence_range(AppendSequence::first(), 10)
        .unwrap();
    assert_eq!(range.events.len(), 2);
    assert_eq!(range.events[0].id, first.event_id);
    assert_eq!(range.events[1].id, second.event_id);
    assert_eq!(range.next_start, Some(AppendSequence::new(3)));
}

#[test]
fn missing_required_payload_field_is_rejected_before_store() {
    let runtime = runtime();
    let mut request = append_request("stopped");
    request.payload.remove("status");

    let result = runtime.append_event(request);

    assert!(matches!(
        result,
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::MissingRequiredField { .. }
        })
    ));
    assert!(runtime
        .read_sequence_range(AppendSequence::first(), 10)
        .unwrap()
        .events
        .is_empty());
}

#[test]
fn unknown_payload_field_is_rejected() {
    let runtime = runtime();
    let mut request = append_request("stopped");
    request
        .payload
        .insert("business-specific".to_owned(), PropertyValue::Boolean(true));

    let result = runtime.append_event(request);

    assert!(matches!(
        result,
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::UnknownPayloadField { .. }
        })
    ));
}

#[test]
fn payload_type_mismatch_is_rejected() {
    let runtime = runtime();
    let mut request = append_request("stopped");
    request
        .payload
        .insert("status".to_owned(), PropertyValue::Boolean(true));

    let result = runtime.append_event(request);

    assert!(matches!(
        result,
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::TypeMismatch { .. }
        })
    ));
}

#[test]
fn payload_constraint_violation_is_rejected() {
    let runtime = runtime();
    let mut request = append_request("stopped");
    request.payload.insert(
        "comment".to_owned(),
        PropertyValue::Text {
            value: "not allowed!".to_owned(),
            language: None,
        },
    );

    let result = runtime.append_event(request);

    assert!(matches!(
        result,
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::ConstraintViolation { .. }
        })
    ));
}

#[test]
fn missing_timestamps_and_source_are_rejected() {
    let runtime = runtime();
    let mut missing_occurred_at = append_request("stopped");
    missing_occurred_at.occurred_at = EventTimestamp::new("");
    assert!(matches!(
        runtime.append_event(missing_occurred_at),
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::MissingOccurredAt
        })
    ));

    let mut missing_recorded_at = append_request("stopped");
    missing_recorded_at.recorded_at = EventTimestamp::new("");
    assert!(matches!(
        runtime.append_event(missing_recorded_at),
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::MissingRecordedAt
        })
    ));

    let mut missing_source = append_request("stopped");
    missing_source.source = EventSource::new("");
    assert!(matches!(
        runtime.append_event(missing_source),
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::MissingEventSource
        })
    ));
}

#[test]
fn malformed_timestamps_are_rejected() {
    let runtime = runtime();
    let mut request = append_request("stopped");
    request.occurred_at = EventTimestamp::new("2026-07-15 10:00:00");

    let result = runtime.append_event(request);

    assert!(matches!(
        result,
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::MalformedOccurredAt { .. }
        })
    ));
}

#[test]
fn object_reference_does_not_require_object_runtime_existence() {
    let runtime = runtime();

    let result = runtime.append_event(append_request("stopped"));

    assert!(result.is_ok());
    let stored = runtime.read_event(&result.unwrap().event_id).unwrap();
    assert!(stored
        .object_refs
        .contains(&ObjectId::new("missing-machine")));
}

#[test]
fn malformed_object_reference_is_rejected_structurally_only() {
    let runtime = runtime();
    let mut request = append_request("stopped");
    request.payload.insert(
        "machine".to_owned(),
        PropertyValue::Reference(ObjectId::new("")),
    );

    let result = runtime.append_event(request);

    assert!(matches!(
        result,
        Err(EventEngineError::ValidationFailed {
            failure: ValidationError::MalformedObjectReference { .. }
        })
    ));
}

#[test]
fn duplicate_identity_error_is_distinct() {
    let runtime = EventEngine::new(
        InMemoryEventStore::default(),
        FixedEventIds::new(&["same", "same"]),
        FixedSequences::new(&[1, 2]),
    );
    runtime.register_event_type(event_type(type_ref())).unwrap();
    runtime.append_event(append_request("stopped")).unwrap();

    let result = runtime.append_event(append_request("running"));

    assert!(matches!(
        result,
        Err(EventEngineError::DuplicateIdentity { .. })
    ));
}

#[test]
fn append_sequence_conflict_error_is_distinct() {
    let runtime = EventEngine::new(
        InMemoryEventStore::default(),
        FixedEventIds::new(&["event-1", "event-2"]),
        FixedSequences::new(&[1, 1]),
    );
    runtime.register_event_type(event_type(type_ref())).unwrap();
    runtime.append_event(append_request("stopped")).unwrap();

    let result = runtime.append_event(append_request("running"));

    assert!(matches!(
        result,
        Err(EventEngineError::AppendSequenceConflict { .. })
    ));
}

#[test]
fn event_type_and_event_not_found_errors_are_distinct() {
    let runtime = runtime();
    let mut missing_type = append_request("stopped");
    missing_type.event_type = EventTypeRef::new("missing-type", 1);

    assert!(matches!(
        runtime.append_event(missing_type),
        Err(EventEngineError::EventTypeNotFound { .. })
    ));
    assert!(matches!(
        runtime.read_event(&EventId::new("missing-event")),
        Err(EventEngineError::EventNotFound { .. })
    ));
}

#[test]
fn malformed_metadata_and_duplicate_schema_are_distinct() {
    let runtime = EventEngine::new(
        InMemoryEventStore::default(),
        FixedEventIds::new(&["event-1"]),
        FixedSequences::new(&[1]),
    );
    let mut malformed = event_type(type_ref());
    malformed
        .payload_fields
        .get_mut("comment")
        .unwrap()
        .constraints
        .pattern = Some("[".to_owned());

    assert!(matches!(
        runtime.register_event_type(malformed),
        Err(EventEngineError::MalformedMetadata {
            failure: MetadataError::InvalidPattern { .. }
        })
    ));

    runtime.register_event_type(event_type(type_ref())).unwrap();
    assert!(matches!(
        runtime.register_event_type(event_type(type_ref())),
        Err(EventEngineError::MalformedMetadata {
            failure: MetadataError::EventTypeAlreadyRegistered { .. }
        })
    ));
}

#[test]
fn schema_version_is_append_only_and_prior_events_remain_unchanged() {
    let runtime = runtime();
    let first = runtime.append_event(append_request("stopped")).unwrap();

    let mut v2 = event_type(type_ref_v2());
    v2.payload_fields.insert(
        "optional-note".to_owned(),
        PayloadFieldDefinition::new(
            "optional-note",
            PropertyValueKind::Text,
            false,
            PayloadConstraints::default(),
        ),
    );
    runtime.register_event_type(v2).unwrap();

    let reread = runtime.read_event(&first.event_id).unwrap();
    assert_eq!(reread.event_type, type_ref());
    assert_eq!(reread, first.record);
}

#[test]
fn forced_storage_failure_leaves_log_unchanged() {
    let mut storage = InMemoryEventStore {
        fail_next_append: true,
        ..InMemoryEventStore::default()
    };
    storage.types.insert(type_ref(), event_type(type_ref()));
    let runtime = EventEngine::new(
        storage,
        FixedEventIds::new(&["event-1"]),
        FixedSequences::new(&[1]),
    );

    let result = runtime.append_event(append_request("stopped"));

    assert!(matches!(
        result,
        Err(EventEngineError::StorageProviderFailure { .. })
    ));
    assert!(runtime
        .read_sequence_range(AppendSequence::first(), 10)
        .unwrap()
        .events
        .is_empty());
}

#[test]
fn storage_provider_can_be_swapped_without_changing_callers() {
    fn conformance<S: StorageProvider>(storage: S) -> EventEngineResult<EventRecord> {
        let runtime = EventEngine::new(
            storage,
            FixedEventIds::new(&["event-1"]),
            FixedSequences::new(&[1]),
        );
        runtime.register_event_type(event_type(type_ref()))?;
        let appended = runtime.append_event(append_request("stopped"))?;
        runtime.read_event(&appended.event_id)
    }

    let first = conformance(InMemoryEventStore::default()).unwrap();
    let second = conformance(AlternateMemoryEventStore(InMemoryEventStore::default())).unwrap();

    assert_eq!(first, second);
}

#[test]
fn deterministic_replay_produces_identical_log() {
    fn scenario() -> String {
        let runtime = runtime();
        runtime.append_event(append_request("stopped")).unwrap();
        runtime.append_event(append_request("running")).unwrap();

        runtime
            .read_sequence_range(AppendSequence::first(), 10)
            .unwrap()
            .events
            .iter()
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    assert_eq!(scenario(), scenario());
}

#[test]
fn deterministic_error_for_repeated_invalid_input() {
    fn scenario() -> String {
        let runtime = runtime();
        let mut request = append_request("stopped");
        request
            .payload
            .insert("aaa-unknown".to_owned(), PropertyValue::Boolean(true));
        request
            .payload
            .insert("zzz-unknown".to_owned(), PropertyValue::Boolean(false));

        format!("{:?}", runtime.append_event(request).unwrap_err())
    }

    assert_eq!(scenario(), scenario());
    assert!(scenario().contains("aaa-unknown"));
}

#[test]
fn concurrent_appends_receive_distinct_sequences() {
    let runtime = runtime();
    let barrier = Arc::new(Barrier::new(2));

    let first_runtime = runtime.clone();
    let first_barrier = Arc::clone(&barrier);
    let first = thread::spawn(move || {
        first_barrier.wait();
        first_runtime.append_event(append_request("stopped"))
    });

    let second_runtime = runtime.clone();
    let second_barrier = Arc::clone(&barrier);
    let second = thread::spawn(move || {
        second_barrier.wait();
        second_runtime.append_event(append_request("running"))
    });

    let first = first.join().unwrap().unwrap();
    let second = second.join().unwrap().unwrap();
    assert_ne!(first.append_sequence, second.append_sequence);

    let range = runtime
        .read_sequence_range(AppendSequence::first(), 10)
        .unwrap();
    assert_eq!(range.events.len(), 2);
    assert_eq!(range.events[0].append_sequence, AppendSequence::new(1));
    assert_eq!(range.events[1].append_sequence, AppendSequence::new(2));
}

#[test]
fn deterministic_field_order_is_preserved_on_read() {
    let runtime = runtime();
    let mut request = append_request("stopped");
    request.payload.insert(
        "comment".to_owned(),
        PropertyValue::Text {
            value: "Alpha".to_owned(),
            language: Some("en".to_owned()),
        },
    );
    request
        .payload
        .insert("temperature".to_owned(), PropertyValue::Number(42.0));

    let appended = runtime.append_event(request).unwrap();
    let first_read = runtime.read_event(&appended.event_id).unwrap();
    let second_read = runtime.read_event(&appended.event_id).unwrap();
    let ordered_fields = first_read.payload.keys().cloned().collect::<Vec<_>>();

    assert_eq!(first_read, second_read);
    assert_eq!(
        ordered_fields,
        vec![
            "comment".to_owned(),
            "machine".to_owned(),
            "status".to_owned(),
            "temperature".to_owned()
        ]
    );
}

#[test]
fn invalid_sequence_range_is_rejected() {
    let runtime = runtime();

    let result = runtime.read_sequence_range(AppendSequence::new(0), 10);

    assert!(matches!(
        result,
        Err(EventEngineError::InvalidAppendSequenceRange { .. })
    ));
    assert!(matches!(
        runtime.read_sequence_range(AppendSequence::first(), 0),
        Err(EventEngineError::InvalidAppendSequenceRange { .. })
    ));
}

#[test]
fn public_api_has_no_update_delete_filter_search_or_aggregate_operations() {
    let public_methods = [
        "register_event_type",
        "get_event_type",
        "append_event",
        "read_event",
        "read_sequence_range",
    ];

    for method in public_methods {
        assert!(!method.contains("update"));
        assert!(!method.contains("delete"));
        assert!(!method.contains("remove"));
        assert!(!method.contains("filter"));
        assert!(!method.contains("search"));
        assert!(!method.contains("sort"));
        assert!(!method.contains("aggregate"));
        assert!(!method.contains("query"));
    }
}

#[test]
fn crate_has_no_forbidden_engine_or_package_dependencies() {
    let manifest = include_str!("../Cargo.toml");
    let implementation = [
        include_str!("lib.rs"),
        include_str!("types.rs"),
        include_str!("errors.rs"),
        include_str!("storage.rs"),
        include_str!("validation.rs"),
    ]
    .join("\n");

    for forbidden_manifest_reference in [
        "object-runtime",
        "rule-engine",
        "process-engine",
        "query-engine",
        "transaction-engine",
        "packages/",
        "packages\\\\",
        "content-package",
    ] {
        assert!(!manifest.contains(forbidden_manifest_reference));
    }

    for forbidden_code_reference in [
        "open_eqms_object_runtime",
        "open_eqms_rule_engine",
        "open_eqms_process_engine",
        "open_eqms_query_engine",
        "open_eqms_transaction_engine",
        "open_eqms_security",
        "object_runtime::",
        "rule_engine::",
        "process_engine::",
        "query_engine::",
        "transaction_engine::",
        "security::",
        "packages/",
        "packages\\\\",
        "content_package::",
    ] {
        assert!(!implementation.contains(forbidden_code_reference));
    }
}

#[test]
fn implementation_does_not_use_wall_clock_sources() {
    let implementation = [
        include_str!("lib.rs"),
        include_str!("types.rs"),
        include_str!("errors.rs"),
        include_str!("storage.rs"),
        include_str!("validation.rs"),
    ]
    .join("\n");

    assert!(!implementation.contains("SystemTime"));
    assert!(!implementation.contains("Instant"));
    assert!(!implementation.contains("chrono"));
    assert!(!implementation.contains("now()"));
    assert!(!implementation.contains("now_"));
}
