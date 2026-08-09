use open_eqms_event_engine::types::EventId;
use open_eqms_runtime_contracts::ObjectId;
use open_eqms_transaction_engine::types::TransactionId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepId {
    ValidateAssetInput,
    RegisterMetadata,
    CreateAssetObject,
    AppendAssetRegisteredEvent,
    AppendRegistrationTransaction,
    ValidateCalibrationInput,
    AppendCalibrationPerformedEvent,
    AppendCalibrationAcceptedEvent,
    StageCalibrationObjectUpdate,
    StageCalibrationTransaction,
    CommitCalibrationUnitOfWork,
}

impl StepId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ValidateAssetInput => "validate_asset_input",
            Self::RegisterMetadata => "register_metadata",
            Self::CreateAssetObject => "create_asset_object",
            Self::AppendAssetRegisteredEvent => "append_asset_registered_event",
            Self::AppendRegistrationTransaction => "append_registration_transaction",
            Self::ValidateCalibrationInput => "validate_calibration_input",
            Self::AppendCalibrationPerformedEvent => "append_calibration_performed_event",
            Self::AppendCalibrationAcceptedEvent => "append_calibration_accepted_event",
            Self::StageCalibrationObjectUpdate => "stage_calibration_object_update",
            Self::StageCalibrationTransaction => "stage_calibration_transaction",
            Self::CommitCalibrationUnitOfWork => "commit_calibration_unit_of_work",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConsistencyStatus {
    Complete,
    PartiallyCompleted,
    FailedBeforeMutation,
}

impl ConsistencyStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Complete => "Complete",
            Self::PartiallyCompleted => "PartiallyCompleted",
            Self::FailedBeforeMutation => "FailedBeforeMutation",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationOutcome {
    pub completed_steps: Vec<StepId>,
    pub failed_step: Option<StepId>,
    pub created_object_id: Option<ObjectId>,
    pub created_event_ids: Vec<EventId>,
    pub created_transaction_ids: Vec<TransactionId>,
    pub consistency_status: ConsistencyStatus,
    pub recovery_guidance: String,
}

impl OperationOutcome {
    pub fn new() -> Self {
        Self {
            completed_steps: Vec::new(),
            failed_step: None,
            created_object_id: None,
            created_event_ids: Vec::new(),
            created_transaction_ids: Vec::new(),
            consistency_status: ConsistencyStatus::FailedBeforeMutation,
            recovery_guidance: String::new(),
        }
    }

    pub fn is_complete(&self) -> bool {
        self.consistency_status == ConsistencyStatus::Complete
    }

    pub fn fail_before_mutation(failed_step: StepId, recovery_guidance: impl Into<String>) -> Self {
        Self {
            failed_step: Some(failed_step),
            recovery_guidance: recovery_guidance.into(),
            ..Self::new()
        }
    }

    pub fn mark_complete(&mut self, recovery_guidance: impl Into<String>) {
        self.failed_step = None;
        self.consistency_status = ConsistencyStatus::Complete;
        self.recovery_guidance = recovery_guidance.into();
    }

    pub fn mark_partial(&mut self, failed_step: StepId, recovery_guidance: impl Into<String>) {
        self.failed_step = Some(failed_step);
        self.consistency_status = ConsistencyStatus::PartiallyCompleted;
        self.recovery_guidance = recovery_guidance.into();
    }

    pub fn to_cli_report(&self) -> String {
        let object_id = self
            .created_object_id
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "none".to_owned());
        let event_ids = join_ids(self.created_event_ids.iter().map(ToString::to_string));
        let transaction_ids =
            join_ids(self.created_transaction_ids.iter().map(ToString::to_string));
        let completed_steps = join_ids(
            self.completed_steps
                .iter()
                .map(|step| step.as_str().to_owned()),
        );
        let failed_step = self
            .failed_step
            .as_ref()
            .map(StepId::as_str)
            .unwrap_or("none");

        format!(
            "consistency_status: {}\ncompleted_steps: {}\nfailed_step: {}\ncreated_object_id: {}\ncreated_event_ids: {}\ncreated_transaction_ids: {}\nrecovery_guidance: {}",
            self.consistency_status.as_str(),
            completed_steps,
            failed_step,
            object_id,
            event_ids,
            transaction_ids,
            self.recovery_guidance
        )
    }
}

impl Default for OperationOutcome {
    fn default() -> Self {
        Self::new()
    }
}

fn join_ids(values: impl Iterator<Item = String>) -> String {
    let collected = values.collect::<Vec<_>>();
    if collected.is_empty() {
        "none".to_owned()
    } else {
        collected.join(",")
    }
}
