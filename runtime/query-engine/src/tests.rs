use super::*;
use crate::capabilities::{validate_query_source_contract, QuerySourceProvider};
use crate::ordering::{
    canonical_query_bytes, canonical_semantic_record_bytes, compare_stable_ordering_keys,
    semantic_query_equal,
};
use crate::validation::{validate_query_definition, validate_query_definition_against_schema};
use open_eqms_runtime_contracts::{ObjectId, PropertyValue, PropertyValueKind};
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
}

impl SyntheticSource {
    fn new(capabilities: SourceCapabilities) -> Self {
        Self {
            source: source_ref(),
            capabilities,
            schema: schema(),
        }
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
fn minimal_query_uses_no_identity_or_persistence_contract() {
    let definition = query();

    validate_query_definition(&definition).unwrap();
    assert_eq!(definition.temporal_scope, TemporalScope::Current);
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

fn production_sources() -> [(&'static str, &'static str); 7] {
    [
        ("capabilities.rs", include_str!("capabilities.rs")),
        ("errors.rs", include_str!("errors.rs")),
        ("lib.rs", include_str!("lib.rs")),
        ("limits.rs", include_str!("limits.rs")),
        ("ordering.rs", include_str!("ordering.rs")),
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
fn public_surface_has_no_executor_registration_storage_or_planner_contract() {
    let lib = include_str!("lib.rs");
    let capabilities = include_str!("capabilities.rs");
    let source = [lib, capabilities].join("\n");

    for forbidden_public_form in [
        "pub struct QueryEngine",
        "pub enum QueryEngine",
        "pub trait QueryEngine",
        "pub fn execute",
        "pub fn register",
        "pub fn plan",
        "pub mod planner",
        "pub mod executor",
        "pub mod storage",
    ] {
        assert!(
            !source.contains(forbidden_public_form),
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
            "pub struct ContextPackage",
            "pub enum ContextPackage",
            "pub mod context_package",
            "pub struct SavedQuery",
            "pub enum SavedQuery",
            "pub type SavedQuery",
            "pub struct Cursor",
            "pub enum Cursor",
            "pub type Cursor",
            "pub struct ContinuationToken",
            "pub enum ContinuationToken",
            "pub type ContinuationToken",
            "pub struct ConsistencyBoundary",
            "pub enum ConsistencyBoundary",
            "pub type ConsistencyBoundary",
        ] {
            assert!(
                !source.contains(forbidden_public_form),
                "{file} contains deferred placeholder {forbidden_public_form}"
            );
        }
    }
}

#[test]
fn error_taxonomy_excludes_deferred_error_variants() {
    let errors = include_str!("errors.rs");

    for forbidden_error in [
        "PermissionDenied",
        "ConsistencyBoundaryUnavailable",
        "Cursor",
        "ContinuationToken",
        "SourceUnavailable",
        "IncompleteExecution",
        "ExecutionStrategyUnavailable",
    ] {
        assert!(
            !errors.contains(forbidden_error),
            "forbidden deferred error variant found: {forbidden_error}"
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
