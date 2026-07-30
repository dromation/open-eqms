//! Execution limit, timeout, and one-shot cancellation contracts.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::errors::{
    cancelled, execution_limit_exceeded, malformed_query, timeout_elapsed, QueryEngineResult,
    ValidationError,
};

/// Deterministic execution limit category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionLimitKind {
    /// Maximum result count.
    ResultCount,
    /// Maximum evaluation step count.
    EvaluationSteps,
}

impl std::fmt::Display for ExecutionLimitKind {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ResultCount => "result count",
            Self::EvaluationSteps => "evaluation steps",
        })
    }
}

/// Finite one-shot query timeout duration represented in milliseconds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueryTimeout {
    milliseconds: u64,
}

impl QueryTimeout {
    /// Creates a non-zero timeout representation.
    pub fn from_millis(milliseconds: u64) -> QueryEngineResult<Self> {
        if milliseconds == 0 {
            return Err(malformed_query(ValidationError::ZeroTimeout));
        }
        Ok(Self { milliseconds })
    }

    /// Returns the timeout duration in milliseconds.
    pub fn milliseconds(self) -> u64 {
        self.milliseconds
    }
}

/// Execution controls for a finite one-shot query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionLimits {
    /// Optional maximum result count.
    pub max_result_count: Option<usize>,
    /// Optional maximum evaluation step count.
    pub max_evaluation_steps: Option<u64>,
    /// Optional timeout representation.
    pub timeout: Option<QueryTimeout>,
}

impl ExecutionLimits {
    /// Creates unbounded execution controls.
    pub const fn unbounded() -> Self {
        Self {
            max_result_count: None,
            max_evaluation_steps: None,
            timeout: None,
        }
    }

    /// Creates execution controls and validates that supplied limits are non-zero.
    pub fn new(
        max_result_count: Option<usize>,
        max_evaluation_steps: Option<u64>,
        timeout: Option<QueryTimeout>,
    ) -> QueryEngineResult<Self> {
        let limits = Self {
            max_result_count,
            max_evaluation_steps,
            timeout,
        };
        validate_execution_limits(&limits)?;
        Ok(limits)
    }
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self::unbounded()
    }
}

/// One-shot cancellation state for finite query evaluation.
#[derive(Clone, Debug, Default)]
pub struct CancellationState {
    cancelled: Arc<AtomicBool>,
}

impl CancellationState {
    /// Creates a fresh non-cancelled state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests cancellation. Once set, the state remains cancelled.
    pub fn request_cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Reports whether cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Returns a structured cancellation error if cancellation has been requested.
    pub fn check_cancelled(&self) -> QueryEngineResult<()> {
        if self.is_cancelled() {
            return Err(cancelled());
        }
        Ok(())
    }
}

/// Validates execution limits against structural rules only.
pub fn validate_execution_limits(limits: &ExecutionLimits) -> QueryEngineResult<()> {
    if matches!(limits.max_result_count, Some(0)) {
        return Err(malformed_query(ValidationError::ZeroResultLimit));
    }
    if matches!(limits.max_evaluation_steps, Some(0)) {
        return Err(malformed_query(ValidationError::ZeroEvaluationStepLimit));
    }
    Ok(())
}

/// Checks a result count against the configured maximum.
pub fn ensure_result_count_within_limit(
    limits: &ExecutionLimits,
    result_count: usize,
) -> QueryEngineResult<()> {
    if limits
        .max_result_count
        .is_some_and(|maximum| result_count > maximum)
    {
        return Err(execution_limit_exceeded(ExecutionLimitKind::ResultCount));
    }
    Ok(())
}

/// Checks an evaluation step count against the configured maximum.
pub fn ensure_evaluation_steps_within_limit(
    limits: &ExecutionLimits,
    evaluation_steps: u64,
) -> QueryEngineResult<()> {
    if limits
        .max_evaluation_steps
        .is_some_and(|maximum| evaluation_steps > maximum)
    {
        return Err(execution_limit_exceeded(
            ExecutionLimitKind::EvaluationSteps,
        ));
    }
    Ok(())
}

/// Checks caller-supplied elapsed milliseconds against a timeout representation.
pub fn ensure_timeout_not_elapsed(
    timeout: QueryTimeout,
    elapsed_milliseconds: u64,
) -> QueryEngineResult<()> {
    if elapsed_milliseconds > timeout.milliseconds() {
        return Err(timeout_elapsed(timeout));
    }
    Ok(())
}
