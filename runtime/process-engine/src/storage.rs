//! Storage coordination boundary for Process transitions.

use open_eqms_runtime_contracts::UnitOfWork;
use open_eqms_transaction_engine::crypto::CryptographicProvider;

use crate::errors::ProcessEngineResult;

/// Provider-owned shared commit boundary for paired Object and Transaction writes.
///
/// This trait is crate-local to Process Engine V1. It formalizes the existing
/// Object+Transaction shared Unit-of-Work pattern without promoting another
/// shared Runtime contract before a broader reuse point exists.
pub trait SharedTransitionStore {
    /// Begins one shared transition Unit of Work.
    fn begin_shared_transition(&self, unit_of_work: &UnitOfWork) -> ProcessEngineResult<()>;

    /// Commits staged Object and Transaction writes atomically.
    fn commit_shared_transition<C>(
        &self,
        unit_of_work: &UnitOfWork,
        cryptographic_provider: &mut C,
    ) -> ProcessEngineResult<()>
    where
        C: CryptographicProvider;

    /// Rolls back staged Object and Transaction writes.
    fn rollback_shared_transition(&self, unit_of_work: &UnitOfWork) -> ProcessEngineResult<()>;
}
