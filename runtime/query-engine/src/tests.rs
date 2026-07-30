use super::*;
use crate::validation::{validate_query_definition, validate_query_definition_against_schema};
use open_eqms_runtime_contracts::{PropertyValue, PropertyValueKind};
use std::collections::BTreeMap;
use std::collections::BTreeSet;

fn query() -> QueryDefinition {
    QueryDefinition::new(
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

#[test]
fn well_formed_query_definition_is_accepted() {
    let mut relation_types = BTreeSet::new();
    relation_types.insert("uses".to_owned());
    let definition = QueryDefinition::new(
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
