//! Dependency-neutral Runtime data contracts shared by Open-EQMS Runtime engines.
//!
//! This crate contains data contracts only. It intentionally has no dependency
//! on Object Runtime, Event Engine, storage providers, content packages,
//! plugins, validation services, registries, or business semantics.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::fmt;

/// Opaque globally unique identifier for a Runtime Object.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ObjectId(String);

impl ObjectId {
    /// Creates a new opaque Object identifier from a caller-supplied token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier token without assigning any business meaning to it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the identifier token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Monotonic per-object version used for optimistic concurrency.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Version(u64);

impl Version {
    /// Returns the initial version assigned to a newly created Object.
    pub fn initial() -> Self {
        Self(1)
    }

    /// Creates a version from a raw monotonic marker.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw monotonic marker.
    pub fn value(self) -> u64 {
        self.0
    }

    /// Returns the next version after a successful update.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Opaque Unit-of-Work handle representing one physical storage-transaction boundary.
///
/// This handle carries no business meaning, does not imply ordering, and is not a
/// transaction manager. Concrete storage providers own lifecycle behavior for any
/// Unit of Work they choose to participate in.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UnitOfWork(String);

impl UnitOfWork {
    /// Creates a new opaque Unit-of-Work handle from a caller-supplied token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the handle token without assigning any business meaning to it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the handle token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for UnitOfWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Opaque caller/permission context supplied by Security.
///
/// Runtime consumers pass this value through to an authorization provider. They
/// must not inspect, parse, or derive policy decisions from the token.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct CallerPermissionContext(String);

impl CallerPermissionContext {
    /// Creates a caller/permission context from a caller-supplied opaque token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the opaque context token without assigning policy meaning to it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the context token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for CallerPermissionContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Opaque authorization action token supplied to Security.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PermissionAction(String);

impl PermissionAction {
    /// Creates an authorization action from a caller-supplied opaque token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the opaque action token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the action token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for PermissionAction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Opaque target reference evaluated by Security for one authorization action.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AuthorizationTarget {
    source_namespace: String,
    object_type: String,
    object_identifier: String,
    subresource: Option<String>,
}

impl AuthorizationTarget {
    /// Creates a target reference without a subresource component.
    pub fn new(
        source_namespace: impl Into<String>,
        object_type: impl Into<String>,
        object_identifier: impl Into<String>,
    ) -> Self {
        Self {
            source_namespace: source_namespace.into(),
            object_type: object_type.into(),
            object_identifier: object_identifier.into(),
            subresource: None,
        }
    }

    /// Returns a copy of this target with a subresource component attached.
    pub fn with_subresource(mut self, subresource: impl Into<String>) -> Self {
        self.subresource = Some(subresource.into());
        self
    }

    /// Returns the source or namespace component that owns this record.
    pub fn source_namespace(&self) -> &str {
        &self.source_namespace
    }

    /// Returns the object-type component of the target reference.
    pub fn object_type(&self) -> &str {
        &self.object_type
    }

    /// Returns the source-local object identifier component.
    pub fn object_identifier(&self) -> &str {
        &self.object_identifier
    }

    /// Returns the optional field, attachment, or evidence-level subresource token.
    pub fn subresource(&self) -> Option<&str> {
        self.subresource.as_deref()
    }

    /// Reports whether every required target component is populated.
    pub fn has_required_components(&self) -> bool {
        !self.source_namespace.is_empty()
            && !self.object_type.is_empty()
            && !self.object_identifier.is_empty()
    }
}

/// Complete authorization request evaluated by an injected provider.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AuthorizationRequest {
    context: CallerPermissionContext,
    action: PermissionAction,
    target: AuthorizationTarget,
}

impl AuthorizationRequest {
    /// Creates an authorization request from opaque context, action, and target values.
    pub fn new(
        context: CallerPermissionContext,
        action: PermissionAction,
        target: AuthorizationTarget,
    ) -> Self {
        Self {
            context,
            action,
            target,
        }
    }

    /// Returns the opaque caller/permission context.
    pub fn context(&self) -> &CallerPermissionContext {
        &self.context
    }

    /// Returns the opaque authorization action.
    pub fn action(&self) -> &PermissionAction {
        &self.action
    }

    /// Returns the target reference for this request.
    pub fn target(&self) -> &AuthorizationTarget {
        &self.target
    }
}

/// Authorization result decided by Security.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum AuthorizationEffect {
    /// The caller may observe the target for the requested action.
    Allow,
    /// The caller is explicitly denied for the requested action.
    Deny,
    /// The caller is denied and the target's existence must not be disclosed.
    HiddenDeny,
}

/// Minimal operational trace envelope carried with each authorization decision.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct AuthorizationDecisionTrace {
    policy_revision: String,
    decision_id: String,
    decided_at: String,
    provider_id: String,
    reason_code: String,
}

impl AuthorizationDecisionTrace {
    /// Creates a trace envelope supplied by the concrete authorization provider.
    pub fn new(
        policy_revision: impl Into<String>,
        decision_id: impl Into<String>,
        decided_at: impl Into<String>,
        provider_id: impl Into<String>,
        reason_code: impl Into<String>,
    ) -> Self {
        Self {
            policy_revision: policy_revision.into(),
            decision_id: decision_id.into(),
            decided_at: decided_at.into(),
            provider_id: provider_id.into(),
            reason_code: reason_code.into(),
        }
    }

    /// Returns the policy or ruleset revision reference.
    pub fn policy_revision(&self) -> &str {
        &self.policy_revision
    }

    /// Returns the provider-supplied decision or evaluation identifier.
    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    /// Returns the provider-supplied decision timestamp token.
    pub fn decided_at(&self) -> &str {
        &self.decided_at
    }

    /// Returns the provider identifier that produced the decision.
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns the internal provider reason code.
    pub fn reason_code(&self) -> &str {
        &self.reason_code
    }
}

/// Authorization decision plus its operational trace envelope.
#[derive(Clone, Debug)]
pub struct AuthorizationDecision {
    effect: AuthorizationEffect,
    trace: AuthorizationDecisionTrace,
}

impl AuthorizationDecision {
    /// Creates a decision from an effect and provider-supplied trace envelope.
    pub fn new(effect: AuthorizationEffect, trace: AuthorizationDecisionTrace) -> Self {
        Self { effect, trace }
    }

    /// Creates an allow decision.
    pub fn allow(trace: AuthorizationDecisionTrace) -> Self {
        Self::new(AuthorizationEffect::Allow, trace)
    }

    /// Creates an explicit deny decision.
    pub fn deny(trace: AuthorizationDecisionTrace) -> Self {
        Self::new(AuthorizationEffect::Deny, trace)
    }

    /// Creates a hidden-deny decision.
    pub fn hidden_deny(trace: AuthorizationDecisionTrace) -> Self {
        Self::new(AuthorizationEffect::HiddenDeny, trace)
    }

    /// Returns the decision effect chosen by Security.
    pub fn effect(&self) -> AuthorizationEffect {
        self.effect
    }

    /// Returns the operational trace envelope carried with the decision.
    pub fn trace(&self) -> &AuthorizationDecisionTrace {
        &self.trace
    }
}

impl PartialEq for AuthorizationDecision {
    fn eq(&self, other: &Self) -> bool {
        self.effect == other.effect
    }
}

impl Eq for AuthorizationDecision {}

/// Authorization request and decision captured together for semantic comparison.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationEvaluation {
    request: AuthorizationRequest,
    decision: AuthorizationDecision,
}

impl AuthorizationEvaluation {
    /// Creates an authorization evaluation record.
    pub fn new(request: AuthorizationRequest, decision: AuthorizationDecision) -> Self {
        Self { request, decision }
    }

    /// Returns the evaluated authorization request.
    pub fn request(&self) -> &AuthorizationRequest {
        &self.request
    }

    /// Returns the decision produced for the request.
    pub fn decision(&self) -> &AuthorizationDecision {
        &self.decision
    }
}

/// Contract implemented by a provider that owns authorization policy evaluation.
///
/// The shared contracts crate defines only the dependency-neutral trait shape.
/// It does not implement Security, roles, capabilities, policy lookup, or
/// denial-disclosure policy.
pub trait AuthorizationProvider {
    /// Provider-specific authorization error type.
    type Error;

    /// Decides one authorization request.
    fn decide(&self, request: &AuthorizationRequest) -> Result<AuthorizationDecision, Self::Error>;
}

/// Contract implemented by a provider that owns Unit-of-Work lifecycle behavior.
///
/// The shared contracts crate defines the shape only. It does not begin, commit,
/// roll back, store, coordinate, or globally register Units of Work.
pub trait UnitOfWorkLifecycle {
    /// Provider-specific lifecycle error type.
    type Error;

    /// Begins participation in a Unit of Work owned by the concrete provider.
    fn begin_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error>;

    /// Commits the provider-owned work staged against the supplied handle.
    fn commit_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error>;

    /// Rolls back the provider-owned work staged against the supplied handle.
    fn rollback_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error>;
}

/// Contract implemented by a provider that can stage writes into a Unit of Work.
///
/// The `Write` type is owned by the participating component or storage provider.
/// This keeps Runtime contracts neutral and prevents this crate from importing
/// engine-specific record types.
pub trait UnitOfWorkStaging<Write> {
    /// Provider-specific staging error type.
    type Error;

    /// Stages one provider-owned write against the supplied Unit-of-Work handle.
    fn stage_unit_of_work_write(
        &mut self,
        unit_of_work: &UnitOfWork,
        write: Write,
    ) -> Result<(), Self::Error>;
}

/// Stable marker used by one component slot in a `ConsistencyBoundary`.
///
/// The namespace scopes the marker so superficially similar values from
/// different stores, deployments, or sources cannot compare equal by accident.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct StableConsistencyMarker {
    namespace: String,
    marker: String,
}

impl StableConsistencyMarker {
    /// Creates a stable marker with an explicit namespace.
    pub fn new(namespace: impl Into<String>, marker: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            marker: marker.into(),
        }
    }

    /// Returns the marker namespace.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Returns the marker value within its namespace.
    pub fn marker(&self) -> &str {
        &self.marker
    }

    /// Reports whether both namespace and marker are populated.
    pub fn has_required_components(&self) -> bool {
        !self.namespace.is_empty() && !self.marker.is_empty()
    }
}

/// Structured reason a requested consistency-boundary slot could not be produced.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ConsistencyBoundaryUnavailableReason {
    /// The owning component does not yet expose a public API for this marker.
    ComponentApiNotAvailable,
    /// The owning source is transiently unreachable.
    TransientlyUnreachable,
    /// The owning source permanently does not support this boundary component.
    PermanentlyUnsupported,
    /// The caller lacks access to the information required to produce the marker.
    AuthorizationRestricted,
    /// A previously used marker was lost or is no longer verifiable.
    MarkerLostOrUnverifiable,
    /// The source produced only a weaker consistency guarantee than requested.
    WeakerConsistencyGuarantee,
}

impl ConsistencyBoundaryUnavailableReason {
    /// Returns the stable canonical token for this reason.
    pub fn canonical_token(self) -> &'static str {
        match self {
            Self::ComponentApiNotAvailable => "component-api-not-available",
            Self::TransientlyUnreachable => "transiently-unreachable",
            Self::PermanentlyUnsupported => "permanently-unsupported",
            Self::AuthorizationRestricted => "authorization-restricted",
            Self::MarkerLostOrUnverifiable => "marker-lost-or-unverifiable",
            Self::WeakerConsistencyGuarantee => "weaker-consistency-guarantee",
        }
    }
}

/// Structured unavailable state for one requested consistency-boundary slot.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConsistencyBoundaryUnavailable {
    reason: ConsistencyBoundaryUnavailableReason,
    detail: String,
}

impl ConsistencyBoundaryUnavailable {
    /// Creates an unavailable marker with a structured reason and opaque detail token.
    pub fn new(reason: ConsistencyBoundaryUnavailableReason, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
        }
    }

    /// Returns the structured unavailable reason.
    pub fn reason(&self) -> ConsistencyBoundaryUnavailableReason {
        self.reason
    }

    /// Returns the opaque unavailable detail token.
    pub fn detail(&self) -> &str {
        &self.detail
    }
}

/// Three-state consistency-boundary slot.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ConsistencySlot<T> {
    /// The component was not needed for this boundary.
    NotRequested,
    /// The component was needed, but no stable marker could be supplied.
    Unavailable(ConsistencyBoundaryUnavailable),
    /// The component was needed and supplied a stable marker.
    Available(T),
}

impl<T> ConsistencySlot<T> {
    /// Reports whether this slot carries an available value.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available(_))
    }

    /// Reports whether this slot was not requested.
    pub fn is_not_requested(&self) -> bool {
        matches!(self, Self::NotRequested)
    }

    /// Reports whether this slot was requested but unavailable.
    pub fn is_unavailable(&self) -> bool {
        matches!(self, Self::Unavailable(_))
    }
}

/// Shared consistency boundary attached to reproducible Runtime read results.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ConsistencyBoundary {
    object_runtime: ConsistencySlot<StableConsistencyMarker>,
    event_engine: ConsistencySlot<StableConsistencyMarker>,
    transaction_engine: ConsistencySlot<StableConsistencyMarker>,
    computed_value_source: ConsistencySlot<StableConsistencyMarker>,
    concept_mapping: ConsistencySlot<StableConsistencyMarker>,
    query_definition: ConsistencySlot<StableConsistencyMarker>,
}

impl ConsistencyBoundary {
    /// Creates a boundary whose slots are all not requested.
    pub fn empty() -> Self {
        Self {
            object_runtime: ConsistencySlot::NotRequested,
            event_engine: ConsistencySlot::NotRequested,
            transaction_engine: ConsistencySlot::NotRequested,
            computed_value_source: ConsistencySlot::NotRequested,
            concept_mapping: ConsistencySlot::NotRequested,
            query_definition: ConsistencySlot::NotRequested,
        }
    }

    /// Returns a copy with the Object Runtime slot replaced.
    pub fn with_object_runtime(mut self, slot: ConsistencySlot<StableConsistencyMarker>) -> Self {
        self.object_runtime = slot;
        self
    }

    /// Returns a copy with the Event Engine slot replaced.
    pub fn with_event_engine(mut self, slot: ConsistencySlot<StableConsistencyMarker>) -> Self {
        self.event_engine = slot;
        self
    }

    /// Returns a copy with the Transaction Engine slot replaced.
    pub fn with_transaction_engine(
        mut self,
        slot: ConsistencySlot<StableConsistencyMarker>,
    ) -> Self {
        self.transaction_engine = slot;
        self
    }

    /// Returns a copy with the Computed Value Source slot replaced.
    pub fn with_computed_value_source(
        mut self,
        slot: ConsistencySlot<StableConsistencyMarker>,
    ) -> Self {
        self.computed_value_source = slot;
        self
    }

    /// Returns a copy with the Concept Mapping slot replaced.
    pub fn with_concept_mapping(mut self, slot: ConsistencySlot<StableConsistencyMarker>) -> Self {
        self.concept_mapping = slot;
        self
    }

    /// Returns a copy with the Query Definition slot replaced.
    pub fn with_query_definition(mut self, slot: ConsistencySlot<StableConsistencyMarker>) -> Self {
        self.query_definition = slot;
        self
    }

    /// Returns the Object Runtime component slot.
    pub fn object_runtime(&self) -> &ConsistencySlot<StableConsistencyMarker> {
        &self.object_runtime
    }

    /// Returns the Event Engine component slot.
    pub fn event_engine(&self) -> &ConsistencySlot<StableConsistencyMarker> {
        &self.event_engine
    }

    /// Returns the Transaction Engine component slot.
    pub fn transaction_engine(&self) -> &ConsistencySlot<StableConsistencyMarker> {
        &self.transaction_engine
    }

    /// Returns the Computed Value Source component slot.
    pub fn computed_value_source(&self) -> &ConsistencySlot<StableConsistencyMarker> {
        &self.computed_value_source
    }

    /// Returns the Concept Mapping component slot.
    pub fn concept_mapping(&self) -> &ConsistencySlot<StableConsistencyMarker> {
        &self.concept_mapping
    }

    /// Returns the Query Definition component slot.
    pub fn query_definition(&self) -> &ConsistencySlot<StableConsistencyMarker> {
        &self.query_definition
    }

    /// Returns the deterministic canonical string representation.
    pub fn canonical_string(&self) -> String {
        let mut output = String::from("open-eqms.consistency-boundary.v1\n");
        append_consistency_slot("object-runtime", &self.object_runtime, &mut output);
        append_consistency_slot("event-engine", &self.event_engine, &mut output);
        append_consistency_slot("transaction-engine", &self.transaction_engine, &mut output);
        append_consistency_slot(
            "computed-value-source",
            &self.computed_value_source,
            &mut output,
        );
        append_consistency_slot("concept-mapping", &self.concept_mapping, &mut output);
        append_consistency_slot("query-definition", &self.query_definition, &mut output);
        output
    }

    /// Returns the deterministic canonical byte representation.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.canonical_string().into_bytes()
    }
}

impl Default for ConsistencyBoundary {
    fn default() -> Self {
        Self::empty()
    }
}

fn append_consistency_slot(
    name: &str,
    slot: &ConsistencySlot<StableConsistencyMarker>,
    output: &mut String,
) {
    output.push_str(name);
    output.push('=');
    match slot {
        ConsistencySlot::NotRequested => output.push_str("not-requested"),
        ConsistencySlot::Unavailable(unavailable) => {
            output.push_str("unavailable;reason=");
            output.push_str(unavailable.reason().canonical_token());
            output.push_str(";detail=");
            append_length_prefixed(unavailable.detail(), output);
        }
        ConsistencySlot::Available(marker) => {
            output.push_str("available;namespace=");
            append_length_prefixed(marker.namespace(), output);
            output.push_str(";marker=");
            append_length_prefixed(marker.marker(), output);
        }
    }
    output.push('\n');
}

fn append_length_prefixed(value: &str, output: &mut String) {
    output.push_str(&value.len().to_string());
    output.push(':');
    output.push_str(value);
}

/// Primitive property or payload value kinds shared by Runtime data contracts.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PropertyValueKind {
    /// User-authored text, optionally tagged by language on the value.
    Text,
    /// Reference to a localization key.
    LocalizedTextKey,
    /// Numeric value.
    Number,
    /// Boolean value.
    Boolean,
    /// Caller-supplied date/time token.
    DateTime,
    /// Enumerated token.
    EnumValue,
    /// Reference to another Object.
    Reference,
    /// Reference to an attachment.
    AttachmentReference,
    /// Binary blob.
    BinaryBlob,
}

/// A tagged primitive Runtime value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    /// User-authored text and optional language tag.
    Text {
        /// Text value.
        value: String,
        /// Optional language tag for user-authored text.
        language: Option<String>,
    },
    /// Reference to a package-supplied localized text key.
    LocalizedTextKey(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Boolean(bool),
    /// Caller-supplied date/time token.
    DateTime(String),
    /// Enumerated token.
    EnumValue(String),
    /// Object reference value.
    Reference(ObjectId),
    /// Attachment reference token.
    AttachmentReference(String),
    /// Binary blob value.
    BinaryBlob(Vec<u8>),
}

impl PropertyValue {
    /// Returns the primitive kind represented by this value.
    pub fn kind(&self) -> PropertyValueKind {
        match self {
            Self::Text { .. } => PropertyValueKind::Text,
            Self::LocalizedTextKey(_) => PropertyValueKind::LocalizedTextKey,
            Self::Number(_) => PropertyValueKind::Number,
            Self::Boolean(_) => PropertyValueKind::Boolean,
            Self::DateTime(_) => PropertyValueKind::DateTime,
            Self::EnumValue(_) => PropertyValueKind::EnumValue,
            Self::Reference(_) => PropertyValueKind::Reference,
            Self::AttachmentReference(_) => PropertyValueKind::AttachmentReference,
            Self::BinaryBlob(_) => PropertyValueKind::BinaryBlob,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorizationDecision, AuthorizationDecisionTrace, AuthorizationEffect,
        AuthorizationEvaluation, AuthorizationProvider, AuthorizationRequest, AuthorizationTarget,
        CallerPermissionContext, ConsistencyBoundary, ConsistencyBoundaryUnavailable,
        ConsistencyBoundaryUnavailableReason, ConsistencySlot, ObjectId, PermissionAction,
        PropertyValue, PropertyValueKind, StableConsistencyMarker, UnitOfWork, UnitOfWorkLifecycle,
        UnitOfWorkStaging, Version,
    };
    use std::collections::BTreeSet;

    #[test]
    fn object_id_preserves_opaque_token_behavior() {
        let id = ObjectId::new("object-001");

        assert_eq!("object-001", id.as_str());
        assert_eq!("object-001", id.to_string());
        assert!(!id.is_empty());
        assert!(ObjectId::new("").is_empty());
    }

    #[test]
    fn object_id_ordering_is_deterministic() {
        let ids = BTreeSet::from([
            ObjectId::new("object-c"),
            ObjectId::new("object-a"),
            ObjectId::new("object-b"),
        ]);
        let ordered = ids.iter().map(ObjectId::as_str).collect::<Vec<_>>();

        assert_eq!(ordered, ["object-a", "object-b", "object-c"]);
    }

    #[test]
    fn version_preserves_monotonic_behavior() {
        let initial = Version::initial();
        let next = initial.next();

        assert_eq!(initial.value(), 1);
        assert_eq!(next, Version::new(2));
        assert!(next > initial);
    }

    #[test]
    fn property_value_kind_mapping_is_backward_compatible() {
        let reference_id = ObjectId::new("target-001");
        let values = [
            PropertyValue::Text {
                value: "text".to_owned(),
                language: Some("en".to_owned()),
            },
            PropertyValue::LocalizedTextKey("label.key".to_owned()),
            PropertyValue::Number(42.0),
            PropertyValue::Boolean(true),
            PropertyValue::DateTime("2026-07-15T10:00:00Z".to_owned()),
            PropertyValue::EnumValue("approved".to_owned()),
            PropertyValue::Reference(reference_id),
            PropertyValue::AttachmentReference("attachment-001".to_owned()),
            PropertyValue::BinaryBlob(vec![1, 2, 3]),
        ];
        let kinds = values.iter().map(PropertyValue::kind).collect::<Vec<_>>();

        assert_eq!(
            kinds,
            [
                PropertyValueKind::Text,
                PropertyValueKind::LocalizedTextKey,
                PropertyValueKind::Number,
                PropertyValueKind::Boolean,
                PropertyValueKind::DateTime,
                PropertyValueKind::EnumValue,
                PropertyValueKind::Reference,
                PropertyValueKind::AttachmentReference,
                PropertyValueKind::BinaryBlob,
            ]
        );
    }

    #[test]
    fn shared_values_clone_and_compare_deterministically() {
        let value = PropertyValue::Reference(ObjectId::new("object-001"));

        assert_eq!(value, value.clone());
        assert_eq!(PropertyValueKind::Reference, value.kind());
    }

    #[test]
    fn unit_of_work_preserves_opaque_token_behavior() {
        let unit_of_work = UnitOfWork::new("uow-001");

        assert_eq!("uow-001", unit_of_work.as_str());
        assert_eq!("uow-001", unit_of_work.to_string());
        assert!(!unit_of_work.is_empty());
        assert!(UnitOfWork::new("").is_empty());
    }

    #[test]
    fn unit_of_work_contracts_are_provider_owned() {
        #[derive(Default)]
        struct TestParticipant {
            lifecycle: Vec<String>,
            staged: Vec<String>,
        }

        impl UnitOfWorkLifecycle for TestParticipant {
            type Error = String;

            fn begin_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error> {
                self.lifecycle
                    .push(format!("begin:{}", unit_of_work.as_str()));
                Ok(())
            }

            fn commit_unit_of_work(
                &mut self,
                unit_of_work: &UnitOfWork,
            ) -> Result<(), Self::Error> {
                self.lifecycle
                    .push(format!("commit:{}", unit_of_work.as_str()));
                Ok(())
            }

            fn rollback_unit_of_work(
                &mut self,
                unit_of_work: &UnitOfWork,
            ) -> Result<(), Self::Error> {
                self.lifecycle
                    .push(format!("rollback:{}", unit_of_work.as_str()));
                Ok(())
            }
        }

        impl UnitOfWorkStaging<String> for TestParticipant {
            type Error = String;

            fn stage_unit_of_work_write(
                &mut self,
                unit_of_work: &UnitOfWork,
                write: String,
            ) -> Result<(), Self::Error> {
                self.staged
                    .push(format!("{}:{}", unit_of_work.as_str(), write));
                Ok(())
            }
        }

        let mut participant = TestParticipant::default();
        let unit_of_work = UnitOfWork::new("uow-001");

        participant.begin_unit_of_work(&unit_of_work).unwrap();
        participant
            .stage_unit_of_work_write(&unit_of_work, "write-001".to_owned())
            .unwrap();
        participant.commit_unit_of_work(&unit_of_work).unwrap();

        assert_eq!(participant.lifecycle, ["begin:uow-001", "commit:uow-001"]);
        assert_eq!(participant.staged, ["uow-001:write-001"]);
    }

    #[test]
    fn authorization_target_names_source_type_record_and_subresource() {
        let target = AuthorizationTarget::new("object-runtime", "calibration", "object-001")
            .with_subresource("field:lifecycle_state");

        assert!(target.has_required_components());
        assert_eq!("object-runtime", target.source_namespace());
        assert_eq!("calibration", target.object_type());
        assert_eq!("object-001", target.object_identifier());
        assert_eq!(Some("field:lifecycle_state"), target.subresource());
    }

    #[test]
    fn authorization_decision_equality_ignores_trace_metadata() {
        let first = AuthorizationDecision::allow(AuthorizationDecisionTrace::new(
            "policy-v1",
            "decision-001",
            "2026-09-09T08:00:00Z",
            "security-test",
            "allow",
        ));
        let second = AuthorizationDecision::allow(AuthorizationDecisionTrace::new(
            "policy-v2",
            "decision-002",
            "2026-09-09T09:00:00Z",
            "security-test",
            "allow",
        ));
        let denial = AuthorizationDecision::deny(AuthorizationDecisionTrace::new(
            "policy-v1",
            "decision-003",
            "2026-09-09T10:00:00Z",
            "security-test",
            "deny",
        ));

        assert_eq!(first, second);
        assert_ne!(first, denial);
        assert_eq!(AuthorizationEffect::Allow, first.effect());
        assert_eq!("policy-v1", first.trace().policy_revision());
    }

    #[test]
    fn authorization_evaluation_equality_includes_request_identity() {
        let context = CallerPermissionContext::new("caller-context");
        let action = PermissionAction::new("query.read-record");
        let request = AuthorizationRequest::new(
            context.clone(),
            action.clone(),
            AuthorizationTarget::new("object-runtime", "asset", "object-001"),
        );
        let same_effect = AuthorizationDecision::allow(AuthorizationDecisionTrace::new(
            "policy-v1",
            "decision-001",
            "2026-09-09T08:00:00Z",
            "security-test",
            "allow",
        ));
        let different_trace = AuthorizationDecision::allow(AuthorizationDecisionTrace::new(
            "policy-v2",
            "decision-002",
            "2026-09-09T09:00:00Z",
            "security-test",
            "allow",
        ));
        let different_request = AuthorizationRequest::new(
            context,
            action,
            AuthorizationTarget::new("object-runtime", "asset", "object-002"),
        );

        assert_eq!(
            AuthorizationEvaluation::new(request.clone(), same_effect.clone()),
            AuthorizationEvaluation::new(request, different_trace)
        );
        assert_ne!(
            AuthorizationEvaluation::new(
                AuthorizationRequest::new(
                    CallerPermissionContext::new("caller-context"),
                    PermissionAction::new("query.read-record"),
                    AuthorizationTarget::new("object-runtime", "asset", "object-001"),
                ),
                same_effect.clone(),
            ),
            AuthorizationEvaluation::new(different_request, same_effect)
        );
    }

    #[test]
    fn authorization_provider_is_injected_contract_only() {
        struct AllowAllProvider;

        impl AuthorizationProvider for AllowAllProvider {
            type Error = String;

            fn decide(
                &self,
                _request: &AuthorizationRequest,
            ) -> Result<AuthorizationDecision, Self::Error> {
                Ok(AuthorizationDecision::allow(
                    AuthorizationDecisionTrace::new(
                        "policy-v1",
                        "decision-001",
                        "2026-09-09T08:00:00Z",
                        "security-test",
                        "allow",
                    ),
                ))
            }
        }

        let request = AuthorizationRequest::new(
            CallerPermissionContext::new("caller-context"),
            PermissionAction::new("query.read-record"),
            AuthorizationTarget::new("event-engine", "event", "event-001"),
        );
        let decision = AllowAllProvider.decide(&request).unwrap();

        assert_eq!(AuthorizationEffect::Allow, decision.effect());
    }

    #[test]
    fn consistency_boundary_slots_are_three_state() {
        let boundary = ConsistencyBoundary::empty()
            .with_object_runtime(ConsistencySlot::Available(StableConsistencyMarker::new(
                "object-store",
                "version:7",
            )))
            .with_event_engine(ConsistencySlot::Unavailable(
                ConsistencyBoundaryUnavailable::new(
                    ConsistencyBoundaryUnavailableReason::TransientlyUnreachable,
                    "offline",
                ),
            ));

        assert!(boundary.object_runtime().is_available());
        assert!(boundary.event_engine().is_unavailable());
        assert!(boundary.transaction_engine().is_not_requested());
    }

    #[test]
    fn consistency_boundary_strict_equality_includes_all_slots() {
        let object_only = ConsistencyBoundary::empty().with_object_runtime(
            ConsistencySlot::Available(StableConsistencyMarker::new("object-store", "version:7")),
        );
        let object_and_event = object_only
            .clone()
            .with_event_engine(ConsistencySlot::Available(StableConsistencyMarker::new(
                "event-store",
                "sequence:9",
            )));

        assert_ne!(object_only, object_and_event);
        assert_eq!(object_only, object_only.clone());
    }

    #[test]
    fn consistency_boundary_canonical_encoding_is_stable_and_ordered() {
        let boundary = ConsistencyBoundary::empty()
            .with_object_runtime(ConsistencySlot::Available(StableConsistencyMarker::new(
                "object-store",
                "version:7",
            )))
            .with_event_engine(ConsistencySlot::Unavailable(
                ConsistencyBoundaryUnavailable::new(
                    ConsistencyBoundaryUnavailableReason::TransientlyUnreachable,
                    "offline",
                ),
            ))
            .with_transaction_engine(ConsistencySlot::Available(StableConsistencyMarker::new(
                "transaction-store",
                "append-index:42",
            )));

        let expected = concat!(
            "open-eqms.consistency-boundary.v1\n",
            "object-runtime=available;namespace=12:object-store;marker=9:version:7\n",
            "event-engine=unavailable;reason=transiently-unreachable;detail=7:offline\n",
            "transaction-engine=available;namespace=17:transaction-store;marker=15:append-index:42\n",
            "computed-value-source=not-requested\n",
            "concept-mapping=not-requested\n",
            "query-definition=not-requested\n",
        );

        assert_eq!(expected, boundary.canonical_string());
        assert_eq!(expected.as_bytes(), boundary.canonical_bytes().as_slice());
    }

    #[test]
    fn runtime_contracts_manifest_has_no_engine_dependencies() {
        let manifest = include_str!("../Cargo.toml");

        for disallowed in [
            "open-eqms-object-runtime",
            "open-eqms-event-engine",
            "open-eqms-transaction-engine",
            "open-eqms-query-engine",
        ] {
            assert!(
                !manifest.contains(disallowed),
                "runtime-contracts must not depend on {disallowed}"
            );
        }
    }
}
