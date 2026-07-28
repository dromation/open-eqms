//! Deterministic canonical content construction for Transaction hashes.

#![allow(dead_code)]

use crate::types::{PriorReference, PropertyValue, RuleEvaluationMetadata, TransactionRecord};

#[allow(clippy::vec_init_then_push)]
pub(crate) fn canonical_transaction_content(record: &TransactionRecord) -> Vec<u8> {
    let mut fields = Vec::<(String, String)>::new();

    fields.push(("id".to_owned(), record.id.as_str().to_owned()));
    fields.push((
        "schema_version".to_owned(),
        record.schema_version.value().to_string(),
    ));
    fields.push(("level".to_owned(), record.level.as_str().to_owned()));
    fields.push(("object_id".to_owned(), record.object_id.as_str().to_owned()));
    fields.push(("operation".to_owned(), record.operation.as_str().to_owned()));
    fields.push(("old_value".to_owned(), encode_value(&record.old_value)));
    fields.push(("new_value".to_owned(), encode_value(&record.new_value)));
    fields.push(("actor".to_owned(), record.actor.as_str().to_owned()));
    fields.push(("device".to_owned(), record.device.as_str().to_owned()));
    fields.push((
        "site".to_owned(),
        encode_opt_str(record.site.as_ref().map(|value| value.as_str())),
    ));
    fields.push((
        "edit_timestamp".to_owned(),
        record.edit_timestamp.as_str().to_owned(),
    ));
    fields.push((
        "base_version".to_owned(),
        record.base_version.value().to_string(),
    ));
    fields.push((
        "resulting_version".to_owned(),
        record.resulting_version.value().to_string(),
    ));
    fields.push((
        "append_index".to_owned(),
        record.append_index.value().to_string(),
    ));
    fields.push((
        "transaction_sequence".to_owned(),
        record
            .transaction_sequence
            .map(|value| value.value().to_string())
            .unwrap_or_else(|| "none".to_owned()),
    ));
    fields.push((
        "server_receipt_time".to_owned(),
        encode_opt_str(
            record
                .server_receipt_time
                .as_ref()
                .map(|value| value.as_str()),
        ),
    ));
    fields.push((
        "prior_reference".to_owned(),
        encode_prior_reference(record.prior_reference.as_ref()),
    ));
    fields.push((
        "prior_transaction_hash".to_owned(),
        encode_opt_str(
            record
                .prior_transaction_hash
                .as_ref()
                .map(|value| value.as_str()),
        ),
    ));
    fields.push((
        "rule_evaluation".to_owned(),
        encode_rule_evaluation(record.rule_evaluation.as_ref()),
    ));
    fields.push((
        "signer".to_owned(),
        encode_opt_str(record.signer.as_ref().map(|value| value.as_str())),
    ));
    fields.push((
        "signer_role".to_owned(),
        encode_opt_str(record.signer_role.as_ref().map(|value| value.as_str())),
    ));
    fields.push((
        "signature_meaning".to_owned(),
        encode_opt_str(
            record
                .signature_meaning
                .as_ref()
                .map(|value| value.as_str()),
        ),
    ));
    fields.push((
        "authentication_evidence".to_owned(),
        encode_opt_str(
            record
                .authentication_evidence
                .as_ref()
                .map(|value| value.as_str()),
        ),
    ));
    fields.push((
        "signed_revision".to_owned(),
        record
            .signed_revision
            .map(|value| value.version().value().to_string())
            .unwrap_or_else(|| "none".to_owned()),
    ));
    fields.push((
        "reason".to_owned(),
        encode_opt_str(record.reason.as_ref().map(|value| value.as_str())),
    ));
    fields.push((
        "signing_timestamp".to_owned(),
        encode_opt_str(
            record
                .signing_timestamp
                .as_ref()
                .map(|value| value.as_str()),
        ),
    ));

    let mut bytes = Vec::new();
    for (name, value) in fields {
        push_component(&mut bytes, &name);
        push_component(&mut bytes, &value);
    }
    bytes
}

fn encode_value(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Text { value, language } => {
            format!(
                "text:{}:{}",
                encode_component(value),
                encode_opt_str(language.as_deref())
            )
        }
        PropertyValue::LocalizedTextKey(value) => {
            format!("localized-text-key:{}", encode_component(value))
        }
        PropertyValue::Number(value) => format!("number:{:016x}", value.to_bits()),
        PropertyValue::Boolean(value) => format!("boolean:{value}"),
        PropertyValue::DateTime(value) => format!("date-time:{}", encode_component(value)),
        PropertyValue::EnumValue(value) => format!("enum-value:{}", encode_component(value)),
        PropertyValue::Reference(value) => {
            format!("reference:{}", encode_component(value.as_str()))
        }
        PropertyValue::AttachmentReference(value) => {
            format!("attachment-reference:{}", encode_component(value))
        }
        PropertyValue::BinaryBlob(value) => {
            let bytes = value
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<Vec<_>>()
                .join("");
            format!("binary-blob:{bytes}")
        }
    }
}

fn encode_prior_reference(reference: Option<&PriorReference>) -> String {
    match reference {
        Some(PriorReference::Transaction(transaction_id)) => {
            format!("transaction:{}", encode_component(transaction_id.as_str()))
        }
        Some(PriorReference::Event(event_id)) => format!("event:{}", encode_component(event_id)),
        None => "none".to_owned(),
    }
}

fn encode_rule_evaluation(metadata: Option<&RuleEvaluationMetadata>) -> String {
    let Some(metadata) = metadata else {
        return "none".to_owned();
    };

    let mut encoded = String::new();
    encoded.push_str(&format!(
        "triggering_event_ref={};",
        encode_opt_str(metadata.triggering_event_ref.as_deref())
    ));
    encoded.push_str(&format!(
        "rule_version={};",
        encode_opt_str(metadata.rule_version.as_deref())
    ));
    encoded.push_str(&format!(
        "evaluation_result={};",
        encode_opt_str(metadata.evaluation_result.as_deref())
    ));
    encoded.push_str("generated_actions=");
    for action in &metadata.generated_actions {
        encoded.push_str(&encode_component(action));
    }
    encoded.push(';');
    encoded.push_str("execution_metadata=");
    for (key, value) in &metadata.execution_metadata {
        encoded.push_str(&encode_component(key));
        encoded.push_str(&encode_component(value));
    }
    encoded
}

fn encode_opt_str(value: Option<&str>) -> String {
    value
        .map(encode_component)
        .unwrap_or_else(|| "none".to_owned())
}

fn encode_component(value: &str) -> String {
    format!("{}:{value};", value.len())
}

fn push_component(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(value.len().to_string().as_bytes());
    bytes.push(b':');
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(b';');
}
