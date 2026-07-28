//! Validation logic for Transaction append requests.

use crate::errors::{validation, TransactionEngineError, TransactionEngineResult, ValidationError};
use crate::types::{
    CryptographicSignature, PriorReference, PropertyValue, SignedRevision, SignerRef,
    TransactionLevel, TransactionTimestamp, Version,
};
use crate::AppendTransactionRequest;

pub(crate) fn validate_append_request(
    request: &AppendTransactionRequest,
) -> TransactionEngineResult<()> {
    validate_common_fields(request)?;
    validate_property_value("old_value", &request.old_value)?;
    validate_property_value("new_value", &request.new_value)?;
    validate_prior_reference(request)?;

    if request.level.requires_level2_fields() {
        let server_receipt_time = request.server_receipt_time.as_ref().ok_or_else(|| {
            validation(ValidationError::MissingServerReceiptTime {
                level: request.level,
            })
        })?;
        validate_timestamp("server_receipt_time", server_receipt_time)?;
    } else {
        reject_level_field(
            request.level,
            "server_receipt_time",
            request.server_receipt_time.is_some(),
        )?;
        reject_level_field(
            request.level,
            "prior_transaction_hash",
            request.prior_transaction_hash.is_some(),
        )?;
    }

    if request.level.requires_level3_fields() {
        validate_level3_fields(request)?;
    } else {
        reject_level3_fields(request)?;
    }

    Ok(())
}

pub(crate) fn validate_level3_signature_inputs<'a>(
    level: TransactionLevel,
    signed_revision: Option<&'a SignedRevision>,
    signer: Option<&'a SignerRef>,
    signature: Option<&'a CryptographicSignature>,
) -> TransactionEngineResult<
    Option<(
        &'a SignedRevision,
        &'a SignerRef,
        &'a CryptographicSignature,
    )>,
> {
    if !level.requires_level3_fields() {
        return Ok(None);
    }

    let signed_revision =
        signed_revision.ok_or_else(|| validation(ValidationError::MissingSignedRevision))?;
    let signer = signer.ok_or_else(|| validation(ValidationError::MissingSigner))?;
    let signature = signature.ok_or_else(|| validation(ValidationError::MissingSignature))?;
    Ok(Some((signed_revision, signer, signature)))
}

fn validate_common_fields(request: &AppendTransactionRequest) -> TransactionEngineResult<()> {
    if request.schema_version.is_zero() {
        return Err(validation(ValidationError::EmptySchemaVersion));
    }
    if matches!(request.level, TransactionLevel::Unsupported) {
        return Err(TransactionEngineError::UnsupportedTransactionLevel {
            level: request.level.as_str().to_owned(),
        });
    }
    if request.object_id.is_empty() {
        return Err(validation(ValidationError::EmptyObjectId));
    }
    if request.operation.is_empty() {
        return Err(validation(ValidationError::EmptyOperation));
    }
    if request.actor.is_empty() {
        return Err(validation(ValidationError::EmptyActor));
    }
    if request.device.is_empty() {
        return Err(validation(ValidationError::EmptyDevice));
    }
    if request.site.as_ref().is_some_and(|site| site.is_empty()) {
        return Err(validation(ValidationError::EmptySite));
    }
    validate_timestamp("edit_timestamp", &request.edit_timestamp)?;
    validate_version_step(request.base_version, request.resulting_version)
}

fn validate_version_step(
    base_version: Version,
    resulting_version: Version,
) -> TransactionEngineResult<()> {
    if base_version.next() != resulting_version {
        return Err(validation(ValidationError::ResultingVersionMismatch {
            base_version,
            resulting_version,
        }));
    }
    Ok(())
}

fn validate_prior_reference(request: &AppendTransactionRequest) -> TransactionEngineResult<()> {
    match &request.prior_reference {
        Some(PriorReference::Transaction(transaction_id)) if transaction_id.is_empty() => {
            return Err(validation(ValidationError::EmptyPriorTransactionReference));
        }
        Some(PriorReference::Event(event_id)) if event_id.is_empty() => {
            return Err(validation(ValidationError::EmptyPriorEventReference));
        }
        _ => {}
    }

    if request
        .prior_transaction_hash
        .as_ref()
        .is_some_and(|hash| hash.is_empty())
    {
        return Err(validation(ValidationError::EmptyPriorTransactionHash));
    }

    if request.prior_transaction_hash.is_some()
        && !matches!(
            request.prior_reference,
            Some(PriorReference::Transaction(_))
        )
    {
        return Err(validation(
            ValidationError::PriorTransactionHashRequiresTransactionReference,
        ));
    }

    Ok(())
}

fn validate_level3_fields(request: &AppendTransactionRequest) -> TransactionEngineResult<()> {
    let signer = request
        .signer
        .as_ref()
        .ok_or_else(|| validation(ValidationError::MissingSigner))?;
    if signer.is_empty() {
        return Err(validation(ValidationError::MissingSigner));
    }

    let signer_role = request
        .signer_role
        .as_ref()
        .ok_or_else(|| validation(ValidationError::MissingSignerRole))?;
    if signer_role.is_empty() {
        return Err(validation(ValidationError::MissingSignerRole));
    }

    let signature_meaning = request
        .signature_meaning
        .as_ref()
        .ok_or_else(|| validation(ValidationError::MissingSignatureMeaning))?;
    if signature_meaning.is_empty() {
        return Err(validation(ValidationError::MissingSignatureMeaning));
    }

    let authentication_evidence = request
        .authentication_evidence
        .as_ref()
        .ok_or_else(|| validation(ValidationError::MissingAuthenticationEvidence))?;
    if authentication_evidence.is_empty() {
        return Err(validation(ValidationError::MissingAuthenticationEvidence));
    }

    let signed_revision = request
        .signed_revision
        .ok_or_else(|| validation(ValidationError::MissingSignedRevision))?;
    if signed_revision.version() != request.resulting_version {
        return Err(validation(ValidationError::SignedRevisionMismatch {
            signed_revision: signed_revision.version(),
            expected: request.resulting_version,
        }));
    }

    let reason = request
        .reason
        .as_ref()
        .ok_or_else(|| validation(ValidationError::MissingReason))?;
    if reason.is_empty() {
        return Err(validation(ValidationError::MissingReason));
    }

    let signing_timestamp = request.signing_timestamp.as_ref().ok_or_else(|| {
        validation(ValidationError::MissingTimestamp {
            field: "signing_timestamp".to_owned(),
        })
    })?;
    validate_timestamp("signing_timestamp", signing_timestamp)?;

    let signature = request
        .signature
        .as_ref()
        .ok_or_else(|| validation(ValidationError::MissingSignature))?;
    if signature.is_empty() {
        return Err(validation(ValidationError::MissingSignature));
    }

    Ok(())
}

fn reject_level3_fields(request: &AppendTransactionRequest) -> TransactionEngineResult<()> {
    reject_level_field(request.level, "signer", request.signer.is_some())?;
    reject_level_field(request.level, "signer_role", request.signer_role.is_some())?;
    reject_level_field(
        request.level,
        "signature_meaning",
        request.signature_meaning.is_some(),
    )?;
    reject_level_field(
        request.level,
        "authentication_evidence",
        request.authentication_evidence.is_some(),
    )?;
    reject_level_field(
        request.level,
        "signed_revision",
        request.signed_revision.is_some(),
    )?;
    reject_level_field(request.level, "reason", request.reason.is_some())?;
    reject_level_field(
        request.level,
        "signing_timestamp",
        request.signing_timestamp.is_some(),
    )?;
    reject_level_field(request.level, "signature", request.signature.is_some())
}

fn reject_level_field(
    level: TransactionLevel,
    field: &str,
    present: bool,
) -> TransactionEngineResult<()> {
    if present {
        return Err(validation(ValidationError::FieldNotAllowedForLevel {
            field: field.to_owned(),
            level,
        }));
    }
    Ok(())
}

fn validate_timestamp(
    field: &str,
    timestamp: &TransactionTimestamp,
) -> TransactionEngineResult<()> {
    if timestamp.is_empty() {
        return Err(validation(ValidationError::MissingTimestamp {
            field: field.to_owned(),
        }));
    }
    if !is_utc_timestamp(timestamp.as_str()) {
        return Err(validation(ValidationError::MalformedTimestamp {
            field: field.to_owned(),
            value: timestamp.as_str().to_owned(),
        }));
    }
    Ok(())
}

fn is_utc_timestamp(value: &str) -> bool {
    value.len() >= 20
        && value.as_bytes().get(4) == Some(&b'-')
        && value.as_bytes().get(7) == Some(&b'-')
        && value.as_bytes().get(10) == Some(&b'T')
        && value.as_bytes().get(13) == Some(&b':')
        && value.as_bytes().get(16) == Some(&b':')
        && value.ends_with('Z')
        && value
            .chars()
            .enumerate()
            .all(|(index, character)| match index {
                4 | 7 => character == '-',
                10 => character == 'T',
                13 | 16 => character == ':',
                index if index == value.len() - 1 => character == 'Z',
                19 if value.len() > 20 => character == '.',
                _ => character.is_ascii_digit(),
            })
}

fn validate_property_value(field: &str, value: &PropertyValue) -> TransactionEngineResult<()> {
    match value {
        PropertyValue::Text { language, .. } => {
            if language
                .as_ref()
                .is_some_and(|language| language.is_empty())
            {
                return Err(malformed_value(field, "language tag must not be empty"));
            }
        }
        PropertyValue::LocalizedTextKey(value)
        | PropertyValue::DateTime(value)
        | PropertyValue::EnumValue(value)
        | PropertyValue::AttachmentReference(value) => {
            if value.is_empty() {
                return Err(malformed_value(field, "token must not be empty"));
            }
        }
        PropertyValue::Number(value) => {
            if !value.is_finite() {
                return Err(malformed_value(field, "number must be finite"));
            }
        }
        PropertyValue::Reference(object_id) => {
            if object_id.is_empty() {
                return Err(malformed_value(field, "object reference must not be empty"));
            }
        }
        PropertyValue::Boolean(_) | PropertyValue::BinaryBlob(_) => {}
    }
    Ok(())
}

fn malformed_value(field: &str, reason: &str) -> TransactionEngineError {
    validation(ValidationError::MalformedPropertyValue {
        field: field.to_owned(),
        reason: reason.to_owned(),
    })
}
