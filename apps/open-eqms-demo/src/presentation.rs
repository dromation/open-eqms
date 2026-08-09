use open_eqms_event_engine::types::EventRecord;
use open_eqms_object_runtime::types::ObjectRecord;
use open_eqms_runtime_contracts::{ObjectId, PropertyValue};
use open_eqms_transaction_engine::types::{PriorReference, TransactionRecord};
use std::fmt::Write;

pub fn render_asset(asset: &ObjectRecord) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Asset {}", asset.id);
    let _ = writeln!(
        output,
        "object_type: {}@{}",
        asset.object_type.name, asset.object_type.version
    );
    let _ = writeln!(output, "version: {}", asset.version.value());
    let _ = writeln!(
        output,
        "lifecycle_state: {}",
        asset.lifecycle_state.as_str()
    );
    let _ = writeln!(
        output,
        "permission_scope: {}",
        asset.permission_scope.as_str()
    );
    let _ = writeln!(output, "owner: {}", asset.ownership.owner);
    output.push_str("properties:\n");
    for (name, value) in &asset.properties {
        let _ = writeln!(output, "  {name}: {}", render_property_value(value));
    }
    output.push_str("relations:\n");
    if asset.relations.is_empty() {
        output.push_str("  none\n");
    } else {
        for relation in &asset.relations {
            let _ = writeln!(
                output,
                "  {} -> {}",
                relation.relation_type, relation.target_id
            );
        }
    }
    output
}

pub fn render_timeline(
    asset_id: &ObjectId,
    events: &[EventRecord],
    transactions: &[TransactionRecord],
) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "Timeline {asset_id}");

    let mut ordered_events = events.iter().collect::<Vec<_>>();
    ordered_events.sort_by_key(|event| event.append_sequence);
    let _ = writeln!(
        output,
        "Events ({}; bounded Event range read, app-layer filter):",
        ordered_events.len()
    );
    if ordered_events.is_empty() {
        output.push_str("  none\n");
    } else {
        for event in ordered_events {
            let _ = writeln!(
                output,
                "  event {} {} {}@{} occurred_at={} recorded_at={} source={}",
                event.append_sequence.value(),
                event.id,
                event.event_type.name,
                event.event_type.version,
                event.occurred_at.as_str(),
                event.recorded_at.as_str(),
                event.source.as_str()
            );
            for (name, value) in &event.payload {
                let _ = writeln!(output, "    {name}: {}", render_property_value(value));
            }
        }
    }

    let mut ordered_transactions = transactions.iter().collect::<Vec<_>>();
    ordered_transactions.sort_by_key(|transaction| transaction.append_index);
    let _ = writeln!(
        output,
        "Transactions ({}; bounded Transaction range read, app-layer filter):",
        ordered_transactions.len()
    );
    if ordered_transactions.is_empty() {
        output.push_str("  none\n");
    } else {
        for transaction in ordered_transactions {
            let _ = writeln!(
                output,
                "  transaction {} {} level={} operation={}",
                transaction.append_index.value(),
                transaction.id,
                transaction.level,
                transaction.operation.as_str()
            );
            let _ = writeln!(
                output,
                "    versions: {} -> {}",
                transaction.base_version.value(),
                transaction.resulting_version.value()
            );
            let _ = writeln!(
                output,
                "    edit_timestamp: {}",
                transaction.edit_timestamp.as_str()
            );
            let _ = writeln!(
                output,
                "    transaction_sequence: {}",
                transaction
                    .transaction_sequence
                    .map(|sequence| sequence.value().to_string())
                    .unwrap_or_else(|| "none".to_owned())
            );
            let _ = writeln!(
                output,
                "    server_receipt_time: {}",
                transaction
                    .server_receipt_time
                    .as_ref()
                    .map(|timestamp| timestamp.as_str())
                    .unwrap_or("none")
            );
            let _ = writeln!(
                output,
                "    prior_reference: {}",
                render_prior_reference(transaction.prior_reference.as_ref())
            );
            let _ = writeln!(
                output,
                "    prior_transaction_hash: {}",
                transaction
                    .prior_transaction_hash
                    .as_ref()
                    .map(|hash| hash.as_str())
                    .unwrap_or("none")
            );
            let _ = writeln!(
                output,
                "    transaction_hash: {}",
                transaction
                    .transaction_hash
                    .as_ref()
                    .map(|hash| hash.as_str())
                    .unwrap_or("none")
            );
        }
    }

    output
}

fn render_prior_reference(prior_reference: Option<&PriorReference>) -> String {
    match prior_reference {
        Some(PriorReference::Transaction(transaction_id)) => {
            format!("transaction:{transaction_id}")
        }
        Some(PriorReference::Event(event_id)) => format!("event:{event_id}"),
        None => "none".to_owned(),
    }
}

fn render_property_value(value: &PropertyValue) -> String {
    match value {
        PropertyValue::Text { value, language } => match language {
            Some(language) => format!("{value} [{language}]"),
            None => value.clone(),
        },
        PropertyValue::LocalizedTextKey(value)
        | PropertyValue::DateTime(value)
        | PropertyValue::EnumValue(value)
        | PropertyValue::AttachmentReference(value) => value.clone(),
        PropertyValue::Number(value) => value.to_string(),
        PropertyValue::Boolean(value) => value.to_string(),
        PropertyValue::Reference(object_id) => object_id.to_string(),
        PropertyValue::BinaryBlob(value) => format!("<{} bytes>", value.len()),
    }
}
