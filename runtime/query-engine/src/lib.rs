//! Open-EQMS Query Engine implementation for SPEC-004.
//!
//! This crate implements the deterministic, finite, one-shot Query Engine core:
//! structured query validation, read-only source-provider execution,
//! per-candidate authorization filtering, consistency-boundary recording,
//! provenance, classification, projection, sorting, aggregation,
//! deterministic pagination, SavedQuery registration/replay, bounded Context
//! Packages, and local bounded Parallel Inquiry Fabric execution.
//!
//! It intentionally does not implement real Object Runtime, Event Engine, or
//! Transaction Engine adapters; Security policy evaluation; SavedQuery
//! persistence; richer multi-source Context Package population;
//! synchronization; Process Engine behavior; Rule Engine behavior; AI-agent
//! behavior; distributed inquiry execution; or SPEC-005 work.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod capabilities;
pub mod errors;
pub mod execution;
pub mod limits;
#[cfg(test)]
mod ordering;
pub mod saved_query;
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
pub use crate::saved_query::SavedQueryCatalog;
pub use crate::types::{
    saved_query_authorization_target, AggregationFunction, AggregationSpec, AuthorizationProvider,
    AuthorizationRequest, AuthorizationTarget, CallerPermissionContext, ConsistencyBoundary,
    ConsistencyBoundaryUnavailable, ConsistencyBoundaryUnavailableReason, ConsistencySlot,
    ContextPackage, ContextPackageId, ContextPackageRequest, ContinuationToken, EvidenceConflict,
    GroupSpec, InquiryBranchDefinition, InquiryBranchId, InquiryBranchReport, InquiryBranchStatus,
    InquiryEvidence, InquiryExecutionMode, InquiryExecutionPolicy, InquiryExecutionRequest,
    InquiryId, InquiryResult, Pagination, PartialResultPolicy, PermissionAction, Predicate,
    PresentationType, Projection, QueryCompleteness, QueryDefinition, QueryExecutionRequest,
    QueryRecord, QueryResult, QueryResultId, QueryResultItem, QuerySchema, QuerySourceRef,
    ResultClassification, ResultProvenance, SavedQueryChange, SavedQueryDefinition,
    SavedQueryExecutionRequest, SavedQueryId, SavedQueryValidationStatus, SortDirection, SortSpec,
    SourceCapabilities, SourceCapability, StableConsistencyMarker, StableOrderingKey,
    TemporalScope, TraversalDirection, TraversalSpec, Version, CURRENT_SAVED_QUERY_SCHEMA_VERSION,
};
pub use crate::validation::{
    validate_context_package_request, validate_inquiry_execution_request,
    validate_query_definition, validate_query_definition_against_schema,
    validate_query_execution_request, validate_query_record_against_schema,
    validate_saved_query_definition,
};

#[cfg(test)]
mod tests;
