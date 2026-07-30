//! Query Engine data contracts for the approved partial SPEC-004 scope.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

pub use open_eqms_runtime_contracts::{PropertyValue, PropertyValueKind};

/// Structured query definition for finite one-shot queries.
#[derive(Clone, Debug, PartialEq)]
pub struct QueryDefinition {
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
}

impl QueryDefinition {
    /// Creates a new query definition without execution controls.
    pub fn new(
        temporal_scope: TemporalScope,
        traversal: Option<TraversalSpec>,
        partial_result_policy: PartialResultPolicy,
        presentation_type: PresentationType,
    ) -> Self {
        Self {
            temporal_scope,
            predicate: None,
            projection: Projection::AllFields,
            sort: Vec::new(),
            group: None,
            aggregations: Vec::new(),
            traversal,
            partial_result_policy,
            presentation_type,
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
}

/// Caller-supplied schema description used for structural field validation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QuerySchema {
    /// Fields keyed by language-neutral field name.
    pub fields: BTreeMap<String, PropertyValueKind>,
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
