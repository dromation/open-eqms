//! Open-EQMS Query Engine implementation for SPEC-004.
//!
//! This partial crate currently contains only Slices 1-6: query model
//! contracts, validation, source capability contracts, deterministic ordering
//! helpers, structured errors, execution limits, and finite one-shot
//! cancellation state.
//!
//! It intentionally does not implement real Object Runtime, Event Engine, or
//! Transaction Engine adapters; reading or returning real engine data;
//! permission filtering; Security integration; SavedQuery persistence; Context
//! Packages; cursor or continuation-token contracts; ConsistencyBoundary;
//! execution planners; parallel execution; synchronization; Process Engine
//! behavior; Rule Engine behavior; AI-agent behavior; or SPEC-005 work.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capabilities;
pub mod errors;
pub mod limits;
#[cfg(test)]
mod ordering;
pub mod types;
pub mod validation;

pub use crate::capabilities::{validate_query_source_contract, QuerySourceProvider};
pub use crate::errors::{QueryEngineError, QueryEngineResult, ValidationError};
pub use crate::limits::{
    ensure_evaluation_steps_within_limit, ensure_result_count_within_limit,
    ensure_timeout_not_elapsed, validate_execution_limits, CancellationState, ExecutionLimitKind,
    ExecutionLimits, QueryTimeout,
};
pub use crate::types::{
    AggregationFunction, AggregationSpec, GroupSpec, PartialResultPolicy, Predicate,
    PresentationType, Projection, QueryDefinition, QuerySchema, QuerySourceRef, SortDirection,
    SortSpec, SourceCapabilities, SourceCapability, StableOrderingKey, TemporalScope,
    TraversalDirection, TraversalSpec,
};

#[cfg(test)]
mod tests;
