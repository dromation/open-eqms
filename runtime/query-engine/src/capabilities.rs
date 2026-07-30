//! Query source capability contracts.

use crate::errors::{unknown_source, unsupported_capability, QueryEngineResult};
use crate::types::{
    Projection, QueryDefinition, QuerySchema, QuerySourceRef, SourceCapabilities, SourceCapability,
    TemporalScope,
};
use crate::validation::{validate_query_definition, validate_query_definition_against_schema};

/// Capability-only contract for a source that can be queried later by an executor.
pub trait QuerySourceProvider {
    /// Stable source reference represented by this provider.
    fn source_ref(&self) -> &QuerySourceRef;

    /// Declared query capabilities for this source.
    fn capabilities(&self) -> SourceCapabilities;

    /// Declared field schema for structural validation.
    fn describe_schema(&self) -> QueryEngineResult<QuerySchema>;
}

/// Validates that a query is structurally valid and supported by one declared source.
pub fn validate_query_source_contract<P: QuerySourceProvider>(
    provider: &P,
    query: &QueryDefinition,
) -> QueryEngineResult<()> {
    validate_query_definition(query)?;
    if provider.source_ref() != &query.source {
        return Err(unknown_source(query.source.clone()));
    }

    ensure_capability_support(provider.source_ref(), provider.capabilities(), query)?;
    let schema = provider.describe_schema()?;
    validate_query_definition_against_schema(query, &schema)
}

fn ensure_capability_support(
    source: &QuerySourceRef,
    capabilities: SourceCapabilities,
    query: &QueryDefinition,
) -> QueryEngineResult<()> {
    if !capabilities.temporal_scope && !matches!(query.temporal_scope, TemporalScope::Current) {
        return Err(unsupported_capability(
            source.clone(),
            SourceCapability::TemporalScope,
        ));
    }
    if !capabilities.traversal && query.traversal.is_some() {
        return Err(unsupported_capability(
            source.clone(),
            SourceCapability::Traversal,
        ));
    }
    if !capabilities.predicates && query.predicate.is_some() {
        return Err(unsupported_capability(
            source.clone(),
            SourceCapability::Predicate,
        ));
    }
    if !capabilities.projection && matches!(query.projection, Projection::SelectedFields(_)) {
        return Err(unsupported_capability(
            source.clone(),
            SourceCapability::Projection,
        ));
    }
    if !capabilities.sorting && !query.sort.is_empty() {
        return Err(unsupported_capability(
            source.clone(),
            SourceCapability::Sorting,
        ));
    }
    if !capabilities.grouping && query.group.is_some() {
        return Err(unsupported_capability(
            source.clone(),
            SourceCapability::Grouping,
        ));
    }
    if !capabilities.aggregation && !query.aggregations.is_empty() {
        return Err(unsupported_capability(
            source.clone(),
            SourceCapability::Aggregation,
        ));
    }

    Ok(())
}
