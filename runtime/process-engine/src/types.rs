//! Data contracts for the Open-EQMS Process Engine.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use open_eqms_event_engine::types::{
    EventId, EventTypeDefinition, EventTypeRef, PayloadConstraints, PayloadFieldDefinition,
};
use open_eqms_runtime_contracts::{
    ObjectId, PropertyValue, PropertyValueKind, UnitOfWork, Version,
};
use open_eqms_transaction_engine::types::{
    ActorRef, DeviceRef, SiteRef, TransactionId, TransactionTimestamp,
};

const PROCESS_PROPERTY_PREFIX: &str = "process.";
const CURRENT_NODE_PROPERTY_SUFFIX: &str = ".current_node";
const LAST_TRANSACTION_PROPERTY_SUFFIX: &str = ".last_transaction_id";

macro_rules! opaque_string_type {
    ($(#[$meta:meta])* pub struct $name:ident;) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Creates a new opaque token from a caller-supplied value.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Returns the stored token without assigning business meaning.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Reports whether the stored token is empty.
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

opaque_string_type! {
    /// Opaque Process Definition identifier.
    pub struct ProcessDefinitionId;
}

opaque_string_type! {
    /// Opaque Process node identifier scoped to one Process Definition.
    pub struct ProcessNodeId;
}

/// Directed graph edge between two Process nodes.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ProcessTransition {
    /// Source node.
    pub from: ProcessNodeId,
    /// Target node.
    pub to: ProcessNodeId,
    /// Language-neutral transition name.
    pub name: String,
}

impl ProcessTransition {
    /// Creates one directed Process transition.
    pub fn new(from: ProcessNodeId, to: ProcessNodeId, name: impl Into<String>) -> Self {
        Self {
            from,
            to,
            name: name.into(),
        }
    }
}

/// Immutable Process Definition graph for one schema version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessDefinition {
    /// Stable Process Definition identifier.
    pub id: ProcessDefinitionId,
    /// Definition schema version.
    pub schema_version: Version,
    /// Nodes available in this definition version.
    pub nodes: BTreeSet<ProcessNodeId>,
    /// Directed transitions allowed by this definition version.
    pub transitions: Vec<ProcessTransition>,
    /// Nodes that may start this process on an Object with no current-node value.
    pub entry_nodes: BTreeSet<ProcessNodeId>,
}

impl ProcessDefinition {
    /// Creates an immutable Process Definition value.
    pub fn new(
        id: ProcessDefinitionId,
        schema_version: Version,
        nodes: BTreeSet<ProcessNodeId>,
        transitions: Vec<ProcessTransition>,
        entry_nodes: BTreeSet<ProcessNodeId>,
    ) -> Self {
        Self {
            id,
            schema_version,
            nodes,
            transitions,
            entry_nodes,
        }
    }
}

/// Request to record one caller-initiated Process transition.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionRequest {
    /// Object being transitioned.
    pub object_id: ObjectId,
    /// Process Definition identifier to evaluate.
    pub definition_id: ProcessDefinitionId,
    /// Process Definition schema version to evaluate.
    pub schema_version: Version,
    /// Requested target node.
    pub to: ProcessNodeId,
    /// Actor responsible for the transition.
    pub actor: ActorRef,
    /// Device reference supplied for the paired Transaction.
    pub device: DeviceRef,
    /// Optional site reference supplied for the paired Transaction.
    pub site: Option<SiteRef>,
    /// Caller-supplied edit timestamp for the transition.
    pub edit_timestamp: TransactionTimestamp,
    /// Caller-supplied server receipt timestamp for the Level 2 Transaction and Event record.
    pub server_receipt_time: TransactionTimestamp,
    /// Caller-supplied Unit-of-Work handle for the paired Object and Transaction writes.
    pub unit_of_work: UnitOfWork,
}

/// Process transition consistency status disclosed to the caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum TransitionConsistencyStatus {
    /// Object update, Transaction append, and Event append all completed.
    Complete,
    /// Object update and Transaction append committed, but the Event append failed afterward.
    PartiallyCompleted,
    /// Mutation did not complete before a durable state change.
    FailedBeforeMutation,
}

/// Result returned after a legal Process transition is attempted.
#[derive(Clone, Debug, PartialEq)]
pub struct TransitionOutcome {
    /// Object that was transitioned.
    pub object_id: ObjectId,
    /// Process Definition identifier used for the transition.
    pub definition_id: ProcessDefinitionId,
    /// Process Definition schema version used for the transition.
    pub schema_version: Version,
    /// Node observed before the transition, or `None` for an entry transition.
    pub prior_node: Option<ProcessNodeId>,
    /// Node written by the transition.
    pub new_node: ProcessNodeId,
    /// Transaction identity assigned to the paired Level 2 Transaction.
    pub transaction_id: TransactionId,
    /// Event identity assigned to the independent transition Event, when appended.
    pub event_id: Option<EventId>,
    /// Consistency status across the paired state/audit write and independent Event append.
    pub consistency_status: TransitionConsistencyStatus,
    /// Deterministic recovery guidance when a post-commit Event append failed.
    pub recovery_guidance: Option<String>,
}

/// Returns the reserved current-node property key for a Process Definition.
pub fn current_node_property_key(definition_id: &ProcessDefinitionId) -> String {
    format!(
        "{PROCESS_PROPERTY_PREFIX}{}{CURRENT_NODE_PROPERTY_SUFFIX}",
        definition_id.as_str()
    )
}

/// Returns the reserved last-Transaction chaining property key for a Process Definition.
pub fn last_transaction_property_key(definition_id: &ProcessDefinitionId) -> String {
    format!(
        "{PROCESS_PROPERTY_PREFIX}{}{LAST_TRANSACTION_PROPERTY_SUFFIX}",
        definition_id.as_str()
    )
}

/// Returns the Event Type reference used for Process transition facts.
pub fn process_transition_event_type_ref() -> EventTypeRef {
    EventTypeRef::new("process.transitioned", 1)
}

/// Returns the Event Type definition expected by Process transition Event appends.
pub fn process_transition_event_type_definition() -> EventTypeDefinition {
    let fields = [
        ("definition_id", PropertyValueKind::Text),
        ("from_node", PropertyValueKind::Text),
        ("to_node", PropertyValueKind::Text),
        ("transition_name", PropertyValueKind::Text),
        ("actor", PropertyValueKind::Text),
        ("edit_timestamp", PropertyValueKind::DateTime),
    ]
    .into_iter()
    .map(|(name, kind)| {
        (
            name.to_owned(),
            PayloadFieldDefinition::new(name, kind, true, PayloadConstraints::default()),
        )
    })
    .collect::<BTreeMap<_, _>>();

    EventTypeDefinition::new(process_transition_event_type_ref(), fields)
}

pub(crate) fn extract_current_node_definition_id(key: &str) -> Option<ProcessDefinitionId> {
    let middle = key
        .strip_prefix(PROCESS_PROPERTY_PREFIX)?
        .strip_suffix(CURRENT_NODE_PROPERTY_SUFFIX)?;
    Some(ProcessDefinitionId::new(middle))
}

pub(crate) fn process_node_value(node_id: &ProcessNodeId) -> PropertyValue {
    PropertyValue::EnumValue(node_id.as_str().to_owned())
}

pub(crate) fn absent_node_value() -> PropertyValue {
    PropertyValue::Text {
        value: "not-started".to_owned(),
        language: None,
    }
}

pub(crate) fn transaction_id_value(transaction_id: &TransactionId) -> PropertyValue {
    PropertyValue::Text {
        value: transaction_id.as_str().to_owned(),
        language: None,
    }
}
