use super::*;
use crate::validation::validate_query_definition;
use std::collections::BTreeSet;

fn query() -> QueryDefinition {
    QueryDefinition::new(
        TemporalScope::Current,
        None,
        PartialResultPolicy::RejectPartial,
        PresentationType::RecordSet,
    )
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
