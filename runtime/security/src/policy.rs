//! Minimal deterministic authorization policy provider.

use open_eqms_runtime_contracts::{
    AuthorizationDecision, AuthorizationProvider, AuthorizationRequest,
};

use crate::errors::SecurityResult;
use crate::types::SecurityPolicy;

/// Minimal in-memory Security Engine implementing the shared authorization provider contract.
#[derive(Clone, Eq, PartialEq)]
pub struct SecurityEngine {
    policy: SecurityPolicy,
}

impl SecurityEngine {
    /// Creates a Security Engine over one immutable in-memory policy.
    pub fn new(policy: SecurityPolicy) -> Self {
        Self { policy }
    }

    /// Evaluates one authorization request using exact-match policy rules only.
    pub fn decide_authorization(
        &self,
        request: &AuthorizationRequest,
    ) -> SecurityResult<AuthorizationDecision> {
        let (effect, reason_code, key) = self.policy.decide_effect(request)?;
        let trace = self.policy.decision_trace(&key, effect, reason_code);
        Ok(AuthorizationDecision::new(effect, trace))
    }
}

impl AuthorizationProvider for SecurityEngine {
    type Error = crate::errors::SecurityError;

    fn decide(&self, request: &AuthorizationRequest) -> Result<AuthorizationDecision, Self::Error> {
        self.decide_authorization(request)
    }
}
