//! Data contracts for the Open-EQMS Transaction Engine.

use std::collections::BTreeMap;
use std::fmt;

pub use open_eqms_runtime_contracts::{ObjectId, PropertyValue, UnitOfWork, Version};

macro_rules! opaque_string_type {
    ($(#[$meta:meta])* pub struct $name:ident;) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
        pub struct $name(String);

        impl $name {
            /// Creates a new opaque token from a caller-supplied value.
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            /// Returns the stored token without assigning business meaning.
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Reports whether the stored token is empty.
            pub fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

opaque_string_type! {
    /// Opaque globally unique identifier for a Transaction.
    pub struct TransactionId;
}

opaque_string_type! {
    /// Caller-supplied opaque operation descriptor.
    pub struct OperationDescriptor;
}

opaque_string_type! {
    /// Caller-supplied actor reference.
    pub struct ActorRef;
}

opaque_string_type! {
    /// Caller-supplied device reference.
    pub struct DeviceRef;
}

opaque_string_type! {
    /// Optional caller-supplied site or deployment reference.
    pub struct SiteRef;
}

opaque_string_type! {
    /// Caller-supplied timestamp token for Transaction metadata.
    pub struct TransactionTimestamp;
}

opaque_string_type! {
    /// Caller-supplied signer reference for Level 3 Transactions.
    pub struct SignerRef;
}

opaque_string_type! {
    /// Caller-supplied signer role token for Level 3 Transactions.
    pub struct SignerRole;
}

opaque_string_type! {
    /// Language-neutral signature meaning token for Level 3 Transactions.
    pub struct SignatureMeaning;
}

opaque_string_type! {
    /// Opaque authentication-evidence reference for Level 3 Transactions.
    pub struct AuthenticationEvidenceRef;
}

opaque_string_type! {
    /// Caller-supplied reason token or text for Level 3 Transactions.
    pub struct Reason;
}

opaque_string_type! {
    /// Caller-supplied prior Transaction hash input for Level 2+ hash chaining.
    pub struct PriorTransactionHash;
}

opaque_string_type! {
    /// Transaction Engine computed hash for a Level 2+ Transaction.
    pub struct TransactionHash;
}

opaque_string_type! {
    /// Caller-supplied cryptographic signature bytes encoded as an opaque token.
    pub struct CryptographicSignature;
}

/// Transaction record schema version.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TransactionSchemaVersion(u32);

impl TransactionSchemaVersion {
    /// Creates a schema version marker from a raw value.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw schema version value.
    pub fn value(self) -> u32 {
        self.0
    }

    /// Reports whether this is outside the valid public range.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Regulatory level carried by every Transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum TransactionLevel {
    /// Ordinary edit.
    Level1,
    /// Process or significant event.
    Level2,
    /// Confirmed regulated milestone.
    Level3,
    /// Unsupported marker rejected before append.
    Unsupported,
}

impl TransactionLevel {
    /// Reports whether this level carries Level 2 metadata.
    pub fn requires_level2_fields(self) -> bool {
        matches!(self, Self::Level2 | Self::Level3)
    }

    /// Reports whether this level carries Level 3 metadata.
    pub fn requires_level3_fields(self) -> bool {
        matches!(self, Self::Level3)
    }

    /// Returns a stable label for diagnostics and canonical content.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Level1 => "level-1",
            Self::Level2 => "level-2",
            Self::Level3 => "level-3",
            Self::Unsupported => "unsupported",
        }
    }
}

impl fmt::Display for TransactionLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Strictly increasing technical append-order marker assigned to every Transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AppendIndex(u64);

impl AppendIndex {
    /// Returns the first valid append index.
    pub fn first() -> Self {
        Self(1)
    }

    /// Creates an append index from a raw monotonic marker.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw monotonic marker.
    pub fn value(self) -> u64 {
        self.0
    }

    /// Returns the next append index marker.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }

    /// Reports whether this marker is outside the valid public range.
    pub fn is_zero(self) -> bool {
        self.0 == 0
    }
}

/// Strictly increasing regulatory sequence marker for Level 2 and Level 3 Transactions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TransactionSequence(u64);

impl TransactionSequence {
    /// Returns the first valid transaction sequence.
    pub fn first() -> Self {
        Self(1)
    }

    /// Creates a transaction sequence from a raw monotonic marker.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw monotonic marker.
    pub fn value(self) -> u64 {
        self.0
    }

    /// Returns the next transaction sequence marker.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Opaque signed revision represented as a shared Object `Version`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct SignedRevision(Version);

impl SignedRevision {
    /// Creates a signed revision marker from a shared Version.
    pub fn new(version: Version) -> Self {
        Self(version)
    }

    /// Returns the shared Version this signed revision identifies.
    pub fn version(self) -> Version {
        self.0
    }
}

/// Optional caller-supplied link to a prior Transaction or triggering Event.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum PriorReference {
    /// Link to a prior Transaction in the Transaction/Audit Log.
    Transaction(TransactionId),
    /// Opaque link to a Business Event without depending on the Event Engine crate.
    Event(String),
}

/// Opaque rule-evaluation metadata recorded directly on a Transaction.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuleEvaluationMetadata {
    /// Optional opaque triggering Event reference.
    pub triggering_event_ref: Option<String>,
    /// Optional opaque rule version token.
    pub rule_version: Option<String>,
    /// Optional opaque evaluation result token.
    pub evaluation_result: Option<String>,
    /// Opaque generated-action tokens.
    pub generated_actions: Vec<String>,
    /// Deterministic opaque execution metadata.
    pub execution_metadata: BTreeMap<String, String>,
}

/// Immutable Transaction/Audit Log record.
#[derive(Clone, Debug, PartialEq)]
pub struct TransactionRecord {
    /// Transaction identity.
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
    /// Assigned technical append order.
    pub append_index: AppendIndex,
    /// Assigned Level 2+ regulatory sequence.
    pub transaction_sequence: Option<TransactionSequence>,
    /// Level 2+ server receipt time.
    pub server_receipt_time: Option<TransactionTimestamp>,
    /// Optional link to a prior Transaction or triggering Event.
    pub prior_reference: Option<PriorReference>,
    /// Optional caller-supplied prior Transaction hash input.
    pub prior_transaction_hash: Option<PriorTransactionHash>,
    /// Level 2+ computed transaction hash.
    pub transaction_hash: Option<TransactionHash>,
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

impl TransactionRecord {
    /// Creates a full immutable Transaction record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: TransactionId,
        schema_version: TransactionSchemaVersion,
        level: TransactionLevel,
        object_id: ObjectId,
        operation: OperationDescriptor,
        old_value: PropertyValue,
        new_value: PropertyValue,
        actor: ActorRef,
        device: DeviceRef,
        site: Option<SiteRef>,
        edit_timestamp: TransactionTimestamp,
        base_version: Version,
        resulting_version: Version,
        append_index: AppendIndex,
        transaction_sequence: Option<TransactionSequence>,
        server_receipt_time: Option<TransactionTimestamp>,
        prior_reference: Option<PriorReference>,
        prior_transaction_hash: Option<PriorTransactionHash>,
        transaction_hash: Option<TransactionHash>,
        rule_evaluation: Option<RuleEvaluationMetadata>,
        signer: Option<SignerRef>,
        signer_role: Option<SignerRole>,
        signature_meaning: Option<SignatureMeaning>,
        authentication_evidence: Option<AuthenticationEvidenceRef>,
        signed_revision: Option<SignedRevision>,
        reason: Option<Reason>,
        signing_timestamp: Option<TransactionTimestamp>,
        signature: Option<CryptographicSignature>,
    ) -> Self {
        Self {
            id,
            schema_version,
            level,
            object_id,
            operation,
            old_value,
            new_value,
            actor,
            device,
            site,
            edit_timestamp,
            base_version,
            resulting_version,
            append_index,
            transaction_sequence,
            server_receipt_time,
            prior_reference,
            prior_transaction_hash,
            transaction_hash,
            rule_evaluation,
            signer,
            signer_role,
            signature_meaning,
            authentication_evidence,
            signed_revision,
            reason,
            signing_timestamp,
            signature,
        }
    }
}
