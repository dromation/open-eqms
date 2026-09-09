use super::*;
use crate::capabilities::{
    validate_query_source_contract, ExecutableQuerySourceProvider, QuerySourceProvider,
};
use crate::execution::canonical_query_result_bytes;
use crate::ordering::{
    canonical_query_bytes, canonical_semantic_record_bytes, compare_stable_ordering_keys,
    semantic_query_equal,
};
use crate::validation::{validate_query_definition, validate_query_definition_against_schema};
use open_eqms_runtime_contracts::{
    AuthorizationDecision, AuthorizationDecisionTrace, AuthorizationEffect, AuthorizationProvider,
    AuthorizationRequest, ConsistencyBoundary, ConsistencyBoundaryUnavailable,
    ConsistencyBoundaryUnavailableReason, ConsistencySlot, ObjectId, PropertyValue,
    PropertyValueKind, StableConsistencyMarker, Version,
};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;

fn source_ref() -> QuerySourceRef {
    QuerySourceRef::new("synthetic-source")
}

fn query() -> QueryDefinition {
    QueryDefinition::new(
        source_ref(),
        TemporalScope::Current,
        None,
        PartialResultPolicy::RejectPartial,
        PresentationType::RecordSet,
    )
}

fn schema() -> QuerySchema {
    QuerySchema::new(BTreeMap::from([
        ("status".to_owned(), PropertyValueKind::EnumValue),
        ("created_at".to_owned(), PropertyValueKind::DateTime),
        ("score".to_owned(), PropertyValueKind::Number),
    ]))
}

struct SyntheticSource {
    source: QuerySourceRef,
    capabilities: SourceCapabilities,
    schema: QuerySchema,
    records: Vec<QueryRecord>,
    boundary: ConsistencyBoundary,
}

impl SyntheticSource {
    fn new(capabilities: SourceCapabilities) -> Self {
        Self {
            source: source_ref(),
            capabilities,
            schema: schema(),
            records: Vec::new(),
            boundary: boundary(),
        }
    }

    fn with_records(mut self, records: Vec<QueryRecord>) -> Self {
        self.records = records;
        self
    }

    fn with_boundary(mut self, boundary: ConsistencyBoundary) -> Self {
        self.boundary = boundary;
        self
    }
}

impl QuerySourceProvider for SyntheticSource {
    fn source_ref(&self) -> &QuerySourceRef {
        &self.source
    }

    fn capabilities(&self) -> SourceCapabilities {
        self.capabilities
    }

    fn describe_schema(&self) -> QueryEngineResult<QuerySchema> {
        Ok(self.schema.clone())
    }
}

impl ExecutableQuerySourceProvider for SyntheticSource {
    fn consistency_boundary(&self) -> QueryEngineResult<ConsistencyBoundary> {
        Ok(self.boundary.clone())
    }

    fn read_records(
        &self,
        cancellation: &CancellationState,
    ) -> QueryEngineResult<Vec<QueryRecord>> {
        cancellation.check_cancelled()?;
        Ok(self.records.clone())
    }
}

#[derive(Default)]
struct SyntheticAuthorization {
    decisions_by_record: BTreeMap<String, AuthorizationEffect>,
    calls: RefCell<Vec<String>>,
    failure: Option<String>,
}

#[derive(Default)]
struct SyntheticSavedQueryCatalog {
    saved_queries: BTreeMap<SavedQueryId, SavedQueryDefinition>,
    failure: Option<String>,
}

impl SyntheticSavedQueryCatalog {
    fn failing(message: impl Into<String>) -> Self {
        Self {
            failure: Some(message.into()),
            ..Self::default()
        }
    }
}

impl SavedQueryCatalog for SyntheticSavedQueryCatalog {
    type Error = String;

    fn save_query(&mut self, saved_query: SavedQueryDefinition) -> Result<(), Self::Error> {
        if let Some(message) = &self.failure {
            return Err(message.clone());
        }
        self.saved_queries
            .insert(saved_query.id.clone(), saved_query);
        Ok(())
    }

    fn load_query(
        &self,
        saved_query_id: &SavedQueryId,
    ) -> Result<Option<SavedQueryDefinition>, Self::Error> {
        if let Some(message) = &self.failure {
            return Err(message.clone());
        }
        Ok(self.saved_queries.get(saved_query_id).cloned())
    }
}

impl SyntheticAuthorization {
    fn with_decision(mut self, record_id: impl Into<String>, effect: AuthorizationEffect) -> Self {
        self.decisions_by_record.insert(record_id.into(), effect);
        self
    }

    fn failing(message: impl Into<String>) -> Self {
        Self {
            failure: Some(message.into()),
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.borrow().clone()
    }
}

impl AuthorizationProvider for SyntheticAuthorization {
    type Error = String;

    fn decide(&self, request: &AuthorizationRequest) -> Result<AuthorizationDecision, Self::Error> {
        if let Some(message) = &self.failure {
            return Err(message.clone());
        }

        let record_id = request.target().object_identifier().to_owned();
        self.calls.borrow_mut().push(record_id.clone());
        let effect = self
            .decisions_by_record
            .get(&record_id)
            .copied()
            .unwrap_or(AuthorizationEffect::Allow);

        Ok(AuthorizationDecision::new(
            effect,
            AuthorizationDecisionTrace::new(
                "policy-v1",
                format!("decision-{record_id}"),
                "2026-09-09T08:00:00Z",
                "synthetic-security",
                match effect {
                    AuthorizationEffect::Allow => "allow",
                    AuthorizationEffect::Deny => "deny",
                    AuthorizationEffect::HiddenDeny => "hidden-deny",
                },
            ),
        ))
    }
}

fn boundary() -> ConsistencyBoundary {
    ConsistencyBoundary::empty().with_query_definition(ConsistencySlot::Available(
        StableConsistencyMarker::new("query-definition", "semantic-v1"),
    ))
}

fn record(record_id: &str, status: &str, score: f64) -> QueryRecord {
    QueryRecord::source_fact(
        source_ref(),
        "synthetic-record",
        record_id,
        StableOrderingKey::new(vec![record_id.to_owned()]),
        BTreeMap::from([
            (
                "status".to_owned(),
                PropertyValue::EnumValue(status.to_owned()),
            ),
            ("score".to_owned(), PropertyValue::Number(score)),
            (
                "created_at".to_owned(),
                PropertyValue::DateTime(format!("2026-09-09T08:00:0{}Z", score as u8)),
            ),
        ]),
    )
    .with_permission_scope("scope:synthetic")
    .with_source_version(Version::new(2))
    .with_integrity(format!("integrity-{record_id}"))
}

fn saved_query_definition() -> SavedQueryDefinition {
    SavedQueryDefinition::new(
        SavedQueryId::new("saved-001"),
        "Active records",
        Version::initial(),
        "quality-owner",
        "scope:quality",
        SavedQueryValidationStatus::ApprovedForRegulatedUse,
        vec![SavedQueryChange::new(
            "quality-owner",
            "2026-09-09T08:00:00Z",
            "initial approved query",
        )],
        query().with_predicate(Predicate::Equals {
            field: "status".to_owned(),
            value: PropertyValue::EnumValue("active".to_owned()),
        }),
    )
}

#[test]
fn well_formed_query_definition_is_accepted() {
    let mut relation_types = BTreeSet::new();
    relation_types.insert("uses".to_owned());
    let definition = QueryDefinition::new(
        source_ref(),
        TemporalScope::TimeRange {
            from: Some("2026-07-15T10:00:00Z".to_owned()),
            to: Some("2026-07-15T11:00:00Z".to_owned()),
        },
        Some(TraversalSpec::new(
            TraversalDirection::Outbound,
            2,
            relation_types,
        )),
        PartialResultPolicy::AllowIncomplete,
        PresentationType::Timeline,
    );

    validate_query_definition(&definition).unwrap();
}

#[test]
fn malformed_query_definition_is_rejected() {
    let mut relation_types = BTreeSet::new();
    relation_types.insert(String::new());
    let definition = QueryDefinition::new(
        source_ref(),
        TemporalScope::Current,
        Some(TraversalSpec::new(
            TraversalDirection::Both,
            1,
            relation_types,
        )),
        PartialResultPolicy::RejectPartial,
        PresentationType::LinkedObjectView,
    );

    let error = validate_query_definition(&definition).unwrap_err();

    assert!(matches!(
        error,
        QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptyTraversalRelationType
        }
    ));
}

#[test]
fn empty_temporal_and_traversal_values_are_rejected() {
    let point = QueryDefinition::new(
        source_ref(),
        TemporalScope::PointInTime { at: String::new() },
        None,
        PartialResultPolicy::RejectPartial,
        PresentationType::RecordSet,
    );
    assert!(matches!(
        validate_query_definition(&point),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptyTemporalPoint
        })
    ));

    let traversal = QueryDefinition::new(
        source_ref(),
        TemporalScope::Current,
        Some(TraversalSpec::new(
            TraversalDirection::Inbound,
            0,
            BTreeSet::new(),
        )),
        PartialResultPolicy::RejectPartial,
        PresentationType::RecordSet,
    );
    assert!(matches!(
        validate_query_definition(&traversal),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptyTraversalDepth
        })
    ));
}

#[test]
fn invalid_temporal_range_is_rejected() {
    let definition = QueryDefinition::new(
        source_ref(),
        TemporalScope::TimeRange {
            from: Some("2026-07-15T12:00:00Z".to_owned()),
            to: Some("2026-07-15T11:00:00Z".to_owned()),
        },
        None,
        PartialResultPolicy::RejectPartial,
        PresentationType::RecordSet,
    );

    assert!(matches!(
        validate_query_definition(&definition),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::InvalidTemporalRange
        })
    ));
}

#[test]
fn minimal_query_executes_with_caller_supplied_result_identity() {
    let definition = query();
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![record(
        "record-001",
        "active",
        5.0,
    )]);
    let request = QueryExecutionRequest::new(
        definition,
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-001"),
    );

    let result = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), request)
        .unwrap();

    assert_eq!("result-001", result.id.as_str());
    assert_eq!(Version::initial(), result.version);
    assert_eq!(QueryCompleteness::Complete, result.completeness);
    assert_eq!(1, result.items.len());
}

#[test]
fn empty_query_source_is_rejected() {
    let definition = QueryDefinition::new(
        QuerySourceRef::new(String::new()),
        TemporalScope::Current,
        None,
        PartialResultPolicy::RejectPartial,
        PresentationType::RecordSet,
    );

    assert!(matches!(
        validate_query_definition(&definition),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptyQuerySource
        })
    ));
}

#[test]
fn predicate_projection_sort_group_and_aggregation_structures_validate() {
    let mut projection = BTreeSet::new();
    projection.insert("status".to_owned());
    projection.insert("score".to_owned());
    let mut group_fields = BTreeSet::new();
    group_fields.insert("status".to_owned());
    let definition = query()
        .with_predicate(Predicate::And(vec![
            Predicate::Exists {
                field: "status".to_owned(),
            },
            Predicate::Range {
                field: "score".to_owned(),
                lower: Some(PropertyValue::Number(1.0)),
                upper: Some(PropertyValue::Number(5.0)),
            },
        ]))
        .with_projection(Projection::SelectedFields(projection))
        .with_sort(vec![SortSpec::new("created_at", SortDirection::Descending)])
        .with_group(GroupSpec::new(group_fields))
        .with_aggregations(vec![
            AggregationSpec::new(AggregationFunction::Count, None, "row_count"),
            AggregationSpec::new(
                AggregationFunction::Average,
                Some("score".to_owned()),
                "average_score",
            ),
        ]);

    validate_query_definition_against_schema(&definition, &schema()).unwrap();
}

#[test]
fn source_capabilities_accept_supported_query() {
    let provider = SyntheticSource::new(SourceCapabilities::all());
    let mut projection = BTreeSet::new();
    projection.insert("status".to_owned());
    let definition = query()
        .with_predicate(Predicate::Exists {
            field: "status".to_owned(),
        })
        .with_projection(Projection::SelectedFields(projection))
        .with_sort(vec![SortSpec::new("created_at", SortDirection::Ascending)]);

    validate_query_source_contract(&provider, &definition).unwrap();
}

#[test]
fn source_capabilities_reject_unsupported_aggregation() {
    let provider = SyntheticSource::new(SourceCapabilities {
        aggregation: false,
        ..SourceCapabilities::all()
    });
    let definition = query().with_aggregations(vec![AggregationSpec::new(
        AggregationFunction::Average,
        Some("score".to_owned()),
        "average_score",
    )]);

    let error = validate_query_source_contract(&provider, &definition).unwrap_err();

    assert!(matches!(
        error,
        QueryEngineError::UnsupportedCapability {
            source,
            capability: SourceCapability::Aggregation
        } if source == source_ref()
    ));
}

#[test]
fn source_contract_rejects_unknown_source_without_registry() {
    let provider = SyntheticSource::new(SourceCapabilities::all());
    let definition = QueryDefinition::new(
        QuerySourceRef::new("other-source"),
        TemporalScope::Current,
        None,
        PartialResultPolicy::RejectPartial,
        PresentationType::RecordSet,
    );

    let error = validate_query_source_contract(&provider, &definition).unwrap_err();

    assert!(matches!(
        error,
        QueryEngineError::UnknownSource { source } if source.as_str() == "other-source"
    ));
}

#[test]
fn invalid_schema_field_is_rejected_structurally() {
    let definition = query().with_predicate(Predicate::Equals {
        field: "missing".to_owned(),
        value: PropertyValue::EnumValue("approved".to_owned()),
    });

    let error = validate_query_definition_against_schema(&definition, &schema()).unwrap_err();

    assert!(matches!(
        error,
        QueryEngineError::InvalidField { field } if field == "missing"
    ));
}

#[test]
fn execution_projects_authorized_records_with_provenance_and_boundary() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-002", "inactive", 4.0),
        record("record-001", "active", 5.0),
    ]);
    let mut projection = BTreeSet::new();
    projection.insert("status".to_owned());
    let definition = query()
        .with_predicate(Predicate::Equals {
            field: "status".to_owned(),
            value: PropertyValue::EnumValue("active".to_owned()),
        })
        .with_projection(Projection::SelectedFields(projection));
    let request = QueryExecutionRequest::new(
        definition.clone(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-001"),
    );

    let result = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), request)
        .unwrap();

    assert_eq!(boundary(), result.consistency_boundary);
    assert_eq!(PresentationType::RecordSet, result.presentation_type);
    assert_eq!(1, result.items.len());
    assert_eq!(
        Some(&PropertyValue::EnumValue("active".to_owned())),
        result.items[0].fields.get("status")
    );
    assert!(!result.items[0].fields.contains_key("score"));
    assert_eq!(
        ResultClassification::SourceFact,
        result.items[0].classification
    );
    assert_eq!(definition, result.items[0].provenance.query_definition);
    assert_eq!(
        BTreeSet::from(["record-001".to_owned()]),
        result.items[0].provenance.source_record_ids
    );
    assert_eq!(
        Some("scope:synthetic".to_owned()),
        result.items[0].provenance.permission_scope
    );
}

#[test]
fn denied_and_hidden_denied_records_do_not_influence_aggregate_results() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-001", "active", 5.0),
        record("record-002", "active", 10.0),
        record("record-003", "active", 20.0),
    ]);
    let definition = query().with_aggregations(vec![
        AggregationSpec::new(AggregationFunction::Count, None, "count"),
        AggregationSpec::new(AggregationFunction::Sum, Some("score".to_owned()), "sum"),
    ]);
    let authorization = SyntheticAuthorization::default()
        .with_decision("record-002", AuthorizationEffect::Deny)
        .with_decision("record-003", AuthorizationEffect::HiddenDeny);
    let request = QueryExecutionRequest::new(
        definition,
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-aggregate"),
    );

    let result = QueryEngine::new()
        .execute_one_shot(&source, &authorization, request)
        .unwrap();

    assert_eq!(
        authorization.calls(),
        ["record-001", "record-002", "record-003"]
    );
    assert_eq!(1, result.items.len());
    assert_eq!(
        Some(&PropertyValue::Number(1.0)),
        result.items[0].fields.get("count")
    );
    assert_eq!(
        Some(&PropertyValue::Number(5.0)),
        result.items[0].fields.get("sum")
    );
    assert_eq!(
        BTreeSet::from(["record-001".to_owned()]),
        result.items[0].provenance.source_record_ids
    );
}

#[test]
fn sorting_is_deterministic_and_uses_stable_key_as_tie_breaker() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-c", "active", 5.0),
        record("record-a", "active", 5.0),
        record("record-b", "active", 7.0),
    ]);
    let definition = query().with_sort(vec![SortSpec::new("score", SortDirection::Ascending)]);
    let request = QueryExecutionRequest::new(
        definition,
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-sorted"),
    );

    let result = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), request)
        .unwrap();
    let ordered = result
        .items
        .iter()
        .map(|item| {
            item.provenance
                .source_record_ids
                .iter()
                .next()
                .unwrap()
                .clone()
        })
        .collect::<Vec<_>>();

    assert_eq!(ordered, ["record-a", "record-c", "record-b"]);
}

#[test]
fn pagination_returns_deterministic_next_cursor_and_continuation_page() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-001", "active", 1.0),
        record("record-002", "active", 2.0),
        record("record-003", "active", 3.0),
    ]);
    let first_request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-page-1"),
    )
    .with_pagination(Pagination::first_page(2));

    let first = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), first_request)
        .unwrap();
    let cursor = first.next_cursor.clone().unwrap();
    let first_ids = first
        .items
        .iter()
        .flat_map(|item| item.provenance.source_record_ids.iter().cloned())
        .collect::<Vec<_>>();

    let second_request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-page-2"),
    )
    .with_pagination(Pagination::after(cursor, 2));
    let second = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), second_request)
        .unwrap();
    let second_ids = second
        .items
        .iter()
        .flat_map(|item| item.provenance.source_record_ids.iter().cloned())
        .collect::<Vec<_>>();

    assert_eq!(first_ids, ["record-001", "record-002"]);
    assert_eq!(second_ids, ["record-003"]);
    assert!(second.next_cursor.is_none());
}

#[test]
fn pagination_rejects_empty_malformed_and_expired_cursors() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![record(
        "record-001",
        "active",
        1.0,
    )]);
    let empty_cursor_request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-empty-cursor"),
    )
    .with_pagination(Pagination {
        after: Some(ContinuationToken::new("")),
        max_items: Some(1),
    });
    assert!(matches!(
        QueryEngine::new().execute_one_shot(
            &source,
            &SyntheticAuthorization::default(),
            empty_cursor_request,
        ),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptyContinuationToken
        })
    ));

    let malformed_cursor_request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-malformed-cursor"),
    )
    .with_pagination(Pagination {
        after: Some(ContinuationToken::new("not-a-query-cursor")),
        max_items: Some(1),
    });
    assert!(matches!(
        QueryEngine::new().execute_one_shot(
            &source,
            &SyntheticAuthorization::default(),
            malformed_cursor_request,
        ),
        Err(QueryEngineError::InvalidCursor { .. })
    ));

    let first_page_source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-001", "active", 1.0),
        record("record-002", "active", 2.0),
    ]);
    let first_page = QueryEngine::new()
        .execute_one_shot(
            &first_page_source,
            &SyntheticAuthorization::default(),
            QueryExecutionRequest::new(
                query(),
                CallerPermissionContext::new("caller-context"),
                QueryResultId::new("result-page-1"),
            )
            .with_pagination(Pagination::first_page(1)),
        )
        .unwrap();
    let expired_boundary =
        ConsistencyBoundary::empty().with_query_definition(ConsistencySlot::Available(
            StableConsistencyMarker::new("query-definition", "semantic-v2"),
        ));
    let second_page_source = first_page_source.with_boundary(expired_boundary);
    assert!(matches!(
        QueryEngine::new().execute_one_shot(
            &second_page_source,
            &SyntheticAuthorization::default(),
            QueryExecutionRequest::new(
                query(),
                CallerPermissionContext::new("caller-context"),
                QueryResultId::new("result-page-2"),
            )
            .with_pagination(Pagination::after(first_page.next_cursor.unwrap(), 1)),
        ),
        Err(QueryEngineError::ExpiredCursor { .. })
    ));
}

#[test]
fn identical_query_boundary_and_authorization_produce_identical_result_bytes() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![record(
        "record-001",
        "active",
        5.0,
    )]);
    let definition = query();
    let first_request = QueryExecutionRequest::new(
        definition.clone(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-001"),
    );
    let second_request = QueryExecutionRequest::new(
        definition,
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-001"),
    );

    let first = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), first_request)
        .unwrap();
    let second = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), second_request)
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(
        canonical_query_result_bytes(&first),
        canonical_query_result_bytes(&second)
    );
}

#[test]
fn statistical_correlation_classification_is_preserved_not_retagged() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![record(
        "record-001",
        "active",
        5.0,
    )
    .with_classification(ResultClassification::StatisticalCorrelation)]);
    let request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-correlation"),
    );

    let result = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), request)
        .unwrap();

    assert_eq!(
        ResultClassification::StatisticalCorrelation,
        result.items[0].classification
    );
}

#[test]
fn unavailable_boundary_is_rejected_unless_partial_results_are_allowed() {
    let unavailable_boundary = ConsistencyBoundary::empty().with_event_engine(
        ConsistencySlot::Unavailable(ConsistencyBoundaryUnavailable::new(
            ConsistencyBoundaryUnavailableReason::ComponentApiNotAvailable,
            "event boundary API not implemented",
        )),
    );
    let source = SyntheticSource::new(SourceCapabilities::all())
        .with_records(vec![record("record-001", "active", 5.0)])
        .with_boundary(unavailable_boundary.clone());
    let rejected = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-rejected"),
    );

    assert!(matches!(
        QueryEngine::new().execute_one_shot(&source, &SyntheticAuthorization::default(), rejected),
        Err(QueryEngineError::ConsistencyBoundaryUnavailable {
            reason: ConsistencyBoundaryUnavailableReason::ComponentApiNotAvailable,
            ..
        })
    ));

    let allowed_definition = QueryDefinition::new(
        source_ref(),
        TemporalScope::Current,
        None,
        PartialResultPolicy::AllowIncomplete,
        PresentationType::RecordSet,
    );
    let allowed = QueryExecutionRequest::new(
        allowed_definition,
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-allowed"),
    );
    let result = QueryEngine::new()
        .execute_one_shot(&source, &SyntheticAuthorization::default(), allowed)
        .unwrap();

    assert_eq!(unavailable_boundary, result.consistency_boundary);
    assert_eq!(
        QueryCompleteness::Incomplete {
            reason: "component-api-not-available".to_owned()
        },
        result.completeness
    );
}

#[test]
fn cancellation_and_authorization_provider_failure_are_distinct_errors() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![record(
        "record-001",
        "active",
        5.0,
    )]);
    let request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-cancelled"),
    );
    let cancellation = CancellationState::new();
    cancellation.request_cancel();

    assert!(matches!(
        QueryEngine::new().execute_one_shot_with_cancellation(
            &source,
            &SyntheticAuthorization::default(),
            request.clone(),
            &cancellation,
        ),
        Err(QueryEngineError::Cancelled)
    ));

    assert!(matches!(
        QueryEngine::new().execute_one_shot(
            &source,
            &SyntheticAuthorization::failing("policy backend unavailable"),
            request,
        ),
        Err(QueryEngineError::AuthorizationProviderUnavailable { message })
            if message == "policy backend unavailable"
    ));
}

#[test]
fn saved_query_registration_rejects_missing_mandatory_metadata() {
    let mut missing_owner = saved_query_definition();
    missing_owner.owner.clear();
    assert!(matches!(
        QueryEngine::new().register_saved_query(
            &mut SyntheticSavedQueryCatalog::default(),
            &SyntheticAuthorization::default(),
            CallerPermissionContext::new("caller-context"),
            missing_owner,
        ),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptySavedQueryOwner
        })
    ));

    let mut missing_scope = saved_query_definition();
    missing_scope.access_permission_scope.clear();
    assert!(matches!(
        QueryEngine::new().register_saved_query(
            &mut SyntheticSavedQueryCatalog::default(),
            &SyntheticAuthorization::default(),
            CallerPermissionContext::new("caller-context"),
            missing_scope,
        ),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptySavedQueryAccessPermissionScope
        })
    ));

    let mut missing_history = saved_query_definition();
    missing_history.change_history.clear();
    assert!(matches!(
        QueryEngine::new().register_saved_query(
            &mut SyntheticSavedQueryCatalog::default(),
            &SyntheticAuthorization::default(),
            CallerPermissionContext::new("caller-context"),
            missing_history,
        ),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptySavedQueryChangeHistory
        })
    ));
}

#[test]
fn saved_query_registration_is_authorized_and_uses_injected_catalog() {
    let saved_query = saved_query_definition();
    let mut catalog = SyntheticSavedQueryCatalog::default();
    let authorization = SyntheticAuthorization::default();

    QueryEngine::new()
        .register_saved_query(
            &mut catalog,
            &authorization,
            CallerPermissionContext::new("caller-context"),
            saved_query.clone(),
        )
        .unwrap();

    assert_eq!(authorization.calls(), ["saved-001"]);
    assert_eq!(
        Some(saved_query),
        catalog.load_query(&SavedQueryId::new("saved-001")).unwrap()
    );
}

#[test]
fn saved_query_registration_denial_does_not_write_catalog() {
    let mut catalog = SyntheticSavedQueryCatalog::default();
    let authorization =
        SyntheticAuthorization::default().with_decision("saved-001", AuthorizationEffect::Deny);

    assert!(matches!(
        QueryEngine::new().register_saved_query(
            &mut catalog,
            &authorization,
            CallerPermissionContext::new("caller-context"),
            saved_query_definition(),
        ),
        Err(QueryEngineError::PermissionDenied { target })
            if target.object_identifier() == "saved-001"
    ));
    assert!(catalog
        .load_query(&SavedQueryId::new("saved-001"))
        .unwrap()
        .is_none());
}

#[test]
fn saved_query_execution_replays_stored_definition_through_one_shot_executor() {
    let saved_query = saved_query_definition();
    let mut catalog = SyntheticSavedQueryCatalog::default();
    catalog.save_query(saved_query).unwrap();
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-001", "active", 5.0),
        record("record-002", "inactive", 7.0),
    ]);
    let request = SavedQueryExecutionRequest::new(
        SavedQueryId::new("saved-001"),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-saved"),
    );

    let result = QueryEngine::new()
        .execute_saved_query(
            &catalog,
            &source,
            &SyntheticAuthorization::default(),
            request,
        )
        .unwrap();

    assert_eq!("result-saved", result.id.as_str());
    assert_eq!(1, result.items.len());
    assert_eq!(
        BTreeSet::from(["record-001".to_owned()]),
        result.items[0].provenance.source_record_ids
    );
}

#[test]
fn saved_query_hidden_denial_is_observable_as_not_found() {
    let mut catalog = SyntheticSavedQueryCatalog::default();
    catalog.save_query(saved_query_definition()).unwrap();
    let source = SyntheticSource::new(SourceCapabilities::all());
    let authorization = SyntheticAuthorization::default()
        .with_decision("saved-001", AuthorizationEffect::HiddenDeny);
    let request = SavedQueryExecutionRequest::new(
        SavedQueryId::new("saved-001"),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-hidden"),
    );

    assert!(matches!(
        QueryEngine::new().execute_saved_query(&catalog, &source, &authorization, request),
        Err(QueryEngineError::SavedQueryNotFound { saved_query_id })
            if saved_query_id.as_str() == "saved-001"
    ));
}

#[test]
fn saved_query_schema_mismatch_and_catalog_failure_are_distinct() {
    let mut mismatched = saved_query_definition();
    mismatched.schema_version = CURRENT_SAVED_QUERY_SCHEMA_VERSION + 1;
    assert!(matches!(
        QueryEngine::new().register_saved_query(
            &mut SyntheticSavedQueryCatalog::default(),
            &SyntheticAuthorization::default(),
            CallerPermissionContext::new("caller-context"),
            mismatched,
        ),
        Err(QueryEngineError::SavedQuerySchemaVersionMismatch { reason })
            if reason.contains("expected schema version")
    ));

    assert!(matches!(
        QueryEngine::new().register_saved_query(
            &mut SyntheticSavedQueryCatalog::failing("catalog offline"),
            &SyntheticAuthorization::default(),
            CallerPermissionContext::new("caller-context"),
            saved_query_definition(),
        ),
        Err(QueryEngineError::SavedQueryCatalogUnavailable { message })
            if message == "catalog offline"
    ));
}

#[test]
fn context_package_wraps_authorized_query_result_without_interpretation() {
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-001", "active", 5.0),
        record("record-002", "active", 7.0),
    ]);
    let authorization = SyntheticAuthorization::default()
        .with_decision("record-002", AuthorizationEffect::HiddenDeny);
    let execution_request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-context"),
    );
    let package_request = ContextPackageRequest::new(
        ContextPackageId::new("context-001"),
        execution_request,
        4,
        1,
    );

    let package = QueryEngine::new()
        .build_context_package(&source, &authorization, package_request)
        .unwrap();

    assert_eq!("context-001", package.id.as_str());
    assert_eq!(Version::initial(), package.version);
    assert_eq!("caller-context", package.caller_context.as_str());
    assert_eq!(1, package.max_depth);
    assert_eq!(boundary(), package.query_result.consistency_boundary);
    assert_eq!(1, package.query_result.items.len());
    assert_eq!(
        BTreeSet::from(["record-001".to_owned()]),
        package.query_result.items[0].provenance.source_record_ids
    );
}

#[test]
fn context_package_limits_are_validated_and_enforced() {
    let execution_request = QueryExecutionRequest::new(
        query(),
        CallerPermissionContext::new("caller-context"),
        QueryResultId::new("result-context"),
    );
    let source = SyntheticSource::new(SourceCapabilities::all()).with_records(vec![
        record("record-001", "active", 5.0),
        record("record-002", "active", 7.0),
    ]);

    assert!(matches!(
        QueryEngine::new().build_context_package(
            &source,
            &SyntheticAuthorization::default(),
            ContextPackageRequest::new(
                ContextPackageId::new("context-001"),
                execution_request.clone(),
                0,
                1,
            ),
        ),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::ZeroContextPackageItemLimit
        })
    ));

    assert!(matches!(
        QueryEngine::new().build_context_package(
            &source,
            &SyntheticAuthorization::default(),
            ContextPackageRequest::new(
                ContextPackageId::new("context-001"),
                execution_request.clone(),
                4,
                0,
            ),
        ),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::ZeroContextPackageDepthLimit
        })
    ));

    assert!(matches!(
        QueryEngine::new().build_context_package(
            &source,
            &SyntheticAuthorization::default(),
            ContextPackageRequest::new(
                ContextPackageId::new("context-001"),
                execution_request,
                1,
                1,
            ),
        ),
        Err(QueryEngineError::ExecutionLimitExceeded {
            limit: ExecutionLimitKind::ResultCount
        })
    ));
}

#[test]
fn malformed_predicate_and_aggregation_shapes_are_rejected() {
    let empty_branch = query().with_predicate(Predicate::And(Vec::new()));
    assert!(matches!(
        validate_query_definition(&empty_branch),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::EmptyPredicateBranch
        })
    ));

    let missing_field = query().with_aggregations(vec![AggregationSpec::new(
        AggregationFunction::Sum,
        None,
        "sum_score",
    )]);
    assert!(matches!(
        validate_query_definition(&missing_field),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::MissingAggregationField
        })
    ));
}

#[test]
fn stable_ordering_keys_sort_deterministically() {
    let mut keys = [
        StableOrderingKey::new(vec!["record-b".to_owned(), "002".to_owned()]),
        StableOrderingKey::new(vec!["record-a".to_owned(), "003".to_owned()]),
        StableOrderingKey::new(vec!["record-a".to_owned(), "001".to_owned()]),
    ];

    keys.sort_by(compare_stable_ordering_keys);

    let ordered = keys
        .iter()
        .map(|key| key.segments().join(":"))
        .collect::<Vec<_>>();
    assert_eq!(ordered, ["record-a:001", "record-a:003", "record-b:002"]);
}

#[test]
fn canonical_query_semantics_are_independent_of_set_insertion_order() {
    let mut first_projection = BTreeSet::new();
    first_projection.insert("score".to_owned());
    first_projection.insert("status".to_owned());
    let mut second_projection = BTreeSet::new();
    second_projection.insert("status".to_owned());
    second_projection.insert("score".to_owned());

    let first = query().with_projection(Projection::SelectedFields(first_projection));
    let second = query().with_projection(Projection::SelectedFields(second_projection));

    assert!(semantic_query_equal(&first, &second));
    assert_eq!(
        canonical_query_bytes(&first),
        canonical_query_bytes(&second)
    );
}

#[test]
fn canonical_query_semantics_detect_real_structural_difference() {
    let first = query().with_predicate(Predicate::Exists {
        field: "status".to_owned(),
    });
    let second = query().with_predicate(Predicate::Exists {
        field: "score".to_owned(),
    });

    assert!(!semantic_query_equal(&first, &second));
}

#[test]
fn canonical_semantic_record_excludes_operational_fields() {
    struct SyntheticRecordFixture {
        stable_key: StableOrderingKey,
        semantic_fields: BTreeMap<String, PropertyValue>,
        execution_token: String,
        observed_at: String,
    }

    let first = SyntheticRecordFixture {
        stable_key: StableOrderingKey::new(vec!["record-001".to_owned()]),
        semantic_fields: BTreeMap::from([
            ("score".to_owned(), PropertyValue::Number(5.0)),
            (
                "owner".to_owned(),
                PropertyValue::Reference(ObjectId::new("object-001")),
            ),
        ]),
        execution_token: "run-001".to_owned(),
        observed_at: "2026-07-15T10:00:00Z".to_owned(),
    };
    let second = SyntheticRecordFixture {
        stable_key: first.stable_key.clone(),
        semantic_fields: first.semantic_fields.clone(),
        execution_token: "run-002".to_owned(),
        observed_at: "2026-07-15T11:00:00Z".to_owned(),
    };

    assert_ne!(first.execution_token, second.execution_token);
    assert_ne!(first.observed_at, second.observed_at);
    assert_eq!(
        canonical_semantic_record_bytes(&first.stable_key, &first.semantic_fields),
        canonical_semantic_record_bytes(&second.stable_key, &second.semantic_fields)
    );
}

#[test]
fn execution_limits_are_part_of_query_validation() {
    let limits = ExecutionLimits::new(
        Some(100),
        Some(1_000),
        Some(QueryTimeout::from_millis(250).unwrap()),
    )
    .unwrap();
    let definition = query().with_execution_limits(limits);

    validate_query_definition(&definition).unwrap();
    assert_eq!(definition.execution_limits, limits);
}

#[test]
fn zero_execution_controls_are_malformed_query_values() {
    let zero_result_limit = query().with_execution_limits(ExecutionLimits {
        max_result_count: Some(0),
        max_evaluation_steps: None,
        timeout: None,
    });
    assert!(matches!(
        validate_query_definition(&zero_result_limit),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::ZeroResultLimit
        })
    ));

    let zero_step_limit = query().with_execution_limits(ExecutionLimits {
        max_result_count: None,
        max_evaluation_steps: Some(0),
        timeout: None,
    });
    assert!(matches!(
        validate_query_definition(&zero_step_limit),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::ZeroEvaluationStepLimit
        })
    ));

    assert!(matches!(
        QueryTimeout::from_millis(0),
        Err(QueryEngineError::MalformedQuery {
            failure: ValidationError::ZeroTimeout
        })
    ));
}

#[test]
fn limit_helpers_return_distinct_limit_errors() {
    let limits = ExecutionLimits::new(Some(2), Some(10), None).unwrap();

    assert!(matches!(
        ensure_result_count_within_limit(&limits, 3),
        Err(QueryEngineError::ExecutionLimitExceeded {
            limit: ExecutionLimitKind::ResultCount
        })
    ));
    assert!(matches!(
        ensure_evaluation_steps_within_limit(&limits, 11),
        Err(QueryEngineError::ExecutionLimitExceeded {
            limit: ExecutionLimitKind::EvaluationSteps
        })
    ));
    ensure_result_count_within_limit(&limits, 2).unwrap();
    ensure_evaluation_steps_within_limit(&limits, 10).unwrap();
}

#[test]
fn timeout_representation_is_deterministic_and_not_wall_clock_based() {
    let timeout = QueryTimeout::from_millis(250).unwrap();

    ensure_timeout_not_elapsed(timeout, 250).unwrap();
    assert!(matches!(
        ensure_timeout_not_elapsed(timeout, 251),
        Err(QueryEngineError::Timeout { timeout: observed }) if observed == timeout
    ));
}

#[test]
fn cancellation_state_is_one_shot_and_clone_visible() {
    let cancellation = CancellationState::new();
    let clone = cancellation.clone();

    cancellation.check_cancelled().unwrap();
    clone.request_cancel();

    assert!(cancellation.is_cancelled());
    assert!(clone.is_cancelled());
    assert!(matches!(
        cancellation.check_cancelled(),
        Err(QueryEngineError::Cancelled)
    ));
}

fn production_sources() -> [(&'static str, &'static str); 9] {
    [
        ("capabilities.rs", include_str!("capabilities.rs")),
        ("errors.rs", include_str!("errors.rs")),
        ("execution.rs", include_str!("execution.rs")),
        ("lib.rs", include_str!("lib.rs")),
        ("limits.rs", include_str!("limits.rs")),
        ("ordering.rs", include_str!("ordering.rs")),
        ("saved_query.rs", include_str!("saved_query.rs")),
        ("types.rs", include_str!("types.rs")),
        ("validation.rs", include_str!("validation.rs")),
    ]
}

#[test]
fn crate_manifest_depends_only_on_runtime_contracts() {
    let manifest = include_str!("../Cargo.toml");

    assert!(manifest.contains("open-eqms-runtime-contracts"));
    for forbidden_dependency in [
        "open-eqms-object-runtime",
        "open-eqms-event-engine",
        "open-eqms-transaction-engine",
    ] {
        assert!(
            !manifest.contains(forbidden_dependency),
            "forbidden dependency found: {forbidden_dependency}"
        );
    }

    let dependency_lines = manifest
        .lines()
        .skip_while(|line| *line != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        dependency_lines,
        ["open-eqms-runtime-contracts = { path = \"../runtime-contracts\" }"]
    );
}

#[test]
fn production_sources_have_no_engine_adapter_dependencies_or_calls() {
    for (file, source) in production_sources() {
        for forbidden in [
            "open_eqms_object_runtime",
            "open_eqms_event_engine",
            "open_eqms_transaction_engine",
            "open-eqms-object-runtime",
            "open-eqms-event-engine",
            "open-eqms-transaction-engine",
            "read_object",
            "read_event",
            "read_transaction",
        ] {
            assert!(
                !source.contains(forbidden),
                "{file} contains forbidden integration token {forbidden}"
            );
        }
    }
}

#[test]
fn public_surface_exposes_executor_without_storage_registry_or_planner_contract() {
    let lib = include_str!("lib.rs");
    let capabilities = include_str!("capabilities.rs");

    assert!(lib.contains("pub mod execution"));
    assert!(lib.contains("pub use crate::execution::QueryEngine"));
    assert!(capabilities.contains("pub trait ExecutableQuerySourceProvider"));

    for forbidden_public_form in [
        "pub fn register",
        "pub fn plan",
        "pub mod planner",
        "pub mod storage",
    ] {
        assert!(
            !lib.contains(forbidden_public_form),
            "forbidden public surface found: {forbidden_public_form}"
        );
    }
}

#[test]
fn deferred_contracts_are_not_present_as_public_placeholders() {
    for (file, source) in production_sources() {
        for forbidden_public_form in [
            "pub trait PermissionProvider",
            "pub struct PermissionProvider",
            "pub mod context_package",
            "pub struct Cursor",
            "pub enum Cursor",
            "pub type Cursor",
            "pub enum ContinuationToken",
            "pub type ContinuationToken",
        ] {
            assert!(
                !source.contains(forbidden_public_form),
                "{file} contains deferred placeholder {forbidden_public_form}"
            );
        }
    }
}

#[test]
fn error_taxonomy_includes_execution_error_variants() {
    let errors = include_str!("errors.rs");

    for required_error in [
        "PermissionDenied",
        "ConsistencyBoundaryUnavailable",
        "SourceUnavailable",
        "IncompleteExecution",
        "ExecutionStrategyUnavailable",
        "AuthorizationProviderUnavailable",
        "SavedQuerySchemaVersionMismatch",
        "InvalidCursor",
        "ExpiredCursor",
    ] {
        assert!(
            errors.contains(required_error),
            "required execution error variant missing: {required_error}"
        );
    }
}

#[test]
fn canonical_semantics_do_not_define_public_hash_contract() {
    for (file, source) in production_sources() {
        for forbidden_hash_surface in [
            "pub struct QueryHash",
            "pub enum QueryHash",
            "pub type QueryHash",
            "DefaultHasher",
            "Sha256",
            "hash_query",
        ] {
            assert!(
                !source.contains(forbidden_hash_surface),
                "{file} contains forbidden hash surface {forbidden_hash_surface}"
            );
        }
    }
}

#[test]
fn contract_validation_accepts_full_slices_without_data_access() {
    let mut projection = BTreeSet::new();
    projection.insert("status".to_owned());
    projection.insert("score".to_owned());
    let mut group_fields = BTreeSet::new();
    group_fields.insert("status".to_owned());
    let provider = SyntheticSource::new(SourceCapabilities::all());
    let definition = query()
        .with_predicate(Predicate::Or(vec![
            Predicate::Exists {
                field: "status".to_owned(),
            },
            Predicate::Range {
                field: "score".to_owned(),
                lower: Some(PropertyValue::Number(1.0)),
                upper: Some(PropertyValue::Number(9.0)),
            },
        ]))
        .with_projection(Projection::SelectedFields(projection))
        .with_sort(vec![SortSpec::new("created_at", SortDirection::Ascending)])
        .with_group(GroupSpec::new(group_fields))
        .with_aggregations(vec![AggregationSpec::new(
            AggregationFunction::Count,
            None,
            "row_count",
        )])
        .with_execution_limits(ExecutionLimits::new(Some(100), Some(1_000), None).unwrap());

    validate_query_source_contract(&provider, &definition).unwrap();
}
