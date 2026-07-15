//! Structured Event Engine error taxonomy.

use crate::types::{AppendSequence, EventId, EventTypeRef, ObjectId};
use std::fmt;

/// Result type returned by Event Engine APIs.
pub type EventEngineResult<T> = Result<T, EventEngineError>;

/// Top-level structured errors returned by the Event Engine.
#[derive(Clone, Debug, PartialEq)]
pub enum EventEngineError {
    /// Requested Event Type has not been registered.
    EventTypeNotFound {
        /// Missing Event Type reference.
        event_type: EventTypeRef,
    },
    /// Requested Event does not exist.
    EventNotFound {
        /// Missing Event identifier.
        event_id: EventId,
    },
    /// Event append data failed validation against registered metadata.
    ValidationFailed {
        /// Machine-distinguishable validation failure detail.
        failure: ValidationError,
    },
    /// Append-sequence range request is invalid.
    InvalidAppendSequenceRange {
        /// Requested starting append sequence.
        start: AppendSequence,
        /// Requested maximum batch size.
        max: usize,
        /// Deterministic reason for rejection.
        reason: String,
    },
    /// Generated Event identity already exists.
    DuplicateIdentity {
        /// Duplicate Event identifier.
        event_id: EventId,
    },
    /// Generated append sequence already exists.
    AppendSequenceConflict {
        /// Duplicate append sequence.
        append_sequence: AppendSequence,
    },
    /// Event Type metadata is malformed or unsafe to register.
    MalformedMetadata {
        /// Machine-distinguishable metadata failure detail.
        failure: MetadataError,
    },
    /// Append sequence generator failed outside semantic checks.
    AppendSequenceAllocationFailure {
        /// Failure description.
        message: String,
    },
    /// Storage provider failed outside the Event Engine's semantic checks.
    StorageProviderFailure {
        /// Provider failure description.
        message: String,
    },
}

impl fmt::Display for EventEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EventTypeNotFound { event_type } => {
                write!(
                    formatter,
                    "event type not found: {}@{}",
                    event_type.name, event_type.version
                )
            }
            Self::EventNotFound { event_id } => write!(formatter, "event not found: {event_id}"),
            Self::ValidationFailed { failure } => write!(formatter, "validation failed: {failure}"),
            Self::InvalidAppendSequenceRange { start, max, reason } => write!(
                formatter,
                "invalid append sequence range from {} with max {}: {}",
                start.value(),
                max,
                reason
            ),
            Self::DuplicateIdentity { event_id } => {
                write!(formatter, "duplicate event identity: {event_id}")
            }
            Self::AppendSequenceConflict { append_sequence } => write!(
                formatter,
                "append sequence conflict: {}",
                append_sequence.value()
            ),
            Self::MalformedMetadata { failure } => {
                write!(formatter, "malformed metadata: {failure}")
            }
            Self::AppendSequenceAllocationFailure { message } => {
                write!(formatter, "append sequence allocation failure: {message}")
            }
            Self::StorageProviderFailure { message } => {
                write!(formatter, "storage provider failure: {message}")
            }
        }
    }
}

impl std::error::Error for EventEngineError {}

/// Machine-distinguishable validation failure detail.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationError {
    /// Payload field was supplied but is not defined by Event Type metadata.
    UnknownPayloadField {
        /// Unknown payload field name.
        field: String,
    },
    /// Required payload field is absent.
    MissingRequiredField {
        /// Required payload field name.
        field: String,
    },
    /// Payload value kind differs from metadata.
    TypeMismatch {
        /// Payload field name.
        field: String,
        /// Expected value kind.
        expected: String,
        /// Actual value kind.
        actual: String,
    },
    /// Payload value violates structural constraints.
    ConstraintViolation {
        /// Payload field name.
        field: String,
        /// Deterministic reason for the violation.
        reason: String,
    },
    /// Occurred At timestamp is absent.
    MissingOccurredAt,
    /// Occurred At timestamp is malformed.
    MalformedOccurredAt {
        /// Malformed timestamp value.
        value: String,
    },
    /// Recorded At timestamp is absent.
    MissingRecordedAt,
    /// Recorded At timestamp is malformed.
    MalformedRecordedAt {
        /// Malformed timestamp value.
        value: String,
    },
    /// Event Source is absent.
    MissingEventSource,
    /// Object reference token is structurally malformed.
    MalformedObjectReference {
        /// Field or metadata element containing the bad reference.
        field: String,
        /// Malformed Object identifier.
        object_id: ObjectId,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownPayloadField { field } => {
                write!(formatter, "unknown payload field {field}")
            }
            Self::MissingRequiredField { field } => {
                write!(formatter, "missing required payload field {field}")
            }
            Self::TypeMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "payload field {field} expected {expected}, got {actual}"
            ),
            Self::ConstraintViolation { field, reason } => {
                write!(
                    formatter,
                    "payload field {field} violates constraint: {reason}"
                )
            }
            Self::MissingOccurredAt => formatter.write_str("occurred_at must not be empty"),
            Self::MalformedOccurredAt { value } => {
                write!(formatter, "occurred_at is malformed: {value}")
            }
            Self::MissingRecordedAt => formatter.write_str("recorded_at must not be empty"),
            Self::MalformedRecordedAt { value } => {
                write!(formatter, "recorded_at is malformed: {value}")
            }
            Self::MissingEventSource => formatter.write_str("event source must not be empty"),
            Self::MalformedObjectReference { field, object_id } => {
                write!(
                    formatter,
                    "object reference {field} is malformed: {object_id}"
                )
            }
        }
    }
}

/// Machine-distinguishable metadata validation failure detail.
#[derive(Clone, Debug, PartialEq)]
pub enum MetadataError {
    /// Event Type name is empty.
    EmptyTypeName,
    /// Event Type version is zero.
    EmptyTypeVersion,
    /// Metadata payload field name is empty.
    EmptyPayloadFieldName,
    /// Payload field key and definition name differ.
    PayloadFieldKeyMismatch {
        /// Payload field map key.
        key: String,
        /// Payload field definition name.
        name: String,
    },
    /// Numeric or length minimum exceeds maximum.
    InvalidRange {
        /// Metadata field containing the invalid range.
        field: String,
    },
    /// Regular expression pattern could not be compiled.
    InvalidPattern {
        /// Payload field name containing the invalid pattern.
        field: String,
        /// Regex engine error message.
        reason: String,
    },
    /// Event Type schema version is already registered and immutable.
    EventTypeAlreadyRegistered {
        /// Existing Event Type reference.
        event_type: EventTypeRef,
    },
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTypeName => formatter.write_str("event type name must not be empty"),
            Self::EmptyTypeVersion => formatter.write_str("event type version must not be zero"),
            Self::EmptyPayloadFieldName => {
                formatter.write_str("payload field name must not be empty")
            }
            Self::PayloadFieldKeyMismatch { key, name } => {
                write!(
                    formatter,
                    "payload field key {key} does not match name {name}"
                )
            }
            Self::InvalidRange { field } => write!(formatter, "invalid range for {field}"),
            Self::InvalidPattern { field, reason } => {
                write!(
                    formatter,
                    "invalid pattern for payload field {field}: {reason}"
                )
            }
            Self::EventTypeAlreadyRegistered { event_type } => write!(
                formatter,
                "event type schema is already registered: {}@{}",
                event_type.name, event_type.version
            ),
        }
    }
}
