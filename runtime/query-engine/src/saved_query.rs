//! SavedQuery catalog boundary.

use crate::types::{SavedQueryDefinition, SavedQueryId};

/// Abstract persistence boundary for reusable query metadata.
///
/// This trait defines the Query Engine side of SavedQuery registration and
/// replay without choosing a concrete storage provider. A future adapter may
/// persist these definitions as Object Runtime records or through another
/// approved mechanism without changing the Query Engine's core execution API.
pub trait SavedQueryCatalog {
    /// Catalog-specific error type.
    type Error;

    /// Stores or replaces one validated SavedQuery definition.
    fn save_query(&mut self, saved_query: SavedQueryDefinition) -> Result<(), Self::Error>;

    /// Retrieves one SavedQuery definition by identity.
    fn load_query(
        &self,
        saved_query_id: &SavedQueryId,
    ) -> Result<Option<SavedQueryDefinition>, Self::Error>;
}
