//! Public data types for the minimal Security authorization provider.

use std::collections::BTreeMap;

use crate::errors::{
    malformed_policy, malformed_rule, SecurityError, SecurityResult, SecurityValidationError,
};
use open_eqms_runtime_contracts::{
    AuthorizationDecisionTrace, AuthorizationEffect, AuthorizationRequest,
};

/// Opaque policy revision token carried in decision traces.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SecurityPolicyRevision(String);

impl SecurityPolicyRevision {
    /// Creates a policy revision from a caller-supplied opaque token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the opaque policy revision token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the revision token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for SecurityPolicyRevision {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Opaque Security provider identity token carried in decision traces.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SecurityProviderId(String);

impl SecurityProviderId {
    /// Creates a provider identity from a caller-supplied opaque token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the opaque provider identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the provider identity token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for SecurityProviderId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Deterministic trace configuration supplied to the in-memory provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationTraceConfig {
    decision_id_namespace: String,
    decided_at: String,
}

impl AuthorizationTraceConfig {
    /// Creates a trace configuration from explicit deterministic tokens.
    pub fn new(decision_id_namespace: impl Into<String>, decided_at: impl Into<String>) -> Self {
        Self {
            decision_id_namespace: decision_id_namespace.into(),
            decided_at: decided_at.into(),
        }
    }

    /// Returns the deterministic decision-id namespace.
    pub fn decision_id_namespace(&self) -> &str {
        &self.decision_id_namespace
    }

    /// Returns the deterministic decided-at token.
    pub fn decided_at(&self) -> &str {
        &self.decided_at
    }
}

/// Public exact-match authorization rule.
#[derive(Clone, Eq, PartialEq)]
pub struct ExactAuthorizationRule {
    key: AuthorizationRequestKey,
    effect: AuthorizationEffect,
}

impl ExactAuthorizationRule {
    /// Creates one exact-match rule for one authorization request identity.
    pub fn new(request: AuthorizationRequest, effect: AuthorizationEffect) -> SecurityResult<Self> {
        let key = AuthorizationRequestKey::from_request(&request)?;
        Ok(Self { key, effect })
    }
}

/// Minimal deterministic exact-match policy.
#[derive(Clone, Eq, PartialEq)]
pub struct SecurityPolicy {
    policy_revision: SecurityPolicyRevision,
    provider_id: SecurityProviderId,
    default_denial_effect: AuthorizationEffect,
    trace_config: AuthorizationTraceConfig,
    rules: BTreeMap<AuthorizationRequestKey, AuthorizationEffect>,
}

impl SecurityPolicy {
    /// Creates an empty exact-match policy with an explicit denial default.
    pub fn new(
        policy_revision: SecurityPolicyRevision,
        provider_id: SecurityProviderId,
        default_denial_effect: AuthorizationEffect,
        trace_config: AuthorizationTraceConfig,
    ) -> SecurityResult<Self> {
        validate_policy_inputs(
            &policy_revision,
            &provider_id,
            default_denial_effect,
            &trace_config,
        )?;
        Ok(Self {
            policy_revision,
            provider_id,
            default_denial_effect,
            trace_config,
            rules: BTreeMap::new(),
        })
    }

    /// Creates an exact-match policy from the supplied rules.
    pub fn with_rules(
        policy_revision: SecurityPolicyRevision,
        provider_id: SecurityProviderId,
        default_denial_effect: AuthorizationEffect,
        trace_config: AuthorizationTraceConfig,
        rules: Vec<ExactAuthorizationRule>,
    ) -> SecurityResult<Self> {
        let mut policy = Self::new(
            policy_revision,
            provider_id,
            default_denial_effect,
            trace_config,
        )?;
        for rule in rules {
            policy.add_exact_rule(rule)?;
        }
        Ok(policy)
    }

    /// Registers one exact-match authorization rule.
    pub fn add_exact_rule(&mut self, rule: ExactAuthorizationRule) -> SecurityResult<()> {
        if self.rules.contains_key(&rule.key) {
            return Err(SecurityError::DuplicateRule);
        }
        self.rules.insert(rule.key, rule.effect);
        Ok(())
    }

    pub(crate) fn decide_effect(
        &self,
        request: &AuthorizationRequest,
    ) -> SecurityResult<(AuthorizationEffect, &'static str, AuthorizationRequestKey)> {
        let key = AuthorizationRequestKey::from_request(request)?;
        let Some(effect) = self.rules.get(&key).copied() else {
            return Ok((self.default_denial_effect, self.default_reason_code(), key));
        };
        Ok((effect, "exact-rule", key))
    }

    pub(crate) fn decision_trace(
        &self,
        key: &AuthorizationRequestKey,
        effect: AuthorizationEffect,
        reason_code: &str,
    ) -> AuthorizationDecisionTrace {
        AuthorizationDecisionTrace::new(
            self.policy_revision.as_str(),
            self.decision_id(key, effect, reason_code),
            self.trace_config.decided_at(),
            self.provider_id.as_str(),
            reason_code,
        )
    }

    fn default_reason_code(&self) -> &'static str {
        match self.default_denial_effect {
            AuthorizationEffect::Deny => "default-deny",
            AuthorizationEffect::HiddenDeny => "default-hidden-deny",
            AuthorizationEffect::Allow => "invalid-default-allow",
        }
    }

    fn decision_id(
        &self,
        key: &AuthorizationRequestKey,
        effect: AuthorizationEffect,
        reason_code: &str,
    ) -> String {
        format!(
            "{}|effect={}|reason={}|{}",
            self.trace_config.decision_id_namespace(),
            effect_token(effect),
            reason_code,
            key.canonical_string()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct AuthorizationRequestKey {
    context: String,
    action: String,
    target_source_namespace: String,
    target_object_type: String,
    target_object_identifier: String,
    target_subresource: Option<String>,
}

impl AuthorizationRequestKey {
    pub(crate) fn from_request(request: &AuthorizationRequest) -> SecurityResult<Self> {
        if request.context().is_empty() {
            return Err(malformed_rule(
                SecurityValidationError::EmptyCallerPermissionContext,
            ));
        }
        if request.action().is_empty() {
            return Err(malformed_rule(
                SecurityValidationError::EmptyPermissionAction,
            ));
        }
        if request.target().source_namespace().is_empty() {
            return Err(malformed_rule(
                SecurityValidationError::EmptyTargetSourceNamespace,
            ));
        }
        if request.target().object_type().is_empty() {
            return Err(malformed_rule(
                SecurityValidationError::EmptyTargetObjectType,
            ));
        }
        if request.target().object_identifier().is_empty() {
            return Err(malformed_rule(
                SecurityValidationError::EmptyTargetObjectIdentifier,
            ));
        }
        if request.target().subresource().is_some_and(str::is_empty) {
            return Err(malformed_rule(
                SecurityValidationError::EmptyTargetSubresource,
            ));
        }
        Ok(Self {
            context: request.context().as_str().to_owned(),
            action: request.action().as_str().to_owned(),
            target_source_namespace: request.target().source_namespace().to_owned(),
            target_object_type: request.target().object_type().to_owned(),
            target_object_identifier: request.target().object_identifier().to_owned(),
            target_subresource: request.target().subresource().map(str::to_owned),
        })
    }

    fn canonical_string(&self) -> String {
        let subresource = self
            .target_subresource
            .as_deref()
            .map(length_prefixed)
            .unwrap_or_else(|| "none".to_owned());
        format!(
            "context={};action={};source={};type={};id={};subresource={}",
            length_prefixed(&self.context),
            length_prefixed(&self.action),
            length_prefixed(&self.target_source_namespace),
            length_prefixed(&self.target_object_type),
            length_prefixed(&self.target_object_identifier),
            subresource,
        )
    }
}

fn validate_policy_inputs(
    policy_revision: &SecurityPolicyRevision,
    provider_id: &SecurityProviderId,
    default_denial_effect: AuthorizationEffect,
    trace_config: &AuthorizationTraceConfig,
) -> SecurityResult<()> {
    if policy_revision.is_empty() {
        return Err(malformed_policy(
            SecurityValidationError::EmptyPolicyRevision,
        ));
    }
    if provider_id.is_empty() {
        return Err(malformed_policy(SecurityValidationError::EmptyProviderId));
    }
    if trace_config.decision_id_namespace().is_empty() {
        return Err(malformed_policy(
            SecurityValidationError::EmptyDecisionIdNamespace,
        ));
    }
    if trace_config.decided_at().is_empty() {
        return Err(malformed_policy(SecurityValidationError::EmptyDecidedAt));
    }
    if matches!(default_denial_effect, AuthorizationEffect::Allow) {
        return Err(malformed_policy(
            SecurityValidationError::DefaultEffectMustDeny,
        ));
    }
    Ok(())
}

fn effect_token(effect: AuthorizationEffect) -> &'static str {
    match effect {
        AuthorizationEffect::Allow => "allow",
        AuthorizationEffect::Deny => "deny",
        AuthorizationEffect::HiddenDeny => "hidden-deny",
    }
}

fn length_prefixed(value: &str) -> String {
    format!("{}:{value}", value.len())
}
