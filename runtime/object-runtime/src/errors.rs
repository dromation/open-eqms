//! Structured Object Runtime error taxonomy.

use crate::types::{ObjectId, ObjectTypeRef, Version};
use std::fmt;

/// Result type returned by Object Runtime APIs.
pub type ObjectRuntimeResult<T> = Result<T, ObjectRuntimeError>;

/// Top-level structured errors returned by the Object Runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectRuntimeError {
    /// Requested Object does not exist.
    ObjectNotFound {
        /// Missing Object identifier.
        object_id: ObjectId,
    },
    /// Requested Object Type has not been registered.
    ObjectTypeNotFound {
        /// Missing Object Type reference.
        object_type: ObjectTypeRef,
    },
    /// Update base version does not match the current stored version.
    VersionConflict {
        /// Object being updated.
        object_id: ObjectId,
        /// Caller-supplied base version.
        expected: Version,
        /// Current stored version.
        actual: Version,
    },
    /// Object data failed validation against registered metadata.
    ValidationFailed {
        /// Machine-distinguishable validation failure detail.
        failure: ValidationError,
    },
    /// Relation write failed structural validation.
    InvalidRelation {
        /// Machine-distinguishable relation failure detail.
        failure: InvalidRelationError,
    },
    /// Generated or supplied Object identity already exists.
    DuplicateIdentity {
        /// Duplicate Object identifier.
        object_id: ObjectId,
    },
    /// Object Type metadata is malformed or unsafe to register.
    MalformedMetadata {
        /// Machine-distinguishable metadata failure detail.
        failure: MetadataError,
    },
    /// Storage provider failed outside the Object Runtime's semantic checks.
    StorageProviderFailure {
        /// Provider failure description.
        message: String,
    },
}

impl fmt::Display for ObjectRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ObjectNotFound { object_id } => {
                write!(formatter, "object not found: {object_id}")
            }
            Self::ObjectTypeNotFound { object_type } => {
                write!(
                    formatter,
                    "object type not found: {}@{}",
                    object_type.name, object_type.version
                )
            }
            Self::VersionConflict {
                object_id,
                expected,
                actual,
            } => write!(
                formatter,
                "version conflict on {object_id}: expected {}, actual {}",
                expected.value(),
                actual.value()
            ),
            Self::ValidationFailed { failure } => write!(formatter, "validation failed: {failure}"),
            Self::InvalidRelation { failure } => write!(formatter, "invalid relation: {failure}"),
            Self::DuplicateIdentity { object_id } => {
                write!(formatter, "duplicate object identity: {object_id}")
            }
            Self::MalformedMetadata { failure } => {
                write!(formatter, "malformed metadata: {failure}")
            }
            Self::StorageProviderFailure { message } => {
                write!(formatter, "storage provider failure: {message}")
            }
        }
    }
}

impl std::error::Error for ObjectRuntimeError {}

/// Machine-distinguishable validation failure detail.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationError {
    /// Property was supplied but is not defined by Object Type metadata.
    UnknownProperty {
        /// Unknown property name.
        property: String,
    },
    /// Required property is absent.
    MissingRequiredProperty {
        /// Required property name.
        property: String,
    },
    /// Property value kind differs from metadata.
    TypeMismatch {
        /// Property name.
        property: String,
        /// Expected value kind.
        expected: String,
        /// Actual value kind.
        actual: String,
    },
    /// Property value violates structural constraints.
    ConstraintViolation {
        /// Property name.
        property: String,
        /// Deterministic reason for the violation.
        reason: String,
    },
    /// Lifecycle state token is absent.
    EmptyLifecycleState,
    /// Create or update omitted required owner.
    EmptyOwner,
    /// Create or update omitted required permission scope.
    EmptyPermissionScope,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownProperty { property } => write!(formatter, "unknown property {property}"),
            Self::MissingRequiredProperty { property } => {
                write!(formatter, "missing required property {property}")
            }
            Self::TypeMismatch {
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "property {property} expected {expected}, got {actual}"
            ),
            Self::ConstraintViolation { property, reason } => {
                write!(
                    formatter,
                    "property {property} violates constraint: {reason}"
                )
            }
            Self::EmptyLifecycleState => formatter.write_str("lifecycle state must not be empty"),
            Self::EmptyOwner => formatter.write_str("owner must not be empty"),
            Self::EmptyPermissionScope => formatter.write_str("permission scope must not be empty"),
        }
    }
}

/// Machine-distinguishable relation validation failure detail.
#[derive(Clone, Debug, PartialEq)]
pub enum InvalidRelationError {
    /// Relation type is not declared on the Object Type.
    UnknownRelationType {
        /// Unknown relation type token.
        relation_type: String,
    },
    /// Relation target Object does not exist.
    TargetDoesNotExist {
        /// Missing target Object identifier.
        target_id: ObjectId,
    },
    /// Relation cardinality constraints are not satisfied.
    CardinalityViolated {
        /// Relation type token.
        relation_type: String,
        /// Minimum required relation count.
        min: usize,
        /// Maximum allowed relation count.
        max: Option<usize>,
        /// Actual relation count.
        actual: usize,
    },
}

impl fmt::Display for InvalidRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownRelationType { relation_type } => {
                write!(formatter, "unknown relation type {relation_type}")
            }
            Self::TargetDoesNotExist { target_id } => {
                write!(formatter, "target object does not exist: {target_id}")
            }
            Self::CardinalityViolated {
                relation_type,
                min,
                max,
                actual,
            } => write!(
                formatter,
                "relation {relation_type} cardinality violated: min {min}, max {max:?}, actual {actual}"
            ),
        }
    }
}

/// Machine-distinguishable metadata validation failure detail.
#[derive(Clone, Debug, PartialEq)]
pub enum MetadataError {
    /// Object Type name is empty.
    EmptyTypeName,
    /// Object Type version is zero.
    EmptyTypeVersion,
    /// Metadata property name is empty.
    EmptyPropertyName,
    /// Property key and definition name differ.
    PropertyKeyMismatch {
        /// Property map key.
        key: String,
        /// Property definition name.
        name: String,
    },
    /// Relation type token is empty.
    EmptyRelationType,
    /// Relation key and definition token differ.
    RelationKeyMismatch {
        /// Relation map key.
        key: String,
        /// Relation definition token.
        relation_type: String,
    },
    /// Numeric or length minimum exceeds maximum.
    InvalidRange {
        /// Metadata field containing the invalid range.
        field: String,
    },
    /// Regular expression pattern could not be compiled.
    InvalidPattern {
        /// Property name containing the invalid pattern.
        property: String,
        /// Regex engine error message.
        reason: String,
    },
    /// Metadata update would invalidate existing instances without a migration marker.
    WouldInvalidateExistingInstances {
        /// Object Type reference being registered.
        object_type: ObjectTypeRef,
    },
}

impl fmt::Display for MetadataError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTypeName => formatter.write_str("object type name must not be empty"),
            Self::EmptyTypeVersion => formatter.write_str("object type version must not be zero"),
            Self::EmptyPropertyName => formatter.write_str("property name must not be empty"),
            Self::PropertyKeyMismatch { key, name } => {
                write!(formatter, "property key {key} does not match name {name}")
            }
            Self::EmptyRelationType => formatter.write_str("relation type must not be empty"),
            Self::RelationKeyMismatch { key, relation_type } => write!(
                formatter,
                "relation key {key} does not match relation type {relation_type}"
            ),
            Self::InvalidRange { field } => write!(formatter, "invalid range for {field}"),
            Self::InvalidPattern { property, reason } => {
                write!(
                    formatter,
                    "invalid pattern for property {property}: {reason}"
                )
            }
            Self::WouldInvalidateExistingInstances { object_type } => write!(
                formatter,
                "metadata update would invalidate existing instances of {}@{}",
                object_type.name, object_type.version
            ),
        }
    }
}
