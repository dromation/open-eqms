use open_eqms_event_engine::storage::{
    AppendSequenceGenerator, EventIdGenerator, StorageProvider as EventStorageProvider,
};
use open_eqms_event_engine::types::{
    EventTypeDefinition, EventTypeRef, PayloadConstraints, PayloadFieldDefinition,
};
use open_eqms_event_engine::{errors::EventEngineResult, EventEngine};
use open_eqms_object_runtime::storage::{
    ObjectIdGenerator, StorageProvider as ObjectStorageProvider,
};
use open_eqms_object_runtime::types::{
    ObjectTypeDefinition, ObjectTypeRef, PropertyDefinition, StructuralConstraints,
};
use open_eqms_object_runtime::{errors::ObjectRuntimeResult, ObjectRuntime};
use open_eqms_runtime_contracts::PropertyValueKind;
use std::collections::{BTreeMap, BTreeSet};

pub const ASSET_OBJECT_TYPE_NAME: &str = "asset.equipment";
pub const ASSET_REGISTERED_EVENT_TYPE_NAME: &str = "asset.registered";
pub const ASSET_CALIBRATION_PERFORMED_EVENT_TYPE_NAME: &str = "asset.calibration_performed";
pub const ASSET_CALIBRATION_ACCEPTED_EVENT_TYPE_NAME: &str = "asset.calibration_accepted";

pub fn asset_object_type_ref() -> ObjectTypeRef {
    ObjectTypeRef::new(ASSET_OBJECT_TYPE_NAME, 1)
}

pub fn asset_registered_event_type_ref() -> EventTypeRef {
    EventTypeRef::new(ASSET_REGISTERED_EVENT_TYPE_NAME, 1)
}

pub fn asset_calibration_performed_event_type_ref() -> EventTypeRef {
    EventTypeRef::new(ASSET_CALIBRATION_PERFORMED_EVENT_TYPE_NAME, 1)
}

pub fn asset_calibration_accepted_event_type_ref() -> EventTypeRef {
    EventTypeRef::new(ASSET_CALIBRATION_ACCEPTED_EVENT_TYPE_NAME, 1)
}

pub fn asset_object_type_definition() -> ObjectTypeDefinition {
    ObjectTypeDefinition::new(
        asset_object_type_ref(),
        asset_property_definitions(),
        BTreeMap::new(),
        None,
    )
}

pub fn asset_event_type_definitions() -> Vec<EventTypeDefinition> {
    vec![
        EventTypeDefinition::new(
            asset_registered_event_type_ref(),
            payload_fields([
                payload_field("asset_id", PropertyValueKind::Reference, true),
                payload_field("asset_class", PropertyValueKind::EnumValue, true)
                    .with_allowed_values(asset_class_values()),
                payload_field("registered_at", PropertyValueKind::DateTime, true),
            ]),
        ),
        EventTypeDefinition::new(
            asset_calibration_performed_event_type_ref(),
            payload_fields([
                payload_field("asset_id", PropertyValueKind::Reference, true),
                payload_field("performed_at", PropertyValueKind::DateTime, true),
                payload_field("outcome", PropertyValueKind::EnumValue, true).with_allowed_values(
                    BTreeSet::from(["accepted".to_owned(), "rejected".to_owned()]),
                ),
                payload_field("note", PropertyValueKind::Text, false),
            ]),
        ),
        EventTypeDefinition::new(
            asset_calibration_accepted_event_type_ref(),
            payload_fields([
                payload_field("asset_id", PropertyValueKind::Reference, true),
                payload_field("accepted_at", PropertyValueKind::DateTime, true),
                payload_field("next_calibration_due", PropertyValueKind::DateTime, true),
            ]),
        ),
    ]
}

pub fn register_object_metadata<S, G>(runtime: &ObjectRuntime<S, G>) -> ObjectRuntimeResult<()>
where
    S: ObjectStorageProvider,
    G: ObjectIdGenerator,
{
    runtime.register_object_type(asset_object_type_definition())
}

pub fn register_event_metadata<S, I, Q>(engine: &EventEngine<S, I, Q>) -> EventEngineResult<()>
where
    S: EventStorageProvider,
    I: EventIdGenerator,
    Q: AppendSequenceGenerator,
{
    for definition in asset_event_type_definitions() {
        engine.register_event_type(definition)?;
    }
    Ok(())
}

fn asset_property_definitions() -> BTreeMap<String, PropertyDefinition> {
    [
        property("identity_label", PropertyValueKind::Text, true),
        property("asset_class", PropertyValueKind::EnumValue, true)
            .with_allowed_values(asset_class_values()),
        property("organization", PropertyValueKind::Text, true),
        property("location", PropertyValueKind::Text, true),
        property("manufacturer", PropertyValueKind::Text, true),
        property("model", PropertyValueKind::Text, true),
        property("serial_number", PropertyValueKind::Text, true),
        property("status", PropertyValueKind::EnumValue, true).with_allowed_values(BTreeSet::from(
            [
                "registered".to_owned(),
                "in_service".to_owned(),
                "out_of_service".to_owned(),
            ],
        )),
        property("responsible_reference", PropertyValueKind::Text, true),
        property("registered_at", PropertyValueKind::DateTime, true),
        property("calibration_required", PropertyValueKind::Boolean, true),
        property(
            "calibration_interval_days",
            PropertyValueKind::Number,
            false,
        ),
        property("next_calibration_due", PropertyValueKind::DateTime, false),
    ]
    .into_iter()
    .map(|definition| (definition.name.clone(), definition))
    .collect()
}

fn asset_class_values() -> BTreeSet<String> {
    BTreeSet::from([
        "measuring_equipment".to_owned(),
        "machine".to_owned(),
        "robot".to_owned(),
        "tool".to_owned(),
        "fixture".to_owned(),
        "generic".to_owned(),
    ])
}

fn property(name: &str, kind: PropertyValueKind, required: bool) -> PropertyDefinition {
    PropertyDefinition::new(name, kind, required, text_constraints())
}

fn payload_field(name: &str, kind: PropertyValueKind, required: bool) -> PayloadFieldDefinition {
    PayloadFieldDefinition::new(name, kind, required, payload_text_constraints())
}

fn payload_fields<const N: usize>(
    fields: [PayloadFieldDefinition; N],
) -> BTreeMap<String, PayloadFieldDefinition> {
    fields
        .into_iter()
        .map(|field| (field.name.clone(), field))
        .collect()
}

fn text_constraints() -> StructuralConstraints {
    StructuralConstraints {
        min_length: Some(1),
        max_length: Some(128),
        ..StructuralConstraints::default()
    }
}

fn payload_text_constraints() -> PayloadConstraints {
    PayloadConstraints {
        min_length: Some(1),
        max_length: Some(256),
        ..PayloadConstraints::default()
    }
}

trait WithObjectAllowedValues {
    fn with_allowed_values(self, values: BTreeSet<String>) -> Self;
}

impl WithObjectAllowedValues for PropertyDefinition {
    fn with_allowed_values(mut self, values: BTreeSet<String>) -> Self {
        self.constraints.allowed_enum_values = values;
        self
    }
}

trait WithPayloadAllowedValues {
    fn with_allowed_values(self, values: BTreeSet<String>) -> Self;
}

impl WithPayloadAllowedValues for PayloadFieldDefinition {
    fn with_allowed_values(mut self, values: BTreeSet<String>) -> Self {
        self.constraints.allowed_enum_values = values;
        self
    }
}
