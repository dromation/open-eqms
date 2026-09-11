//! Structured Process Engine error taxonomy.

use std::fmt;

use open_eqms_object_runtime::errors::ObjectRuntimeError;
use open_eqms_object_runtime::types::{ObjectTypeRef, PropertyValueKind};
use open_eqms_runtime_contracts::{ObjectId, Version};
use open_eqms_transaction_engine::errors::TransactionEngineError;

use crate::types::{ProcessDefinitionId, ProcessNodeId};

/// Result type returned by Process Engine APIs.
pub type ProcessEngineResult<T> = Result<T, ProcessEngineError>;

/// Top-level structured errors returned by the Process Engine.
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessEngineError {
    /// Requested Process Definition has not been registered.
    DefinitionNotFound {
        /// Missing Process Definition identifier.
        definition_id: ProcessDefinitionId,
        /// Missing Process Definition schema version.
        schema_version: Version,
    },
    /// Requested Process Definition schema version was already registered.
    DuplicateDefinition {
        /// Duplicate Process Definition identifier.
        definition_id: ProcessDefinitionId,
        /// Duplicate Process Definition schema version.
        schema_version: Version,
    },
    /// Process Definition graph is malformed.
    MalformedDefinition {
        /// Machine-distinguishable validation failure.
        failure: ProcessDefinitionValidationError,
    },
    /// Process Definition registry lock or storage failed.
    DefinitionRegistryFailure {
        /// Deterministic failure message.
        message: String,
    },
    /// Generated or supplied Unit-of-Work handle was invalid.
    UnitOfWorkFailure {
        /// Deterministic failure message.
        message: String,
    },
    /// Shared Unit-of-Work begin failed before mutation.
    UnitOfWorkBeginFailure {
        /// Deterministic failure message.
        message: String,
    },
    /// Shared Unit-of-Work commit failed before durable partial mutation.
    UnitOfWorkCommitFailure {
        /// Deterministic failure message.
        message: String,
    },
    /// Shared Unit-of-Work rollback failed while handling another failure.
    UnitOfWorkRollbackFailure {
        /// Original failure that triggered rollback.
        original: String,
        /// Rollback failure detail.
        rollback: String,
    },
    /// Object being transitioned does not exist.
    ObjectNotFound {
        /// Missing Object identifier.
        object_id: ObjectId,
    },
    /// Object Type referenced by the Object does not exist.
    ObjectTypeNotFound {
        /// Missing Object Type reference.
        object_type: ObjectTypeRef,
    },
    /// Object Type does not declare a required Process property.
    ObjectTypeMissingProcessPropertyDeclaration {
        /// Object Type that is not ready for this Process Definition.
        object_type: ObjectTypeRef,
        /// Missing property key.
        property_key: String,
    },
    /// Object Type declares a Process property with the wrong value kind.
    ObjectTypeProcessPropertyKindMismatch {
        /// Object Type that is not ready for this Process Definition.
        object_type: ObjectTypeRef,
        /// Property key with the wrong kind.
        property_key: String,
        /// Expected value kind.
        expected: PropertyValueKind,
        /// Actual value kind.
        actual: PropertyValueKind,
    },
    /// Object is already active in a different Process Definition.
    ObjectAlreadyInDifferentProcess {
        /// Active Process Definition found on the Object.
        active_definition_id: ProcessDefinitionId,
    },
    /// Stored current-node value does not name a node in the selected Process Definition.
    UnknownCurrentState {
        /// Stored value that could not be interpreted as a known current node.
        recorded_value: String,
    },
    /// No directed transition exists from the current state to the requested target node.
    NoSuchTransitionEdge {
        /// Current node, or `None` when the Object has not entered this Process yet.
        from: Option<ProcessNodeId>,
        /// Requested target node.
        to: ProcessNodeId,
    },
    /// Object Runtime returned an unexpected failure.
    ObjectRuntimeFailure {
        /// Wrapped Object Runtime error.
        source: ObjectRuntimeError,
    },
    /// Transaction Engine returned an unexpected failure.
    TransactionEngineFailure {
        /// Wrapped Transaction Engine error.
        source: TransactionEngineError,
    },
    /// Precondition checks and Object Runtime validation disagreed.
    InternalPreconditionInvariantViolated {
        /// Process property key involved in the invariant failure.
        property_key: String,
        /// Wrapped Object Runtime error.
        source: ObjectRuntimeError,
    },
}

impl fmt::Display for ProcessEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefinitionNotFound {
                definition_id,
                schema_version,
            } => write!(
                formatter,
                "process definition not found: {definition_id}@{}",
                schema_version.value()
            ),
            Self::DuplicateDefinition {
                definition_id,
                schema_version,
            } => write!(
                formatter,
                "process definition already registered: {definition_id}@{}",
                schema_version.value()
            ),
            Self::MalformedDefinition { failure } => {
                write!(formatter, "malformed process definition: {failure}")
            }
            Self::DefinitionRegistryFailure { message } => {
                write!(formatter, "process definition registry failure: {message}")
            }
            Self::UnitOfWorkFailure { message } => {
                write!(formatter, "unit of work failure: {message}")
            }
            Self::UnitOfWorkBeginFailure { message } => {
                write!(formatter, "unit of work begin failure: {message}")
            }
            Self::UnitOfWorkCommitFailure { message } => {
                write!(formatter, "unit of work commit failure: {message}")
            }
            Self::UnitOfWorkRollbackFailure { original, rollback } => write!(
                formatter,
                "rollback failed after {original}; rollback failure: {rollback}"
            ),
            Self::ObjectNotFound { object_id } => write!(formatter, "object not found: {object_id}"),
            Self::ObjectTypeNotFound { object_type } => write!(
                formatter,
                "object type not found: {}@{}",
                object_type.name, object_type.version
            ),
            Self::ObjectTypeMissingProcessPropertyDeclaration {
                object_type,
                property_key,
            } => write!(
                formatter,
                "object type {}@{} does not declare process property {property_key}",
                object_type.name, object_type.version
            ),
            Self::ObjectTypeProcessPropertyKindMismatch {
                object_type,
                property_key,
                expected,
                actual,
            } => write!(
                formatter,
                "object type {}@{} declares process property {property_key} as {actual:?}, expected {expected:?}",
                object_type.name, object_type.version
            ),
            Self::ObjectAlreadyInDifferentProcess {
                active_definition_id,
            } => write!(
                formatter,
                "object is already active in process definition {active_definition_id}"
            ),
            Self::UnknownCurrentState { recorded_value } => {
                write!(formatter, "unknown current process state {recorded_value}")
            }
            Self::NoSuchTransitionEdge { from, to } => {
                write!(formatter, "no process transition from {from:?} to {to}")
            }
            Self::ObjectRuntimeFailure { source } => {
                write!(formatter, "object runtime failure: {source}")
            }
            Self::TransactionEngineFailure { source } => {
                write!(formatter, "transaction engine failure: {source}")
            }
            Self::InternalPreconditionInvariantViolated {
                property_key,
                source,
            } => write!(
                formatter,
                "process property precondition invariant failed for {property_key}: {source}"
            ),
        }
    }
}

impl std::error::Error for ProcessEngineError {}

/// Machine-distinguishable Process Definition validation failure detail.
#[derive(Clone, Debug, PartialEq)]
pub enum ProcessDefinitionValidationError {
    /// Process Definition identifier is empty.
    EmptyDefinitionId,
    /// Process Definition schema version is zero.
    EmptySchemaVersion,
    /// Process Definition contains no nodes.
    EmptyNodeSet,
    /// Process Definition contains no entry nodes.
    EmptyEntryNodeSet,
    /// A Process node identifier is empty.
    EmptyNodeId,
    /// A Process transition name is empty.
    EmptyTransitionName {
        /// Source node of the malformed transition.
        from: ProcessNodeId,
        /// Target node of the malformed transition.
        to: ProcessNodeId,
    },
    /// An entry node is not declared in the node set.
    EntryNodeNotDeclared {
        /// Undeclared entry node.
        node: ProcessNodeId,
    },
    /// A transition source node is not declared in the node set.
    TransitionSourceNotDeclared {
        /// Undeclared source node.
        node: ProcessNodeId,
    },
    /// A transition target node is not declared in the node set.
    TransitionTargetNotDeclared {
        /// Undeclared target node.
        node: ProcessNodeId,
    },
    /// More than one transition uses the same source and target.
    DuplicateTransitionEdge {
        /// Duplicate transition source node.
        from: ProcessNodeId,
        /// Duplicate transition target node.
        to: ProcessNodeId,
    },
}

impl fmt::Display for ProcessDefinitionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDefinitionId => formatter.write_str("definition id must not be empty"),
            Self::EmptySchemaVersion => formatter.write_str("schema version must not be zero"),
            Self::EmptyNodeSet => formatter.write_str("node set must not be empty"),
            Self::EmptyEntryNodeSet => formatter.write_str("entry node set must not be empty"),
            Self::EmptyNodeId => formatter.write_str("node id must not be empty"),
            Self::EmptyTransitionName { from, to } => {
                write!(formatter, "transition from {from} to {to} must have a name")
            }
            Self::EntryNodeNotDeclared { node } => {
                write!(formatter, "entry node {node} is not declared")
            }
            Self::TransitionSourceNotDeclared { node } => {
                write!(formatter, "transition source {node} is not declared")
            }
            Self::TransitionTargetNotDeclared { node } => {
                write!(formatter, "transition target {node} is not declared")
            }
            Self::DuplicateTransitionEdge { from, to } => {
                write!(formatter, "duplicate transition edge from {from} to {to}")
            }
        }
    }
}
