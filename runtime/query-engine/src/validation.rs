//! Structural validation for query definitions.

use crate::errors::{invalid_field, malformed_query, QueryEngineResult, ValidationError};
use crate::types::{
    AggregationFunction, AggregationSpec, GroupSpec, Predicate, Projection, QueryDefinition,
    QuerySchema, SortSpec, TemporalScope, TraversalSpec,
};

/// Validates a query definition against local structural rules only.
pub fn validate_query_definition(query: &QueryDefinition) -> QueryEngineResult<()> {
    validate_temporal_scope(&query.temporal_scope)?;
    if let Some(predicate) = &query.predicate {
        validate_predicate_shape(predicate)?;
    }
    validate_projection_shape(&query.projection)?;
    validate_sort_shape(&query.sort)?;
    if let Some(group) = &query.group {
        validate_group_shape(group)?;
    }
    validate_aggregation_shape(&query.aggregations)?;
    if let Some(traversal) = &query.traversal {
        validate_traversal_spec(traversal)?;
    }
    Ok(())
}

/// Validates a query definition against local structural rules and a caller-supplied schema.
pub fn validate_query_definition_against_schema(
    query: &QueryDefinition,
    schema: &QuerySchema,
) -> QueryEngineResult<()> {
    validate_query_definition(query)?;
    if let Some(predicate) = &query.predicate {
        validate_predicate_fields(predicate, schema)?;
    }
    validate_projection_fields(&query.projection, schema)?;
    validate_sort_fields(&query.sort, schema)?;
    if let Some(group) = &query.group {
        validate_group_fields(group, schema)?;
    }
    validate_aggregation_fields(&query.aggregations, schema)
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

fn validate_predicate_shape(predicate: &Predicate) -> QueryEngineResult<()> {
    match predicate {
        Predicate::AlwaysTrue
        | Predicate::Equals { .. }
        | Predicate::Exists { .. }
        | Predicate::Range { .. } => Ok(()),
        Predicate::And(children) | Predicate::Or(children) => {
            if children.is_empty() {
                return Err(malformed_query(ValidationError::EmptyPredicateBranch));
            }
            for child in children {
                validate_predicate_shape(child)?;
            }
            Ok(())
        }
        Predicate::Not(child) => validate_predicate_shape(child),
    }
}

fn validate_projection_shape(projection: &Projection) -> QueryEngineResult<()> {
    match projection {
        Projection::AllFields => Ok(()),
        Projection::SelectedFields(fields) => {
            if fields.iter().any(String::is_empty) {
                return Err(malformed_query(ValidationError::EmptyProjectionField));
            }
            Ok(())
        }
    }
}

fn validate_sort_shape(sort: &[SortSpec]) -> QueryEngineResult<()> {
    if sort.iter().any(|spec| spec.field.is_empty()) {
        return Err(malformed_query(ValidationError::EmptySortField));
    }
    Ok(())
}

fn validate_group_shape(group: &GroupSpec) -> QueryEngineResult<()> {
    if group.fields.iter().any(String::is_empty) {
        return Err(malformed_query(ValidationError::EmptyGroupField));
    }
    Ok(())
}

fn validate_aggregation_shape(aggregations: &[AggregationSpec]) -> QueryEngineResult<()> {
    for aggregation in aggregations {
        if aggregation.alias.is_empty() {
            return Err(malformed_query(ValidationError::EmptyAggregationAlias));
        }
        match aggregation.function {
            AggregationFunction::Count if aggregation.field.is_some() => {
                return Err(malformed_query(ValidationError::UnexpectedAggregationField));
            }
            AggregationFunction::Count => {}
            _ if aggregation.field.is_none() => {
                return Err(malformed_query(ValidationError::MissingAggregationField));
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_predicate_fields(predicate: &Predicate, schema: &QuerySchema) -> QueryEngineResult<()> {
    match predicate {
        Predicate::AlwaysTrue => Ok(()),
        Predicate::Equals { field, .. }
        | Predicate::Exists { field }
        | Predicate::Range { field, .. } => validate_field(schema, field),
        Predicate::And(children) | Predicate::Or(children) => {
            for child in children {
                validate_predicate_fields(child, schema)?;
            }
            Ok(())
        }
        Predicate::Not(child) => validate_predicate_fields(child, schema),
    }
}

fn validate_projection_fields(
    projection: &Projection,
    schema: &QuerySchema,
) -> QueryEngineResult<()> {
    match projection {
        Projection::AllFields => Ok(()),
        Projection::SelectedFields(fields) => {
            for field in fields {
                validate_field(schema, field)?;
            }
            Ok(())
        }
    }
}

fn validate_sort_fields(sort: &[SortSpec], schema: &QuerySchema) -> QueryEngineResult<()> {
    for spec in sort {
        validate_field(schema, &spec.field)?;
    }
    Ok(())
}

fn validate_group_fields(group: &GroupSpec, schema: &QuerySchema) -> QueryEngineResult<()> {
    for field in &group.fields {
        validate_field(schema, field)?;
    }
    Ok(())
}

fn validate_aggregation_fields(
    aggregations: &[AggregationSpec],
    schema: &QuerySchema,
) -> QueryEngineResult<()> {
    for aggregation in aggregations {
        if let Some(field) = &aggregation.field {
            validate_field(schema, field)?;
        }
    }
    Ok(())
}

fn validate_field(schema: &QuerySchema, field: &str) -> QueryEngineResult<()> {
    if !schema.contains_field(field) {
        return Err(invalid_field(field));
    }
    Ok(())
}
