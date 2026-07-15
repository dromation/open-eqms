//! Validation logic for Event Type metadata and Event append requests.

use crate::errors::{EventEngineError, EventEngineResult, MetadataError, ValidationError};
use crate::types::{
    EventSource, EventTimestamp, EventTypeDefinition, ObjectId, PayloadConstraints, PropertyValue,
    PropertyValueKind,
};
use crate::AppendEventRequest;
use regex_lite::Regex;
use std::collections::BTreeMap;

pub(crate) fn validate_metadata(definition: &EventTypeDefinition) -> EventEngineResult<()> {
    if definition.type_ref.name.is_empty() {
        return Err(metadata(MetadataError::EmptyTypeName));
    }
    if definition.type_ref.version == 0 {
        return Err(metadata(MetadataError::EmptyTypeVersion));
    }

    for (key, field) in &definition.payload_fields {
        if field.name.is_empty() {
            return Err(metadata(MetadataError::EmptyPayloadFieldName));
        }
        if key != &field.name {
            return Err(metadata(MetadataError::PayloadFieldKeyMismatch {
                key: key.clone(),
                name: field.name.clone(),
            }));
        }
        validate_constraints(&field.name, &field.constraints)?;
    }

    Ok(())
}

pub(crate) fn validate_append_request(
    definition: &EventTypeDefinition,
    request: &AppendEventRequest,
) -> EventEngineResult<()> {
    validate_timestamp(&request.occurred_at, TimestampField::OccurredAt)?;
    validate_timestamp(&request.recorded_at, TimestampField::RecordedAt)?;
    validate_source(&request.source)?;
    validate_object_refs(&request.object_refs)?;
    validate_payload(definition, &request.payload)
}

fn validate_constraints(field: &str, constraints: &PayloadConstraints) -> EventEngineResult<()> {
    if constraints
        .min_length
        .zip(constraints.max_length)
        .is_some_and(|(min, max)| min > max)
    {
        return Err(metadata(MetadataError::InvalidRange {
            field: format!("{field} length"),
        }));
    }
    if constraints
        .min_number
        .zip(constraints.max_number)
        .is_some_and(|(min, max)| min > max)
    {
        return Err(metadata(MetadataError::InvalidRange {
            field: format!("{field} number"),
        }));
    }
    if let Some(pattern) = &constraints.pattern {
        Regex::new(pattern).map_err(|error| {
            metadata(MetadataError::InvalidPattern {
                field: field.to_owned(),
                reason: error.to_string(),
            })
        })?;
    }
    Ok(())
}

fn validate_timestamp(timestamp: &EventTimestamp, field: TimestampField) -> EventEngineResult<()> {
    if timestamp.is_empty() {
        return Err(validation(match field {
            TimestampField::OccurredAt => ValidationError::MissingOccurredAt,
            TimestampField::RecordedAt => ValidationError::MissingRecordedAt,
        }));
    }
    if !is_utc_timestamp(timestamp.as_str()) {
        return Err(validation(match field {
            TimestampField::OccurredAt => ValidationError::MalformedOccurredAt {
                value: timestamp.as_str().to_owned(),
            },
            TimestampField::RecordedAt => ValidationError::MalformedRecordedAt {
                value: timestamp.as_str().to_owned(),
            },
        }));
    }
    Ok(())
}

fn is_utc_timestamp(value: &str) -> bool {
    value.len() >= 20
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value.as_bytes().get(10) == Some(&b'T')
        && value.as_bytes().get(13) == Some(&b':')
        && value.as_bytes().get(16) == Some(&b':')
        && value.ends_with('Z')
        && value
            .chars()
            .enumerate()
            .all(|(index, character)| match index {
                4 | 7 => character == '-',
                10 => character == 'T',
                13 | 16 => character == ':',
                index if index == value.len() - 1 => character == 'Z',
                19 if value.len() > 20 => character == '.',
                _ => character.is_ascii_digit(),
            })
}

fn validate_source(source: &EventSource) -> EventEngineResult<()> {
    if source.is_empty() {
        return Err(validation(ValidationError::MissingEventSource));
    }
    Ok(())
}

fn validate_object_refs(
    object_refs: &std::collections::BTreeSet<ObjectId>,
) -> EventEngineResult<()> {
    for object_id in object_refs {
        if object_id.is_empty() {
            return Err(validation(ValidationError::MalformedObjectReference {
                field: "object_refs".to_owned(),
                object_id: object_id.clone(),
            }));
        }
    }
    Ok(())
}

fn validate_payload(
    definition: &EventTypeDefinition,
    payload: &BTreeMap<String, PropertyValue>,
) -> EventEngineResult<()> {
    for field_name in payload.keys() {
        if !definition.payload_fields.contains_key(field_name) {
            return Err(validation(ValidationError::UnknownPayloadField {
                field: field_name.clone(),
            }));
        }
    }

    for (field_name, field_definition) in &definition.payload_fields {
        match payload.get(field_name) {
            Some(value) => {
                let actual = value.kind();
                if actual != field_definition.kind {
                    return Err(validation(ValidationError::TypeMismatch {
                        field: field_name.clone(),
                        expected: kind_name(&field_definition.kind).to_owned(),
                        actual: kind_name(&actual).to_owned(),
                    }));
                }
                validate_value_constraints(field_name, value, &field_definition.constraints)?;
                validate_payload_reference(field_name, value)?;
            }
            None if field_definition.required => {
                return Err(validation(ValidationError::MissingRequiredField {
                    field: field_name.clone(),
                }));
            }
            None => {}
        }
    }

    Ok(())
}

fn validate_value_constraints(
    field_name: &str,
    value: &PropertyValue,
    constraints: &PayloadConstraints,
) -> EventEngineResult<()> {
    if let Some(length) = value_length(value) {
        if constraints.min_length.is_some_and(|min| length < min) {
            return Err(constraint(field_name, "length below minimum"));
        }
        if constraints.max_length.is_some_and(|max| length > max) {
            return Err(constraint(field_name, "length above maximum"));
        }
    }

    if let PropertyValue::Number(number) = value {
        if constraints.min_number.is_some_and(|min| *number < min) {
            return Err(constraint(field_name, "number below minimum"));
        }
        if constraints.max_number.is_some_and(|max| *number > max) {
            return Err(constraint(field_name, "number above maximum"));
        }
    }

    if let PropertyValue::EnumValue(value) = value {
        if !constraints.allowed_enum_values.is_empty()
            && !constraints.allowed_enum_values.contains(value)
        {
            return Err(constraint(field_name, "enum value is not allowed"));
        }
    }

    if let Some(pattern) = &constraints.pattern {
        if let Some(text) = string_value(value) {
            let regex = Regex::new(pattern).map_err(|error| {
                metadata(MetadataError::InvalidPattern {
                    field: field_name.to_owned(),
                    reason: error.to_string(),
                })
            })?;
            if !regex.is_match(text) {
                return Err(constraint(field_name, "pattern did not match"));
            }
        }
    }

    Ok(())
}

fn validate_payload_reference(field_name: &str, value: &PropertyValue) -> EventEngineResult<()> {
    if let PropertyValue::Reference(object_id) = value {
        if object_id.is_empty() {
            return Err(validation(ValidationError::MalformedObjectReference {
                field: field_name.to_owned(),
                object_id: object_id.clone(),
            }));
        }
    }
    Ok(())
}

fn value_length(value: &PropertyValue) -> Option<usize> {
    match value {
        PropertyValue::Text { value, .. }
        | PropertyValue::LocalizedTextKey(value)
        | PropertyValue::DateTime(value)
        | PropertyValue::EnumValue(value)
        | PropertyValue::AttachmentReference(value) => Some(value.len()),
        PropertyValue::BinaryBlob(value) => Some(value.len()),
        PropertyValue::Number(_) | PropertyValue::Boolean(_) | PropertyValue::Reference(_) => None,
    }
}

fn string_value(value: &PropertyValue) -> Option<&str> {
    match value {
        PropertyValue::Text { value, .. }
        | PropertyValue::LocalizedTextKey(value)
        | PropertyValue::DateTime(value)
        | PropertyValue::EnumValue(value)
        | PropertyValue::AttachmentReference(value) => Some(value),
        PropertyValue::Number(_)
        | PropertyValue::Boolean(_)
        | PropertyValue::Reference(_)
        | PropertyValue::BinaryBlob(_) => None,
    }
}

fn kind_name(kind: &PropertyValueKind) -> &'static str {
    match kind {
        PropertyValueKind::Text => "text",
        PropertyValueKind::LocalizedTextKey => "localized-text-key",
        PropertyValueKind::Number => "number",
        PropertyValueKind::Boolean => "boolean",
        PropertyValueKind::DateTime => "date/time",
        PropertyValueKind::EnumValue => "enum-value",
        PropertyValueKind::Reference => "reference",
        PropertyValueKind::AttachmentReference => "attachment-reference",
        PropertyValueKind::BinaryBlob => "binary-blob",
    }
}

fn constraint(field: &str, reason: &str) -> EventEngineError {
    validation(ValidationError::ConstraintViolation {
        field: field.to_owned(),
        reason: reason.to_owned(),
    })
}

fn validation(failure: ValidationError) -> EventEngineError {
    EventEngineError::ValidationFailed { failure }
}

fn metadata(failure: MetadataError) -> EventEngineError {
    EventEngineError::MalformedMetadata { failure }
}

#[derive(Clone, Copy)]
enum TimestampField {
    OccurredAt,
    RecordedAt,
}
