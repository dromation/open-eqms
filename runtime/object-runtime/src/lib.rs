//! Open-EQMS Object Runtime implementation for SPEC-001.
//!
//! This crate owns Current Object State only. It stores generic Objects,
//! validates them against inert Object Type metadata, and applies
//! deterministic, versioned, single-object updates through an abstract
//! storage-provider boundary.
//!
//! It intentionally does not implement events, rules, processes, queries,
//! GUI, AI, synchronization, concrete databases, content packages,
//! transaction/audit logging, security authorization, statistics, KPIs, or
//! package loading.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod errors;
pub mod storage;
pub mod types;

mod validation;

use crate::errors::{MetadataError, ObjectRuntimeError, ObjectRuntimeResult};
use crate::storage::{ObjectIdGenerator, StorageProvider};
use crate::types::{
    AttachmentSetRef, CommentThreadRef, ExternalReference, LifecycleState, ObjectId, ObjectRecord,
    ObjectTypeDefinition, ObjectTypeRef, OwnershipInfo, PermissionScopeRef, PropertyChange,
    PropertyValue, Relation, RetentionRuleRef, Version,
};
use crate::validation::{
    validate_metadata, validate_owner_and_scope, validate_record_shape,
    validate_record_with_targets,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

/// Object Runtime facade implementing the SPEC-001 public API.
pub struct ObjectRuntime<S, G>
where
    S: StorageProvider,
    G: ObjectIdGenerator,
{
    storage: Arc<Mutex<S>>,
    id_generator: Arc<Mutex<G>>,
}

impl<S, G> Clone for ObjectRuntime<S, G>
where
    S: StorageProvider,
    G: ObjectIdGenerator,
{
    fn clone(&self) -> Self {
        Self {
            storage: Arc::clone(&self.storage),
            id_generator: Arc::clone(&self.id_generator),
        }
    }
}

impl<S, G> ObjectRuntime<S, G>
where
    S: StorageProvider,
    G: ObjectIdGenerator,
{
    /// Creates an Object Runtime over an abstract storage provider and identity generator.
    pub fn new(storage: S, id_generator: G) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            id_generator: Arc::new(Mutex::new(id_generator)),
        }
    }

    /// Registers or updates an Object Type metadata definition.
    pub fn register_object_type(
        &self,
        definition: ObjectTypeDefinition,
    ) -> ObjectRuntimeResult<()> {
        validate_metadata(&definition)?;

        let mut storage = self.lock_storage()?;
        let mut would_invalidate = false;
        storage.visit_objects(&mut |record| {
            if record.object_type.name == definition.type_ref.name
                && validate_record_shape(&definition, record).is_err()
            {
                would_invalidate = true;
            }
            Ok(())
        })?;

        if would_invalidate && definition.migration_marker.is_none() {
            return Err(ObjectRuntimeError::MalformedMetadata {
                failure: MetadataError::WouldInvalidateExistingInstances {
                    object_type: definition.type_ref.clone(),
                },
            });
        }

        storage.put_type(definition)
    }

    /// Retrieves a registered Object Type metadata definition by exact name/version.
    pub fn get_object_type(
        &self,
        object_type: &ObjectTypeRef,
    ) -> ObjectRuntimeResult<ObjectTypeDefinition> {
        let storage = self.lock_storage()?;
        storage
            .get_type(object_type)?
            .ok_or_else(|| ObjectRuntimeError::ObjectTypeNotFound {
                object_type: object_type.clone(),
            })
    }

    /// Creates a new Object after validating all supplied current-state data.
    pub fn create_object(
        &self,
        request: CreateObjectRequest,
    ) -> ObjectRuntimeResult<CreateObjectResult> {
        validate_owner_and_scope(&request.ownership, &request.permission_scope)?;

        let mut storage = self.lock_storage()?;
        let definition = storage.get_type(&request.object_type)?.ok_or_else(|| {
            ObjectRuntimeError::ObjectTypeNotFound {
                object_type: request.object_type.clone(),
            }
        })?;

        let object_id = self.next_id()?;
        if storage.object_exists(&object_id)? {
            return Err(ObjectRuntimeError::DuplicateIdentity {
                object_id: object_id.clone(),
            });
        }

        let record = ObjectRecord::new(
            object_id.clone(),
            request.object_type,
            request.properties,
            request.relations,
            request.lifecycle_state,
            request.ownership,
            request.permission_scope,
            Version::initial(),
            request.retention_rule,
            request.external_references,
            request.comments_ref,
            request.attachments_ref,
        );

        validate_record_with_targets(&definition, &record, |target_id| {
            storage.object_exists(target_id)
        })?;

        storage.insert_object(record.clone())?;
        Ok(CreateObjectResult {
            object_id,
            version: record.version,
            record,
        })
    }

    /// Reads the full current Object state by identity.
    pub fn read_object(&self, object_id: &ObjectId) -> ObjectRuntimeResult<ObjectRecord> {
        let storage = self.lock_storage()?;
        storage
            .get_object(object_id)?
            .ok_or_else(|| ObjectRuntimeError::ObjectNotFound {
                object_id: object_id.clone(),
            })
    }

    /// Applies an atomic versioned update to one Object.
    pub fn update_object(&self, request: UpdateObjectRequest) -> ObjectRuntimeResult<ObjectRecord> {
        let mut storage = self.lock_storage()?;
        let current = storage.get_object(&request.object_id)?.ok_or_else(|| {
            ObjectRuntimeError::ObjectNotFound {
                object_id: request.object_id.clone(),
            }
        })?;

        if current.version != request.base_version {
            return Err(ObjectRuntimeError::VersionConflict {
                object_id: current.id,
                expected: request.base_version,
                actual: current.version,
            });
        }

        let definition = storage.get_type(&current.object_type)?.ok_or_else(|| {
            ObjectRuntimeError::ObjectTypeNotFound {
                object_type: current.object_type.clone(),
            }
        })?;

        let mut updated = apply_changes(current, request.changes);
        updated.version = updated.version.next();

        validate_record_with_targets(&definition, &updated, |target_id| {
            storage.object_exists(target_id)
        })?;

        storage.replace_object(updated.clone(), request.base_version)?;
        Ok(updated)
    }

    /// Adds one Relation through the normal versioned Object update path.
    pub fn add_relation(
        &self,
        request: RelationUpdateRequest,
    ) -> ObjectRuntimeResult<ObjectRecord> {
        let mut changes = ObjectChanges::default();
        changes.relations_to_add.insert(request.relation);
        self.update_object(UpdateObjectRequest {
            object_id: request.object_id,
            base_version: request.base_version,
            changes,
        })
    }

    /// Removes one Relation through the normal versioned Object update path.
    pub fn remove_relation(
        &self,
        request: RelationUpdateRequest,
    ) -> ObjectRuntimeResult<ObjectRecord> {
        let mut changes = ObjectChanges::default();
        changes.relations_to_remove.insert(request.relation);
        self.update_object(UpdateObjectRequest {
            object_id: request.object_id,
            base_version: request.base_version,
            changes,
        })
    }

    /// Visits all Object records without accepting filters, predicates, sort order, or aggregation.
    pub fn visit_all_objects(
        &self,
        mut visitor: impl FnMut(&ObjectRecord) -> ObjectRuntimeResult<()>,
    ) -> ObjectRuntimeResult<()> {
        let storage = self.lock_storage()?;
        storage.visit_objects(&mut visitor)
    }

    fn lock_storage(&self) -> ObjectRuntimeResult<std::sync::MutexGuard<'_, S>> {
        self.storage
            .lock()
            .map_err(|_| ObjectRuntimeError::StorageProviderFailure {
                message: "storage lock poisoned".to_owned(),
            })
    }

    fn next_id(&self) -> ObjectRuntimeResult<ObjectId> {
        let mut generator =
            self.id_generator
                .lock()
                .map_err(|_| ObjectRuntimeError::StorageProviderFailure {
                    message: "object id generator lock poisoned".to_owned(),
                })?;
        Ok(generator.next_id())
    }
}

/// Request to create an Object.
pub struct CreateObjectRequest {
    /// Object Type reference to validate against.
    pub object_type: ObjectTypeRef,
    /// Initial property values.
    pub properties: BTreeMap<String, PropertyValue>,
    /// Initial directed relations.
    pub relations: BTreeSet<Relation>,
    /// Initial lifecycle-state token.
    pub lifecycle_state: LifecycleState,
    /// Initial ownership information.
    pub ownership: OwnershipInfo,
    /// Initial permission-scope reference.
    pub permission_scope: PermissionScopeRef,
    /// Optional retention-rule reference.
    pub retention_rule: Option<RetentionRuleRef>,
    /// Initial external references.
    pub external_references: BTreeSet<ExternalReference>,
    /// Optional comments pointer.
    pub comments_ref: Option<CommentThreadRef>,
    /// Optional attachments pointer.
    pub attachments_ref: Option<AttachmentSetRef>,
}

/// Result returned by a successful Object create operation.
pub struct CreateObjectResult {
    /// Newly assigned Object identifier.
    pub object_id: ObjectId,
    /// Newly assigned Object version.
    pub version: Version,
    /// Full newly stored Object record.
    pub record: ObjectRecord,
}

/// Request to update one Object.
pub struct UpdateObjectRequest {
    /// Object identifier to update.
    pub object_id: ObjectId,
    /// Caller-supplied base version read before the update.
    pub base_version: Version,
    /// Atomic set of changes to apply.
    pub changes: ObjectChanges,
}

/// Atomic set of changes applied to one Object.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ObjectChanges {
    /// Property mutations keyed by property name.
    pub property_changes: BTreeMap<String, PropertyChange>,
    /// Relations to add.
    pub relations_to_add: BTreeSet<Relation>,
    /// Relations to remove.
    pub relations_to_remove: BTreeSet<Relation>,
    /// Replacement lifecycle-state token.
    pub lifecycle_state: Option<LifecycleState>,
    /// Replacement ownership information.
    pub ownership: Option<OwnershipInfo>,
    /// Replacement permission-scope reference.
    pub permission_scope: Option<PermissionScopeRef>,
    /// Replacement retention-rule reference; `Some(None)` clears the reference.
    pub retention_rule: Option<Option<RetentionRuleRef>>,
    /// Replacement external references.
    pub external_references: Option<BTreeSet<ExternalReference>>,
    /// Replacement comments pointer; `Some(None)` clears the pointer.
    pub comments_ref: Option<Option<CommentThreadRef>>,
    /// Replacement attachments pointer; `Some(None)` clears the pointer.
    pub attachments_ref: Option<Option<AttachmentSetRef>>,
}

/// Request to add or remove one Relation.
pub struct RelationUpdateRequest {
    /// Source Object identifier.
    pub object_id: ObjectId,
    /// Caller-supplied base version read before the relation update.
    pub base_version: Version,
    /// Relation to add or remove.
    pub relation: Relation,
}

fn apply_changes(mut record: ObjectRecord, changes: ObjectChanges) -> ObjectRecord {
    for (name, change) in changes.property_changes {
        match change {
            PropertyChange::Set(value) => {
                record.properties.insert(name, value);
            }
            PropertyChange::Remove => {
                record.properties.remove(&name);
            }
        }
    }

    for relation in changes.relations_to_remove {
        record.relations.remove(&relation);
    }
    for relation in changes.relations_to_add {
        record.relations.insert(relation);
    }

    if let Some(lifecycle_state) = changes.lifecycle_state {
        record.lifecycle_state = lifecycle_state;
    }
    if let Some(ownership) = changes.ownership {
        record.ownership = ownership;
    }
    if let Some(permission_scope) = changes.permission_scope {
        record.permission_scope = permission_scope;
    }
    if let Some(retention_rule) = changes.retention_rule {
        record.retention_rule = retention_rule;
    }
    if let Some(external_references) = changes.external_references {
        record.external_references = external_references;
    }
    if let Some(comments_ref) = changes.comments_ref {
        record.comments_ref = comments_ref;
    }
    if let Some(attachments_ref) = changes.attachments_ref {
        record.attachments_ref = attachments_ref;
    }

    record
}

#[cfg(test)]
mod tests;
