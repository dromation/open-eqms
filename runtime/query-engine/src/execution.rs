//! Finite one-shot Query Engine execution.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::capabilities::{validate_query_source_contract, ExecutableQuerySourceProvider};
use crate::errors::{
    authorization_provider_unavailable, consistency_boundary_unavailable, invalid_source_record,
    saved_query_catalog_unavailable, saved_query_not_found, QueryEngineError, QueryEngineResult,
};
use crate::limits::{
    ensure_evaluation_steps_within_limit, ensure_result_count_within_limit, CancellationState,
};
#[cfg(test)]
use crate::ordering::canonical_query_bytes;
use crate::saved_query::SavedQueryCatalog;
use crate::types::{
    saved_query_authorization_target, AggregationFunction, AggregationSpec, AuthorizationEffect,
    AuthorizationProvider, AuthorizationRequest, AuthorizationTarget, CallerPermissionContext,
    ConsistencyBoundary, ConsistencySlot, ContextPackage, ContextPackageRequest, ContinuationToken,
    EvidenceConflict, InquiryBranchId, InquiryBranchReport, InquiryBranchStatus, InquiryEvidence,
    InquiryExecutionMode, InquiryExecutionRequest, Pagination, PartialResultPolicy,
    PermissionAction, Predicate, Projection, PropertyValue, QueryCompleteness, QueryDefinition,
    QueryExecutionRequest, QueryRecord, QueryResult, QueryResultItem, ResultClassification,
    ResultProvenance, SavedQueryDefinition, SavedQueryExecutionRequest, SortDirection, SortSpec,
    StableOrderingKey, Version,
};
use crate::validation::{
    validate_context_package_request, validate_inquiry_execution_request,
    validate_query_execution_request, validate_query_record_against_schema,
    validate_saved_query_definition,
};

/// SPEC-004 Query Engine facade for finite one-shot execution.
#[derive(Clone, Debug, Default)]
pub struct QueryEngine;

impl QueryEngine {
    /// Creates a Query Engine facade.
    pub fn new() -> Self {
        Self
    }

    /// Executes one finite query with a fresh non-cancelled state.
    pub fn execute_one_shot<P, A>(
        &self,
        provider: &P,
        authorization_provider: &A,
        request: QueryExecutionRequest,
    ) -> QueryEngineResult<QueryResult>
    where
        P: ExecutableQuerySourceProvider + ?Sized,
        A: AuthorizationProvider,
        A::Error: fmt::Display,
    {
        let cancellation = CancellationState::new();
        self.execute_one_shot_with_cancellation(
            provider,
            authorization_provider,
            request,
            &cancellation,
        )
    }

    /// Executes one finite query while honoring a caller-visible cancellation state.
    pub fn execute_one_shot_with_cancellation<P, A>(
        &self,
        provider: &P,
        authorization_provider: &A,
        request: QueryExecutionRequest,
        cancellation: &CancellationState,
    ) -> QueryEngineResult<QueryResult>
    where
        P: ExecutableQuerySourceProvider + ?Sized,
        A: AuthorizationProvider,
        A::Error: fmt::Display,
    {
        validate_query_execution_request(&request)?;
        validate_query_source_contract(provider, &request.query)?;
        cancellation.check_cancelled()?;

        let consistency_boundary = provider.consistency_boundary()?;
        let completeness = boundary_completeness(&request.query, &consistency_boundary)?;
        cancellation.check_cancelled()?;

        let schema = provider.describe_schema()?;
        let candidates = provider.read_records(cancellation)?;
        let mut authorized_records = Vec::new();
        let mut evaluation_steps = 0;

        for record in candidates {
            cancellation.check_cancelled()?;
            validate_query_record_against_schema(&request.query.source, &schema, &record)?;
            if !authorize_record(authorization_provider, &request.caller_context, &record)? {
                continue;
            }

            evaluation_steps += 1;
            ensure_evaluation_steps_within_limit(
                &request.query.execution_limits,
                evaluation_steps,
            )?;
            if predicate_matches(&record, request.query.predicate.as_ref())? {
                authorized_records.push(record);
            }
        }

        order_records(&mut authorized_records, &request.query.sort);
        let items = if request.query.aggregations.is_empty() {
            records_to_items(&request.query, authorized_records)
        } else {
            aggregate_records(&request.query, &authorized_records)?
        };
        let (items, next_cursor) =
            paginate_items(items, &request.pagination, &consistency_boundary)?;
        ensure_result_count_within_limit(&request.query.execution_limits, items.len())?;

        Ok(QueryResult {
            id: request.result_id,
            version: Version::initial(),
            query_definition: request.query.clone(),
            consistency_boundary,
            presentation_type: request.query.presentation_type,
            items,
            completeness,
            next_cursor,
        })
    }

    /// Registers one validated SavedQuery through an injected catalog.
    pub fn register_saved_query<C, A>(
        &self,
        catalog: &mut C,
        authorization_provider: &A,
        caller_context: CallerPermissionContext,
        saved_query: SavedQueryDefinition,
    ) -> QueryEngineResult<()>
    where
        C: SavedQueryCatalog,
        C::Error: fmt::Display,
        A: AuthorizationProvider,
        A::Error: fmt::Display,
    {
        validate_saved_query_definition(&saved_query)?;
        let target = saved_query.authorization_target();
        let effect = decide_authorization(
            authorization_provider,
            caller_context,
            PermissionAction::new("query.register-saved-query"),
            target.clone(),
        )?;
        if !matches!(effect, AuthorizationEffect::Allow) {
            return Err(QueryEngineError::PermissionDenied { target });
        }

        catalog
            .save_query(saved_query)
            .map_err(|error| saved_query_catalog_unavailable(error.to_string()))
    }

    /// Executes one SavedQuery by retrieving its definition and running it once.
    pub fn execute_saved_query<C, P, A>(
        &self,
        catalog: &C,
        provider: &P,
        authorization_provider: &A,
        request: SavedQueryExecutionRequest,
    ) -> QueryEngineResult<QueryResult>
    where
        C: SavedQueryCatalog,
        C::Error: fmt::Display,
        P: ExecutableQuerySourceProvider + ?Sized,
        A: AuthorizationProvider,
        A::Error: fmt::Display,
    {
        let target = saved_query_authorization_target(&request.saved_query_id);
        let effect = decide_authorization(
            authorization_provider,
            request.caller_context.clone(),
            PermissionAction::new("query.execute-saved-query"),
            target.clone(),
        )?;
        match effect {
            AuthorizationEffect::Allow => {}
            AuthorizationEffect::Deny => return Err(QueryEngineError::PermissionDenied { target }),
            AuthorizationEffect::HiddenDeny => {
                return Err(saved_query_not_found(request.saved_query_id));
            }
        }

        let saved_query = catalog
            .load_query(&request.saved_query_id)
            .map_err(|error| saved_query_catalog_unavailable(error.to_string()))?
            .ok_or_else(|| saved_query_not_found(request.saved_query_id.clone()))?;
        validate_saved_query_definition(&saved_query)?;

        self.execute_one_shot(
            provider,
            authorization_provider,
            QueryExecutionRequest::new(
                saved_query.query_definition,
                request.caller_context,
                request.result_id,
            ),
        )
    }

    /// Builds one bounded Context Package from an ordinary one-shot query result.
    pub fn build_context_package<P, A>(
        &self,
        provider: &P,
        authorization_provider: &A,
        request: ContextPackageRequest,
    ) -> QueryEngineResult<ContextPackage>
    where
        P: ExecutableQuerySourceProvider + ?Sized,
        A: AuthorizationProvider,
        A::Error: fmt::Display,
    {
        validate_context_package_request(&request)?;
        let caller_context = request.execution_request.caller_context.clone();
        let package_id = request.package_id;
        let max_items = request.max_items;
        let max_depth = request.max_depth;
        let result =
            self.execute_one_shot(provider, authorization_provider, request.execution_request)?;
        if result.items.len() > max_items {
            return Err(QueryEngineError::ExecutionLimitExceeded {
                limit: crate::limits::ExecutionLimitKind::ResultCount,
            });
        }

        Ok(ContextPackage {
            id: package_id,
            version: Version::initial(),
            caller_context,
            max_depth,
            query_result: result,
        })
    }

    /// Executes one bounded local inquiry over structured branch query operations.
    pub fn execute_parallel_inquiry<A>(
        &self,
        providers: &[&dyn ExecutableQuerySourceProvider],
        authorization_provider: &A,
        request: InquiryExecutionRequest,
    ) -> QueryEngineResult<crate::types::InquiryResult>
    where
        A: AuthorizationProvider,
        A::Error: fmt::Display,
    {
        let cancellation = CancellationState::new();
        self.execute_parallel_inquiry_with_cancellation(
            providers,
            authorization_provider,
            request,
            &cancellation,
        )
    }

    /// Executes one bounded local inquiry while honoring caller-visible cancellation.
    pub fn execute_parallel_inquiry_with_cancellation<A>(
        &self,
        providers: &[&dyn ExecutableQuerySourceProvider],
        authorization_provider: &A,
        request: InquiryExecutionRequest,
        cancellation: &CancellationState,
    ) -> QueryEngineResult<crate::types::InquiryResult>
    where
        A: AuthorizationProvider,
        A::Error: fmt::Display,
    {
        validate_inquiry_execution_request(&request)?;
        if providers.len() != request.branches.len() {
            return Err(crate::errors::malformed_query(
                crate::errors::ValidationError::InquiryBranchProviderMismatch,
            ));
        }

        let execution_mode = select_inquiry_execution_mode(&request)?;
        let mut branch_executions = request
            .branches
            .iter()
            .cloned()
            .zip(providers.iter().copied())
            .collect::<Vec<_>>();
        branch_executions.sort_by(|(left, _), (right, _)| left.branch_id.cmp(&right.branch_id));

        let mut branch_reports = Vec::new();
        let mut completed_results = Vec::new();
        for (branch, provider) in branch_executions {
            let branch_request = QueryExecutionRequest::new(
                branch.query.clone(),
                request.caller_context.clone(),
                branch.result_id.clone(),
            );
            match self.execute_one_shot_with_cancellation(
                provider,
                authorization_provider,
                branch_request,
                cancellation,
            ) {
                Ok(result) => {
                    let item_count = result.items.len();
                    if matches!(
                        request.partial_result_policy,
                        PartialResultPolicy::RejectPartial
                    ) && !matches!(result.completeness, QueryCompleteness::Complete)
                    {
                        return Err(QueryEngineError::IncompleteExecution {
                            reason: format!(
                                "inquiry branch {} returned an incomplete result",
                                branch.branch_id.as_str()
                            ),
                        });
                    }
                    branch_reports.push(InquiryBranchReport {
                        branch_id: branch.branch_id.clone(),
                        status: InquiryBranchStatus::Completed,
                        result_id: Some(branch.result_id),
                        item_count,
                    });
                    completed_results.push((branch.branch_id, result));
                }
                Err(error) => {
                    let status = branch_status_from_error(&error);
                    if matches!(
                        request.partial_result_policy,
                        PartialResultPolicy::RejectPartial
                    ) {
                        return Err(QueryEngineError::IncompleteExecution {
                            reason: format!(
                                "inquiry branch {} did not complete: {}",
                                branch.branch_id.as_str(),
                                branch_status_reason(&status)
                            ),
                        });
                    }
                    branch_reports.push(InquiryBranchReport {
                        branch_id: branch.branch_id,
                        status,
                        result_id: None,
                        item_count: 0,
                    });
                }
            }
        }

        let consistency_boundary = shared_inquiry_boundary(&completed_results)?;
        let (evidence, conflicts) = fuse_inquiry_evidence(&completed_results);
        let completeness = if branch_reports
            .iter()
            .all(|report| matches!(report.status, InquiryBranchStatus::Completed))
            && completed_results
                .iter()
                .all(|(_, result)| matches!(result.completeness, QueryCompleteness::Complete))
        {
            QueryCompleteness::Complete
        } else {
            QueryCompleteness::Incomplete {
                reason: "one-or-more-inquiry-branches-incomplete".to_owned(),
            }
        };

        Ok(crate::types::InquiryResult {
            inquiry_id: request.inquiry_id,
            version: Version::initial(),
            context_package_id: request.context_package_id,
            caller_context: request.caller_context,
            execution_mode,
            consistency_boundary,
            evidence,
            branch_reports,
            conflicts,
            completeness,
        })
    }
}

fn select_inquiry_execution_mode(
    request: &InquiryExecutionRequest,
) -> QueryEngineResult<InquiryExecutionMode> {
    let requires_cooperative_semantics = request
        .branches
        .iter()
        .any(|branch| branch.requires_evidence_exchange);
    if !request.policy.parallel_fabric_enabled {
        if requires_cooperative_semantics {
            return Err(QueryEngineError::ExecutionStrategyUnavailable {
                reason: "cooperative inquiry requires the Parallel Inquiry Fabric".to_owned(),
            });
        }
        return Ok(InquiryExecutionMode::Linear);
    }
    if requires_cooperative_semantics
        && !matches!(
            request.policy.preferred_mode,
            InquiryExecutionMode::CooperativeParallel
        )
    {
        return Err(QueryEngineError::ExecutionStrategyUnavailable {
            reason: "cooperative inquiry requires cooperative parallel execution".to_owned(),
        });
    }

    Ok(request.policy.preferred_mode)
}

fn branch_status_from_error(error: &QueryEngineError) -> InquiryBranchStatus {
    match error {
        QueryEngineError::Cancelled => InquiryBranchStatus::Cancelled,
        QueryEngineError::Timeout { timeout } => InquiryBranchStatus::TimedOut {
            reason: format!("{} ms", timeout.milliseconds()),
        },
        QueryEngineError::SourceUnavailable { message, .. } => InquiryBranchStatus::Unavailable {
            reason: message.clone(),
        },
        QueryEngineError::ConsistencyBoundaryUnavailable { reason, .. } => {
            InquiryBranchStatus::Unavailable {
                reason: reason.canonical_token().to_owned(),
            }
        }
        QueryEngineError::ExecutionStrategyUnavailable { reason } => {
            InquiryBranchStatus::Unavailable {
                reason: reason.clone(),
            }
        }
        other => InquiryBranchStatus::Failed {
            reason: other.to_string(),
        },
    }
}

fn branch_status_reason(status: &InquiryBranchStatus) -> String {
    match status {
        InquiryBranchStatus::Completed => "completed".to_owned(),
        InquiryBranchStatus::Failed { reason }
        | InquiryBranchStatus::Unavailable { reason }
        | InquiryBranchStatus::TimedOut { reason } => reason.clone(),
        InquiryBranchStatus::Cancelled => "cancelled".to_owned(),
    }
}

fn shared_inquiry_boundary(
    completed_results: &[(InquiryBranchId, QueryResult)],
) -> QueryEngineResult<ConsistencyBoundary> {
    let Some((_, first)) = completed_results.first() else {
        return Ok(ConsistencyBoundary::empty());
    };
    for (branch_id, result) in completed_results.iter().skip(1) {
        if result.consistency_boundary != first.consistency_boundary {
            return Err(QueryEngineError::ExecutionStrategyUnavailable {
                reason: format!(
                    "inquiry branch {} resolved a different consistency boundary",
                    branch_id.as_str()
                ),
            });
        }
    }
    Ok(first.consistency_boundary.clone())
}

fn fuse_inquiry_evidence(
    completed_results: &[(InquiryBranchId, QueryResult)],
) -> (Vec<InquiryEvidence>, Vec<EvidenceConflict>) {
    let mut evidence_by_key = BTreeMap::<String, InquiryEvidence>::new();
    let mut key_by_scope = BTreeMap::<String, String>::new();
    let mut conflicts = Vec::new();

    for (branch_id, result) in completed_results {
        for item in &result.items {
            let integrity_identifier = evidence_integrity_identifier(item);
            let evidence_key = evidence_key(item, &integrity_identifier);
            let scope_key = evidence_scope_key(item);
            if let Some(previous_key) = key_by_scope.get(&scope_key) {
                if previous_key != &evidence_key {
                    let previous_integrity = evidence_by_key
                        .get(previous_key)
                        .map(|evidence| evidence.integrity_identifier.clone())
                        .unwrap_or_else(|| previous_key.clone());
                    conflicts.push(EvidenceConflict {
                        first_integrity_identifier: previous_integrity,
                        second_integrity_identifier: integrity_identifier.clone(),
                        reason: "evidence scope contains contradictory normalized content"
                            .to_owned(),
                    });
                }
            } else {
                key_by_scope.insert(scope_key, evidence_key.clone());
            }

            evidence_by_key
                .entry(evidence_key)
                .and_modify(|evidence| {
                    evidence.producing_branch_ids.insert(branch_id.clone());
                })
                .or_insert_with(|| InquiryEvidence {
                    producing_branch_ids: BTreeSet::from([branch_id.clone()]),
                    item: item.clone(),
                    integrity_identifier,
                });
        }
    }

    (evidence_by_key.into_values().collect(), conflicts)
}

fn evidence_integrity_identifier(item: &QueryResultItem) -> String {
    item.provenance
        .integrity
        .clone()
        .unwrap_or_else(|| format!("derived-evidence:{}", evidence_scope_key(item)))
}

fn evidence_scope_key(item: &QueryResultItem) -> String {
    format!(
        "source={}|type={}|records={}",
        encode_text_token(item.provenance.source.as_str()),
        item.provenance
            .source_record_type
            .as_deref()
            .map(encode_text_token)
            .unwrap_or_else(|| "none".to_owned()),
        item.provenance
            .source_record_ids
            .iter()
            .map(|record_id| encode_text_token(record_id))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn evidence_key(item: &QueryResultItem, integrity_identifier: &str) -> String {
    let mut fields = Vec::new();
    for (field, value) in &item.fields {
        fields.push(format!(
            "{}={}",
            encode_text_token(field),
            property_value_token(value)
        ));
    }
    format!(
        "{}|classification={:?}|integrity={}|fields={}",
        evidence_scope_key(item),
        item.classification,
        encode_text_token(integrity_identifier),
        fields.join(",")
    )
}

fn paginate_items(
    items: Vec<QueryResultItem>,
    pagination: &Pagination,
    consistency_boundary: &ConsistencyBoundary,
) -> QueryEngineResult<(Vec<QueryResultItem>, Option<ContinuationToken>)> {
    let start = if let Some(cursor) = &pagination.after {
        parse_cursor(cursor, consistency_boundary)?
    } else {
        0
    };
    if start > items.len() {
        return Err(QueryEngineError::InvalidCursor {
            reason: "cursor offset is beyond the result set".to_owned(),
        });
    }

    let Some(max_items) = pagination.max_items else {
        return Ok((items.into_iter().skip(start).collect(), None));
    };
    let end = start.saturating_add(max_items).min(items.len());
    let has_next_page = end < items.len();
    let page = items
        .into_iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<Vec<_>>();
    let next_cursor = if has_next_page {
        Some(make_cursor(end, consistency_boundary))
    } else {
        None
    };
    Ok((page, next_cursor))
}

fn make_cursor(offset: usize, consistency_boundary: &ConsistencyBoundary) -> ContinuationToken {
    let boundary = consistency_boundary.canonical_string();
    ContinuationToken::new(format!(
        "open-eqms.query-cursor.v1|{offset}|{}|{boundary}",
        boundary.len()
    ))
}

fn parse_cursor(
    cursor: &ContinuationToken,
    consistency_boundary: &ConsistencyBoundary,
) -> QueryEngineResult<usize> {
    let parts = cursor.as_str().splitn(4, '|').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != "open-eqms.query-cursor.v1" {
        return Err(QueryEngineError::InvalidCursor {
            reason: "cursor does not match the Query Engine cursor format".to_owned(),
        });
    }
    let offset = parts[1]
        .parse::<usize>()
        .map_err(|_| QueryEngineError::InvalidCursor {
            reason: "cursor offset is not a valid number".to_owned(),
        })?;
    let boundary_len = parts[2]
        .parse::<usize>()
        .map_err(|_| QueryEngineError::InvalidCursor {
            reason: "cursor boundary length is not a valid number".to_owned(),
        })?;
    let boundary = parts[3];
    if boundary.len() != boundary_len {
        return Err(QueryEngineError::InvalidCursor {
            reason: "cursor boundary length does not match token content".to_owned(),
        });
    }
    if boundary != consistency_boundary.canonical_string() {
        return Err(QueryEngineError::ExpiredCursor {
            reason: "cursor was produced for a different consistency boundary".to_owned(),
        });
    }
    Ok(offset)
}

fn boundary_completeness(
    query: &QueryDefinition,
    consistency_boundary: &ConsistencyBoundary,
) -> QueryEngineResult<QueryCompleteness> {
    for slot in [
        consistency_boundary.object_runtime(),
        consistency_boundary.event_engine(),
        consistency_boundary.transaction_engine(),
        consistency_boundary.computed_value_source(),
        consistency_boundary.concept_mapping(),
        consistency_boundary.query_definition(),
    ] {
        if let ConsistencySlot::Unavailable(unavailable) = slot {
            if matches!(
                query.partial_result_policy,
                PartialResultPolicy::RejectPartial
            ) {
                return Err(consistency_boundary_unavailable(
                    query.source.clone(),
                    unavailable.reason(),
                ));
            }
            return Ok(QueryCompleteness::Incomplete {
                reason: unavailable.reason().canonical_token().to_owned(),
            });
        }
    }
    Ok(QueryCompleteness::Complete)
}

fn authorize_record<A>(
    authorization_provider: &A,
    caller_context: &CallerPermissionContext,
    record: &QueryRecord,
) -> QueryEngineResult<bool>
where
    A: AuthorizationProvider,
    A::Error: fmt::Display,
{
    Ok(matches!(
        decide_authorization(
            authorization_provider,
            caller_context.clone(),
            PermissionAction::new("query.read-record"),
            record.authorization_target(),
        )?,
        AuthorizationEffect::Allow
    ))
}

fn decide_authorization<A>(
    authorization_provider: &A,
    caller_context: CallerPermissionContext,
    action: PermissionAction,
    target: AuthorizationTarget,
) -> QueryEngineResult<AuthorizationEffect>
where
    A: AuthorizationProvider,
    A::Error: fmt::Display,
{
    let request = AuthorizationRequest::new(caller_context, action, target);
    let decision = authorization_provider
        .decide(&request)
        .map_err(|error| authorization_provider_unavailable(error.to_string()))?;
    Ok(decision.effect())
}

fn predicate_matches(
    record: &QueryRecord,
    predicate: Option<&Predicate>,
) -> QueryEngineResult<bool> {
    let Some(predicate) = predicate else {
        return Ok(true);
    };

    match predicate {
        Predicate::AlwaysTrue => Ok(true),
        Predicate::Equals { field, value } => Ok(record.fields.get(field) == Some(value)),
        Predicate::Exists { field } => Ok(record.fields.contains_key(field)),
        Predicate::Range {
            field,
            lower,
            upper,
        } => {
            let Some(actual) = record.fields.get(field) else {
                return Ok(false);
            };
            if let Some(lower) = lower {
                if compare_property_values(actual, lower)
                    .is_some_and(|ordering| ordering == Ordering::Less)
                    || compare_property_values(actual, lower).is_none()
                {
                    return Ok(false);
                }
            }
            if let Some(upper) = upper {
                if compare_property_values(actual, upper)
                    .is_some_and(|ordering| ordering == Ordering::Greater)
                    || compare_property_values(actual, upper).is_none()
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Predicate::And(children) => {
            for child in children {
                if !predicate_matches(record, Some(child))? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Predicate::Or(children) => {
            for child in children {
                if predicate_matches(record, Some(child))? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Predicate::Not(child) => Ok(!predicate_matches(record, Some(child))?),
    }
}

fn order_records(records: &mut [QueryRecord], sort: &[SortSpec]) {
    records.sort_by(|left, right| {
        for spec in sort {
            let ordering = compare_optional_property_values(
                left.fields.get(&spec.field),
                right.fields.get(&spec.field),
            );
            let ordering = match spec.direction {
                SortDirection::Ascending => ordering,
                SortDirection::Descending => ordering.reverse(),
            };
            if ordering != Ordering::Equal {
                return ordering;
            }
        }
        left.ordering_key
            .segments()
            .cmp(right.ordering_key.segments())
    });
}

fn records_to_items(query: &QueryDefinition, records: Vec<QueryRecord>) -> Vec<QueryResultItem> {
    records
        .into_iter()
        .map(|record| {
            let fields = project_fields(&record.fields, &query.projection);
            let provenance = provenance_for_record(query, &record);
            QueryResultItem {
                ordering_key: record.ordering_key,
                fields,
                classification: record.classification,
                provenance,
            }
        })
        .collect()
}

fn aggregate_records(
    query: &QueryDefinition,
    records: &[QueryRecord],
) -> QueryEngineResult<Vec<QueryResultItem>> {
    let mut buckets = BTreeMap::<String, AggregateBucket>::new();
    for record in records {
        let (key, fields) = group_key_and_fields(query, record);
        buckets
            .entry(key)
            .or_insert_with(|| AggregateBucket {
                group_fields: fields,
                records: Vec::new(),
            })
            .records
            .push(record);
    }

    let mut items = Vec::new();
    for (key, bucket) in buckets {
        let mut fields = bucket.group_fields;
        for aggregation in &query.aggregations {
            let value = evaluate_aggregation(query, aggregation, &bucket.records)?;
            fields.insert(aggregation.alias.clone(), value);
        }

        let source_record_ids = bucket
            .records
            .iter()
            .map(|record| record.record_id.clone())
            .collect::<BTreeSet<_>>();
        let provenance = ResultProvenance {
            source: query.source.clone(),
            source_record_type: Some("aggregate".to_owned()),
            source_record_ids,
            query_definition: query.clone(),
            timestamps: BTreeMap::new(),
            source_version: None,
            permission_scope: None,
            integrity: None,
            access_restriction: None,
        };
        items.push(QueryResultItem {
            ordering_key: StableOrderingKey::new(vec![format!("aggregate:{key}")]),
            fields,
            classification: ResultClassification::ComputedValue,
            provenance,
        });
    }

    Ok(items)
}

struct AggregateBucket<'a> {
    group_fields: BTreeMap<String, PropertyValue>,
    records: Vec<&'a QueryRecord>,
}

fn group_key_and_fields(
    query: &QueryDefinition,
    record: &QueryRecord,
) -> (String, BTreeMap<String, PropertyValue>) {
    let Some(group) = &query.group else {
        return ("all".to_owned(), BTreeMap::new());
    };

    let mut key_parts = Vec::new();
    let mut fields = BTreeMap::new();
    for field in &group.fields {
        let value = record.fields.get(field).cloned();
        key_parts.push(format!(
            "{}={}",
            encode_text_token(field),
            value
                .as_ref()
                .map(property_value_token)
                .unwrap_or_else(|| "missing".to_owned())
        ));
        if let Some(value) = value {
            fields.insert(field.clone(), value);
        }
    }
    (key_parts.join("|"), fields)
}

fn evaluate_aggregation(
    query: &QueryDefinition,
    aggregation: &AggregationSpec,
    records: &[&QueryRecord],
) -> QueryEngineResult<PropertyValue> {
    match aggregation.function {
        AggregationFunction::Count => Ok(PropertyValue::Number(records.len() as f64)),
        AggregationFunction::Sum => Ok(PropertyValue::Number(
            numeric_values(query, aggregation, records)?
                .into_iter()
                .sum(),
        )),
        AggregationFunction::Average => {
            let values = numeric_values(query, aggregation, records)?;
            if values.is_empty() {
                return Ok(PropertyValue::Number(0.0));
            }
            Ok(PropertyValue::Number(
                values.iter().sum::<f64>() / values.len() as f64,
            ))
        }
        AggregationFunction::Min => {
            let Some(value) = records
                .iter()
                .filter_map(|record| {
                    aggregation
                        .field
                        .as_ref()
                        .and_then(|field| record.fields.get(field))
                })
                .min_by(|left, right| {
                    compare_property_values(left, right).unwrap_or(Ordering::Equal)
                })
                .cloned()
            else {
                return Ok(PropertyValue::Number(0.0));
            };
            Ok(value)
        }
        AggregationFunction::Max => {
            let Some(value) = records
                .iter()
                .filter_map(|record| {
                    aggregation
                        .field
                        .as_ref()
                        .and_then(|field| record.fields.get(field))
                })
                .max_by(|left, right| {
                    compare_property_values(left, right).unwrap_or(Ordering::Equal)
                })
                .cloned()
            else {
                return Ok(PropertyValue::Number(0.0));
            };
            Ok(value)
        }
        AggregationFunction::DistinctCount => {
            let distinct = records
                .iter()
                .filter_map(|record| {
                    aggregation
                        .field
                        .as_ref()
                        .and_then(|field| record.fields.get(field))
                })
                .map(property_value_token)
                .collect::<BTreeSet<_>>();
            Ok(PropertyValue::Number(distinct.len() as f64))
        }
    }
}

fn numeric_values(
    query: &QueryDefinition,
    aggregation: &AggregationSpec,
    records: &[&QueryRecord],
) -> QueryEngineResult<Vec<f64>> {
    let Some(field) = aggregation.field.as_ref() else {
        return Ok(Vec::new());
    };
    let mut values = Vec::new();
    for record in records {
        if let Some(value) = record.fields.get(field) {
            let PropertyValue::Number(value) = value else {
                return Err(invalid_source_record(
                    query.source.clone(),
                    record.record_id.clone(),
                    format!("aggregation field {field} is not numeric"),
                ));
            };
            if !value.is_finite() {
                return Err(invalid_source_record(
                    query.source.clone(),
                    record.record_id.clone(),
                    format!("aggregation field {field} is not finite"),
                ));
            }
            values.push(*value);
        }
    }
    Ok(values)
}

fn project_fields(
    fields: &BTreeMap<String, PropertyValue>,
    projection: &Projection,
) -> BTreeMap<String, PropertyValue> {
    match projection {
        Projection::AllFields => fields.clone(),
        Projection::SelectedFields(selected) => selected
            .iter()
            .filter_map(|field| {
                fields
                    .get(field)
                    .cloned()
                    .map(|value| (field.clone(), value))
            })
            .collect(),
    }
}

fn provenance_for_record(query: &QueryDefinition, record: &QueryRecord) -> ResultProvenance {
    ResultProvenance {
        source: record.source.clone(),
        source_record_type: Some(record.record_type.clone()),
        source_record_ids: BTreeSet::from([record.record_id.clone()]),
        query_definition: query.clone(),
        timestamps: record.timestamps.clone(),
        source_version: record.source_version,
        permission_scope: record.permission_scope.clone(),
        integrity: record.integrity.clone(),
        access_restriction: None,
    }
}

fn compare_optional_property_values(
    left: Option<&PropertyValue>,
    right: Option<&PropertyValue>,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => compare_property_values(left, right)
            .unwrap_or_else(|| property_value_token(left).cmp(&property_value_token(right))),
        (None, Some(_)) => Ordering::Less,
        (Some(_), None) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_property_values(left: &PropertyValue, right: &PropertyValue) -> Option<Ordering> {
    match (left, right) {
        (
            PropertyValue::Text {
                value: left_value,
                language: left_language,
            },
            PropertyValue::Text {
                value: right_value,
                language: right_language,
            },
        ) => Some((left_value, left_language).cmp(&(right_value, right_language))),
        (PropertyValue::LocalizedTextKey(left), PropertyValue::LocalizedTextKey(right))
        | (PropertyValue::DateTime(left), PropertyValue::DateTime(right))
        | (PropertyValue::EnumValue(left), PropertyValue::EnumValue(right))
        | (PropertyValue::AttachmentReference(left), PropertyValue::AttachmentReference(right)) => {
            Some(left.cmp(right))
        }
        (PropertyValue::Number(left), PropertyValue::Number(right)) => left.partial_cmp(right),
        (PropertyValue::Boolean(left), PropertyValue::Boolean(right)) => Some(left.cmp(right)),
        (PropertyValue::Reference(left), PropertyValue::Reference(right)) => Some(left.cmp(right)),
        (PropertyValue::BinaryBlob(left), PropertyValue::BinaryBlob(right)) => {
            Some(left.cmp(right))
        }
        _ => None,
    }
}

/// Returns crate-private canonical bytes for deterministic result comparison tests.
#[cfg(test)]
pub(crate) fn canonical_query_result_bytes(result: &QueryResult) -> Vec<u8> {
    let mut encoder = CanonicalEncoder::default();
    encoder.label("query-result");
    encoder.token(result.id.as_str());
    encoder.token(&result.version.value().to_string());
    encoder.bytes(&canonical_query_bytes(&result.query_definition));
    encoder.token(&result.consistency_boundary.canonical_string());
    encoder.label(match result.presentation_type {
        crate::types::PresentationType::RecordSet => "record-set",
        crate::types::PresentationType::LinkedObjectView => "linked-object-view",
        crate::types::PresentationType::Timeline => "timeline",
        crate::types::PresentationType::ChartReadySeries => "chart-ready-series",
        crate::types::PresentationType::Matrix => "matrix",
        crate::types::PresentationType::DocumentReferenceSet => "document-reference-set",
        crate::types::PresentationType::EvidencePackage => "evidence-package",
    });
    match &result.completeness {
        QueryCompleteness::Complete => encoder.label("complete"),
        QueryCompleteness::Incomplete { reason } => {
            encoder.label("incomplete");
            encoder.token(reason);
        }
    }
    encode_optional_string(
        &mut encoder,
        result.next_cursor.as_ref().map(|cursor| cursor.as_str()),
    );
    encoder.token(&result.items.len().to_string());
    for item in &result.items {
        encode_ordering_key(&mut encoder, &item.ordering_key);
        encode_classification(&mut encoder, item.classification);
        encode_fields(&mut encoder, &item.fields);
        encode_provenance(&mut encoder, &item.provenance);
    }
    encoder.finish()
}

#[derive(Default)]
#[cfg(test)]
struct CanonicalEncoder {
    bytes: Vec<u8>,
}

#[cfg(test)]
impl CanonicalEncoder {
    fn label(&mut self, label: &str) {
        self.token(label);
    }

    fn token(&mut self, token: &str) {
        self.bytes(token.as_bytes());
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes
            .extend_from_slice(bytes.len().to_string().as_bytes());
        self.bytes.push(b':');
        self.bytes.extend_from_slice(bytes);
        self.bytes.push(b';');
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
fn encode_fields(encoder: &mut CanonicalEncoder, fields: &BTreeMap<String, PropertyValue>) {
    encoder.token(&fields.len().to_string());
    for (field, value) in fields {
        encoder.token(field);
        encoder.token(&property_value_token(value));
    }
}

#[cfg(test)]
fn encode_ordering_key(encoder: &mut CanonicalEncoder, ordering_key: &StableOrderingKey) {
    encoder.token(&ordering_key.segments().len().to_string());
    for segment in ordering_key.segments() {
        encoder.token(segment);
    }
}

#[cfg(test)]
fn encode_classification(encoder: &mut CanonicalEncoder, classification: ResultClassification) {
    encoder.label(match classification {
        ResultClassification::SourceFact => "source-fact",
        ResultClassification::ComputedValue => "computed-value",
        ResultClassification::StatisticalCorrelation => "statistical-correlation",
        ResultClassification::AiGeneratedHypothesis => "ai-generated-hypothesis",
        ResultClassification::HumanApprovedConclusion => "human-approved-conclusion",
    });
}

#[cfg(test)]
fn encode_provenance(encoder: &mut CanonicalEncoder, provenance: &ResultProvenance) {
    encoder.token(provenance.source.as_str());
    match &provenance.source_record_type {
        Some(record_type) => {
            encoder.label("some");
            encoder.token(record_type);
        }
        None => encoder.label("none"),
    }
    encoder.token(&provenance.source_record_ids.len().to_string());
    for record_id in &provenance.source_record_ids {
        encoder.token(record_id);
    }
    encoder.bytes(&canonical_query_bytes(&provenance.query_definition));
    encode_string_map(encoder, &provenance.timestamps);
    encode_optional_version(encoder, provenance.source_version);
    encode_optional_string(encoder, provenance.permission_scope.as_deref());
    encode_optional_string(encoder, provenance.integrity.as_deref());
    encode_optional_string(encoder, provenance.access_restriction.as_deref());
}

#[cfg(test)]
fn encode_string_map(encoder: &mut CanonicalEncoder, values: &BTreeMap<String, String>) {
    encoder.token(&values.len().to_string());
    for (key, value) in values {
        encoder.token(key);
        encoder.token(value);
    }
}

#[cfg(test)]
fn encode_optional_version(encoder: &mut CanonicalEncoder, value: Option<Version>) {
    if let Some(value) = value {
        encoder.label("some");
        encoder.token(&value.value().to_string());
    } else {
        encoder.label("none");
    }
}

#[cfg(test)]
fn encode_optional_string(encoder: &mut CanonicalEncoder, value: Option<&str>) {
    if let Some(value) = value {
        encoder.label("some");
        encoder.token(value);
    } else {
        encoder.label("none");
    }
}

fn property_value_token(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Text { value, language } => {
            format!(
                "text:{}:{}",
                encode_text_token(value),
                language
                    .as_deref()
                    .map(encode_text_token)
                    .unwrap_or_else(|| "none".to_owned())
            )
        }
        PropertyValue::LocalizedTextKey(value) => {
            format!("localized-text-key:{}", encode_text_token(value))
        }
        PropertyValue::Number(value) => format!("number:{}", value.to_bits()),
        PropertyValue::Boolean(value) => format!("boolean:{value}"),
        PropertyValue::DateTime(value) => format!("datetime:{}", encode_text_token(value)),
        PropertyValue::EnumValue(value) => format!("enum:{}", encode_text_token(value)),
        PropertyValue::Reference(value) => {
            format!("reference:{}", encode_text_token(value.as_str()))
        }
        PropertyValue::AttachmentReference(value) => {
            format!("attachment-reference:{}", encode_text_token(value))
        }
        PropertyValue::BinaryBlob(value) => {
            let bytes = value
                .iter()
                .map(u8::to_string)
                .collect::<Vec<_>>()
                .join(",");
            format!("binary:{}:{bytes}", value.len())
        }
    }
}

fn encode_text_token(value: &str) -> String {
    format!("{}:{value}", value.len())
}
