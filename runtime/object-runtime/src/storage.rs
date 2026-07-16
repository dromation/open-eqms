//! Storage-provider and identity-generation boundaries for the Object Runtime.

use crate::errors::{ObjectRuntimeError, ObjectRuntimeResult};
use crate::types::{ObjectId, ObjectRecord, ObjectTypeDefinition, ObjectTypeRef, Version};
use open_eqms_runtime_contracts::UnitOfWork;

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

    /// Begins provider-owned participation in a Unit of Work.
    ///
    /// The default implementation reports that this provider does not support
    /// Unit-of-Work participation. Providers that support staging own the actual
    /// lifecycle behavior.
    fn begin_unit_of_work(&mut self, _unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        Err(unsupported_unit_of_work())
    }

    /// Stages one Object replacement into an externally supplied Unit of Work.
    ///
    /// The write must not become visible until the concrete provider commits the
    /// same Unit of Work.
    fn replace_object_in_unit_of_work(
        &mut self,
        _unit_of_work: &UnitOfWork,
        _record: ObjectRecord,
        _expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        Err(unsupported_unit_of_work())
    }

    /// Commits provider-owned work staged against a Unit of Work.
    fn commit_unit_of_work(&mut self, _unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        Err(unsupported_unit_of_work())
    }

    /// Rolls back provider-owned work staged against a Unit of Work.
    fn rollback_unit_of_work(&mut self, _unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        Err(unsupported_unit_of_work())
    }

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

fn unsupported_unit_of_work() -> ObjectRuntimeError {
    ObjectRuntimeError::StorageProviderFailure {
        message: "unit-of-work participation is not supported by this storage provider".to_owned(),
    }
}
