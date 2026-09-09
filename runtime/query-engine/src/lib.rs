//! Open-EQMS Query Engine implementation for SPEC-004.
//!
//! This crate implements the deterministic, finite, one-shot Query Engine core:
//! structured query validation, read-only source-provider execution,
//! per-candidate authorization filtering, consistency-boundary recording,
//! provenance, classification, projection, sorting, and aggregation.
//!
//! It intentionally does not implement real Object Runtime, Event Engine, or
//! Transaction Engine adapters; Security policy evaluation; SavedQuery
//! persistence; populated Context Packages; cursor or continuation-token
//! contracts; parallel execution; synchronization; Process Engine behavior;
//! Rule Engine behavior; AI-agent behavior; or SPEC-005 work.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capabilities;
pub mod errors;
pub mod execution;
pub mod limits;
#[cfg(test)]
mod ordering;
pub mod types;
pub mod validation;

pub use crate::capabilities::{
    validate_query_source_contract, ExecutableQuerySourceProvider, QuerySourceProvider,
};
pub use crate::errors::{QueryEngineError, QueryEngineResult, ValidationError};
pub use crate::execution::QueryEngine;
pub use crate::limits::{
    ensure_evaluation_steps_within_limit, ensure_result_count_within_limit,
    ensure_timeout_not_elapsed, validate_execution_limits, CancellationState, ExecutionLimitKind,
    ExecutionLimits, QueryTimeout,
};
pub use crate::types::{
    AggregationFunction, AggregationSpec, AuthorizationProvider, AuthorizationRequest,
    AuthorizationTarget, CallerPermissionContext, ConsistencyBoundary,
    ConsistencyBoundaryUnavailable, ConsistencyBoundaryUnavailableReason, ConsistencySlot,
    GroupSpec, PartialResultPolicy, PermissionAction, Predicate, PresentationType, Projection,
    QueryCompleteness, QueryDefinition, QueryExecutionRequest, QueryRecord, QueryResult,
    QueryResultId, QueryResultItem, QuerySchema, QuerySourceRef, ResultClassification,
    ResultProvenance, SortDirection, SortSpec, SourceCapabilities, SourceCapability,
    StableConsistencyMarker, StableOrderingKey, TemporalScope, TraversalDirection, TraversalSpec,
    Version,
};

#[cfg(test)]
mod tests;
