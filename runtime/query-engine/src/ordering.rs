//! Deterministic ordering and crate-private canonical semantics.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use crate::types::{
    AggregationFunction, AggregationSpec, GroupSpec, PartialResultPolicy, Predicate,
    PresentationType, Projection, PropertyValue, QueryDefinition, SortDirection, SortSpec,
    StableOrderingKey, TemporalScope, TraversalDirection, TraversalSpec,
};

/// Compares stable ordering keys with deterministic lexicographic semantics.
pub(crate) fn compare_stable_ordering_keys(
    left: &StableOrderingKey,
    right: &StableOrderingKey,
) -> Ordering {
    left.segments().cmp(right.segments())
}

/// Returns whether two query definitions are semantically identical.
pub(crate) fn semantic_query_equal(left: &QueryDefinition, right: &QueryDefinition) -> bool {
    canonical_query_bytes(left) == canonical_query_bytes(right)
}

/// Encodes a query definition into crate-private canonical bytes.
pub(crate) fn canonical_query_bytes(query: &QueryDefinition) -> Vec<u8> {
    let mut encoder = CanonicalEncoder::default();
    encoder.label("query");
    encoder.token(query.source.as_str());
    encode_temporal_scope(&mut encoder, &query.temporal_scope);
    encode_optional_predicate(&mut encoder, query.predicate.as_ref());
    encode_projection(&mut encoder, &query.projection);
    encode_sort(&mut encoder, &query.sort);
    encode_optional_group(&mut encoder, query.group.as_ref());
    encode_aggregations(&mut encoder, &query.aggregations);
    encode_optional_traversal(&mut encoder, query.traversal.as_ref());
    encode_partial_result_policy(&mut encoder, query.partial_result_policy);
    encode_presentation_type(&mut encoder, query.presentation_type);
    encoder.finish()
}

/// Encodes a synthetic semantic record fixture into crate-private canonical bytes.
pub(crate) fn canonical_semantic_record_bytes(
    ordering_key: &StableOrderingKey,
    fields: &BTreeMap<String, PropertyValue>,
) -> Vec<u8> {
    let mut encoder = CanonicalEncoder::default();
    encoder.label("semantic-record");
    encode_stable_ordering_key(&mut encoder, ordering_key);
    for (field, value) in fields {
        encoder.label("field");
        encoder.token(field);
        encode_property_value(&mut encoder, value);
    }
    encoder.finish()
}

#[derive(Default)]
struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    fn label(&mut self, label: &str) {
        self.token(label);
    }

    fn token(&mut self, token: &str) {
        self.token_bytes(token.as_bytes());
    }

    fn token_bytes(&mut self, bytes: &[u8]) {
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

fn encode_temporal_scope(encoder: &mut CanonicalEncoder, scope: &TemporalScope) {
    encoder.label("temporal");
    match scope {
        TemporalScope::Current => encoder.label("current"),
        TemporalScope::PointInTime { at } => {
            encoder.label("point");
            encoder.token(at);
        }
        TemporalScope::TimeRange { from, to } => {
            encoder.label("range");
            encode_optional_string(encoder, from.as_deref());
            encode_optional_string(encoder, to.as_deref());
        }
        TemporalScope::AllHistory => encoder.label("all-history"),
    }
}

fn encode_optional_predicate(encoder: &mut CanonicalEncoder, predicate: Option<&Predicate>) {
    encoder.label("predicate");
    if let Some(predicate) = predicate {
        encoder.label("some");
        encode_predicate(encoder, predicate);
    } else {
        encoder.label("none");
    }
}

fn encode_predicate(encoder: &mut CanonicalEncoder, predicate: &Predicate) {
    match predicate {
        Predicate::AlwaysTrue => encoder.label("always-true"),
        Predicate::Equals { field, value } => {
            encoder.label("equals");
            encoder.token(field);
            encode_property_value(encoder, value);
        }
        Predicate::Exists { field } => {
            encoder.label("exists");
            encoder.token(field);
        }
        Predicate::Range {
            field,
            lower,
            upper,
        } => {
            encoder.label("range");
            encoder.token(field);
            encode_optional_property_value(encoder, lower.as_ref());
            encode_optional_property_value(encoder, upper.as_ref());
        }
        Predicate::And(children) => encode_predicate_branch(encoder, "and", children),
        Predicate::Or(children) => encode_predicate_branch(encoder, "or", children),
        Predicate::Not(child) => {
            encoder.label("not");
            encode_predicate(encoder, child);
        }
    }
}

fn encode_predicate_branch(encoder: &mut CanonicalEncoder, label: &str, children: &[Predicate]) {
    encoder.label(label);
    encoder.token(&children.len().to_string());
    for child in children {
        encode_predicate(encoder, child);
    }
}

fn encode_projection(encoder: &mut CanonicalEncoder, projection: &Projection) {
    encoder.label("projection");
    match projection {
        Projection::AllFields => encoder.label("all-fields"),
        Projection::SelectedFields(fields) => {
            encoder.label("selected-fields");
            for field in fields {
                encoder.token(field);
            }
        }
    }
}

fn encode_sort(encoder: &mut CanonicalEncoder, sort: &[SortSpec]) {
    encoder.label("sort");
    encoder.token(&sort.len().to_string());
    for spec in sort {
        encoder.token(&spec.field);
        encode_sort_direction(encoder, spec.direction);
    }
}

fn encode_optional_group(encoder: &mut CanonicalEncoder, group: Option<&GroupSpec>) {
    encoder.label("group");
    if let Some(group) = group {
        encoder.label("some");
        for field in &group.fields {
            encoder.token(field);
        }
    } else {
        encoder.label("none");
    }
}

fn encode_aggregations(encoder: &mut CanonicalEncoder, aggregations: &[AggregationSpec]) {
    encoder.label("aggregations");
    encoder.token(&aggregations.len().to_string());
    for aggregation in aggregations {
        encode_aggregation_function(encoder, aggregation.function);
        encode_optional_string(encoder, aggregation.field.as_deref());
        encoder.token(&aggregation.alias);
    }
}

fn encode_optional_traversal(encoder: &mut CanonicalEncoder, traversal: Option<&TraversalSpec>) {
    encoder.label("traversal");
    if let Some(traversal) = traversal {
        encoder.label("some");
        encode_traversal_direction(encoder, traversal.direction);
        encoder.token(&traversal.max_depth.to_string());
        for relation_type in &traversal.relation_types {
            encoder.token(relation_type);
        }
    } else {
        encoder.label("none");
    }
}

fn encode_partial_result_policy(encoder: &mut CanonicalEncoder, policy: PartialResultPolicy) {
    encoder.label(match policy {
        PartialResultPolicy::RejectPartial => "reject-partial",
        PartialResultPolicy::AllowIncomplete => "allow-incomplete",
    });
}

fn encode_presentation_type(encoder: &mut CanonicalEncoder, presentation: PresentationType) {
    encoder.label(match presentation {
        PresentationType::RecordSet => "record-set",
        PresentationType::LinkedObjectView => "linked-object-view",
        PresentationType::Timeline => "timeline",
        PresentationType::ChartReadySeries => "chart-ready-series",
        PresentationType::Matrix => "matrix",
        PresentationType::DocumentReferenceSet => "document-reference-set",
        PresentationType::EvidencePackage => "evidence-package",
    });
}

fn encode_traversal_direction(encoder: &mut CanonicalEncoder, direction: TraversalDirection) {
    encoder.label(match direction {
        TraversalDirection::Outbound => "outbound",
        TraversalDirection::Inbound => "inbound",
        TraversalDirection::Both => "both",
    });
}

fn encode_sort_direction(encoder: &mut CanonicalEncoder, direction: SortDirection) {
    encoder.label(match direction {
        SortDirection::Ascending => "ascending",
        SortDirection::Descending => "descending",
    });
}

fn encode_aggregation_function(encoder: &mut CanonicalEncoder, function: AggregationFunction) {
    encoder.label(match function {
        AggregationFunction::Count => "count",
        AggregationFunction::Sum => "sum",
        AggregationFunction::Average => "average",
        AggregationFunction::Min => "min",
        AggregationFunction::Max => "max",
        AggregationFunction::DistinctCount => "distinct-count",
    });
}

fn encode_stable_ordering_key(encoder: &mut CanonicalEncoder, key: &StableOrderingKey) {
    encoder.label("stable-ordering-key");
    encoder.token(&key.segments().len().to_string());
    for segment in key.segments() {
        encoder.token(segment);
    }
}

fn encode_optional_string(encoder: &mut CanonicalEncoder, value: Option<&str>) {
    if let Some(value) = value {
        encoder.label("some");
        encoder.token(value);
    } else {
        encoder.label("none");
    }
}

fn encode_optional_property_value(encoder: &mut CanonicalEncoder, value: Option<&PropertyValue>) {
    if let Some(value) = value {
        encoder.label("some");
        encode_property_value(encoder, value);
    } else {
        encoder.label("none");
    }
}

fn encode_property_value(encoder: &mut CanonicalEncoder, value: &PropertyValue) {
    match value {
        PropertyValue::Text { value, language } => {
            encoder.label("text");
            encoder.token(value);
            encode_optional_string(encoder, language.as_deref());
        }
        PropertyValue::LocalizedTextKey(value) => {
            encoder.label("localized-text-key");
            encoder.token(value);
        }
        PropertyValue::Number(value) => {
            encoder.label("number");
            encoder.token(&value.to_bits().to_string());
        }
        PropertyValue::Boolean(value) => {
            encoder.label("boolean");
            encoder.token(if *value { "true" } else { "false" });
        }
        PropertyValue::DateTime(value) => {
            encoder.label("datetime");
            encoder.token(value);
        }
        PropertyValue::EnumValue(value) => {
            encoder.label("enum");
            encoder.token(value);
        }
        PropertyValue::Reference(value) => {
            encoder.label("reference");
            encoder.token(value.as_str());
        }
        PropertyValue::AttachmentReference(value) => {
            encoder.label("attachment-reference");
            encoder.token(value);
        }
        PropertyValue::BinaryBlob(value) => {
            encoder.label("binary");
            encoder.token_bytes(value);
        }
    }
}
