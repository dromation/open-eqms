use super::*;

fn trace_config() -> AuthorizationTraceConfig {
    AuthorizationTraceConfig::new("decision-seed", "2026-09-09T10:59:00Z")
}

fn policy(default: AuthorizationEffect) -> SecurityPolicy {
    SecurityPolicy::new(
        SecurityPolicyRevision::new("policy-v1"),
        SecurityProviderId::new("minimal-security"),
        default,
        trace_config(),
    )
    .unwrap()
}

fn request(
    context: &str,
    action: &str,
    identifier: &str,
    subresource: Option<&str>,
) -> AuthorizationRequest {
    let target = AuthorizationTarget::new("object-runtime", "asset", identifier);
    let target = if let Some(subresource) = subresource {
        target.with_subresource(subresource)
    } else {
        target
    };
    AuthorizationRequest::new(
        CallerPermissionContext::new(context),
        PermissionAction::new(action),
        target,
    )
}

fn request_with_target(
    context: &str,
    action: &str,
    source_namespace: &str,
    object_type: &str,
    identifier: &str,
) -> AuthorizationRequest {
    AuthorizationRequest::new(
        CallerPermissionContext::new(context),
        PermissionAction::new(action),
        AuthorizationTarget::new(source_namespace, object_type, identifier),
    )
}

#[test]
fn exact_allow_rule_returns_allow() {
    let authorization_request = request("caller-001", "query.read-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(authorization_request.clone(), AuthorizationEffect::Allow)
                .unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy)
        .decide(&authorization_request)
        .unwrap();

    assert_eq!(AuthorizationEffect::Allow, decision.effect());
    assert_eq!("exact-rule", decision.trace().reason_code());
}

#[test]
fn exact_explicit_deny_rule_returns_deny() {
    let authorization_request = request("caller-001", "query.read-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(authorization_request.clone(), AuthorizationEffect::Deny)
                .unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy)
        .decide(&authorization_request)
        .unwrap();

    assert_eq!(AuthorizationEffect::Deny, decision.effect());
    assert_eq!("exact-rule", decision.trace().reason_code());
}

#[test]
fn exact_hidden_deny_rule_returns_hidden_deny() {
    let authorization_request = request("caller-001", "query.read-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::Deny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(
                authorization_request.clone(),
                AuthorizationEffect::HiddenDeny,
            )
            .unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy)
        .decide(&authorization_request)
        .unwrap();

    assert_eq!(AuthorizationEffect::HiddenDeny, decision.effect());
    assert_eq!("exact-rule", decision.trace().reason_code());
}

#[test]
fn missing_rule_returns_configured_default_denial_effect() {
    let visible_default = SecurityEngine::new(policy(AuthorizationEffect::Deny))
        .decide(&request(
            "caller-001",
            "query.read-record",
            "object-001",
            None,
        ))
        .unwrap();
    let hidden_default = SecurityEngine::new(policy(AuthorizationEffect::HiddenDeny))
        .decide(&request(
            "caller-001",
            "query.read-record",
            "object-001",
            None,
        ))
        .unwrap();

    assert_eq!(AuthorizationEffect::Deny, visible_default.effect());
    assert_eq!("default-deny", visible_default.trace().reason_code());
    assert_eq!(AuthorizationEffect::HiddenDeny, hidden_default.effect());
    assert_eq!("default-hidden-deny", hidden_default.trace().reason_code());
}

#[test]
fn exact_rule_for_one_action_does_not_authorize_another_action() {
    let allowed_request = request("caller-001", "query.read-record", "object-001", None);
    let denied_request = request("caller-001", "query.write-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(allowed_request, AuthorizationEffect::Allow).unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy).decide(&denied_request).unwrap();

    assert_eq!(AuthorizationEffect::HiddenDeny, decision.effect());
    assert_eq!("default-hidden-deny", decision.trace().reason_code());
}

#[test]
fn exact_rule_for_one_caller_context_does_not_authorize_another_context() {
    let allowed_request = request("caller-001", "query.read-record", "object-001", None);
    let denied_request = request("caller-002", "query.read-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(allowed_request, AuthorizationEffect::Allow).unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy).decide(&denied_request).unwrap();

    assert_eq!(AuthorizationEffect::HiddenDeny, decision.effect());
}

#[test]
fn exact_rule_for_one_object_identifier_does_not_authorize_another_identifier() {
    let allowed_request = request("caller-001", "query.read-record", "object-001", None);
    let denied_request = request("caller-001", "query.read-record", "object-002", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(allowed_request, AuthorizationEffect::Allow).unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy).decide(&denied_request).unwrap();

    assert_eq!(AuthorizationEffect::HiddenDeny, decision.effect());
}

#[test]
fn exact_rule_for_one_subresource_does_not_authorize_sibling_subresource() {
    let allowed_request = request(
        "caller-001",
        "query.read-record",
        "object-001",
        Some("field:status"),
    );
    let denied_request = request(
        "caller-001",
        "query.read-record",
        "object-001",
        Some("field:score"),
    );
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(allowed_request, AuthorizationEffect::Allow).unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy).decide(&denied_request).unwrap();

    assert_eq!(AuthorizationEffect::HiddenDeny, decision.effect());
}

#[test]
fn rule_without_subresource_does_not_authorize_subresource() {
    let parent_request = request("caller-001", "query.read-record", "object-001", None);
    let subresource_request = request(
        "caller-001",
        "query.read-record",
        "object-001",
        Some("field:status"),
    );
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(parent_request, AuthorizationEffect::Allow).unwrap(),
        )
        .unwrap();

    let decision = SecurityEngine::new(policy)
        .decide(&subresource_request)
        .unwrap();

    assert_eq!(AuthorizationEffect::HiddenDeny, decision.effect());
}

#[test]
fn decision_traces_are_present_and_deterministic_under_replay() {
    let authorization_request = request("caller-001", "query.read-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(authorization_request.clone(), AuthorizationEffect::Allow)
                .unwrap(),
        )
        .unwrap();
    let engine = SecurityEngine::new(policy);

    let first = engine.decide(&authorization_request).unwrap();
    let second = engine.decide(&authorization_request).unwrap();

    assert_eq!(first.trace(), second.trace());
    assert_eq!("policy-v1", first.trace().policy_revision());
    assert_eq!("minimal-security", first.trace().provider_id());
    assert_eq!("2026-09-09T10:59:00Z", first.trace().decided_at());
    assert!(first
        .trace()
        .decision_id()
        .starts_with("decision-seed|effect=allow|reason=exact-rule|"));
}

#[test]
fn authorization_decision_semantic_equality_ignores_trace_metadata() {
    let first = AuthorizationDecision::allow(AuthorizationDecisionTrace::new(
        "policy-v1",
        "decision-001",
        "2026-09-09T10:59:00Z",
        "minimal-security",
        "exact-rule",
    ));
    let second = AuthorizationDecision::allow(AuthorizationDecisionTrace::new(
        "policy-v2",
        "decision-002",
        "2026-09-09T11:00:00Z",
        "other-security",
        "default-hidden-deny",
    ));
    let denial = AuthorizationDecision::deny(AuthorizationDecisionTrace::new(
        "policy-v1",
        "decision-003",
        "2026-09-09T11:01:00Z",
        "minimal-security",
        "exact-rule",
    ));

    assert_eq!(first, second);
    assert_ne!(first, denial);
}

#[test]
fn with_rules_constructs_policy_independent_of_rule_order() {
    let allow_request = request("caller-001", "query.read-record", "object-001", None);
    let deny_request = request("caller-001", "query.read-record", "object-002", None);
    let allow_rule =
        ExactAuthorizationRule::new(allow_request.clone(), AuthorizationEffect::Allow).unwrap();
    let deny_rule =
        ExactAuthorizationRule::new(deny_request.clone(), AuthorizationEffect::Deny).unwrap();
    let first_policy = SecurityPolicy::with_rules(
        SecurityPolicyRevision::new("policy-v1"),
        SecurityProviderId::new("minimal-security"),
        AuthorizationEffect::HiddenDeny,
        trace_config(),
        vec![allow_rule.clone(), deny_rule.clone()],
    )
    .unwrap();
    let second_policy = SecurityPolicy::with_rules(
        SecurityPolicyRevision::new("policy-v1"),
        SecurityProviderId::new("minimal-security"),
        AuthorizationEffect::HiddenDeny,
        trace_config(),
        vec![deny_rule, allow_rule],
    )
    .unwrap();

    let first_engine = SecurityEngine::new(first_policy);
    let second_engine = SecurityEngine::new(second_policy);

    assert_eq!(
        first_engine.decide(&allow_request).unwrap().trace(),
        second_engine.decide(&allow_request).unwrap().trace()
    );
    assert_eq!(
        first_engine.decide(&deny_request).unwrap().trace(),
        second_engine.decide(&deny_request).unwrap().trace()
    );
}

#[test]
fn provider_works_through_shared_authorization_provider_trait() {
    fn decide_via_trait<P: AuthorizationProvider<Error = SecurityError>>(
        provider: &P,
        request: &AuthorizationRequest,
    ) -> AuthorizationDecision {
        provider.decide(request).unwrap()
    }

    let authorization_request = request("caller-001", "query.read-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(authorization_request.clone(), AuthorizationEffect::Allow)
                .unwrap(),
        )
        .unwrap();
    let engine = SecurityEngine::new(policy);

    let decision = decide_via_trait(&engine, &authorization_request);

    assert_eq!(AuthorizationEffect::Allow, decision.effect());
}

#[test]
fn malformed_policy_inputs_are_rejected() {
    assert!(matches!(
        SecurityPolicy::new(
            SecurityPolicyRevision::new(""),
            SecurityProviderId::new("minimal-security"),
            AuthorizationEffect::HiddenDeny,
            trace_config(),
        ),
        Err(SecurityError::MalformedPolicy {
            failure: SecurityValidationError::EmptyPolicyRevision
        })
    ));
    assert!(matches!(
        SecurityPolicy::new(
            SecurityPolicyRevision::new("policy-v1"),
            SecurityProviderId::new("minimal-security"),
            AuthorizationEffect::Allow,
            trace_config(),
        ),
        Err(SecurityError::MalformedPolicy {
            failure: SecurityValidationError::DefaultEffectMustDeny
        })
    ));
    assert!(matches!(
        SecurityPolicy::new(
            SecurityPolicyRevision::new("policy-v1"),
            SecurityProviderId::new("minimal-security"),
            AuthorizationEffect::HiddenDeny,
            AuthorizationTraceConfig::new("", "2026-09-09T10:59:00Z"),
        ),
        Err(SecurityError::MalformedPolicy {
            failure: SecurityValidationError::EmptyDecisionIdNamespace
        })
    ));
}

#[test]
fn malformed_target_components_are_rejected() {
    assert!(matches!(
        ExactAuthorizationRule::new(
            request_with_target("caller-001", "query.read-record", "", "asset", "object-001"),
            AuthorizationEffect::Allow,
        ),
        Err(SecurityError::MalformedRule {
            failure: SecurityValidationError::EmptyTargetSourceNamespace
        })
    ));
    assert!(matches!(
        ExactAuthorizationRule::new(
            request_with_target(
                "caller-001",
                "query.read-record",
                "object-runtime",
                "",
                "object-001"
            ),
            AuthorizationEffect::Allow,
        ),
        Err(SecurityError::MalformedRule {
            failure: SecurityValidationError::EmptyTargetObjectType
        })
    ));
    assert!(matches!(
        ExactAuthorizationRule::new(
            request_with_target(
                "caller-001",
                "query.read-record",
                "object-runtime",
                "asset",
                ""
            ),
            AuthorizationEffect::Allow,
        ),
        Err(SecurityError::MalformedRule {
            failure: SecurityValidationError::EmptyTargetObjectIdentifier
        })
    ));
}

#[test]
fn malformed_rule_inputs_and_duplicates_are_rejected() {
    assert!(matches!(
        ExactAuthorizationRule::new(
            request("", "query.read-record", "object-001", None),
            AuthorizationEffect::Allow,
        ),
        Err(SecurityError::MalformedRule {
            failure: SecurityValidationError::EmptyCallerPermissionContext
        })
    ));
    assert!(matches!(
        ExactAuthorizationRule::new(
            request("caller-001", "", "object-001", None),
            AuthorizationEffect::Allow,
        ),
        Err(SecurityError::MalformedRule {
            failure: SecurityValidationError::EmptyPermissionAction
        })
    ));
    assert!(matches!(
        ExactAuthorizationRule::new(
            request("caller-001", "query.read-record", "object-001", Some("")),
            AuthorizationEffect::Allow,
        ),
        Err(SecurityError::MalformedRule {
            failure: SecurityValidationError::EmptyTargetSubresource
        })
    ));

    let authorization_request = request("caller-001", "query.read-record", "object-001", None);
    let mut policy = policy(AuthorizationEffect::HiddenDeny);
    policy
        .add_exact_rule(
            ExactAuthorizationRule::new(authorization_request.clone(), AuthorizationEffect::Allow)
                .unwrap(),
        )
        .unwrap();
    assert!(matches!(
        policy.add_exact_rule(
            ExactAuthorizationRule::new(authorization_request, AuthorizationEffect::Deny).unwrap(),
        ),
        Err(SecurityError::DuplicateRule)
    ));
    assert!(matches!(
        SecurityPolicy::with_rules(
            SecurityPolicyRevision::new("policy-v1"),
            SecurityProviderId::new("minimal-security"),
            AuthorizationEffect::HiddenDeny,
            trace_config(),
            vec![
                ExactAuthorizationRule::new(
                    request("caller-001", "query.read-record", "object-002", None),
                    AuthorizationEffect::Allow,
                )
                .unwrap(),
                ExactAuthorizationRule::new(
                    request("caller-001", "query.read-record", "object-002", None),
                    AuthorizationEffect::Deny,
                )
                .unwrap(),
            ],
        ),
        Err(SecurityError::DuplicateRule)
    ));
}

#[test]
fn crate_manifest_depends_only_on_runtime_contracts() {
    let manifest = include_str!("../Cargo.toml");

    assert!(manifest.contains("open-eqms-runtime-contracts"));
    for forbidden_dependency in [
        "open-eqms-object-runtime",
        "open-eqms-event-engine",
        "open-eqms-transaction-engine",
        "open-eqms-query-engine",
        "open-eqms-demo",
        "content",
        "plugins",
        "tokio",
        "async-std",
        "rusqlite",
        "postgres",
    ] {
        assert!(
            !manifest.contains(forbidden_dependency),
            "forbidden dependency found: {forbidden_dependency}"
        );
    }

    let dependency_lines = manifest
        .lines()
        .skip_while(|line| *line != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        dependency_lines,
        ["open-eqms-runtime-contracts = { path = \"../runtime-contracts\" }"]
    );
}

#[test]
fn production_sources_do_not_implement_out_of_scope_security_capabilities() {
    for (file, source) in [
        ("lib.rs", include_str!("lib.rs")),
        ("policy.rs", include_str!("policy.rs")),
        ("types.rs", include_str!("types.rs")),
        ("errors.rs", include_str!("errors.rs")),
    ] {
        for forbidden in [
            "passwordhash",
            "password_hash",
            "verify_password",
            "oauth",
            "oidc",
            "saml",
            "ldap",
            "active directory",
            "sign(",
            "verify(",
            "encrypt",
            "decrypt",
            "Tcp",
            "Udp",
            "tokio",
            "async fn",
            "std::time::SystemTime",
            "std::time::Instant",
            "chrono",
            "open_eqms_object_runtime",
            "open_eqms_event_engine",
            "open_eqms_transaction_engine",
            "open_eqms_query_engine",
        ] {
            assert!(
                !source
                    .to_ascii_lowercase()
                    .contains(&forbidden.to_ascii_lowercase()),
                "{file} contains out-of-scope token {forbidden}"
            );
        }
    }
}
