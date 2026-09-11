//! Validation logic for Process Definitions and transition requests.

use std::collections::BTreeSet;

use open_eqms_object_runtime::errors::{ObjectRuntimeError, ValidationError as ObjectValidation};
use open_eqms_object_runtime::types::{ObjectRecord, ObjectTypeDefinition};
use open_eqms_runtime_contracts::{PropertyValue, PropertyValueKind};
use open_eqms_transaction_engine::types::TransactionId;

use crate::errors::{ProcessDefinitionValidationError, ProcessEngineError, ProcessEngineResult};
use crate::types::{
    current_node_property_key, extract_current_node_definition_id, last_transaction_property_key,
    ProcessDefinition, ProcessDefinitionId, ProcessNodeId,
};

pub(crate) fn validate_process_definition(
    definition: &ProcessDefinition,
) -> ProcessEngineResult<()> {
    if definition.id.is_empty() {
        return Err(malformed(
            ProcessDefinitionValidationError::EmptyDefinitionId,
        ));
    }
    if definition.schema_version.value() == 0 {
        return Err(malformed(
            ProcessDefinitionValidationError::EmptySchemaVersion,
        ));
    }
    if definition.nodes.is_empty() {
        return Err(malformed(ProcessDefinitionValidationError::EmptyNodeSet));
    }
    if definition.entry_nodes.is_empty() {
        return Err(malformed(
            ProcessDefinitionValidationError::EmptyEntryNodeSet,
        ));
    }
    for node in &definition.nodes {
        if node.is_empty() {
            return Err(malformed(ProcessDefinitionValidationError::EmptyNodeId));
        }
    }
    for entry_node in &definition.entry_nodes {
        if entry_node.is_empty() {
            return Err(malformed(ProcessDefinitionValidationError::EmptyNodeId));
        }
        if !definition.nodes.contains(entry_node) {
            return Err(malformed(
                ProcessDefinitionValidationError::EntryNodeNotDeclared {
                    node: entry_node.clone(),
                },
            ));
        }
    }

    let mut edges = BTreeSet::<(ProcessNodeId, ProcessNodeId)>::new();
    for transition in &definition.transitions {
        if transition.from.is_empty() || transition.to.is_empty() {
            return Err(malformed(ProcessDefinitionValidationError::EmptyNodeId));
        }
        if transition.name.is_empty() {
            return Err(malformed(
                ProcessDefinitionValidationError::EmptyTransitionName {
                    from: transition.from.clone(),
                    to: transition.to.clone(),
                },
            ));
        }
        if !definition.nodes.contains(&transition.from) {
            return Err(malformed(
                ProcessDefinitionValidationError::TransitionSourceNotDeclared {
                    node: transition.from.clone(),
                },
            ));
        }
        if !definition.nodes.contains(&transition.to) {
            return Err(malformed(
                ProcessDefinitionValidationError::TransitionTargetNotDeclared {
                    node: transition.to.clone(),
                },
            ));
        }
        if !edges.insert((transition.from.clone(), transition.to.clone())) {
            return Err(malformed(
                ProcessDefinitionValidationError::DuplicateTransitionEdge {
                    from: transition.from.clone(),
                    to: transition.to.clone(),
                },
            ));
        }
    }

    Ok(())
}

pub(crate) fn verify_process_properties_declared(
    object_type: &ObjectTypeDefinition,
    definition_id: &ProcessDefinitionId,
) -> ProcessEngineResult<()> {
    verify_property_kind(
        object_type,
        current_node_property_key(definition_id),
        PropertyValueKind::EnumValue,
    )?;
    verify_property_kind(
        object_type,
        last_transaction_property_key(definition_id),
        PropertyValueKind::Text,
    )
}

pub(crate) fn ensure_no_other_active_process(
    record: &ObjectRecord,
    definition_id: &ProcessDefinitionId,
) -> ProcessEngineResult<()> {
    for property_key in record.properties.keys() {
        let Some(active_definition_id) = extract_current_node_definition_id(property_key) else {
            continue;
        };
        if active_definition_id != *definition_id {
            return Err(ProcessEngineError::ObjectAlreadyInDifferentProcess {
                active_definition_id,
            });
        }
    }
    Ok(())
}

pub(crate) fn current_node_from_record(
    record: &ObjectRecord,
    definition_id: &ProcessDefinitionId,
) -> ProcessEngineResult<Option<ProcessNodeId>> {
    let property_key = current_node_property_key(definition_id);
    let Some(value) = record.properties.get(&property_key) else {
        return Ok(None);
    };

    let node = match value {
        PropertyValue::EnumValue(value) | PropertyValue::Text { value, .. }
            if !value.is_empty() =>
        {
            ProcessNodeId::new(value.clone())
        }
        _ => {
            return Err(ProcessEngineError::UnknownCurrentState {
                recorded_value: format!("{value:?}"),
            });
        }
    };
    Ok(Some(node))
}

pub(crate) fn prior_transaction_from_record(
    record: &ObjectRecord,
    definition_id: &ProcessDefinitionId,
) -> ProcessEngineResult<Option<TransactionId>> {
    let property_key = last_transaction_property_key(definition_id);
    let Some(value) = record.properties.get(&property_key) else {
        return Ok(None);
    };

    match value {
        PropertyValue::Text { value, .. } if !value.is_empty() => {
            Ok(Some(TransactionId::new(value.clone())))
        }
        _ => Err(ProcessEngineError::UnknownCurrentState {
            recorded_value: format!("{property_key}={value:?}"),
        }),
    }
}

pub(crate) fn transition_name_for_request(
    definition: &ProcessDefinition,
    current_node: Option<&ProcessNodeId>,
    to: &ProcessNodeId,
) -> ProcessEngineResult<String> {
    if !definition.nodes.contains(to) {
        return Err(ProcessEngineError::NoSuchTransitionEdge {
            from: current_node.cloned(),
            to: to.clone(),
        });
    }

    let Some(from) = current_node else {
        if definition.entry_nodes.contains(to) {
            return Ok("entry".to_owned());
        }
        return Err(ProcessEngineError::NoSuchTransitionEdge {
            from: None,
            to: to.clone(),
        });
    };

    if !definition.nodes.contains(from) {
        return Err(ProcessEngineError::UnknownCurrentState {
            recorded_value: from.as_str().to_owned(),
        });
    }

    definition
        .transitions
        .iter()
        .find(|transition| &transition.from == from && &transition.to == to)
        .map(|transition| transition.name.clone())
        .ok_or_else(|| ProcessEngineError::NoSuchTransitionEdge {
            from: Some(from.clone()),
            to: to.clone(),
        })
}

pub(crate) fn map_object_read_error(error: ObjectRuntimeError) -> ProcessEngineError {
    match error {
        ObjectRuntimeError::ObjectNotFound { object_id } => {
            ProcessEngineError::ObjectNotFound { object_id }
        }
        ObjectRuntimeError::ObjectTypeNotFound { object_type } => {
            ProcessEngineError::ObjectTypeNotFound { object_type }
        }
        source => ProcessEngineError::ObjectRuntimeFailure { source },
    }
}

pub(crate) fn map_object_update_error(
    error: ObjectRuntimeError,
    definition_id: &ProcessDefinitionId,
) -> ProcessEngineError {
    if let ObjectRuntimeError::ValidationFailed {
        failure: ObjectValidation::UnknownProperty { property },
    } = &error
    {
        if property == &current_node_property_key(definition_id)
            || property == &last_transaction_property_key(definition_id)
        {
            return ProcessEngineError::InternalPreconditionInvariantViolated {
                property_key: property.clone(),
                source: error,
            };
        }
    }
    map_object_read_error(error)
}

fn verify_property_kind(
    object_type: &ObjectTypeDefinition,
    property_key: String,
    expected: PropertyValueKind,
) -> ProcessEngineResult<()> {
    let property = object_type.properties.get(&property_key).ok_or_else(|| {
        ProcessEngineError::ObjectTypeMissingProcessPropertyDeclaration {
            object_type: object_type.type_ref.clone(),
            property_key: property_key.clone(),
        }
    })?;
    if property.kind != expected {
        return Err(ProcessEngineError::ObjectTypeProcessPropertyKindMismatch {
            object_type: object_type.type_ref.clone(),
            property_key,
            expected,
            actual: property.kind.clone(),
        });
    }
    Ok(())
}

fn malformed(failure: ProcessDefinitionValidationError) -> ProcessEngineError {
    ProcessEngineError::MalformedDefinition { failure }
}
