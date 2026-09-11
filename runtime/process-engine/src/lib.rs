//! Open-EQMS Process Engine implementation for SPEC-006 V1.
//!
//! This crate owns bounded process graph registration and caller-initiated
//! transition orchestration. It validates transitions against immutable process
//! definitions, stages Object Runtime updates with Level 2 Transaction records
//! in one shared Unit of Work, then appends an independent Event fact.
//!
//! It intentionally does not implement GUI forms, role enforcement, Rule Engine
//! condition evaluation, automatic commands, KPI effects, content package
//! loading, process-definition persistence, authentication, AI, networking, or
//! concrete database providers.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod errors;
pub mod storage;
pub mod types;

mod validation;

pub use crate::errors::{
    ProcessDefinitionValidationError, ProcessEngineError, ProcessEngineResult,
};
pub use crate::storage::SharedTransitionStore;
pub use crate::types::{
    current_node_property_key, last_transaction_property_key,
    process_transition_event_type_definition, process_transition_event_type_ref, ProcessDefinition,
    ProcessDefinitionId, ProcessNodeId, ProcessTransition, TransitionConsistencyStatus,
    TransitionOutcome, TransitionRequest,
};

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use open_eqms_event_engine::storage::{
    AppendSequenceGenerator, EventIdGenerator, StorageProvider as EventStorageProvider,
};
use open_eqms_event_engine::types::{CausationId, EventSource, EventTimestamp, PropertyValue};
use open_eqms_event_engine::{AppendEventRequest, EventEngine};
use open_eqms_object_runtime::storage::{
    ObjectIdGenerator, StorageProvider as ObjectStorageProvider,
};
use open_eqms_object_runtime::types::PropertyChange;
use open_eqms_object_runtime::{ObjectChanges, ObjectRuntime, UpdateObjectRequest};
use open_eqms_transaction_engine::crypto::CryptographicProvider;
use open_eqms_transaction_engine::storage::{TransactionIdGenerator, TransactionStore};
use open_eqms_transaction_engine::types::{
    OperationDescriptor, PriorReference, TransactionLevel, TransactionSchemaVersion,
};
use open_eqms_transaction_engine::{AppendTransactionRequest, AppendTransactionResult};
use open_eqms_transaction_engine::{TransactionEngine, TransactionRange};

use crate::types::{absent_node_value, process_node_value, transaction_id_value};
use crate::validation::{
    current_node_from_record, ensure_no_other_active_process, map_object_read_error,
    map_object_update_error, prior_transaction_from_record, transition_name_for_request,
    validate_process_definition, verify_process_properties_declared,
};

/// Process Engine facade implementing the bounded SPEC-006 V1 public API.
pub struct ProcessEngine<S, OI, ES, EI, EQ, TI, C>
where
    S: ObjectStorageProvider + TransactionStore + SharedTransitionStore + Clone,
    OI: ObjectIdGenerator,
    ES: EventStorageProvider,
    EI: EventIdGenerator,
    EQ: AppendSequenceGenerator,
    TI: TransactionIdGenerator,
    C: CryptographicProvider + Clone,
{
    object_runtime: ObjectRuntime<S, OI>,
    transaction_engine: TransactionEngine<S, TI, C>,
    event_engine: EventEngine<ES, EI, EQ>,
    shared_store: S,
    commit_cryptographic_provider: Arc<Mutex<C>>,
    definitions: Arc<
        Mutex<
            BTreeMap<
                (ProcessDefinitionId, open_eqms_runtime_contracts::Version),
                ProcessDefinition,
            >,
        >,
    >,
}

impl<S, OI, ES, EI, EQ, TI, C> ProcessEngine<S, OI, ES, EI, EQ, TI, C>
where
    S: ObjectStorageProvider + TransactionStore + SharedTransitionStore + Clone,
    OI: ObjectIdGenerator,
    ES: EventStorageProvider,
    EI: EventIdGenerator,
    EQ: AppendSequenceGenerator,
    TI: TransactionIdGenerator,
    C: CryptographicProvider + Clone,
{
    /// Creates a Process Engine over existing Runtime engine provider boundaries.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        object_transaction_storage: S,
        object_id_generator: OI,
        event_storage: ES,
        event_id_generator: EI,
        event_sequence_generator: EQ,
        transaction_id_generator: TI,
        cryptographic_provider: C,
    ) -> Self {
        Self {
            object_runtime: ObjectRuntime::new(
                object_transaction_storage.clone(),
                object_id_generator,
            ),
            transaction_engine: TransactionEngine::new(
                object_transaction_storage.clone(),
                transaction_id_generator,
                cryptographic_provider.clone(),
            ),
            event_engine: EventEngine::new(
                event_storage,
                event_id_generator,
                event_sequence_generator,
            ),
            shared_store: object_transaction_storage,
            commit_cryptographic_provider: Arc::new(Mutex::new(cryptographic_provider)),
            definitions: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Registers one immutable Process Definition schema version.
    pub fn register_process_definition(
        &self,
        definition: ProcessDefinition,
    ) -> ProcessEngineResult<()> {
        validate_process_definition(&definition)?;
        let key = (definition.id.clone(), definition.schema_version);
        let mut definitions = self.lock_definitions()?;
        if definitions.contains_key(&key) {
            return Err(ProcessEngineError::DuplicateDefinition {
                definition_id: definition.id,
                schema_version: definition.schema_version,
            });
        }
        definitions.insert(key, definition);
        Ok(())
    }

    /// Retrieves one registered Process Definition schema version.
    pub fn get_process_definition(
        &self,
        definition_id: &ProcessDefinitionId,
        schema_version: open_eqms_runtime_contracts::Version,
    ) -> ProcessEngineResult<ProcessDefinition> {
        self.definition(definition_id, schema_version)
    }

    /// Records one legal Process transition.
    pub fn record_transition(
        &self,
        request: TransitionRequest,
    ) -> ProcessEngineResult<TransitionOutcome> {
        if request.unit_of_work.is_empty() {
            return Err(ProcessEngineError::UnitOfWorkFailure {
                message: "unit of work must not be empty".to_owned(),
            });
        }

        let definition = self.definition(&request.definition_id, request.schema_version)?;
        let current = self
            .object_runtime
            .read_object(&request.object_id)
            .map_err(map_object_read_error)?;
        let object_type = self
            .object_runtime
            .get_object_type(&current.object_type)
            .map_err(map_object_read_error)?;

        verify_process_properties_declared(&object_type, &definition.id)?;
        ensure_no_other_active_process(&current, &definition.id)?;

        let current_node = current_node_from_record(&current, &definition.id)?;
        let prior_transaction = prior_transaction_from_record(&current, &definition.id)?;
        let transition_name =
            transition_name_for_request(&definition, current_node.as_ref(), &request.to)?;
        let prior_value = current_node
            .as_ref()
            .map(process_node_value)
            .unwrap_or_else(absent_node_value);
        let new_value = process_node_value(&request.to);
        let resulting_version = current.version.next();
        let unit_of_work = request.unit_of_work.clone();

        self.shared_store
            .begin_shared_transition(&unit_of_work)
            .map_err(|error| ProcessEngineError::UnitOfWorkBeginFailure {
                message: error.to_string(),
            })?;

        let transaction_result = self
            .transaction_engine
            .append_transaction_in_unit_of_work(
                AppendTransactionRequest {
                    schema_version: TransactionSchemaVersion::new(1),
                    level: TransactionLevel::Level2,
                    object_id: request.object_id.clone(),
                    operation: OperationDescriptor::new(format!(
                        "process.{}.transition.{transition_name}",
                        request.definition_id.as_str()
                    )),
                    old_value: prior_value.clone(),
                    new_value: new_value.clone(),
                    actor: request.actor.clone(),
                    device: request.device.clone(),
                    site: request.site.clone(),
                    edit_timestamp: request.edit_timestamp.clone(),
                    base_version: current.version,
                    resulting_version,
                    server_receipt_time: Some(request.server_receipt_time.clone()),
                    prior_reference: prior_transaction.map(PriorReference::Transaction),
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
                },
                &unit_of_work,
            )
            .map_err(|error| {
                self.rollback_or_original(
                    &unit_of_work,
                    ProcessEngineError::TransactionEngineFailure { source: error },
                )
            })?;

        let transaction_id = match transaction_result {
            AppendTransactionResult::Staged(staged) => staged.transaction_id,
            AppendTransactionResult::Committed(_) => {
                return Err(self.rollback_or_original(
                    &unit_of_work,
                    ProcessEngineError::TransactionEngineFailure {
                        source: open_eqms_transaction_engine::errors::TransactionEngineError::StorageProviderFailure {
                            message: "unit-of-work append unexpectedly committed immediately".to_owned(),
                        },
                    },
                ));
            }
        };

        let mut changes = ObjectChanges::default();
        changes.property_changes.insert(
            current_node_property_key(&definition.id),
            PropertyChange::Set(new_value),
        );
        changes.property_changes.insert(
            last_transaction_property_key(&definition.id),
            PropertyChange::Set(transaction_id_value(&transaction_id)),
        );

        self.object_runtime
            .update_object_in_unit_of_work(
                UpdateObjectRequest {
                    object_id: request.object_id.clone(),
                    base_version: current.version,
                    changes,
                },
                &unit_of_work,
            )
            .map_err(|error| {
                self.rollback_or_original(
                    &unit_of_work,
                    map_object_update_error(error, &definition.id),
                )
            })?;

        {
            let mut cryptographic_provider = self.lock_commit_cryptographic_provider()?;
            self.shared_store
                .commit_shared_transition(&unit_of_work, &mut *cryptographic_provider)
                .map_err(|error| ProcessEngineError::UnitOfWorkCommitFailure {
                    message: error.to_string(),
                })?;
        }

        let event_result = self.event_engine.append_event(AppendEventRequest {
            event_type: process_transition_event_type_ref(),
            payload: transition_event_payload(
                &definition.id,
                current_node.as_ref(),
                &request.to,
                &transition_name,
                &request.actor,
                &request.edit_timestamp,
            ),
            occurred_at: EventTimestamp::new(request.edit_timestamp.as_str()),
            recorded_at: EventTimestamp::new(request.server_receipt_time.as_str()),
            source: EventSource::new("process-engine"),
            object_refs: BTreeSet::from([request.object_id.clone()]),
            correlation_id: None,
            causation_id: Some(CausationId::new(transaction_id.as_str())),
        });

        let (event_id, consistency_status, recovery_guidance) = match event_result {
            Ok(result) => (
                Some(result.event_id),
                TransitionConsistencyStatus::Complete,
                None,
            ),
            Err(error) => (
                None,
                TransitionConsistencyStatus::PartiallyCompleted,
                Some(format!(
                    "state and transaction committed; append process.transitioned event manually after resolving: {error}"
                )),
            ),
        };

        Ok(TransitionOutcome {
            object_id: request.object_id,
            definition_id: definition.id,
            schema_version: definition.schema_version,
            prior_node: current_node,
            new_node: request.to,
            transaction_id,
            event_id,
            consistency_status,
            recovery_guidance,
        })
    }

    /// Reads finalized Transactions through the owned Transaction Engine.
    pub fn read_transaction_range(
        &self,
        start: open_eqms_transaction_engine::types::AppendIndex,
        max: usize,
    ) -> Result<TransactionRange, open_eqms_transaction_engine::errors::TransactionEngineError>
    {
        self.transaction_engine.read_append_index_range(start, max)
    }

    /// Reads one Object through the owned Object Runtime.
    pub fn read_object(
        &self,
        object_id: &open_eqms_runtime_contracts::ObjectId,
    ) -> Result<
        open_eqms_object_runtime::types::ObjectRecord,
        open_eqms_object_runtime::errors::ObjectRuntimeError,
    > {
        self.object_runtime.read_object(object_id)
    }

    /// Registers an Object Type through the owned Object Runtime.
    pub fn register_object_type(
        &self,
        definition: open_eqms_object_runtime::types::ObjectTypeDefinition,
    ) -> Result<(), open_eqms_object_runtime::errors::ObjectRuntimeError> {
        self.object_runtime.register_object_type(definition)
    }

    /// Creates an Object through the owned Object Runtime.
    pub fn create_object(
        &self,
        request: open_eqms_object_runtime::CreateObjectRequest,
    ) -> Result<
        open_eqms_object_runtime::CreateObjectResult,
        open_eqms_object_runtime::errors::ObjectRuntimeError,
    > {
        self.object_runtime.create_object(request)
    }

    /// Registers an Event Type through the owned Event Engine.
    pub fn register_event_type(
        &self,
        definition: open_eqms_event_engine::types::EventTypeDefinition,
    ) -> Result<(), open_eqms_event_engine::errors::EventEngineError> {
        self.event_engine.register_event_type(definition)
    }

    /// Reads one Event through the owned Event Engine.
    pub fn read_event(
        &self,
        event_id: &open_eqms_event_engine::types::EventId,
    ) -> Result<
        open_eqms_event_engine::types::EventRecord,
        open_eqms_event_engine::errors::EventEngineError,
    > {
        self.event_engine.read_event(event_id)
    }

    fn definition(
        &self,
        definition_id: &ProcessDefinitionId,
        schema_version: open_eqms_runtime_contracts::Version,
    ) -> ProcessEngineResult<ProcessDefinition> {
        self.lock_definitions()?
            .get(&(definition_id.clone(), schema_version))
            .cloned()
            .ok_or_else(|| ProcessEngineError::DefinitionNotFound {
                definition_id: definition_id.clone(),
                schema_version,
            })
    }

    fn lock_definitions(
        &self,
    ) -> ProcessEngineResult<
        std::sync::MutexGuard<
            '_,
            BTreeMap<
                (ProcessDefinitionId, open_eqms_runtime_contracts::Version),
                ProcessDefinition,
            >,
        >,
    > {
        self.definitions
            .lock()
            .map_err(|_| ProcessEngineError::DefinitionRegistryFailure {
                message: "process definition registry lock poisoned".to_owned(),
            })
    }

    fn lock_commit_cryptographic_provider(
        &self,
    ) -> ProcessEngineResult<std::sync::MutexGuard<'_, C>> {
        self.commit_cryptographic_provider.lock().map_err(|_| {
            ProcessEngineError::UnitOfWorkCommitFailure {
                message: "cryptographic provider lock poisoned".to_owned(),
            }
        })
    }

    fn rollback_or_original(
        &self,
        unit_of_work: &open_eqms_runtime_contracts::UnitOfWork,
        original: ProcessEngineError,
    ) -> ProcessEngineError {
        match self.shared_store.rollback_shared_transition(unit_of_work) {
            Ok(()) => original,
            Err(rollback) => ProcessEngineError::UnitOfWorkRollbackFailure {
                original: original.to_string(),
                rollback: rollback.to_string(),
            },
        }
    }
}

fn transition_event_payload(
    definition_id: &ProcessDefinitionId,
    from: Option<&ProcessNodeId>,
    to: &ProcessNodeId,
    transition_name: &str,
    actor: &open_eqms_transaction_engine::types::ActorRef,
    edit_timestamp: &open_eqms_transaction_engine::types::TransactionTimestamp,
) -> BTreeMap<String, PropertyValue> {
    BTreeMap::from([
        (
            "definition_id".to_owned(),
            PropertyValue::Text {
                value: definition_id.as_str().to_owned(),
                language: None,
            },
        ),
        (
            "from_node".to_owned(),
            PropertyValue::Text {
                value: from
                    .map(ProcessNodeId::as_str)
                    .unwrap_or("not-started")
                    .to_owned(),
                language: None,
            },
        ),
        (
            "to_node".to_owned(),
            PropertyValue::Text {
                value: to.as_str().to_owned(),
                language: None,
            },
        ),
        (
            "transition_name".to_owned(),
            PropertyValue::Text {
                value: transition_name.to_owned(),
                language: None,
            },
        ),
        (
            "actor".to_owned(),
            PropertyValue::Text {
                value: actor.as_str().to_owned(),
                language: None,
            },
        ),
        (
            "edit_timestamp".to_owned(),
            PropertyValue::DateTime(edit_timestamp.as_str().to_owned()),
        ),
    ])
}

#[cfg(test)]
mod tests;
