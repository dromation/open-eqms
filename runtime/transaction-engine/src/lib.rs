//! Open-EQMS Transaction Engine implementation for SPEC-003.
//!
//! This crate owns the Transaction/Audit Log only. It records immutable
//! validated state-change facts, validates their structure, assigns append
//! ordering through an abstract Transaction Store, computes Level 2+ hashes
//! through an abstract cryptographic provider, and reads finalized Transactions
//! by identity or Append Index range.
//!
//! It intentionally does not implement Object Runtime behavior, Event Engine
//! behavior, rules, processes, queries, GUI, AI, synchronization, concrete
//! databases, content packages, security authorization, statistics, KPIs,
//! package loading, signing, or cryptographic algorithms.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod crypto;
pub mod errors;
pub mod storage;
pub mod types;

mod canonical;
mod validation;

use crate::crypto::CryptographicProvider;
use crate::errors::{TransactionEngineError, TransactionEngineResult};
use crate::storage::{PreparedTransaction, TransactionIdGenerator, TransactionStore};
use crate::types::{
    ActorRef, AppendIndex, AuthenticationEvidenceRef, CryptographicSignature, DeviceRef, ObjectId,
    OperationDescriptor, PriorReference, PriorTransactionHash, PropertyValue, Reason,
    RuleEvaluationMetadata, SignatureMeaning, SignedRevision, SignerRef, SignerRole, SiteRef,
    TransactionHash, TransactionId, TransactionLevel, TransactionRecord, TransactionSchemaVersion,
    TransactionSequence, TransactionTimestamp, UnitOfWork, Version,
};
use crate::validation::{validate_append_request, validate_level3_signature_inputs};
use std::sync::{Arc, Mutex};

/// Transaction Engine facade implementing the SPEC-003 public API.
pub struct TransactionEngine<S, I, C>
where
    S: TransactionStore,
    I: TransactionIdGenerator,
    C: CryptographicProvider,
{
    storage: Arc<Mutex<S>>,
    id_generator: Arc<Mutex<I>>,
    cryptographic_provider: Arc<Mutex<C>>,
}

impl<S, I, C> Clone for TransactionEngine<S, I, C>
where
    S: TransactionStore,
    I: TransactionIdGenerator,
    C: CryptographicProvider,
{
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
            id_generator: Arc::clone(&self.id_generator),
            cryptographic_provider: Arc::clone(&self.cryptographic_provider),
        }
    }
}

impl<S, I, C> TransactionEngine<S, I, C>
where
    S: TransactionStore,
    I: TransactionIdGenerator,
    C: CryptographicProvider,
{
    /// Creates a Transaction Engine over abstract storage, identity, and crypto providers.
    pub fn new(storage: S, id_generator: I, cryptographic_provider: C) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            id_generator: Arc::new(Mutex::new(id_generator)),
            cryptographic_provider: Arc::new(Mutex::new(cryptographic_provider)),
        }
    }

    /// Appends a Transaction independently, making this call the durable commit point.
    pub fn append_transaction(
        &self,
        request: AppendTransactionRequest,
    ) -> TransactionEngineResult<AppendTransactionResult> {
        self.append(request, None)
    }

    /// Stages a Transaction append inside a caller-supplied Unit of Work.
    pub fn append_transaction_in_unit_of_work(
        &self,
        request: AppendTransactionRequest,
        unit_of_work: &UnitOfWork,
    ) -> TransactionEngineResult<AppendTransactionResult> {
        self.append(request, Some(unit_of_work))
    }

    /// Reads one finalized immutable Transaction by identity.
    pub fn read_transaction(
        &self,
        transaction_id: &TransactionId,
    ) -> TransactionEngineResult<TransactionRecord> {
        let storage = self.lock_storage()?;
        storage.get_transaction(transaction_id)?.ok_or_else(|| {
            TransactionEngineError::TransactionNotFound {
                transaction_id: transaction_id.clone(),
            }
        })
    }

    /// Reads finalized Transactions in strictly ascending Append Index order.
    pub fn read_append_index_range(
        &self,
        start: AppendIndex,
        max: usize,
    ) -> TransactionEngineResult<TransactionRange> {
        if start.is_zero() || max == 0 {
            return Err(TransactionEngineError::InvalidAppendIndexRange {
                start,
                max,
                reason: "start must be non-zero and max must be greater than zero".to_owned(),
            });
        }

        let storage = self.lock_storage()?;
        let transactions = storage.read_append_index_range(start, max)?;
        let next_start = transactions.last().map(|record| record.append_index.next());
        Ok(TransactionRange {
            transactions,
            next_start,
        })
    }

    fn append(
        &self,
        request: AppendTransactionRequest,
        unit_of_work: Option<&UnitOfWork>,
    ) -> TransactionEngineResult<AppendTransactionResult> {
        validate_append_request(&request)?;
        self.verify_level3_signature(&request)?;

        let transaction_id = self.next_id()?;
        let prepared = PreparedTransaction::from_request(transaction_id.clone(), request);
        let mut storage = self.lock_storage()?;

        if let Some(unit_of_work) = unit_of_work {
            storage.stage_transaction(unit_of_work, prepared)?;
            return Ok(AppendTransactionResult::Staged(StagedTransactionAppend {
                transaction_id,
            }));
        }

        let mut cryptographic_provider = self.lock_cryptographic_provider()?;
        let record = storage.append_transaction(prepared, &mut *cryptographic_provider)?;
        Ok(AppendTransactionResult::Committed(Box::new(
            CommittedTransactionAppend {
                transaction_id,
                append_index: record.append_index,
                transaction_sequence: record.transaction_sequence,
                transaction_hash: record.transaction_hash.clone(),
                record,
            },
        )))
    }

    fn verify_level3_signature(
        &self,
        request: &AppendTransactionRequest,
    ) -> TransactionEngineResult<()> {
        let Some((signed_revision, signer, signature)) = validate_level3_signature_inputs(
            request.level,
            request.signed_revision.as_ref(),
            request.signer.as_ref(),
            request.signature.as_ref(),
        )?
        else {
            return Ok(());
        };

        let mut cryptographic_provider = self.lock_cryptographic_provider()?;
        let verified = cryptographic_provider
            .verify(signed_revision, signer, signature)
            .map_err(|message| TransactionEngineError::CryptographicProviderFailure { message })?;
        if !verified {
            return Err(TransactionEngineError::SignatureVerificationFailure {
                signer: signer.clone(),
                reason: "signature did not verify".to_owned(),
            });
        }
        Ok(())
    }

    fn lock_storage(&self) -> TransactionEngineResult<std::sync::MutexGuard<'_, S>> {
        self.storage
            .lock()
            .map_err(|_| TransactionEngineError::StorageProviderFailure {
                message: "storage lock poisoned".to_owned(),
            })
    }

    fn lock_cryptographic_provider(&self) -> TransactionEngineResult<std::sync::MutexGuard<'_, C>> {
        self.cryptographic_provider.lock().map_err(|_| {
            TransactionEngineError::CryptographicProviderFailure {
                message: "cryptographic provider lock poisoned".to_owned(),
            }
        })
    }

    fn next_id(&self) -> TransactionEngineResult<TransactionId> {
        let mut generator = self.id_generator.lock().map_err(|_| {
            TransactionEngineError::StorageProviderFailure {
                message: "transaction id generator lock poisoned".to_owned(),
            }
        })?;
        Ok(generator.next_id())
    }
}

/// Request to append one immutable Transaction.
#[derive(Clone, Debug, PartialEq)]
pub struct AppendTransactionRequest {
    /// Record schema version.
    pub schema_version: TransactionSchemaVersion,
    /// Declared regulatory level.
    pub level: TransactionLevel,
    /// Object this state-change fact concerns.
    pub object_id: ObjectId,
    /// Opaque operation descriptor.
    pub operation: OperationDescriptor,
    /// Old value recorded by the caller.
    pub old_value: PropertyValue,
    /// New value recorded by the caller.
    pub new_value: PropertyValue,
    /// Caller-supplied actor reference.
    pub actor: ActorRef,
    /// Caller-supplied device reference.
    pub device: DeviceRef,
    /// Optional caller-supplied site reference.
    pub site: Option<SiteRef>,
    /// Level 1 edit timestamp.
    pub edit_timestamp: TransactionTimestamp,
    /// Base version before the state change.
    pub base_version: Version,
    /// Resulting version after the state change.
    pub resulting_version: Version,
    /// Level 2+ server receipt time.
    pub server_receipt_time: Option<TransactionTimestamp>,
    /// Optional link to a prior Transaction or triggering Event.
    pub prior_reference: Option<PriorReference>,
    /// Optional caller-supplied prior Transaction hash input.
    pub prior_transaction_hash: Option<PriorTransactionHash>,
    /// Optional rule-evaluation metadata carried on this record.
    pub rule_evaluation: Option<RuleEvaluationMetadata>,
    /// Level 3 signer reference.
    pub signer: Option<SignerRef>,
    /// Level 3 signer role.
    pub signer_role: Option<SignerRole>,
    /// Level 3 signature meaning.
    pub signature_meaning: Option<SignatureMeaning>,
    /// Level 3 authentication-evidence reference.
    pub authentication_evidence: Option<AuthenticationEvidenceRef>,
    /// Level 3 signed revision marker.
    pub signed_revision: Option<SignedRevision>,
    /// Level 3 reason.
    pub reason: Option<Reason>,
    /// Level 3 signing timestamp.
    pub signing_timestamp: Option<TransactionTimestamp>,
    /// Level 3 caller-supplied signature bytes.
    pub signature: Option<CryptographicSignature>,
}

impl PreparedTransaction {
    fn from_request(transaction_id: TransactionId, request: AppendTransactionRequest) -> Self {
        Self {
            id: transaction_id,
            schema_version: request.schema_version,
            level: request.level,
            object_id: request.object_id,
            operation: request.operation,
            old_value: request.old_value,
            new_value: request.new_value,
            actor: request.actor,
            device: request.device,
            site: request.site,
            edit_timestamp: request.edit_timestamp,
            base_version: request.base_version,
            resulting_version: request.resulting_version,
            server_receipt_time: request.server_receipt_time,
            prior_reference: request.prior_reference,
            prior_transaction_hash: request.prior_transaction_hash,
            rule_evaluation: request.rule_evaluation,
            signer: request.signer,
            signer_role: request.signer_role,
            signature_meaning: request.signature_meaning,
            authentication_evidence: request.authentication_evidence,
            signed_revision: request.signed_revision,
            reason: request.reason,
            signing_timestamp: request.signing_timestamp,
            signature: request.signature,
        }
    }
}

/// Result returned by a successful append operation.
#[derive(Clone, Debug, PartialEq)]
pub enum AppendTransactionResult {
    /// Independent append committed durably before returning.
    Committed(Box<CommittedTransactionAppend>),
    /// Unit-of-Work append staged, not yet durable.
    Staged(StagedTransactionAppend),
}

/// Result data returned for an independently committed append.
#[derive(Clone, Debug, PartialEq)]
pub struct CommittedTransactionAppend {
    /// Newly assigned Transaction identifier.
    pub transaction_id: TransactionId,
    /// Newly assigned Append Index.
    pub append_index: AppendIndex,
    /// Newly assigned Level 2+ Transaction Sequence.
    pub transaction_sequence: Option<TransactionSequence>,
    /// Newly computed Level 2+ Transaction hash.
    pub transaction_hash: Option<TransactionHash>,
    /// Full finalized Transaction record.
    pub record: TransactionRecord,
}

/// Result data returned for a Unit-of-Work staged append.
#[derive(Clone, Debug, PartialEq)]
pub struct StagedTransactionAppend {
    /// Newly assigned Transaction identifier.
    pub transaction_id: TransactionId,
}

/// Bounded sequential Transaction read result.
#[derive(Clone, Debug, PartialEq)]
pub struct TransactionRange {
    /// Transactions read in ascending Append Index order.
    pub transactions: Vec<TransactionRecord>,
    /// Continuation point for the next range read, when at least one Transaction was returned.
    pub next_start: Option<AppendIndex>,
}

#[cfg(test)]
mod tests;
