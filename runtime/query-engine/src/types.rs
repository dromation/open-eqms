//! Query Engine data contracts for the approved partial SPEC-004 scope.

use std::collections::BTreeSet;

/// Structured query definition for finite one-shot queries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueryDefinition {
    /// Temporal scope the query is defined against.
    pub temporal_scope: TemporalScope,
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
            traversal,
            partial_result_policy,
            presentation_type,
        }
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
