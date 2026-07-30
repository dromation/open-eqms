//! Structural validation for query definitions.

use crate::errors::{malformed_query, QueryEngineResult, ValidationError};
use crate::types::{QueryDefinition, TemporalScope, TraversalSpec};

/// Validates a query definition against local structural rules only.
pub fn validate_query_definition(query: &QueryDefinition) -> QueryEngineResult<()> {
    validate_temporal_scope(&query.temporal_scope)?;
    if let Some(traversal) = &query.traversal {
        validate_traversal_spec(traversal)?;
    }
    Ok(())
}

fn validate_temporal_scope(scope: &TemporalScope) -> QueryEngineResult<()> {
    match scope {
        TemporalScope::Current | TemporalScope::AllHistory => Ok(()),
        TemporalScope::PointInTime { at } => {
            if at.is_empty() {
                return Err(malformed_query(ValidationError::EmptyTemporalPoint));
            }
            Ok(())
        }
        TemporalScope::TimeRange { from, to } => {
            if from.as_ref().is_some_and(String::is_empty)
                || to.as_ref().is_some_and(String::is_empty)
            {
                return Err(malformed_query(ValidationError::EmptyTemporalRangeBound));
            }
            if let (Some(from), Some(to)) = (from, to) {
                if from > to {
                    return Err(malformed_query(ValidationError::InvalidTemporalRange));
                }
            }
            Ok(())
        }
    }
}

fn validate_traversal_spec(traversal: &TraversalSpec) -> QueryEngineResult<()> {
    if traversal.max_depth == 0 {
        return Err(malformed_query(ValidationError::EmptyTraversalDepth));
    }
    if traversal.relation_types.iter().any(String::is_empty) {
        return Err(malformed_query(ValidationError::EmptyTraversalRelationType));
    }
    Ok(())
}
