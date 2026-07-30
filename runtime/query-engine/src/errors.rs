//! Structured Query Engine error taxonomy for the approved partial scope.

use std::fmt;

use crate::types::{QuerySourceRef, SourceCapability};

/// Result type returned by Query Engine contract and validation APIs.
pub type QueryEngineResult<T> = Result<T, QueryEngineError>;

/// Top-level structured errors returned by the partial Query Engine contracts.
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
