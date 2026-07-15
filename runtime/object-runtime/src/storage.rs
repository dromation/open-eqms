//! Storage-provider and identity-generation boundaries for the Object Runtime.

use crate::errors::ObjectRuntimeResult;
use crate::types::{ObjectId, ObjectRecord, ObjectTypeDefinition, ObjectTypeRef, Version};

/// Abstract persistence boundary used by the Object Runtime.
///
/// Implementations may use any backing store as long as they preserve this contract.
/// The Object Runtime must not depend on concrete database behavior.
pub trait StorageProvider {
    /// Retrieves an Object Type definition by exact name/version reference.
    fn get_type(
        &self,
        object_type: &ObjectTypeRef,
    ) -> ObjectRuntimeResult<Option<ObjectTypeDefinition>>;

    /// Stores or replaces an Object Type definition.
    fn put_type(&mut self, definition: ObjectTypeDefinition) -> ObjectRuntimeResult<()>;

    /// Retrieves an Object record by identity.
    fn get_object(&self, object_id: &ObjectId) -> ObjectRuntimeResult<Option<ObjectRecord>>;

    /// Inserts a new Object record and rejects duplicate identities.
    fn insert_object(&mut self, record: ObjectRecord) -> ObjectRuntimeResult<()>;

    /// Replaces one Object record only when the stored version equals `expected_version`.
    fn replace_object(
        &mut self,
        record: ObjectRecord,
        expected_version: Version,
    ) -> ObjectRuntimeResult<()>;

    /// Reports whether an Object identity exists.
    fn object_exists(&self, object_id: &ObjectId) -> ObjectRuntimeResult<bool>;

    /// Visits every Object record without filtering, sorting, searching, or aggregation.
    fn visit_objects(
        &self,
        visitor: &mut dyn FnMut(&ObjectRecord) -> ObjectRuntimeResult<()>,
    ) -> ObjectRuntimeResult<()>;
}

/// Object identity source used by create operations.
///
/// Production deployments may provide a UUIDv7/ULID-class generator. Tests use a
/// deterministic implementation so replayed API sequences produce byte-identical state.
pub trait ObjectIdGenerator {
    /// Returns the next opaque Object identifier.
    fn next_id(&mut self) -> ObjectId;
}
