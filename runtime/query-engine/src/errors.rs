//! Structured Query Engine error taxonomy for SPEC-004.

use std::fmt;

use crate::limits::{ExecutionLimitKind, QueryTimeout};
use crate::types::{
    AuthorizationTarget, ConsistencyBoundaryUnavailableReason, QuerySourceRef, SavedQueryId,
    SourceCapability,
};

/// Result type returned by Query Engine contract and validation APIs.
pub type QueryEngineResult<T> = Result<T, QueryEngineError>;

/// Top-level structured errors returned by Query Engine contracts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryEngineError {
    /// Query definition failed structural validation.
    MalformedQuery {
        /// Machine-distinguishable validation failure detail.
        failure: ValidationError,
    },
    /// Query references a field not present in the caller-supplied schema.
    InvalidField {
        /// Invalid field reference.
        field: String,
    },
    /// Query source could not be matched to the supplied source contract.
    UnknownSource {
        /// Unknown source reference.
        source: QuerySourceRef,
    },
    /// Query requires a capability not declared by the source.
    UnsupportedCapability {
        /// Source whose contract was checked.
        source: QuerySourceRef,
        /// Unsupported query capability.
        capability: SourceCapability,
    },
    /// Query execution limit was exceeded.
    ExecutionLimitExceeded {
        /// Exceeded limit kind.
        limit: ExecutionLimitKind,
    },
    /// Query timed out.
    Timeout {
        /// Timeout representation that was exceeded.
        timeout: QueryTimeout,
    },
    /// Query was cancelled before finite one-shot completion.
    Cancelled,
    /// Authorization provider could not produce a decision.
    AuthorizationProviderUnavailable {
        /// Provider-supplied failure message.
        message: String,
    },
    /// Caller is explicitly denied for a directly requested target.
    PermissionDenied {
        /// Target that was denied.
        target: AuthorizationTarget,
    },
    /// Query source or computed-value provider could not supply data.
    SourceUnavailable {
        /// Source whose provider failed.
        source: QuerySourceRef,
        /// Provider-supplied failure message.
        message: String,
    },
    /// A requested consistency-boundary component was unavailable.
    ConsistencyBoundaryUnavailable {
        /// Source whose boundary was incomplete.
        source: QuerySourceRef,
        /// Structured unavailable reason.
        reason: ConsistencyBoundaryUnavailableReason,
    },
    /// Query source returned a record that violates its declared schema.
    InvalidSourceRecord {
        /// Source whose provider returned the invalid record.
        source: QuerySourceRef,
        /// Source-local record identity.
        record_id: String,
        /// Machine-readable validation reason.
        reason: String,
    },
    /// Execution could not complete and the caller rejected partial results.
    IncompleteExecution {
        /// Machine-readable incompleteness reason token.
        reason: String,
    },
    /// SavedQuery replay encountered an incompatible schema version.
    SavedQuerySchemaVersionMismatch {
        /// Machine-readable mismatch reason.
        reason: String,
    },
    /// SavedQuery identity could not be found or must be hidden from the caller.
    SavedQueryNotFound {
        /// SavedQuery identity requested by the caller.
        saved_query_id: SavedQueryId,
    },
    /// SavedQuery catalog could not persist or retrieve metadata.
    SavedQueryCatalogUnavailable {
        /// Catalog-supplied failure message.
        message: String,
    },
    /// Continuation token is malformed.
    InvalidCursor {
        /// Machine-readable cursor failure reason.
        reason: String,
    },
    /// Continuation token is well-formed but expired for this execution boundary.
    ExpiredCursor {
        /// Machine-readable cursor expiry reason.
        reason: String,
    },
    /// Requested execution strategy cannot preserve the requested semantics.
    ExecutionStrategyUnavailable {
        /// Machine-readable strategy failure reason.
        reason: String,
    },
}

impl fmt::Display for QueryEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedQuery { failure } => write!(formatter, "malformed query: {failure}"),
            Self::InvalidField { field } => write!(formatter, "invalid query field: {field}"),
            Self::UnknownSource { source } => write!(formatter, "unknown query source: {source}"),
            Self::UnsupportedCapability { source, capability } => {
                write!(
                    formatter,
                    "query source {source} does not support {capability}"
                )
            }
            Self::ExecutionLimitExceeded { limit } => {
                write!(formatter, "query execution limit exceeded: {limit}")
            }
            Self::Timeout { timeout } => {
                write!(
                    formatter,
                    "query timed out after {} ms",
                    timeout.milliseconds()
                )
            }
            Self::Cancelled => formatter.write_str("query cancelled"),
            Self::AuthorizationProviderUnavailable { message } => {
                write!(formatter, "authorization provider unavailable: {message}")
            }
            Self::PermissionDenied { target } => write!(
                formatter,
                "permission denied for {}:{}:{}",
                target.source_namespace(),
                target.object_type(),
                target.object_identifier()
            ),
            Self::SourceUnavailable { source, message } => {
                write!(formatter, "query source {source} unavailable: {message}")
            }
            Self::ConsistencyBoundaryUnavailable { source, reason } => write!(
                formatter,
                "query source {source} could not supply consistency boundary: {}",
                reason.canonical_token()
            ),
            Self::InvalidSourceRecord {
                source,
                record_id,
                reason,
            } => write!(
                formatter,
                "query source {source} returned invalid record {record_id}: {reason}"
            ),
            Self::IncompleteExecution { reason } => {
                write!(formatter, "query execution incomplete: {reason}")
            }
            Self::SavedQuerySchemaVersionMismatch { reason } => {
                write!(formatter, "saved query schema-version mismatch: {reason}")
            }
            Self::SavedQueryNotFound { saved_query_id } => {
                write!(formatter, "saved query not found: {saved_query_id}")
            }
            Self::SavedQueryCatalogUnavailable { message } => {
                write!(formatter, "saved query catalog unavailable: {message}")
            }
            Self::InvalidCursor { reason } => write!(formatter, "invalid cursor: {reason}"),
            Self::ExpiredCursor { reason } => write!(formatter, "expired cursor: {reason}"),
            Self::ExecutionStrategyUnavailable { reason } => {
                write!(formatter, "execution strategy unavailable: {reason}")
            }
        }
    }
}

impl std::error::Error for QueryEngineError {}

/// Machine-distinguishable query validation failure detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    /// Query source token must be non-empty.
    EmptyQuerySource,
    /// Traversal depth must be greater than zero.
    EmptyTraversalDepth,
    /// Traversal relation type token must be non-empty.
    EmptyTraversalRelationType,
    /// Temporal point timestamp token must be non-empty.
    EmptyTemporalPoint,
    /// Temporal range bound token must be non-empty when supplied.
    EmptyTemporalRangeBound,
    /// Temporal range lower bound must not sort after its upper bound.
    InvalidTemporalRange,
    /// Predicate branch must contain at least one predicate.
    EmptyPredicateBranch,
    /// Projection field token must be non-empty.
    EmptyProjectionField,
    /// Sort field token must be non-empty.
    EmptySortField,
    /// Grouping field token must be non-empty.
    EmptyGroupField,
    /// Aggregation alias token must be non-empty.
    EmptyAggregationAlias,
    /// Aggregation function requires an input field.
    MissingAggregationField,
    /// Count aggregation must not supply an input field.
    UnexpectedAggregationField,
    /// Result limit must be greater than zero when supplied.
    ZeroResultLimit,
    /// Evaluation step limit must be greater than zero when supplied.
    ZeroEvaluationStepLimit,
    /// Timeout must be greater than zero when supplied.
    ZeroTimeout,
    /// Query result identity token must be non-empty.
    EmptyQueryResultId,
    /// SavedQuery identity token must be non-empty.
    EmptySavedQueryId,
    /// SavedQuery name must be non-empty.
    EmptySavedQueryName,
    /// SavedQuery owner token must be non-empty.
    EmptySavedQueryOwner,
    /// SavedQuery access-permission scope must be non-empty.
    EmptySavedQueryAccessPermissionScope,
    /// SavedQuery change history must contain at least one entry.
    EmptySavedQueryChangeHistory,
    /// SavedQuery change-history actor token must be non-empty.
    EmptySavedQueryChangeActor,
    /// SavedQuery change-history timestamp token must be non-empty.
    EmptySavedQueryChangeTimestamp,
    /// SavedQuery version must be greater than zero.
    ZeroSavedQueryVersion,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyQuerySource => formatter.write_str("query source must not be empty"),
            Self::EmptyTraversalDepth => {
                formatter.write_str("traversal max depth must be greater than zero")
            }
            Self::EmptyTraversalRelationType => {
                formatter.write_str("traversal relation type must not be empty")
            }
            Self::EmptyTemporalPoint => formatter.write_str("temporal point must not be empty"),
            Self::EmptyTemporalRangeBound => {
                formatter.write_str("temporal range bound must not be empty when supplied")
            }
            Self::InvalidTemporalRange => {
                formatter.write_str("temporal range lower bound must not sort after upper bound")
            }
            Self::EmptyPredicateBranch => formatter.write_str("predicate branch must not be empty"),
            Self::EmptyProjectionField => formatter.write_str("projection field must not be empty"),
            Self::EmptySortField => formatter.write_str("sort field must not be empty"),
            Self::EmptyGroupField => formatter.write_str("group field must not be empty"),
            Self::EmptyAggregationAlias => {
                formatter.write_str("aggregation alias must not be empty")
            }
            Self::MissingAggregationField => {
                formatter.write_str("aggregation function requires an input field")
            }
            Self::UnexpectedAggregationField => {
                formatter.write_str("count aggregation must not supply an input field")
            }
            Self::ZeroResultLimit => formatter.write_str("result limit must be greater than zero"),
            Self::ZeroEvaluationStepLimit => {
                formatter.write_str("evaluation step limit must be greater than zero")
            }
            Self::ZeroTimeout => formatter.write_str("timeout must be greater than zero"),
            Self::EmptyQueryResultId => {
                formatter.write_str("query result identity must not be empty")
            }
            Self::EmptySavedQueryId => {
                formatter.write_str("saved query identity must not be empty")
            }
            Self::EmptySavedQueryName => formatter.write_str("saved query name must not be empty"),
            Self::EmptySavedQueryOwner => {
                formatter.write_str("saved query owner must not be empty")
            }
            Self::EmptySavedQueryAccessPermissionScope => {
                formatter.write_str("saved query access-permission scope must not be empty")
            }
            Self::EmptySavedQueryChangeHistory => {
                formatter.write_str("saved query change history must not be empty")
            }
            Self::EmptySavedQueryChangeActor => {
                formatter.write_str("saved query change actor must not be empty")
            }
            Self::EmptySavedQueryChangeTimestamp => {
                formatter.write_str("saved query change timestamp must not be empty")
            }
            Self::ZeroSavedQueryVersion => {
                formatter.write_str("saved query version must be greater than zero")
            }
        }
    }
}

/// Wraps a validation detail in the top-level malformed-query error.
pub(crate) fn malformed_query(failure: ValidationError) -> QueryEngineError {
    QueryEngineError::MalformedQuery { failure }
}

/// Wraps an invalid field reference in the top-level error taxonomy.
pub(crate) fn invalid_field(field: impl Into<String>) -> QueryEngineError {
    QueryEngineError::InvalidField {
        field: field.into(),
    }
}

/// Wraps an unknown source reference in the top-level error taxonomy.
pub(crate) fn unknown_source(source: QuerySourceRef) -> QueryEngineError {
    QueryEngineError::UnknownSource { source }
}

/// Wraps an unsupported source capability in the top-level error taxonomy.
pub(crate) fn unsupported_capability(
    source: QuerySourceRef,
    capability: SourceCapability,
) -> QueryEngineError {
    QueryEngineError::UnsupportedCapability { source, capability }
}

/// Wraps an execution limit violation in the top-level error taxonomy.
pub(crate) fn execution_limit_exceeded(limit: ExecutionLimitKind) -> QueryEngineError {
    QueryEngineError::ExecutionLimitExceeded { limit }
}

/// Wraps a timeout violation in the top-level error taxonomy.
pub(crate) fn timeout_elapsed(timeout: QueryTimeout) -> QueryEngineError {
    QueryEngineError::Timeout { timeout }
}

/// Returns the top-level cancellation error.
pub(crate) fn cancelled() -> QueryEngineError {
    QueryEngineError::Cancelled
}

/// Wraps an authorization provider failure in the top-level error taxonomy.
pub(crate) fn authorization_provider_unavailable(message: impl Into<String>) -> QueryEngineError {
    QueryEngineError::AuthorizationProviderUnavailable {
        message: message.into(),
    }
}

/// Wraps an unavailable consistency boundary in the top-level error taxonomy.
pub(crate) fn consistency_boundary_unavailable(
    source: QuerySourceRef,
    reason: ConsistencyBoundaryUnavailableReason,
) -> QueryEngineError {
    QueryEngineError::ConsistencyBoundaryUnavailable { source, reason }
}

/// Wraps an invalid source record in the top-level error taxonomy.
pub(crate) fn invalid_source_record(
    source: QuerySourceRef,
    record_id: impl Into<String>,
    reason: impl Into<String>,
) -> QueryEngineError {
    QueryEngineError::InvalidSourceRecord {
        source,
        record_id: record_id.into(),
        reason: reason.into(),
    }
}

/// Wraps a saved-query catalog failure in the top-level error taxonomy.
pub(crate) fn saved_query_catalog_unavailable(message: impl Into<String>) -> QueryEngineError {
    QueryEngineError::SavedQueryCatalogUnavailable {
        message: message.into(),
    }
}

/// Wraps a missing or hidden SavedQuery identity in the top-level error taxonomy.
pub(crate) fn saved_query_not_found(saved_query_id: SavedQueryId) -> QueryEngineError {
    QueryEngineError::SavedQueryNotFound { saved_query_id }
}
