//! Data contracts for the Open-EQMS Event Engine.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub use open_eqms_runtime_contracts::{ObjectId, PropertyValue, PropertyValueKind};

/// Opaque globally unique identifier for an Event.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct EventId(String);

impl EventId {
    /// Creates a new opaque Event identifier from a caller-supplied token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier token without assigning any business meaning to it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the identifier token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for EventId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Reference to a registered Event Type metadata definition.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EventTypeRef {
    /// Language-neutral Event Type name.
    pub name: String,
    /// Metadata schema version for this Event Type.
    pub version: u32,
}

impl EventTypeRef {
    /// Creates a new Event Type reference.
    pub fn new(name: impl Into<String>, version: u32) -> Self {
        Self {
            name: name.into(),
            version,
        }
    }
}

/// Strictly increasing Event Engine append-order marker.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AppendSequence(u64);

impl AppendSequence {
    /// Returns the first valid append sequence.
    pub fn first() -> Self {
        Self(1)
    }

    /// Creates an append sequence from a raw monotonic marker.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw monotonic marker.
    pub fn value(self) -> u64 {
        self.0
    }

    /// Returns the next append sequence marker.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// Reports whether this marker is outside the valid public range.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Caller-supplied timestamp token stored on an Event.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EventTimestamp(String);

impl EventTimestamp {
    /// Creates a timestamp token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored timestamp token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the timestamp token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Caller-supplied opaque source reference for an Event.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EventSource(String);

impl EventSource {
    /// Creates an Event source reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored source reference.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the source reference is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Optional opaque value grouping related Events.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CorrelationId(String);

impl CorrelationId {
    /// Creates a correlation reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored correlation token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Optional opaque value identifying what caused an Event.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CausationId(String);

impl CausationId {
    /// Creates a causation reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored causation token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Structural constraints declared by Event Type metadata.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PayloadConstraints {
    /// Minimum string or blob length, where applicable.
    pub min_length: Option<usize>,
    /// Maximum string or blob length, where applicable.
    pub max_length: Option<usize>,
    /// Minimum numeric value, where applicable.
    pub min_number: Option<f64>,
    /// Maximum numeric value, where applicable.
    pub max_number: Option<f64>,
    /// Deterministic regular-expression pattern for string-like values.
    pub pattern: Option<String>,
    /// Allowed values for enum payload fields.
    pub allowed_enum_values: BTreeSet<String>,
}

/// Definition of one payload field on an Event Type.
#[derive(Clone, Debug, PartialEq)]
pub struct PayloadFieldDefinition {
    /// Language-neutral payload field name.
    pub name: String,
    /// Required payload value kind.
    pub kind: PropertyValueKind,
    /// Whether the payload field must be present on every Event.
    pub required: bool,
    /// Structural validation constraints.
    pub constraints: PayloadConstraints,
}

impl PayloadFieldDefinition {
    /// Creates a payload field definition.
    pub fn new(
        name: impl Into<String>,
        kind: PropertyValueKind,
        required: bool,
        constraints: PayloadConstraints,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            required,
            constraints,
        }
    }
}

/// Versioned Event Type metadata definition.
#[derive(Clone, Debug, PartialEq)]
pub struct EventTypeDefinition {
    /// Name/version pair for this metadata definition.
    pub type_ref: EventTypeRef,
    /// Payload field definitions keyed by language-neutral field name.
    pub payload_fields: BTreeMap<String, PayloadFieldDefinition>,
}

impl EventTypeDefinition {
    /// Creates an Event Type definition.
    pub fn new(
        type_ref: EventTypeRef,
        payload_fields: BTreeMap<String, PayloadFieldDefinition>,
    ) -> Self {
        Self {
            type_ref,
            payload_fields,
        }
    }
}

/// Immutable Event record stored in the Business Event Log.
#[derive(Clone, Debug, PartialEq)]
pub struct EventRecord {
    /// Event identity.
    pub id: EventId,
    /// Event Type name/version this record was validated against.
    pub event_type: EventTypeRef,
    /// Strict append-order marker assigned by the Event Engine.
    pub append_sequence: AppendSequence,
    /// Caller-supplied timestamp describing when the fact happened.
    pub occurred_at: EventTimestamp,
    /// Caller-supplied timestamp describing when the Runtime accepted the Event.
    pub recorded_at: EventTimestamp,
    /// Caller-supplied opaque source reference.
    pub source: EventSource,
    /// Opaque Object references the fact concerns, without existence checks.
    pub object_refs: BTreeSet<ObjectId>,
    /// Immutable payload values keyed by language-neutral field name.
    pub payload: BTreeMap<String, PropertyValue>,
    /// Optional opaque correlation reference.
    pub correlation_id: Option<CorrelationId>,
    /// Optional opaque causation reference.
    pub causation_id: Option<CausationId>,
}

impl EventRecord {
    /// Creates a full immutable Event record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: EventId,
        event_type: EventTypeRef,
        append_sequence: AppendSequence,
        occurred_at: EventTimestamp,
        recorded_at: EventTimestamp,
        source: EventSource,
        object_refs: BTreeSet<ObjectId>,
        payload: BTreeMap<String, PropertyValue>,
        correlation_id: Option<CorrelationId>,
        causation_id: Option<CausationId>,
    ) -> Self {
        Self {
            id,
            event_type,
            append_sequence,
            occurred_at,
            recorded_at,
            source,
            object_refs,
            payload,
            correlation_id,
            causation_id,
        }
    }
}
