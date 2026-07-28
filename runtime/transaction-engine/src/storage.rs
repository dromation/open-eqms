//! Storage-provider and identity-generation boundaries for the Transaction Engine.

use crate::canonical::canonical_transaction_content;
use crate::crypto::CryptographicProvider;
use crate::errors::{TransactionEngineError, TransactionEngineResult};
use crate::types::{
    ActorRef, AppendIndex, AuthenticationEvidenceRef, CryptographicSignature, DeviceRef, ObjectId,
    OperationDescriptor, PriorReference, PriorTransactionHash, PropertyValue, Reason,
    RuleEvaluationMetadata, SignatureMeaning, SignedRevision, SignerRef, SignerRole, SiteRef,
    TransactionId, TransactionLevel, TransactionRecord, TransactionSchemaVersion,
    TransactionSequence, TransactionTimestamp, UnitOfWork, Version,
};

/// Prepared, validated Transaction data that has not yet received durable log numbering.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTransaction {
    /// Transaction identity generated before staging or appending.
    pub id: TransactionId,
    /// Record schema version.
    pub schema_version: TransactionSchemaVersion,
    /// Regulatory level.
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

/// Abstract append-only persistence boundary used by the Transaction Engine.
///
/// Implementations own allocation of Append Index and Transaction Sequence
/// values and must keep allocation, hash computation, and persistence inside one
/// atomic operation. Unit-of-Work staging stores only prepared Transactions; a
/// provider-owned commit finalizes them later through the same allocation and
/// hashing sequence.
pub trait TransactionStore {
    /// Reports whether a Transaction identity exists or is already staged.
    fn transaction_exists(&self, transaction_id: &TransactionId) -> TransactionEngineResult<bool>;

    /// Retrieves a finalized durable Transaction by identity.
    fn get_transaction(
        &self,
        transaction_id: &TransactionId,
    ) -> TransactionEngineResult<Option<TransactionRecord>>;

    /// Atomically assigns numbering, computes any required hash, and durably appends.
    fn append_transaction<C: CryptographicProvider>(
        &mut self,
        prepared: PreparedTransaction,
        cryptographic_provider: &mut C,
    ) -> TransactionEngineResult<TransactionRecord>;

    /// Stages a prepared Transaction against a provider-owned Unit of Work.
    fn stage_transaction(
        &mut self,
        unit_of_work: &UnitOfWork,
        prepared: PreparedTransaction,
    ) -> TransactionEngineResult<()>;

    /// Reads finalized Transactions in ascending Append Index order, starting at `start`.
    fn read_append_index_range(
        &self,
        start: AppendIndex,
        max: usize,
    ) -> TransactionEngineResult<Vec<TransactionRecord>>;
}

/// Transaction identity source used by append operations.
///
/// Production deployments may provide a UUIDv7/ULID-class generator. Tests use a
/// deterministic implementation so replayed API sequences produce identical logs.
pub trait TransactionIdGenerator {
    /// Returns the next opaque Transaction identifier.
    fn next_id(&mut self) -> TransactionId;
}

#[allow(dead_code)]
pub(crate) fn finalize_prepared_transaction<C: CryptographicProvider>(
    prepared: PreparedTransaction,
    append_index: AppendIndex,
    transaction_sequence: Option<TransactionSequence>,
    cryptographic_provider: &mut C,
) -> TransactionEngineResult<TransactionRecord> {
    if prepared.level.requires_level2_fields() && transaction_sequence.is_none() {
        return Err(
            TransactionEngineError::TransactionSequenceAllocationFailure {
                message: "level-2 and level-3 transactions require a transaction sequence"
                    .to_owned(),
            },
        );
    }

    let mut record = TransactionRecord::new(
        prepared.id,
        prepared.schema_version,
        prepared.level,
        prepared.object_id,
        prepared.operation,
        prepared.old_value,
        prepared.new_value,
        prepared.actor,
        prepared.device,
        prepared.site,
        prepared.edit_timestamp,
        prepared.base_version,
        prepared.resulting_version,
        append_index,
        transaction_sequence,
        prepared.server_receipt_time,
        prepared.prior_reference,
        prepared.prior_transaction_hash,
        None,
        prepared.rule_evaluation,
        prepared.signer,
        prepared.signer_role,
        prepared.signature_meaning,
        prepared.authentication_evidence,
        prepared.signed_revision,
        prepared.reason,
        prepared.signing_timestamp,
        prepared.signature,
    );

    if record.level.requires_level2_fields() {
        let canonical_content = canonical_transaction_content(&record);
        let hash = cryptographic_provider
            .hash(&canonical_content)
            .map_err(|message| TransactionEngineError::CryptographicProviderFailure { message })?;
        record.transaction_hash = Some(hash);
    }

    Ok(record)
}
