//! Open-EQMS Security minimal authorization provider for SPEC-007.
//!
//! This crate implements a deterministic, local, in-memory authorization provider
//! over the shared ADR-0004 authorization contract in `runtime-contracts`.
//!
//! It intentionally does not implement authentication, password handling,
//! cryptography, signing, enterprise IAM, external identity stores, databases,
//! networking, package loading, GUI filtering, Query Engine execution,
//! synchronization, Process Engine behavior, Rule Engine behavior, AI behavior, or
//! regulated deployment claims.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod errors;
pub mod policy;
pub mod types;

pub use crate::errors::{SecurityError, SecurityResult, SecurityValidationError};
pub use crate::policy::SecurityEngine;
pub use crate::types::{
    AuthorizationTraceConfig, ExactAuthorizationRule, SecurityPolicy, SecurityPolicyRevision,
    SecurityProviderId,
};
pub use open_eqms_runtime_contracts::{
    AuthorizationDecision, AuthorizationDecisionTrace, AuthorizationEffect, AuthorizationProvider,
    AuthorizationRequest, AuthorizationTarget, CallerPermissionContext, PermissionAction,
};

#[cfg(test)]
mod tests;
