//! Structured errors for the minimal Security authorization provider.

use std::fmt;

/// Result type returned by Security provider APIs.
pub type SecurityResult<T> = Result<T, SecurityError>;

/// Top-level Security provider error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityError {
    /// A policy or trace configuration value is malformed.
    MalformedPolicy {
        /// Machine-distinguishable policy validation failure.
        failure: SecurityValidationError,
    },
    /// A rule or authorization request value is malformed.
    MalformedRule {
        /// Machine-distinguishable rule validation failure.
        failure: SecurityValidationError,
    },
    /// An exact rule for the same request identity already exists.
    DuplicateRule,
}

impl fmt::Display for SecurityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedPolicy { failure } => {
                write!(formatter, "malformed security policy: {failure}")
            }
            Self::MalformedRule { failure } => {
                write!(formatter, "malformed authorization rule: {failure}")
            }
            Self::DuplicateRule => formatter.write_str("duplicate exact authorization rule"),
        }
    }
}

impl std::error::Error for SecurityError {}

/// Machine-distinguishable validation failure detail.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SecurityValidationError {
    /// Policy revision token must be non-empty.
    EmptyPolicyRevision,
    /// Provider identity token must be non-empty.
    EmptyProviderId,
    /// Trace decision-id namespace token must be non-empty.
    EmptyDecisionIdNamespace,
    /// Trace decided-at token must be non-empty.
    EmptyDecidedAt,
    /// Default effect must be a denial.
    DefaultEffectMustDeny,
    /// Caller permission context token must be non-empty.
    EmptyCallerPermissionContext,
    /// Permission action token must be non-empty.
    EmptyPermissionAction,
    /// Authorization target source namespace must be non-empty.
    EmptyTargetSourceNamespace,
    /// Authorization target object type must be non-empty.
    EmptyTargetObjectType,
    /// Authorization target object identifier must be non-empty.
    EmptyTargetObjectIdentifier,
    /// Authorization target subresource must be non-empty when supplied.
    EmptyTargetSubresource,
}

impl fmt::Display for SecurityValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyPolicyRevision => "policy revision must not be empty",
            Self::EmptyProviderId => "provider identity must not be empty",
            Self::EmptyDecisionIdNamespace => "decision-id namespace must not be empty",
            Self::EmptyDecidedAt => "decided-at token must not be empty",
            Self::DefaultEffectMustDeny => "default effect must be Deny or HiddenDeny",
            Self::EmptyCallerPermissionContext => "caller permission context must not be empty",
            Self::EmptyPermissionAction => "permission action must not be empty",
            Self::EmptyTargetSourceNamespace => "target source namespace must not be empty",
            Self::EmptyTargetObjectType => "target object type must not be empty",
            Self::EmptyTargetObjectIdentifier => "target object identifier must not be empty",
            Self::EmptyTargetSubresource => "target subresource must not be empty when supplied",
        })
    }
}

pub(crate) fn malformed_policy(failure: SecurityValidationError) -> SecurityError {
    SecurityError::MalformedPolicy { failure }
}

pub(crate) fn malformed_rule(failure: SecurityValidationError) -> SecurityError {
    SecurityError::MalformedRule { failure }
}
