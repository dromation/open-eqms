//! Validation logic for Object Type metadata and Object records.

use crate::errors::{
    InvalidRelationError, MetadataError, ObjectRuntimeError, ObjectRuntimeResult, ValidationError,
};
use crate::types::{
    ObjectId, ObjectRecord, ObjectTypeDefinition, OwnershipInfo, PermissionScopeRef, PropertyValue,
    PropertyValueKind, Relation, StructuralConstraints,
};
use regex_lite::Regex;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn validate_metadata(definition: &ObjectTypeDefinition) -> ObjectRuntimeResult<()> {
    if definition.type_ref.name.is_empty() {
        return Err(metadata(MetadataError::EmptyTypeName));
    }
    if definition.type_ref.version == 0 {
        return Err(metadata(MetadataError::EmptyTypeVersion));
    }

    for (key, property) in &definition.properties {
        if property.name.is_empty() {
            return Err(metadata(MetadataError::EmptyPropertyName));
        }
        if key != &property.name {
            return Err(metadata(MetadataError::PropertyKeyMismatch {
                key: key.clone(),
                name: property.name.clone(),
            }));
        }
        validate_constraints(&property.name, &property.constraints)?;
    }

    for (key, relation) in &definition.relations {
        if relation.relation_type.is_empty() {
            return Err(metadata(MetadataError::EmptyRelationType));
        }
        if key != &relation.relation_type {
            return Err(metadata(MetadataError::RelationKeyMismatch {
                key: key.clone(),
                relation_type: relation.relation_type.clone(),
            }));
        }
        if relation
            .cardinality
            .max
            .is_some_and(|max| relation.cardinality.min > max)
        {
            return Err(metadata(MetadataError::InvalidRange {
                field: format!("relation {} cardinality", relation.relation_type),
            }));
        }
    }

    Ok(())
}

pub(crate) fn validate_record_shape(
    definition: &ObjectTypeDefinition,
    record: &ObjectRecord,
) -> ObjectRuntimeResult<()> {
    validate_properties(definition, &record.properties)?;
    validate_relation_types_and_cardinality(definition, &record.relations)?;
    validate_lifecycle(&record.lifecycle_state)?;
    validate_owner_and_scope(&record.ownership, &record.permission_scope)?;
    Ok(())
}

pub(crate) fn validate_record_with_targets(
    definition: &ObjectTypeDefinition,
    record: &ObjectRecord,
    mut target_exists: impl FnMut(&ObjectId) -> ObjectRuntimeResult<bool>,
) -> ObjectRuntimeResult<()> {
    validate_record_shape(definition, record)?;
    for relation in &record.relations {
        if !target_exists(&relation.target_id)? {
            return Err(invalid_relation(InvalidRelationError::TargetDoesNotExist {
                target_id: relation.target_id.clone(),
            }));
        }
    }
    Ok(())
}

pub(crate) fn validate_owner_and_scope(
    ownership: &OwnershipInfo,
    permission_scope: &PermissionScopeRef,
) -> ObjectRuntimeResult<()> {
    if ownership.owner.is_empty() {
        return Err(validation(ValidationError::EmptyOwner));
    }
    if permission_scope.is_empty() {
        return Err(validation(ValidationError::EmptyPermissionScope));
    }
    Ok(())
}

fn validate_constraints(
    property: &str,
    constraints: &StructuralConstraints,
) -> ObjectRuntimeResult<()> {
    if constraints
        .min_length
        .zip(constraints.max_length)
        .is_some_and(|(min, max)| min > max)
    {
        return Err(metadata(MetadataError::InvalidRange {
            field: format!("{property} length"),
        }));
    }
    if constraints
        .min_number
        .zip(constraints.max_number)
        .is_some_and(|(min, max)| min > max)
    {
        return Err(metadata(MetadataError::InvalidRange {
            field: format!("{property} number"),
        }));
    }
    if let Some(pattern) = &constraints.pattern {
        Regex::new(pattern).map_err(|error| {
            metadata(MetadataError::InvalidPattern {
                property: property.to_owned(),
                reason: error.to_string(),
            })
        })?;
    }
    Ok(())
}

fn validate_lifecycle(lifecycle_state: &crate::types::LifecycleState) -> ObjectRuntimeResult<()> {
    if lifecycle_state.is_empty() {
        return Err(validation(ValidationError::EmptyLifecycleState));
    }
    Ok(())
}

fn validate_properties(
    definition: &ObjectTypeDefinition,
    properties: &BTreeMap<String, PropertyValue>,
) -> ObjectRuntimeResult<()> {
    for property_name in properties.keys() {
        if !definition.properties.contains_key(property_name) {
            return Err(validation(ValidationError::UnknownProperty {
                property: property_name.clone(),
            }));
        }
    }

    for (property_name, property_definition) in &definition.properties {
        match properties.get(property_name) {
            Some(value) => {
                let actual = value.kind();
                if actual != property_definition.kind {
                    return Err(validation(ValidationError::TypeMismatch {
                        property: property_name.clone(),
                        expected: kind_name(&property_definition.kind).to_owned(),
                        actual: kind_name(&actual).to_owned(),
                    }));
                }
                validate_value_constraints(property_name, value, &property_definition.constraints)?;
            }
            None if property_definition.required => {
                return Err(validation(ValidationError::MissingRequiredProperty {
                    property: property_name.clone(),
                }));
            }
            None => {}
        }
    }

    Ok(())
}

fn validate_value_constraints(
    property_name: &str,
    value: &PropertyValue,
    constraints: &StructuralConstraints,
) -> ObjectRuntimeResult<()> {
    if let Some(length) = value_length(value) {
        if constraints.min_length.is_some_and(|min| length < min) {
            return Err(constraint(property_name, "length below minimum"));
        }
        if constraints.max_length.is_some_and(|max| length > max) {
            return Err(constraint(property_name, "length above maximum"));
        }
    }

    if let PropertyValue::Number(number) = value {
        if constraints.min_number.is_some_and(|min| *number < min) {
            return Err(constraint(property_name, "number below minimum"));
        }
        if constraints.max_number.is_some_and(|max| *number > max) {
            return Err(constraint(property_name, "number above maximum"));
        }
    }

    if let PropertyValue::EnumValue(value) = value {
        if !constraints.allowed_enum_values.is_empty()
            && !constraints.allowed_enum_values.contains(value)
        {
            return Err(constraint(property_name, "enum value is not allowed"));
        }
    }

    if let Some(pattern) = &constraints.pattern {
        if let Some(text) = string_value(value) {
            let regex = Regex::new(pattern).map_err(|error| {
                metadata(MetadataError::InvalidPattern {
                    property: property_name.to_owned(),
                    reason: error.to_string(),
                })
            })?;
            if !regex.is_match(text) {
                return Err(constraint(property_name, "pattern did not match"));
            }
        }
    }

    Ok(())
}

fn validate_relation_types_and_cardinality(
    definition: &ObjectTypeDefinition,
    relations: &BTreeSet<Relation>,
) -> ObjectRuntimeResult<()> {
    let mut counts = BTreeMap::<String, usize>::new();

    for relation in relations {
        if !definition.relations.contains_key(&relation.relation_type) {
            return Err(invalid_relation(
                InvalidRelationError::UnknownRelationType {
                    relation_type: relation.relation_type.clone(),
                },
            ));
        }
        *counts.entry(relation.relation_type.clone()).or_default() += 1;
    }

    for (relation_type, relation_definition) in &definition.relations {
        let actual = counts.get(relation_type).copied().unwrap_or_default();
        let min = relation_definition.cardinality.min;
        let max = relation_definition.cardinality.max;
        if actual < min || max.is_some_and(|limit| actual > limit) {
            return Err(invalid_relation(
                InvalidRelationError::CardinalityViolated {
                    relation_type: relation_type.clone(),
                    min,
                    max,
                    actual,
                },
            ));
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

fn constraint(property: &str, reason: &str) -> ObjectRuntimeError {
    validation(ValidationError::ConstraintViolation {
        property: property.to_owned(),
        reason: reason.to_owned(),
    })
}

fn validation(failure: ValidationError) -> ObjectRuntimeError {
    ObjectRuntimeError::ValidationFailed { failure }
}

fn invalid_relation(failure: InvalidRelationError) -> ObjectRuntimeError {
    ObjectRuntimeError::InvalidRelation { failure }
}

fn metadata(failure: MetadataError) -> ObjectRuntimeError {
    ObjectRuntimeError::MalformedMetadata { failure }
}
