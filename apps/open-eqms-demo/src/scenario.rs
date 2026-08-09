use crate::asset_model::{
    asset_calibration_accepted_event_type_ref, asset_calibration_performed_event_type_ref,
    asset_object_type_ref, asset_registered_event_type_ref, register_event_metadata,
    register_object_metadata,
};
use crate::clock::DeterministicClock;
use crate::crypto::DemoCryptographicProvider;
use crate::ids::{
    DeterministicAppendSequences, DeterministicEventIds, DeterministicObjectIds,
    DeterministicTransactionIds,
};
use crate::outcome::{OperationOutcome, StepId};
use crate::storage::{DemoEventStore, DemoObjectAndTransactionStore};
use open_eqms_event_engine::types::{
    AppendSequence, CorrelationId, EventId, EventRecord, EventSource, EventTimestamp,
};
use open_eqms_event_engine::{AppendEventRequest, EventEngine};
use open_eqms_object_runtime::types::{
    LifecycleState, ObjectRecord, OwnershipInfo, PermissionScopeRef, PropertyChange, Version,
};
use open_eqms_object_runtime::{
    CreateObjectRequest, ObjectChanges, ObjectRuntime, UpdateObjectRequest,
};
use open_eqms_runtime_contracts::{ObjectId, PropertyValue, UnitOfWork};
use open_eqms_transaction_engine::types::{
    ActorRef, AppendIndex, DeviceRef, OperationDescriptor, PriorReference, SiteRef, TransactionId,
    TransactionLevel, TransactionRecord, TransactionSchemaVersion, TransactionTimestamp,
};
use open_eqms_transaction_engine::{
    AppendTransactionRequest, AppendTransactionResult, TransactionEngine,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

const DEMO_RANGE_READ_LIMIT: usize = 100;

pub type DemoObjectRuntime = ObjectRuntime<DemoObjectAndTransactionStore, DeterministicObjectIds>;
pub type DemoEventEngine =
    EventEngine<DemoEventStore, DeterministicEventIds, DeterministicAppendSequences>;
pub type DemoTransactionEngine = TransactionEngine<
    DemoObjectAndTransactionStore,
    DeterministicTransactionIds,
    DemoCryptographicProvider,
>;

pub struct DemoApp {
    object_transaction_store: DemoObjectAndTransactionStore,
    event_store: DemoEventStore,
    object_runtime: DemoObjectRuntime,
    event_engine: DemoEventEngine,
    transaction_engine: DemoTransactionEngine,
    clock: DeterministicClock,
    metadata_registered: bool,
    registration_transactions_by_asset: BTreeMap<ObjectId, TransactionId>,
}

impl DemoApp {
    pub fn new() -> Self {
        let object_transaction_store = DemoObjectAndTransactionStore::default();
        let event_store = DemoEventStore::default();
        let object_runtime = ObjectRuntime::new(
            object_transaction_store.clone(),
            DeterministicObjectIds::new("asset"),
        );
        let event_engine = EventEngine::new(
            event_store.clone(),
            DeterministicEventIds::new("event"),
            DeterministicAppendSequences::new(),
        );
        let transaction_engine = TransactionEngine::new(
            object_transaction_store.clone(),
            DeterministicTransactionIds::new("txn"),
            DemoCryptographicProvider::default(),
        );

        Self {
            object_transaction_store,
            event_store,
            object_runtime,
            event_engine,
            transaction_engine,
            clock: DeterministicClock::demo(),
            metadata_registered: false,
            registration_transactions_by_asset: BTreeMap::new(),
        }
    }

    pub fn register_asset(&mut self, input: RegisterAssetInput) -> OperationOutcome {
        if let Err(message) = input.validate() {
            return OperationOutcome::fail_before_mutation(StepId::ValidateAssetInput, message);
        }

        let mut outcome = OperationOutcome::new();
        outcome.completed_steps.push(StepId::ValidateAssetInput);

        if let Err(error) = self.ensure_metadata_registered() {
            outcome.mark_partial(
                StepId::RegisterMetadata,
                format!("metadata registration failed before asset records were created: {error}"),
            );
            return outcome;
        }
        outcome.completed_steps.push(StepId::RegisterMetadata);

        let created = match self.object_runtime.create_object(input.create_request()) {
            Ok(created) => created,
            Err(error) => {
                outcome.mark_partial(
                    StepId::CreateAssetObject,
                    format!("object creation failed before event or transaction append: {error}"),
                );
                return outcome;
            }
        };
        outcome.completed_steps.push(StepId::CreateAssetObject);
        outcome.created_object_id = Some(created.object_id.clone());

        let registered_event = match self.append_registered_event(&input, &created.object_id) {
            Ok(event) => event,
            Err(error) => {
                outcome.mark_partial(
                    StepId::AppendAssetRegisteredEvent,
                    format!(
                        "asset object {} exists, but the registration event was not appended: {error}. Next safe step: inspect the object and append the missing registration event; do not delete immutable records.",
                        created.object_id
                    ),
                );
                return outcome;
            }
        };
        outcome
            .completed_steps
            .push(StepId::AppendAssetRegisteredEvent);
        outcome
            .created_event_ids
            .push(registered_event.event_id.clone());

        let registration_transaction = match self
            .append_registration_transaction(&input, &created.object_id)
        {
            Ok(transaction_id) => transaction_id,
            Err(error) => {
                outcome.mark_partial(
                        StepId::AppendRegistrationTransaction,
                        format!(
                            "asset object {} and event {} exist, but the Level1 registration transaction was not appended: {error}. Next safe step: append the missing registration transaction referencing no prior transaction; do not delete immutable records.",
                            created.object_id, registered_event.event_id
                        ),
                    );
                return outcome;
            }
        };
        outcome
            .completed_steps
            .push(StepId::AppendRegistrationTransaction);
        outcome
            .created_transaction_ids
            .push(registration_transaction.clone());
        self.registration_transactions_by_asset
            .insert(created.object_id, registration_transaction);
        outcome.mark_complete("asset registration complete; all sequential records were created.");
        outcome
    }

    pub fn record_calibration(&mut self, input: RecordCalibrationInput) -> OperationOutcome {
        if let Err(message) = input.validate() {
            return OperationOutcome::fail_before_mutation(
                StepId::ValidateCalibrationInput,
                message,
            );
        }

        let mut outcome = OperationOutcome::new();
        outcome
            .completed_steps
            .push(StepId::ValidateCalibrationInput);

        if let Err(error) = self.ensure_metadata_registered() {
            outcome.mark_partial(
                StepId::RegisterMetadata,
                format!(
                    "metadata registration failed before calibration records were created: {error}"
                ),
            );
            return outcome;
        }
        outcome.completed_steps.push(StepId::RegisterMetadata);

        let current_asset = match self.object_runtime.read_object(&input.asset_id) {
            Ok(asset) => asset,
            Err(error) => {
                return OperationOutcome::fail_before_mutation(
                    StepId::ValidateCalibrationInput,
                    format!("asset {} is not registered; no Event or Transaction append was attempted: {error}", input.asset_id),
                );
            }
        };
        let Some(registration_transaction_id) = self
            .registration_transactions_by_asset
            .get(&input.asset_id)
            .cloned()
        else {
            return OperationOutcome::fail_before_mutation(
                StepId::ValidateCalibrationInput,
                format!("asset {} has no known registration transaction; no Event or Transaction append was attempted.", input.asset_id),
            );
        };

        let performed_event = match self.append_calibration_performed_event(&input) {
            Ok(event) => event,
            Err(error) => {
                outcome.mark_partial(
                    StepId::AppendCalibrationPerformedEvent,
                    format!("calibration performed event was not appended: {error}"),
                );
                return outcome;
            }
        };
        outcome
            .completed_steps
            .push(StepId::AppendCalibrationPerformedEvent);
        outcome
            .created_event_ids
            .push(performed_event.event_id.clone());

        if input.outcome == CalibrationOutcome::Rejected {
            outcome.mark_complete(
                "calibration rejection recorded as asset.calibration_performed only; no acceptance event, object update, or Level2 transaction was created.",
            );
            return outcome;
        }

        let accepted_event = match self.append_calibration_accepted_event(&input) {
            Ok(event) => event,
            Err(error) => {
                outcome.mark_partial(
                    StepId::AppendCalibrationAcceptedEvent,
                    format!(
                        "performed event {} exists, but the acceptance event was not appended: {error}. Next safe step: inspect Events and retry acceptance recording; do not delete immutable records.",
                        performed_event.event_id
                    ),
                );
                return outcome;
            }
        };
        outcome
            .completed_steps
            .push(StepId::AppendCalibrationAcceptedEvent);
        outcome
            .created_event_ids
            .push(accepted_event.event_id.clone());

        let unit_of_work = UnitOfWork::new(format!("uow-calibration-{}", input.asset_id.as_str()));
        if let Err(error) = self
            .object_transaction_store
            .begin_shared_unit_of_work(&unit_of_work)
        {
            outcome.mark_partial(
                StepId::StageCalibrationObjectUpdate,
                format!("calibration events exist, but the shared Unit of Work could not begin: {error}. Next safe step: retry the object update plus Level2 transaction pairing."),
            );
            return outcome;
        }

        let updated_asset = match self.object_runtime.update_object_in_unit_of_work(
            UpdateObjectRequest {
                object_id: input.asset_id.clone(),
                base_version: current_asset.version,
                changes: input.acceptance_changes(),
            },
            &unit_of_work,
        ) {
            Ok(updated) => updated,
            Err(error) => {
                let _ = self
                    .object_transaction_store
                    .rollback_shared_unit_of_work(&unit_of_work);
                outcome.mark_partial(
                    StepId::StageCalibrationObjectUpdate,
                    format!("calibration events exist, but the object update could not be staged: {error}. Next safe step: retry the object update plus Level2 transaction pairing."),
                );
                return outcome;
            }
        };
        outcome
            .completed_steps
            .push(StepId::StageCalibrationObjectUpdate);

        let staged_transaction = match self.transaction_engine.append_transaction_in_unit_of_work(
            calibration_transaction_request(
                &input,
                &current_asset,
                &updated_asset,
                registration_transaction_id,
            ),
            &unit_of_work,
        ) {
            Ok(AppendTransactionResult::Staged(staged)) => staged.transaction_id,
            Ok(AppendTransactionResult::Committed(_)) => {
                let _ = self
                    .object_transaction_store
                    .rollback_shared_unit_of_work(&unit_of_work);
                outcome.mark_partial(
                    StepId::StageCalibrationTransaction,
                    "calibration transaction unexpectedly committed outside the shared Unit of Work. Next safe step: stop and inspect storage state.",
                );
                return outcome;
            }
            Err(error) => {
                let _ = self
                    .object_transaction_store
                    .rollback_shared_unit_of_work(&unit_of_work);
                outcome.mark_partial(
                    StepId::StageCalibrationTransaction,
                    format!("calibration events exist, but the Level2 transaction could not be staged: {error}. The paired object update was rolled back; retry the object update plus Level2 transaction pairing."),
                );
                return outcome;
            }
        };
        outcome
            .completed_steps
            .push(StepId::StageCalibrationTransaction);

        if let Err(error) = self
            .object_transaction_store
            .commit_shared_unit_of_work(&unit_of_work, &mut DemoCryptographicProvider::default())
        {
            let _ = self
                .object_transaction_store
                .rollback_shared_unit_of_work(&unit_of_work);
            outcome.mark_partial(
                StepId::CommitCalibrationUnitOfWork,
                format!("calibration events exist, but the shared Object update plus Level2 transaction failed to commit: {error}. The paired Object update is not durably visible; retry the shared pairing."),
            );
            return outcome;
        }
        outcome
            .completed_steps
            .push(StepId::CommitCalibrationUnitOfWork);
        outcome.created_transaction_ids.push(staged_transaction);
        outcome.mark_complete(
            "calibration accepted; performed and accepted Events were appended independently, and the Object update plus Level2 transaction committed together.",
        );
        outcome
    }

    pub fn read_asset(&self, object_id: &ObjectId) -> Result<ObjectRecord, String> {
        self.object_runtime
            .read_object(object_id)
            .map_err(|error| error.to_string())
    }

    pub fn show_asset(&self, object_id: &ObjectId) -> Result<String, String> {
        let asset = self.read_asset(object_id)?;
        Ok(crate::presentation::render_asset(&asset))
    }

    pub fn show_timeline(&self, object_id: &ObjectId) -> Result<String, String> {
        self.read_asset(object_id)?;

        // VS-001 does not depend on Query Engine execution. Until that boundary
        // exists, the demo uses bounded Event/Transaction range reads and filters
        // by Object identity in this app layer.
        let events = self
            .event_engine
            .read_sequence_range(AppendSequence::first(), DEMO_RANGE_READ_LIMIT)
            .map_err(|error| error.to_string())?
            .events
            .into_iter()
            .filter(|event| event.object_refs.contains(object_id))
            .collect::<Vec<_>>();
        let transactions = self
            .transaction_engine
            .read_append_index_range(AppendIndex::first(), DEMO_RANGE_READ_LIMIT)
            .map_err(|error| error.to_string())?
            .transactions
            .into_iter()
            .filter(|transaction| &transaction.object_id == object_id)
            .collect::<Vec<_>>();

        Ok(crate::presentation::render_timeline(
            object_id,
            &events,
            &transactions,
        ))
    }

    pub fn run_demo(&mut self) -> String {
        let mut output = String::new();
        output.push_str("VS-001 traceable asset registration demo\n");

        output.push_str("\nregister-asset outcome\n");
        let registration = self.register_asset(RegisterAssetInput::demo());
        let _ = writeln!(output, "{}", registration.to_cli_report());

        let Some(asset_id) = registration.created_object_id.clone() else {
            output.push_str("\nrecord-calibration skipped: registration did not create an asset\n");
            return output;
        };

        output.push_str("\nrecord-calibration outcome\n");
        let calibration =
            self.record_calibration(RecordCalibrationInput::accepted(asset_id.clone()));
        let _ = writeln!(output, "{}", calibration.to_cli_report());

        output.push_str("\nshow-asset\n");
        match self.show_asset(&asset_id) {
            Ok(report) => output.push_str(&report),
            Err(error) => {
                let _ = writeln!(output, "error: {error}");
            }
        }

        output.push_str("\nshow-timeline\n");
        match self.show_timeline(&asset_id) {
            Ok(report) => output.push_str(&report),
            Err(error) => {
                let _ = writeln!(output, "error: {error}");
            }
        }

        output
    }

    pub fn read_event(&self, event_id: &EventId) -> Result<EventRecord, String> {
        self.event_engine
            .read_event(event_id)
            .map_err(|error| error.to_string())
    }

    pub fn read_transaction(
        &self,
        transaction_id: &TransactionId,
    ) -> Result<TransactionRecord, String> {
        self.transaction_engine
            .read_transaction(transaction_id)
            .map_err(|error| error.to_string())
    }

    pub fn fail_next_registration_transaction_append(&self) {
        self.object_transaction_store.fail_next_transaction_append();
    }

    pub fn fail_next_calibration_transaction_stage(&self) {
        self.object_transaction_store.fail_next_transaction_stage();
    }

    pub fn fail_next_calibration_transaction_commit(&self) {
        self.object_transaction_store.fail_next_transaction_commit();
    }

    pub fn object_count(&self) -> usize {
        self.object_transaction_store.object_count()
    }

    pub fn event_count(&self) -> usize {
        self.event_store.event_count()
    }

    pub fn transaction_count(&self) -> usize {
        self.object_transaction_store.transaction_count()
    }

    fn ensure_metadata_registered(&mut self) -> Result<(), String> {
        if self.metadata_registered {
            return Ok(());
        }
        register_object_metadata(&self.object_runtime).map_err(|error| error.to_string())?;
        register_event_metadata(&self.event_engine).map_err(|error| error.to_string())?;
        self.metadata_registered = true;
        Ok(())
    }

    fn append_registered_event(
        &mut self,
        input: &RegisterAssetInput,
        asset_id: &ObjectId,
    ) -> Result<open_eqms_event_engine::AppendEventResult, String> {
        let mut object_refs = BTreeSet::new();
        object_refs.insert(asset_id.clone());

        let result = self
            .event_engine
            .append_event(AppendEventRequest {
                event_type: asset_registered_event_type_ref(),
                payload: BTreeMap::from([
                    (
                        "asset_id".to_owned(),
                        PropertyValue::Reference(asset_id.clone()),
                    ),
                    (
                        "asset_class".to_owned(),
                        PropertyValue::EnumValue(input.asset_class.clone()),
                    ),
                    (
                        "registered_at".to_owned(),
                        PropertyValue::DateTime(input.registered_at.clone()),
                    ),
                ]),
                occurred_at: EventTimestamp::new(input.registered_at.clone()),
                recorded_at: self.clock.next_event_timestamp(),
                source: EventSource::new("demo-cli"),
                object_refs,
                correlation_id: Some(CorrelationId::new(format!(
                    "registration:{}",
                    asset_id.as_str()
                ))),
                causation_id: None,
            })
            .map_err(|error| error.to_string())?;
        Ok(result)
    }

    fn append_registration_transaction(
        &mut self,
        input: &RegisterAssetInput,
        asset_id: &ObjectId,
    ) -> Result<TransactionId, String> {
        let result = self
            .transaction_engine
            .append_transaction(registration_transaction_request(input, asset_id))
            .map_err(|error| error.to_string())?;

        match result {
            AppendTransactionResult::Committed(committed) => Ok(committed.transaction_id),
            AppendTransactionResult::Staged(_) => {
                Err("registration transaction unexpectedly staged".to_owned())
            }
        }
    }

    fn append_calibration_performed_event(
        &mut self,
        input: &RecordCalibrationInput,
    ) -> Result<open_eqms_event_engine::AppendEventResult, String> {
        let mut object_refs = BTreeSet::new();
        object_refs.insert(input.asset_id.clone());

        let mut payload = BTreeMap::from([
            (
                "asset_id".to_owned(),
                PropertyValue::Reference(input.asset_id.clone()),
            ),
            (
                "performed_at".to_owned(),
                PropertyValue::DateTime(input.performed_at.clone()),
            ),
            (
                "outcome".to_owned(),
                PropertyValue::EnumValue(input.outcome.as_str().to_owned()),
            ),
        ]);
        if let Some(note) = &input.note {
            payload.insert(
                "note".to_owned(),
                PropertyValue::Text {
                    value: note.clone(),
                    language: Some("en".to_owned()),
                },
            );
        }

        self.event_engine
            .append_event(AppendEventRequest {
                event_type: asset_calibration_performed_event_type_ref(),
                payload,
                occurred_at: EventTimestamp::new(input.performed_at.clone()),
                recorded_at: self.clock.next_event_timestamp(),
                source: EventSource::new("demo-cli"),
                object_refs,
                correlation_id: Some(CorrelationId::new(format!(
                    "calibration:{}",
                    input.asset_id.as_str()
                ))),
                causation_id: None,
            })
            .map_err(|error| error.to_string())
    }

    fn append_calibration_accepted_event(
        &mut self,
        input: &RecordCalibrationInput,
    ) -> Result<open_eqms_event_engine::AppendEventResult, String> {
        let mut object_refs = BTreeSet::new();
        object_refs.insert(input.asset_id.clone());

        self.event_engine
            .append_event(AppendEventRequest {
                event_type: asset_calibration_accepted_event_type_ref(),
                payload: BTreeMap::from([
                    (
                        "asset_id".to_owned(),
                        PropertyValue::Reference(input.asset_id.clone()),
                    ),
                    (
                        "accepted_at".to_owned(),
                        PropertyValue::DateTime(input.accepted_at.clone()),
                    ),
                    (
                        "next_calibration_due".to_owned(),
                        PropertyValue::DateTime(input.next_calibration_due.clone()),
                    ),
                ]),
                occurred_at: EventTimestamp::new(input.accepted_at.clone()),
                recorded_at: self.clock.next_event_timestamp(),
                source: EventSource::new("demo-cli"),
                object_refs,
                correlation_id: Some(CorrelationId::new(format!(
                    "calibration:{}",
                    input.asset_id.as_str()
                ))),
                causation_id: Some(open_eqms_event_engine::types::CausationId::new(format!(
                    "performed:{}",
                    input.asset_id.as_str()
                ))),
            })
            .map_err(|error| error.to_string())
    }
}

impl Default for DemoApp {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegisterAssetInput {
    pub identity_label: String,
    pub asset_class: String,
    pub organization: String,
    pub location: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub responsible_reference: String,
    pub registered_at: String,
    pub calibration_required: bool,
    pub calibration_interval_days: Option<f64>,
    pub next_calibration_due: Option<String>,
}

impl RegisterAssetInput {
    pub fn demo() -> Self {
        Self {
            identity_label: "Scale A-100".to_owned(),
            asset_class: "measuring_equipment".to_owned(),
            organization: "Open-EQMS Demo".to_owned(),
            location: "Calibration Lab".to_owned(),
            manufacturer: "Demo Instruments".to_owned(),
            model: "DI-100".to_owned(),
            serial_number: "SN-100".to_owned(),
            responsible_reference: "demo-operator".to_owned(),
            registered_at: "2026-07-15T10:00:00Z".to_owned(),
            calibration_required: true,
            calibration_interval_days: Some(365.0),
            next_calibration_due: Some("2026-08-15T10:00:00Z".to_owned()),
        }
    }

    pub fn apply_overrides(&mut self, args: &[String]) -> Result<(), String> {
        for arg in args {
            let Some((name, value)) = arg.strip_prefix("--").and_then(|arg| arg.split_once('='))
            else {
                return Err(format!("invalid register-asset argument: {arg}"));
            };
            match name {
                "identity-label" => self.identity_label = value.to_owned(),
                "asset-class" => self.asset_class = value.to_owned(),
                "organization" => self.organization = value.to_owned(),
                "location" => self.location = value.to_owned(),
                "manufacturer" => self.manufacturer = value.to_owned(),
                "model" => self.model = value.to_owned(),
                "serial-number" => self.serial_number = value.to_owned(),
                "responsible-reference" => self.responsible_reference = value.to_owned(),
                "registered-at" => self.registered_at = value.to_owned(),
                "calibration-required" => {
                    self.calibration_required = match value {
                        "true" => true,
                        "false" => false,
                        _ => return Err("calibration-required must be true or false".to_owned()),
                    };
                }
                "calibration-interval-days" => {
                    self.calibration_interval_days = Some(
                        value
                            .parse::<f64>()
                            .map_err(|_| "calibration-interval-days must be numeric".to_owned())?,
                    );
                }
                "next-calibration-due" => self.next_calibration_due = Some(value.to_owned()),
                _ => return Err(format!("unknown register-asset field: {name}")),
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        for (field, value) in [
            ("identity_label", self.identity_label.as_str()),
            ("asset_class", self.asset_class.as_str()),
            ("organization", self.organization.as_str()),
            ("location", self.location.as_str()),
            ("manufacturer", self.manufacturer.as_str()),
            ("model", self.model.as_str()),
            ("serial_number", self.serial_number.as_str()),
            ("responsible_reference", self.responsible_reference.as_str()),
            ("registered_at", self.registered_at.as_str()),
        ] {
            if value.is_empty() {
                return Err(format!(
                    "{field} is required; no Runtime mutation was attempted."
                ));
            }
        }

        if !matches!(
            self.asset_class.as_str(),
            "measuring_equipment" | "machine" | "robot" | "tool" | "fixture" | "generic"
        ) {
            return Err(
                "asset_class is not allowed; no Runtime mutation was attempted.".to_owned(),
            );
        }

        if self.calibration_required
            && (self.calibration_interval_days.is_none() || self.next_calibration_due.is_none())
        {
            return Err(
                "calibration interval and next due date are required when calibration is required; no Runtime mutation was attempted."
                    .to_owned(),
            );
        }
        Ok(())
    }

    fn create_request(&self) -> CreateObjectRequest {
        let mut properties = BTreeMap::from([
            text_property("identity_label", &self.identity_label),
            enum_property("asset_class", &self.asset_class),
            text_property("organization", &self.organization),
            text_property("location", &self.location),
            text_property("manufacturer", &self.manufacturer),
            text_property("model", &self.model),
            text_property("serial_number", &self.serial_number),
            enum_property("status", "registered"),
            text_property("responsible_reference", &self.responsible_reference),
            datetime_property("registered_at", &self.registered_at),
            (
                "calibration_required".to_owned(),
                PropertyValue::Boolean(self.calibration_required),
            ),
        ]);
        if let Some(interval) = self.calibration_interval_days {
            properties.insert(
                "calibration_interval_days".to_owned(),
                PropertyValue::Number(interval),
            );
        }
        if let Some(next_due) = &self.next_calibration_due {
            properties.insert(
                "next_calibration_due".to_owned(),
                PropertyValue::DateTime(next_due.clone()),
            );
        }

        CreateObjectRequest {
            object_type: asset_object_type_ref(),
            properties,
            relations: BTreeSet::new(),
            lifecycle_state: LifecycleState::new("active"),
            ownership: OwnershipInfo::new(ObjectId::new("demo-operator"), BTreeSet::new()),
            permission_scope: PermissionScopeRef::new("demo-local-scope"),
            retention_rule: None,
            external_references: BTreeSet::new(),
            comments_ref: None,
            attachments_ref: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CalibrationOutcome {
    Accepted,
    Rejected,
}

impl CalibrationOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RecordCalibrationInput {
    pub asset_id: ObjectId,
    pub outcome: CalibrationOutcome,
    pub performed_at: String,
    pub accepted_at: String,
    pub next_calibration_due: String,
    pub note: Option<String>,
}

impl RecordCalibrationInput {
    pub fn accepted(asset_id: ObjectId) -> Self {
        Self {
            asset_id,
            outcome: CalibrationOutcome::Accepted,
            performed_at: "2026-07-15T10:00:02Z".to_owned(),
            accepted_at: "2026-07-15T10:00:03Z".to_owned(),
            next_calibration_due: "2027-07-15T10:00:00Z".to_owned(),
            note: Some("Initial calibration result recorded in merged performed event.".to_owned()),
        }
    }

    pub fn rejected(asset_id: ObjectId) -> Self {
        Self {
            outcome: CalibrationOutcome::Rejected,
            note: Some(
                "Calibration rejected at application boundary after performed event.".to_owned(),
            ),
            ..Self::accepted(asset_id)
        }
    }

    pub fn apply_overrides(&mut self, args: &[String]) -> Result<(), String> {
        for arg in args {
            let Some((name, value)) = arg.strip_prefix("--").and_then(|arg| arg.split_once('='))
            else {
                return Err(format!("invalid record-calibration argument: {arg}"));
            };
            match name {
                "asset-id" => self.asset_id = ObjectId::new(value),
                "outcome" => {
                    self.outcome = match value {
                        "accepted" => CalibrationOutcome::Accepted,
                        "rejected" => CalibrationOutcome::Rejected,
                        _ => return Err("outcome must be accepted or rejected".to_owned()),
                    };
                }
                "performed-at" => self.performed_at = value.to_owned(),
                "accepted-at" => self.accepted_at = value.to_owned(),
                "next-calibration-due" => self.next_calibration_due = value.to_owned(),
                "note" => self.note = Some(value.to_owned()),
                _ => return Err(format!("unknown record-calibration field: {name}")),
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<(), String> {
        if self.asset_id.is_empty() {
            return Err(
                "asset_id is required; no Event or Transaction append was attempted.".to_owned(),
            );
        }
        if self.performed_at.is_empty() {
            return Err(
                "performed_at is required; no Event or Transaction append was attempted."
                    .to_owned(),
            );
        }
        if self.outcome == CalibrationOutcome::Accepted {
            if self.accepted_at.is_empty() {
                return Err(
                    "accepted_at is required for an accepted calibration; no Event or Transaction append was attempted."
                        .to_owned(),
                );
            }
            if self.next_calibration_due.is_empty() {
                return Err(
                    "next_calibration_due is required for an accepted calibration; no Event or Transaction append was attempted."
                        .to_owned(),
                );
            }
        }
        Ok(())
    }

    fn acceptance_changes(&self) -> ObjectChanges {
        let mut changes = ObjectChanges::default();
        changes.property_changes.insert(
            "status".to_owned(),
            PropertyChange::Set(PropertyValue::EnumValue("in_service".to_owned())),
        );
        changes.property_changes.insert(
            "next_calibration_due".to_owned(),
            PropertyChange::Set(PropertyValue::DateTime(self.next_calibration_due.clone())),
        );
        changes
    }
}

fn calibration_transaction_request(
    input: &RecordCalibrationInput,
    current_asset: &ObjectRecord,
    updated_asset: &ObjectRecord,
    registration_transaction_id: TransactionId,
) -> AppendTransactionRequest {
    AppendTransactionRequest {
        schema_version: TransactionSchemaVersion::new(1),
        level: TransactionLevel::Level2,
        object_id: input.asset_id.clone(),
        operation: OperationDescriptor::new("asset.calibration.accepted"),
        old_value: asset_calibration_state_value(current_asset),
        new_value: asset_calibration_state_value(updated_asset),
        actor: ActorRef::new("demo-operator"),
        device: DeviceRef::new("demo-cli"),
        site: Some(SiteRef::new("demo-site")),
        edit_timestamp: TransactionTimestamp::new(input.accepted_at.clone()),
        base_version: current_asset.version,
        resulting_version: updated_asset.version,
        server_receipt_time: Some(TransactionTimestamp::new("2026-07-15T10:00:05Z")),
        prior_reference: Some(PriorReference::Transaction(registration_transaction_id)),
        prior_transaction_hash: None,
        rule_evaluation: None,
        signer: None,
        signer_role: None,
        signature_meaning: None,
        authentication_evidence: None,
        signed_revision: None,
        reason: None,
        signing_timestamp: None,
        signature: None,
    }
}

fn asset_calibration_state_value(record: &ObjectRecord) -> PropertyValue {
    let status = property_text(record, "status");
    let next_due = property_text(record, "next_calibration_due");
    PropertyValue::Text {
        value: format!(
            "status={},next_calibration_due={}",
            status.unwrap_or("missing"),
            next_due.unwrap_or("missing")
        ),
        language: Some("en".to_owned()),
    }
}

fn property_text<'a>(record: &'a ObjectRecord, name: &str) -> Option<&'a str> {
    match record.properties.get(name) {
        Some(PropertyValue::Text { value, .. })
        | Some(PropertyValue::DateTime(value))
        | Some(PropertyValue::EnumValue(value)) => Some(value.as_str()),
        _ => None,
    }
}

fn registration_transaction_request(
    input: &RegisterAssetInput,
    asset_id: &ObjectId,
) -> AppendTransactionRequest {
    AppendTransactionRequest {
        schema_version: TransactionSchemaVersion::new(1),
        level: TransactionLevel::Level1,
        object_id: asset_id.clone(),
        operation: OperationDescriptor::new("asset.register"),
        old_value: PropertyValue::Text {
            value: "no_prior_asset".to_owned(),
            language: Some("en".to_owned()),
        },
        new_value: PropertyValue::Text {
            value: format!(
                "asset_id={},identity_label={},serial_number={}",
                asset_id.as_str(),
                input.identity_label,
                input.serial_number
            ),
            language: Some("en".to_owned()),
        },
        actor: ActorRef::new("demo-operator"),
        device: DeviceRef::new("demo-cli"),
        site: Some(SiteRef::new("demo-site")),
        edit_timestamp: TransactionTimestamp::new(input.registered_at.clone()),
        base_version: Version::new(0),
        resulting_version: Version::initial(),
        server_receipt_time: None,
        prior_reference: None,
        prior_transaction_hash: None,
        rule_evaluation: None,
        signer: None,
        signer_role: None,
        signature_meaning: None,
        authentication_evidence: None,
        signed_revision: None,
        reason: None,
        signing_timestamp: None,
        signature: None,
    }
}

fn text_property(name: &str, value: &str) -> (String, PropertyValue) {
    (
        name.to_owned(),
        PropertyValue::Text {
            value: value.to_owned(),
            language: Some("en".to_owned()),
        },
    )
}

fn enum_property(name: &str, value: &str) -> (String, PropertyValue) {
    (name.to_owned(), PropertyValue::EnumValue(value.to_owned()))
}

fn datetime_property(name: &str, value: &str) -> (String, PropertyValue) {
    (name.to_owned(), PropertyValue::DateTime(value.to_owned()))
}
