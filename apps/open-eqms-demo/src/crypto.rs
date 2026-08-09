use open_eqms_transaction_engine::crypto::CryptographicProvider;
use open_eqms_transaction_engine::types::{
    CryptographicSignature, SignedRevision, SignerRef, TransactionHash,
};

#[derive(Clone, Debug, Default)]
pub struct DemoCryptographicProvider {
    fail_hash: bool,
}

impl DemoCryptographicProvider {
    pub fn with_hash_failure() -> Self {
        Self { fail_hash: true }
    }
}

impl CryptographicProvider for DemoCryptographicProvider {
    fn hash(&mut self, canonical_content: &[u8]) -> Result<TransactionHash, String> {
        if self.fail_hash {
            return Err("injected demo hash failure".to_owned());
        }

        let mut hash = 0xcbf29ce484222325_u64;
        for byte in canonical_content {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }

        Ok(TransactionHash::new(format!(
            "demo-noncrypto-{hash:016x}-{}",
            canonical_content.len()
        )))
    }

    fn verify(
        &mut self,
        _signed_revision: &SignedRevision,
        _signer: &SignerRef,
        _signature: &CryptographicSignature,
    ) -> Result<bool, String> {
        Ok(false)
    }
}
