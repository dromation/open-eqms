//! Structured Transaction Engine error taxonomy.

use crate::types::{AppendIndex, SignerRef, TransactionId, TransactionLevel, Version};
use std::fmt;

/// Result type returned by Transaction Engine APIs.
pub type TransactionEngineResult<T> = Result<T, TransactionEngineError>;

/// Top-level structured errors returned by the Transaction Engine.
#[derive(Clone, Debug, PartialEq)]
pub enum TransactionEngineError {
    /// Requested Transaction does not exist.
    TransactionNotFound {
        /// Missing Transaction identifier.
        transaction_id: TransactionId,
    },
    /// Transaction level is unsupported by this implementation.
    UnsupportedTransactionLevel {
        /// Unsupported level label.
        level: String,
    },
    /// Transaction append data failed structural validation.
    ValidationFailed {
        /// Machine-distinguishable validation failure detail.
        failure: ValidationError,
    },
    /// Generated Transaction identity already exists.
    DuplicateIdentity {
        /// Duplicate Transaction identifier.
        transaction_id: TransactionId,
    },
    /// Append Index allocation failed before any durable record was stored.
    AppendIndexAllocationFailure {
        /// Failure description.
        message: String,
    },
    /// Transaction Sequence allocation failed before any durable record was stored.
    TransactionSequenceAllocationFailure {
        /// Failure description.
        message: String,
    },
    /// Level 3 signature did not verify successfully.
    SignatureVerificationFailure {
        /// Signer reference used for verification.
        signer: SignerRef,
        /// Deterministic failure reason.
        reason: String,
    },
    /// Cryptographic provider failed while hashing or verifying.
    CryptographicProviderFailure {
        /// Provider failure description.
        message: String,
    },
    /// Storage provider failed outside the Transaction Engine's semantic checks.
    StorageProviderFailure {
        /// Provider failure description.
        message: String,
    },
    /// Append Index range request is invalid.
    InvalidAppendIndexRange {
        /// Requested starting append index.
        start: AppendIndex,
        /// Requested maximum batch size.
        max: usize,
        /// Deterministic reason for rejection.
        reason: String,
    },
}

impl fmt::Display for TransactionEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransactionNotFound { transaction_id } => {
                write!(formatter, "transaction not found: {transaction_id}")
            }
            Self::UnsupportedTransactionLevel { level } => {
                write!(formatter, "unsupported transaction level: {level}")
            }
            Self::ValidationFailed { failure } => write!(formatter, "validation failed: {failure}"),
            Self::DuplicateIdentity { transaction_id } => {
                write!(
                    formatter,
                    "duplicate transaction identity: {transaction_id}"
                )
            }
            Self::AppendIndexAllocationFailure { message } => {
                write!(formatter, "append index allocation failure: {message}")
            }
            Self::TransactionSequenceAllocationFailure { message } => {
                write!(
                    formatter,
                    "transaction sequence allocation failure: {message}"
                )
            }
            Self::SignatureVerificationFailure { signer, reason } => {
                write!(
                    formatter,
                    "signature verification failed for {signer}: {reason}"
                )
            }
            Self::CryptographicProviderFailure { message } => {
                write!(formatter, "cryptographic provider failure: {message}")
            }
            Self::StorageProviderFailure { message } => {
                write!(formatter, "storage provider failure: {message}")
            }
            Self::InvalidAppendIndexRange { start, max, reason } => write!(
                formatter,
                "invalid append index range from {} with max {}: {}",
                start.value(),
                max,
                reason
            ),
        }
    }
}

impl std::error::Error for TransactionEngineError {}

/// Machine-distinguishable validation failure detail.
#[derive(Clone, Debug, PartialEq)]
pub enum ValidationError {
    /// Transaction schema version is zero.
    EmptySchemaVersion,
    /// Object identifier is empty.
    EmptyObjectId,
    /// Operation descriptor is empty.
    EmptyOperation,
    /// Actor reference is empty.
    EmptyActor,
    /// Device reference is empty.
    EmptyDevice,
    /// Optional site reference was supplied as an empty token.
    EmptySite,
    /// Timestamp field is missing.
    MissingTimestamp {
        /// Timestamp field name.
        field: String,
    },
    /// Timestamp field is malformed.
    MalformedTimestamp {
        /// Timestamp field name.
        field: String,
        /// Malformed timestamp token.
        value: String,
    },
    /// Resulting version is not exactly one greater than the base version.
    ResultingVersionMismatch {
        /// Caller-supplied base version.
        base_version: Version,
        /// Caller-supplied resulting version.
        resulting_version: Version,
    },
    /// Old or new value is structurally malformed.
    MalformedPropertyValue {
        /// Field name containing the malformed value.
        field: String,
        /// Deterministic reason for rejection.
        reason: String,
    },
    /// Level 2+ server receipt time is missing.
    MissingServerReceiptTime {
        /// Transaction level being appended.
        level: TransactionLevel,
    },
    /// Caller supplied a field that is not valid for the declared level.
    FieldNotAllowedForLevel {
        /// Field name.
        field: String,
        /// Transaction level being appended.
        level: TransactionLevel,
    },
    /// Prior Transaction Hash was supplied without a Transaction-kind Prior Reference.
    PriorTransactionHashRequiresTransactionReference,
    /// Prior Reference points to an empty Transaction identifier.
    EmptyPriorTransactionReference,
    /// Prior Reference points to an empty Event identifier token.
    EmptyPriorEventReference,
    /// Prior Transaction Hash token is empty.
    EmptyPriorTransactionHash,
    /// Level 3 signer reference is missing or empty.
    MissingSigner,
    /// Level 3 signer role is missing or empty.
    MissingSignerRole,
    /// Level 3 signature meaning is missing or empty.
    MissingSignatureMeaning,
    /// Level 3 authentication evidence reference is missing or empty.
    MissingAuthenticationEvidence,
    /// Level 3 signed revision is missing.
    MissingSignedRevision,
    /// Level 3 reason is missing or empty.
    MissingReason,
    /// Level 3 signature is missing or empty.
    MissingSignature,
    /// Level 3 signed revision does not match this Transaction's resulting version.
    SignedRevisionMismatch {
        /// Signed revision supplied by the caller.
        signed_revision: Version,
        /// Resulting version expected by this Transaction.
        expected: Version,
    },
}

impl fmt::Display for ValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptySchemaVersion => formatter.write_str("schema version must not be zero"),
            Self::EmptyObjectId => formatter.write_str("object id must not be empty"),
            Self::EmptyOperation => formatter.write_str("operation must not be empty"),
            Self::EmptyActor => formatter.write_str("actor must not be empty"),
            Self::EmptyDevice => formatter.write_str("device must not be empty"),
            Self::EmptySite => formatter.write_str("site must not be empty when supplied"),
            Self::MissingTimestamp { field } => write!(formatter, "{field} must not be empty"),
            Self::MalformedTimestamp { field, value } => {
                write!(formatter, "{field} is malformed: {value}")
            }
            Self::ResultingVersionMismatch {
                base_version,
                resulting_version,
            } => write!(
                formatter,
                "resulting version {} is not one greater than base version {}",
                resulting_version.value(),
                base_version.value()
            ),
            Self::MalformedPropertyValue { field, reason } => {
                write!(formatter, "{field} is malformed: {reason}")
            }
            Self::MissingServerReceiptTime { level } => {
                write!(formatter, "server receipt time is required for {level}")
            }
            Self::FieldNotAllowedForLevel { field, level } => {
                write!(formatter, "{field} is not allowed for {level}")
            }
            Self::PriorTransactionHashRequiresTransactionReference => formatter
                .write_str("prior transaction hash requires a Transaction-kind prior reference"),
            Self::EmptyPriorTransactionReference => {
                formatter.write_str("prior transaction reference must not be empty")
            }
            Self::EmptyPriorEventReference => {
                formatter.write_str("prior event reference must not be empty")
            }
            Self::EmptyPriorTransactionHash => {
                formatter.write_str("prior transaction hash must not be empty")
            }
            Self::MissingSigner => formatter.write_str("signer is required for level-3"),
            Self::MissingSignerRole => formatter.write_str("signer role is required for level-3"),
            Self::MissingSignatureMeaning => {
                formatter.write_str("signature meaning is required for level-3")
            }
            Self::MissingAuthenticationEvidence => {
                formatter.write_str("authentication evidence is required for level-3")
            }
            Self::MissingSignedRevision => {
                formatter.write_str("signed revision is required for level-3")
            }
            Self::MissingReason => formatter.write_str("reason is required for level-3"),
            Self::MissingSignature => formatter.write_str("signature is required for level-3"),
            Self::SignedRevisionMismatch {
                signed_revision,
                expected,
            } => write!(
                formatter,
                "signed revision {} does not match expected version {}",
                signed_revision.value(),
                expected.value()
            ),
        }
    }
}

/// Wraps a validation detail in the top-level error taxonomy.
pub(crate) fn validation(failure: ValidationError) -> TransactionEngineError {
    TransactionEngineError::ValidationFailed { failure }
}
