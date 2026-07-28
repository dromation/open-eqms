//! Cryptographic provider boundary for the Transaction Engine.

use crate::types::{CryptographicSignature, SignedRevision, SignerRef, TransactionHash};

/// Abstract hash and verify provider used by the Transaction Engine.
///
/// This contract intentionally has no signing capability and chooses no concrete
/// algorithm. Production providers wrap established cryptographic libraries
/// outside this crate; tests use deterministic doubles.
pub trait CryptographicProvider {
    /// Computes a stable hash over canonical Transaction content.
    fn hash(&mut self, canonical_content: &[u8]) -> Result<TransactionHash, String>;

    /// Verifies a caller-supplied Level 3 signature against the signed revision and signer.
    fn verify(
        &mut self,
        signed_revision: &SignedRevision,
        signer: &SignerRef,
        signature: &CryptographicSignature,
    ) -> Result<bool, String>;
}
