//! Query Engine data contracts for SPEC-004.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::limits::ExecutionLimits;

pub use open_eqms_runtime_contracts::{
    AuthorizationEffect, AuthorizationProvider, AuthorizationRequest, AuthorizationTarget,
    CallerPermissionContext, ConsistencyBoundary, ConsistencyBoundaryUnavailable,
    ConsistencyBoundaryUnavailableReason, ConsistencySlot, PermissionAction, PropertyValue,
    PropertyValueKind, StableConsistencyMarker, Version,
};

/// Structured query definition for finite one-shot queries.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryDefinition {
    /// Source reference this query targets.
    pub source: QuerySourceRef,
    /// Temporal scope the query is defined against.
    pub temporal_scope: TemporalScope,
    /// Optional predicate tree.
    pub predicate: Option<Predicate>,
    /// Projection for returned fields.
    pub projection: Projection,
    /// Stable sort specifications.
    pub sort: Vec<SortSpec>,
    /// Optional grouping specification.
    pub group: Option<GroupSpec>,
    /// Aggregation specifications.
    pub aggregations: Vec<AggregationSpec>,
    /// Optional bounded traversal shape.
    pub traversal: Option<TraversalSpec>,
    /// Whether an incomplete result is acceptable to the caller.
    pub partial_result_policy: PartialResultPolicy,
    /// Caller-requested presentation shape for the result.
    pub presentation_type: PresentationType,
    /// Execution controls for finite one-shot evaluation.
    pub execution_limits: ExecutionLimits,
}

impl QueryDefinition {
    /// Creates a new query definition with unbounded execution controls.
    pub fn new(
        source: QuerySourceRef,
        temporal_scope: TemporalScope,
        traversal: Option<TraversalSpec>,
        partial_result_policy: PartialResultPolicy,
        presentation_type: PresentationType,
    ) -> Self {
        Self {
            source,
            temporal_scope,
            predicate: None,
            projection: Projection::AllFields,
            sort: Vec::new(),
            group: None,
            aggregations: Vec::new(),
            traversal,
            partial_result_policy,
            presentation_type,
            execution_limits: ExecutionLimits::unbounded(),
        }
    }

    /// Replaces the optional predicate tree.
    pub fn with_predicate(mut self, predicate: Predicate) -> Self {
        self.predicate = Some(predicate);
        self
    }

    /// Replaces the projection.
    pub fn with_projection(mut self, projection: Projection) -> Self {
        self.projection = projection;
        self
    }

    /// Replaces the stable sort specifications.
    pub fn with_sort(mut self, sort: Vec<SortSpec>) -> Self {
        self.sort = sort;
        self
    }

    /// Replaces the optional grouping specification.
    pub fn with_group(mut self, group: GroupSpec) -> Self {
        self.group = Some(group);
        self
    }

    /// Replaces the aggregation specifications.
    pub fn with_aggregations(mut self, aggregations: Vec<AggregationSpec>) -> Self {
        self.aggregations = aggregations;
        self
    }

    /// Replaces the execution controls.
    pub fn with_execution_limits(mut self, execution_limits: ExecutionLimits) -> Self {
        self.execution_limits = execution_limits;
        self
    }
}

/// Opaque reference to a declared query source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuerySourceRef(String);

impl QuerySourceRef {
    /// Creates a query source reference from a caller-supplied token.
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Returns the opaque source token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the source token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for QuerySourceRef {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Query capability that may or may not be supported by a source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceCapability {
    /// Non-current temporal scopes.
    TemporalScope,
    /// Relationship traversal.
    Traversal,
    /// Predicate evaluation.
    Predicate,
    /// Selected-field projection.
    Projection,
    /// Sorting.
    Sorting,
    /// Grouping.
    Grouping,
    /// Aggregation structure.
    Aggregation,
}

impl std::fmt::Display for SourceCapability {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::TemporalScope => "temporal scope",
            Self::Traversal => "traversal",
            Self::Predicate => "predicate",
            Self::Projection => "projection",
            Self::Sorting => "sorting",
            Self::Grouping => "grouping",
            Self::Aggregation => "aggregation",
        })
    }
}

/// Declared query capabilities for one source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCapabilities {
    /// Supports non-current temporal scopes.
    pub temporal_scope: bool,
    /// Supports relationship traversal.
    pub traversal: bool,
    /// Supports predicates.
    pub predicates: bool,
    /// Supports selected-field projection.
    pub projection: bool,
    /// Supports sorting.
    pub sorting: bool,
    /// Supports grouping.
    pub grouping: bool,
    /// Supports aggregation structures.
    pub aggregation: bool,
}

impl SourceCapabilities {
    /// Declares no optional query capabilities.
    pub const fn none() -> Self {
        Self {
            temporal_scope: false,
            traversal: false,
            predicates: false,
            projection: false,
            sorting: false,
            grouping: false,
            aggregation: false,
        }
    }

    /// Declares every optional query capability in the approved partial scope.
    pub const fn all() -> Self {
        Self {
            temporal_scope: true,
            traversal: true,
            predicates: true,
            projection: true,
            sorting: true,
            grouping: true,
            aggregation: true,
        }
    }
}

impl Default for SourceCapabilities {
    fn default() -> Self {
        Self::none()
    }
}

/// Deterministic in-memory ordering key for query result ordering tests and contracts.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct StableOrderingKey {
    segments: Vec<String>,
}

impl StableOrderingKey {
    /// Creates a stable ordering key from opaque ordered segments.
    pub fn new(segments: Vec<String>) -> Self {
        Self { segments }
    }

    /// Returns the ordered key segments.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Reports whether the key has no segments.
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

/// Caller-supplied schema description used for structural field validation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QuerySchema {
    /// Fields keyed by language-neutral field name.
    pub fields: BTreeMap<String, PropertyValueKind>,
}

/// Opaque identity for one completed Query Result package.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct QueryResultId(String);

impl QueryResultId {
    /// Creates a query-result identity from a caller-supplied opaque token.
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Returns the opaque query-result identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the query-result identity token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for QueryResultId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Current schema version for SavedQuery metadata records.
pub const CURRENT_SAVED_QUERY_SCHEMA_VERSION: u32 = 1;

/// Opaque identity for a saved reusable query definition.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SavedQueryId(String);

impl SavedQueryId {
    /// Creates a SavedQuery identity from a caller-supplied opaque token.
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Returns the opaque SavedQuery identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the SavedQuery identity token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for SavedQueryId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Review status for reusable query definitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum SavedQueryValidationStatus {
    /// Draft query not approved for regulated use.
    Draft,
    /// Reviewed query not yet approved for regulated use.
    Reviewed,
    /// Query approved for regulated reuse.
    ApprovedForRegulatedUse,
    /// Retired query retained for replay and traceability.
    Retired,
}

/// Change-history entry for a SavedQuery definition.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SavedQueryChange {
    /// Actor token responsible for the change.
    pub actor: String,
    /// Caller-supplied timestamp token for the change.
    pub changed_at: String,
    /// Opaque change reason or note.
    pub reason: String,
}

impl SavedQueryChange {
    /// Creates a SavedQuery change-history entry.
    pub fn new(
        actor: impl Into<String>,
        changed_at: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            actor: actor.into(),
            changed_at: changed_at.into(),
            reason: reason.into(),
        }
    }
}

/// Versioned reusable query definition.
#[derive(Clone, Debug, PartialEq)]
pub struct SavedQueryDefinition {
    /// SavedQuery metadata schema version.
    pub schema_version: u32,
    /// SavedQuery identity.
    pub id: SavedQueryId,
    /// Human-readable name.
    pub name: String,
    /// Version of this saved query definition.
    pub version: Version,
    /// Non-empty owner token.
    pub owner: String,
    /// Access-permission scope that governs this saved query.
    pub access_permission_scope: String,
    /// Review status for regulated reuse.
    pub validation_status: SavedQueryValidationStatus,
    /// Complete change history for this saved query version.
    pub change_history: Vec<SavedQueryChange>,
    /// Structural query definition to execute.
    pub query_definition: QueryDefinition,
}

impl SavedQueryDefinition {
    /// Creates a SavedQuery definition using the current metadata schema version.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: SavedQueryId,
        name: impl Into<String>,
        version: Version,
        owner: impl Into<String>,
        access_permission_scope: impl Into<String>,
        validation_status: SavedQueryValidationStatus,
        change_history: Vec<SavedQueryChange>,
        query_definition: QueryDefinition,
    ) -> Self {
        Self {
            schema_version: CURRENT_SAVED_QUERY_SCHEMA_VERSION,
            id,
            name: name.into(),
            version,
            owner: owner.into(),
            access_permission_scope: access_permission_scope.into(),
            validation_status,
            change_history,
            query_definition,
        }
    }

    /// Builds the neutral authorization target for this saved query.
    pub fn authorization_target(&self) -> AuthorizationTarget {
        saved_query_authorization_target(&self.id)
    }
}

/// Request to execute one saved query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedQueryExecutionRequest {
    /// SavedQuery identity to retrieve.
    pub saved_query_id: SavedQueryId,
    /// Opaque caller/permission context supplied by Security.
    pub caller_context: CallerPermissionContext,
    /// Caller-supplied result identity used for deterministic replay.
    pub result_id: QueryResultId,
}

impl SavedQueryExecutionRequest {
    /// Creates a SavedQuery execution request.
    pub fn new(
        saved_query_id: SavedQueryId,
        caller_context: CallerPermissionContext,
        result_id: QueryResultId,
    ) -> Self {
        Self {
            saved_query_id,
            caller_context,
            result_id,
        }
    }
}

/// Builds the neutral authorization target for a SavedQuery identity.
pub fn saved_query_authorization_target(saved_query_id: &SavedQueryId) -> AuthorizationTarget {
    AuthorizationTarget::new("query-engine", "saved-query", saved_query_id.as_str())
}

/// Request for one finite, one-shot query execution.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryExecutionRequest {
    /// Structured query definition to execute.
    pub query: QueryDefinition,
    /// Opaque caller/permission context supplied by Security.
    pub caller_context: CallerPermissionContext,
    /// Caller-supplied result identity used for deterministic replay.
    pub result_id: QueryResultId,
}

impl QueryExecutionRequest {
    /// Creates a one-shot execution request.
    pub fn new(
        query: QueryDefinition,
        caller_context: CallerPermissionContext,
        result_id: QueryResultId,
    ) -> Self {
        Self {
            query,
            caller_context,
            result_id,
        }
    }
}

/// Candidate record supplied by a registered query source.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryRecord {
    /// Source reference that produced this record.
    pub source: QuerySourceRef,
    /// Source-local record type token.
    pub record_type: String,
    /// Source-local record identity token.
    pub record_id: String,
    /// Deterministic source-provided ordering key.
    pub ordering_key: StableOrderingKey,
    /// Queryable fields keyed by language-neutral field name.
    pub fields: BTreeMap<String, PropertyValue>,
    /// Relevant source timestamps keyed by language-neutral timestamp name.
    pub timestamps: BTreeMap<String, String>,
    /// Optional source revision or version marker.
    pub source_version: Option<Version>,
    /// Optional permission-scope token for provenance.
    pub permission_scope: Option<String>,
    /// Optional source integrity token.
    pub integrity: Option<String>,
    /// Classification assigned by the source provider and preserved by Query Engine.
    pub classification: ResultClassification,
}

impl QueryRecord {
    /// Creates a source-fact record from required source, type, identity, ordering, and fields.
    pub fn source_fact(
        source: QuerySourceRef,
        record_type: impl Into<String>,
        record_id: impl Into<String>,
        ordering_key: StableOrderingKey,
        fields: BTreeMap<String, PropertyValue>,
    ) -> Self {
        Self {
            source,
            record_type: record_type.into(),
            record_id: record_id.into(),
            ordering_key,
            fields,
            timestamps: BTreeMap::new(),
            source_version: None,
            permission_scope: None,
            integrity: None,
            classification: ResultClassification::SourceFact,
        }
    }

    /// Returns a copy with timestamp provenance replaced.
    pub fn with_timestamps(mut self, timestamps: BTreeMap<String, String>) -> Self {
        self.timestamps = timestamps;
        self
    }

    /// Returns a copy with a source version marker attached.
    pub fn with_source_version(mut self, source_version: Version) -> Self {
        self.source_version = Some(source_version);
        self
    }

    /// Returns a copy with a permission-scope provenance token attached.
    pub fn with_permission_scope(mut self, permission_scope: impl Into<String>) -> Self {
        self.permission_scope = Some(permission_scope.into());
        self
    }

    /// Returns a copy with a source integrity token attached.
    pub fn with_integrity(mut self, integrity: impl Into<String>) -> Self {
        self.integrity = Some(integrity.into());
        self
    }

    /// Returns a copy with a non-default classification.
    pub fn with_classification(mut self, classification: ResultClassification) -> Self {
        self.classification = classification;
        self
    }

    /// Builds the neutral authorization target for this source record.
    pub fn authorization_target(&self) -> AuthorizationTarget {
        AuthorizationTarget::new(&self.source.0, &self.record_type, &self.record_id)
    }
}

/// Classification carried by every result item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum ResultClassification {
    /// Direct record from an authoritative source.
    SourceFact,
    /// Value already computed by a registered source.
    ComputedValue,
    /// Statistical relationship that must never be treated as causation.
    StatisticalCorrelation,
    /// AI-generated hypothesis supplied by an upstream adapter and preserved as such.
    AiGeneratedHypothesis,
    /// Human-approved conclusion supplied by an authoritative source.
    HumanApprovedConclusion,
}

/// Provenance envelope attached to one normalized result item.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultProvenance {
    /// Query source that supplied the contributing records.
    pub source: QuerySourceRef,
    /// Source-local record type, when a single type describes the contribution.
    pub source_record_type: Option<String>,
    /// Source-local record identities that contributed to the result item.
    pub source_record_ids: BTreeSet<String>,
    /// Query definition applied to produce this result item.
    pub query_definition: QueryDefinition,
    /// Relevant source timestamps retained from contributing records.
    pub timestamps: BTreeMap<String, String>,
    /// Optional source revision or version marker.
    pub source_version: Option<Version>,
    /// Optional permission-scope provenance token.
    pub permission_scope: Option<String>,
    /// Optional source integrity token.
    pub integrity: Option<String>,
    /// Optional access-restriction token disclosed by policy-aware sources.
    pub access_restriction: Option<String>,
}

/// One normalized result item.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryResultItem {
    /// Deterministic ordering key for this item.
    pub ordering_key: StableOrderingKey,
    /// Projected or aggregated result fields.
    pub fields: BTreeMap<String, PropertyValue>,
    /// Exactly one classification tag for this item.
    pub classification: ResultClassification,
    /// Provenance for the data used to produce this item.
    pub provenance: ResultProvenance,
}

/// Completeness state for one finite query execution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QueryCompleteness {
    /// All requested source components were available.
    Complete,
    /// The result is explicit about incomplete execution.
    Incomplete {
        /// Machine-readable incompleteness reason token.
        reason: String,
    },
}

/// Completed finite Query Result package.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryResult {
    /// Immutable result package identity.
    pub id: QueryResultId,
    /// Result package version.
    pub version: Version,
    /// Query definition executed to produce this result.
    pub query_definition: QueryDefinition,
    /// Consistency boundary fixed at the start of execution.
    pub consistency_boundary: ConsistencyBoundary,
    /// Presentation type requested for this result package.
    pub presentation_type: PresentationType,
    /// Normalized result items.
    pub items: Vec<QueryResultItem>,
    /// Completeness state of this execution.
    pub completeness: QueryCompleteness,
}

/// Opaque identity for one Context Package.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ContextPackageId(String);

impl ContextPackageId {
    /// Creates a Context Package identity from a caller-supplied opaque token.
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Returns the opaque Context Package identity token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the Context Package identity token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Display for ContextPackageId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Request to build one bounded Context Package from an ordinary query execution.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextPackageRequest {
    /// Context Package identity supplied by the caller for deterministic replay.
    pub package_id: ContextPackageId,
    /// Query execution used to gather the package's evidence entries.
    pub execution_request: QueryExecutionRequest,
    /// Maximum number of evidence entries allowed in the package.
    pub max_items: usize,
    /// Maximum relationship depth represented by this package request.
    pub max_depth: usize,
}

impl ContextPackageRequest {
    /// Creates a Context Package request.
    pub fn new(
        package_id: ContextPackageId,
        execution_request: QueryExecutionRequest,
        max_items: usize,
        max_depth: usize,
    ) -> Self {
        Self {
            package_id,
            execution_request,
            max_items,
            max_depth,
        }
    }
}

/// Bounded, permission-filtered Context Package for adapter consumption.
#[derive(Clone, Debug, PartialEq)]
pub struct ContextPackage {
    /// Immutable Context Package identity.
    pub id: ContextPackageId,
    /// Context Package version.
    pub version: Version,
    /// Opaque caller/permission context used to build the package.
    pub caller_context: CallerPermissionContext,
    /// Maximum depth requested for related evidence.
    pub max_depth: usize,
    /// Ordinary Query Result whose items form this package's evidence set.
    pub query_result: QueryResult,
}

impl QuerySchema {
    /// Creates a schema description from field kinds.
    pub fn new(fields: BTreeMap<String, PropertyValueKind>) -> Self {
        Self { fields }
    }

    /// Reports whether the schema contains the supplied field.
    pub fn contains_field(&self, field: &str) -> bool {
        self.fields.contains_key(field)
    }
}

/// Temporal scope for a query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemporalScope {
    /// Current available state only.
    Current,
    /// State as of one caller-supplied timestamp token.
    PointInTime {
        /// Caller-supplied timestamp token.
        at: String,
    },
    /// State or facts in a bounded caller-supplied time range.
    TimeRange {
        /// Optional inclusive lower bound token.
        from: Option<String>,
        /// Optional inclusive upper bound token.
        to: Option<String>,
    },
    /// Full available history.
    AllHistory,
}

/// Direction for a bounded relationship traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraversalDirection {
    /// Traverse outward from the starting record.
    Outbound,
    /// Traverse inward toward the starting record.
    Inbound,
    /// Traverse both inward and outward.
    Both,
}

/// Predicate tree over caller-described query fields.
#[derive(Clone, Debug, PartialEq)]
pub enum Predicate {
    /// Always-true predicate.
    AlwaysTrue,
    /// Equality comparison against a shared Runtime value.
    Equals {
        /// Field reference.
        field: String,
        /// Expected value.
        value: PropertyValue,
    },
    /// Field existence predicate.
    Exists {
        /// Field reference.
        field: String,
    },
    /// Inclusive range predicate over a field.
    Range {
        /// Field reference.
        field: String,
        /// Optional lower bound value.
        lower: Option<PropertyValue>,
        /// Optional upper bound value.
        upper: Option<PropertyValue>,
    },
    /// All child predicates must match.
    And(Vec<Predicate>),
    /// At least one child predicate must match.
    Or(Vec<Predicate>),
    /// Negates one child predicate.
    Not(Box<Predicate>),
}

/// Projection of fields to return.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Projection {
    /// Return every available field.
    AllFields,
    /// Return only selected fields.
    SelectedFields(BTreeSet<String>),
}

/// Sort direction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SortDirection {
    /// Ascending order.
    Ascending,
    /// Descending order.
    Descending,
}

/// Stable sort specification for one field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SortSpec {
    /// Field reference.
    pub field: String,
    /// Direction for this field.
    pub direction: SortDirection,
}

impl SortSpec {
    /// Creates a sort specification.
    pub fn new(field: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            field: field.into(),
            direction,
        }
    }
}

/// Grouping specification over one or more fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupSpec {
    /// Field references used as grouping keys.
    pub fields: BTreeSet<String>,
}

impl GroupSpec {
    /// Creates a grouping specification.
    pub fn new(fields: BTreeSet<String>) -> Self {
        Self { fields }
    }
}

/// Deterministic aggregation function shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregationFunction {
    /// Count matching records.
    Count,
    /// Sum over a numeric field.
    Sum,
    /// Average over a numeric field.
    Average,
    /// Minimum value for a field.
    Min,
    /// Maximum value for a field.
    Max,
    /// Distinct value count for a field.
    DistinctCount,
}

/// Aggregation structure only; execution is out of scope for this partial crate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AggregationSpec {
    /// Aggregation function.
    pub function: AggregationFunction,
    /// Optional input field. Must be absent for `Count` and present for other functions.
    pub field: Option<String>,
    /// Output alias.
    pub alias: String,
}

impl AggregationSpec {
    /// Creates an aggregation specification.
    pub fn new(
        function: AggregationFunction,
        field: Option<String>,
        alias: impl Into<String>,
    ) -> Self {
        Self {
            function,
            field,
            alias: alias.into(),
        }
    }
}

/// Bounded relationship traversal structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraversalSpec {
    /// Traversal direction.
    pub direction: TraversalDirection,
    /// Maximum hop depth. Must be greater than zero.
    pub max_depth: usize,
    /// Optional relation-type filters. Empty means every structural relation type.
    pub relation_types: BTreeSet<String>,
}

impl TraversalSpec {
    /// Creates a traversal specification.
    pub fn new(
        direction: TraversalDirection,
        max_depth: usize,
        relation_types: BTreeSet<String>,
    ) -> Self {
        Self {
            direction,
            max_depth,
            relation_types,
        }
    }
}

/// Caller policy for incomplete one-shot results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PartialResultPolicy {
    /// Reject incomplete results.
    RejectPartial,
    /// Allow an explicitly marked incomplete result.
    AllowIncomplete,
}

/// Requested presentation shape for query results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationType {
    /// Plain record set.
    RecordSet,
    /// Linked-object view.
    LinkedObjectView,
    /// Timeline view.
    Timeline,
    /// Chart-ready series.
    ChartReadySeries,
    /// Matrix view.
    Matrix,
    /// Document reference set.
    DocumentReferenceSet,
    /// Evidence package.
    EvidencePackage,
}
