//! Open-EQMS Query Engine implementation for SPEC-004.
//!
//! This partial crate currently contains only Slices 1-6: query model
//! contracts, validation, source capability contracts, deterministic ordering
//! helpers, structured errors, execution limits, and finite one-shot
//! cancellation state.
//!
//! It intentionally does not implement real Object Runtime, Event Engine, or
//! Transaction Engine adapters; reading or returning real engine data;
//! permission filtering; Security integration; SavedQuery persistence; Context
//! Packages; cursor or continuation-token contracts; ConsistencyBoundary;
//! execution planners; parallel execution; synchronization; Process Engine
//! behavior; Rule Engine behavior; AI-agent behavior; or SPEC-005 work.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod errors;
pub mod types;
pub mod validation;

pub use crate::errors::{QueryEngineError, QueryEngineResult, ValidationError};
pub use crate::types::{
    PartialResultPolicy, PresentationType, QueryDefinition, TemporalScope, TraversalDirection,
    TraversalSpec,
};

#[cfg(test)]
mod tests;
