//! Structural validation for query definitions.

use std::collections::BTreeSet;

use crate::errors::{
    invalid_field, invalid_source_record, malformed_query, QueryEngineResult, ValidationError,
};
use crate::limits::validate_execution_limits;
use crate::types::{
    AggregationFunction, AggregationSpec, ContextPackageRequest, GroupSpec,
    InquiryExecutionRequest, Predicate, Projection, QueryDefinition, QueryExecutionRequest,
    QueryRecord, QuerySchema, QuerySourceRef, SavedQueryDefinition, SortSpec, TemporalScope,
    TraversalSpec, CURRENT_SAVED_QUERY_SCHEMA_VERSION,
};

/// Validates a query definition against local structural rules only.
pub fn validate_query_definition(query: &QueryDefinition) -> QueryEngineResult<()> {
    if query.source.is_empty() {
        return Err(malformed_query(ValidationError::EmptyQuerySource));
    }
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
    validate_execution_limits(&query.execution_limits)?;
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

/// Validates a finite one-shot execution request.
pub fn validate_query_execution_request(request: &QueryExecutionRequest) -> QueryEngineResult<()> {
    validate_query_definition(&request.query)?;
    if request.result_id.is_empty() {
        return Err(malformed_query(ValidationError::EmptyQueryResultId));
    }
    if request
        .pagination
        .after
        .as_ref()
        .is_some_and(|token| token.is_empty())
    {
        return Err(malformed_query(ValidationError::EmptyContinuationToken));
    }
    if matches!(request.pagination.max_items, Some(0)) {
        return Err(malformed_query(ValidationError::ZeroPageSize));
    }
    Ok(())
}

/// Validates one source record against source identity and declared schema.
pub fn validate_query_record_against_schema(
    expected_source: &QuerySourceRef,
    schema: &QuerySchema,
    record: &QueryRecord,
) -> QueryEngineResult<()> {
    if &record.source != expected_source {
        return Err(invalid_source_record(
            record.source.clone(),
            record.record_id.clone(),
            "record source does not match query source",
        ));
    }
    if record.record_type.is_empty() {
        return Err(invalid_source_record(
            record.source.clone(),
            record.record_id.clone(),
            "record type must not be empty",
        ));
    }
    if record.record_id.is_empty() {
        return Err(invalid_source_record(
            record.source.clone(),
            record.record_id.clone(),
            "record identity must not be empty",
        ));
    }
    if record.ordering_key.is_empty() {
        return Err(invalid_source_record(
            record.source.clone(),
            record.record_id.clone(),
            "stable ordering key must not be empty",
        ));
    }
    for (field, value) in &record.fields {
        let Some(expected_kind) = schema.fields.get(field) else {
            return Err(invalid_source_record(
                record.source.clone(),
                record.record_id.clone(),
                format!("record field {field} is not declared by source schema"),
            ));
        };
        if &value.kind() != expected_kind {
            return Err(invalid_source_record(
                record.source.clone(),
                record.record_id.clone(),
                format!("record field {field} does not match source schema kind"),
            ));
        }
    }
    Ok(())
}

/// Validates a SavedQuery metadata record before registration or replay.
pub fn validate_saved_query_definition(
    saved_query: &SavedQueryDefinition,
) -> QueryEngineResult<()> {
    if saved_query.schema_version != CURRENT_SAVED_QUERY_SCHEMA_VERSION {
        return Err(
            crate::errors::QueryEngineError::SavedQuerySchemaVersionMismatch {
                reason: format!(
                    "expected schema version {}, got {}",
                    CURRENT_SAVED_QUERY_SCHEMA_VERSION, saved_query.schema_version
                ),
            },
        );
    }
    if saved_query.id.is_empty() {
        return Err(malformed_query(ValidationError::EmptySavedQueryId));
    }
    if saved_query.name.is_empty() {
        return Err(malformed_query(ValidationError::EmptySavedQueryName));
    }
    if saved_query.version.value() == 0 {
        return Err(malformed_query(ValidationError::ZeroSavedQueryVersion));
    }
    if saved_query.owner.is_empty() {
        return Err(malformed_query(ValidationError::EmptySavedQueryOwner));
    }
    if saved_query.access_permission_scope.is_empty() {
        return Err(malformed_query(
            ValidationError::EmptySavedQueryAccessPermissionScope,
        ));
    }
    if saved_query.change_history.is_empty() {
        return Err(malformed_query(
            ValidationError::EmptySavedQueryChangeHistory,
        ));
    }
    for change in &saved_query.change_history {
        if change.actor.is_empty() {
            return Err(malformed_query(ValidationError::EmptySavedQueryChangeActor));
        }
        if change.changed_at.is_empty() {
            return Err(malformed_query(
                ValidationError::EmptySavedQueryChangeTimestamp,
            ));
        }
    }
    validate_query_definition(&saved_query.query_definition)
}

/// Validates a Context Package request before execution.
pub fn validate_context_package_request(request: &ContextPackageRequest) -> QueryEngineResult<()> {
    if request.package_id.is_empty() {
        return Err(malformed_query(ValidationError::EmptyContextPackageId));
    }
    if request.max_items == 0 {
        return Err(malformed_query(
            ValidationError::ZeroContextPackageItemLimit,
        ));
    }
    if request.max_depth == 0 {
        return Err(malformed_query(
            ValidationError::ZeroContextPackageDepthLimit,
        ));
    }
    validate_query_execution_request(&request.execution_request)
}

/// Validates a bounded inquiry request before any branch execution.
pub fn validate_inquiry_execution_request(
    request: &InquiryExecutionRequest,
) -> QueryEngineResult<()> {
    if request.inquiry_id.is_empty() {
        return Err(malformed_query(ValidationError::EmptyInquiryId));
    }
    if request.context_package_id.is_empty() {
        return Err(malformed_query(ValidationError::EmptyContextPackageId));
    }
    if request.branches.is_empty() {
        return Err(malformed_query(ValidationError::EmptyInquiryBranches));
    }
    if request.policy.max_branch_count == 0 {
        return Err(malformed_query(
            ValidationError::ZeroInquiryBranchCountLimit,
        ));
    }
    if request.policy.max_branch_depth == 0 {
        return Err(malformed_query(
            ValidationError::ZeroInquiryBranchDepthLimit,
        ));
    }
    if request.branches.len() > request.policy.max_branch_count {
        return Err(malformed_query(
            ValidationError::InquiryBranchCountLimitExceeded,
        ));
    }

    let mut branch_ids = BTreeSet::new();
    for branch in &request.branches {
        if branch.branch_id.is_empty() {
            return Err(malformed_query(ValidationError::EmptyInquiryBranchId));
        }
        if !branch_ids.insert(branch.branch_id.clone()) {
            return Err(malformed_query(ValidationError::DuplicateInquiryBranchId));
        }
        if branch.parent_inquiry_id != request.inquiry_id {
            return Err(malformed_query(
                ValidationError::InquiryBranchParentMismatch,
            ));
        }
        if branch.purpose.is_empty() {
            return Err(malformed_query(ValidationError::EmptyInquiryBranchPurpose));
        }
        if branch.result_id.is_empty() {
            return Err(malformed_query(ValidationError::EmptyInquiryBranchResultId));
        }
        if branch.branch_depth == 0 {
            return Err(malformed_query(ValidationError::ZeroInquiryBranchDepth));
        }
        if branch.branch_depth > request.policy.max_branch_depth {
            return Err(malformed_query(
                ValidationError::InquiryBranchDepthLimitExceeded,
            ));
        }
        if branch.evidence_requirements.iter().any(String::is_empty) {
            return Err(malformed_query(
                ValidationError::EmptyInquiryEvidenceRequirement,
            ));
        }
        validate_query_definition(&branch.query)?;
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
