//! Finite one-shot Query Engine execution.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::capabilities::{validate_query_source_contract, ExecutableQuerySourceProvider};
use crate::errors::{
    authorization_provider_unavailable, consistency_boundary_unavailable, invalid_source_record,
    QueryEngineResult,
};
use crate::limits::{
    ensure_evaluation_steps_within_limit, ensure_result_count_within_limit, CancellationState,
};
#[cfg(test)]
use crate::ordering::canonical_query_bytes;
use crate::types::{
    AggregationFunction, AggregationSpec, AuthorizationProvider, AuthorizationRequest,
    CallerPermissionContext, ConsistencyBoundary, ConsistencySlot, PartialResultPolicy,
    PermissionAction, Predicate, Projection, PropertyValue, QueryCompleteness, QueryDefinition,
    QueryExecutionRequest, QueryRecord, QueryResult, QueryResultItem, ResultClassification,
    ResultProvenance, SortDirection, SortSpec, StableOrderingKey, Version,
};
use crate::validation::{validate_query_execution_request, validate_query_record_against_schema};

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
        P: ExecutableQuerySourceProvider,
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
        P: ExecutableQuerySourceProvider,
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
        ensure_result_count_within_limit(&request.query.execution_limits, items.len())?;

        Ok(QueryResult {
            id: request.result_id,
            version: Version::initial(),
            query_definition: request.query.clone(),
            consistency_boundary,
            presentation_type: request.query.presentation_type,
            items,
            completeness,
        })
    }
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
    let request = AuthorizationRequest::new(
        caller_context.clone(),
        PermissionAction::new("query.read-record"),
        record.authorization_target(),
    );
    let decision = authorization_provider
        .decide(&request)
        .map_err(|error| authorization_provider_unavailable(error.to_string()))?;

    Ok(matches!(
        decision.effect(),
        crate::types::AuthorizationEffect::Allow
    ))
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
