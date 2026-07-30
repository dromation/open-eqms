//! Structured Query Engine error taxonomy for the approved partial scope.

use std::fmt;

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
}

impl fmt::Display for QueryEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedQuery { failure } => write!(formatter, "malformed query: {failure}"),
        }
    }
}

impl std::error::Error for QueryEngineError {}

/// Machine-distinguishable query validation failure detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
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
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}

/// Wraps a validation detail in the top-level malformed-query error.
pub(crate) fn malformed_query(failure: ValidationError) -> QueryEngineError {
    QueryEngineError::MalformedQuery { failure }
}
