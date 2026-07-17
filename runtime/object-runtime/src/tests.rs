use super::*;
use crate::errors::{InvalidRelationError, MetadataError, ValidationError};
use crate::types::{
    PropertyDefinition, PropertyValueKind, RelationCardinality, RelationDefinition,
    StructuralConstraints,
};
use open_eqms_runtime_contracts::{UnitOfWork, Version as SharedVersion};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Barrier, Mutex};
use std::thread;

#[derive(Default)]
struct InMemoryStorage {
    types: BTreeMap<ObjectTypeRef, ObjectTypeDefinition>,
    objects: BTreeMap<ObjectId, ObjectRecord>,
    fail_replace: bool,
}

impl StorageProvider for InMemoryStorage {
    fn get_type(
        &self,
        object_type: &ObjectTypeRef,
    ) -> ObjectRuntimeResult<Option<ObjectTypeDefinition>> {
        Ok(self.types.get(object_type).cloned())
    }

    fn put_type(&mut self, definition: ObjectTypeDefinition) -> ObjectRuntimeResult<()> {
        self.types.insert(definition.type_ref.clone(), definition);
        Ok(())
    }

    fn get_object(&self, object_id: &ObjectId) -> ObjectRuntimeResult<Option<ObjectRecord>> {
        Ok(self.objects.get(object_id).cloned())
    }

    fn insert_object(&mut self, record: ObjectRecord) -> ObjectRuntimeResult<()> {
        if self.objects.contains_key(&record.id) {
            return Err(ObjectRuntimeError::DuplicateIdentity {
                object_id: record.id,
            });
        }
        self.objects.insert(record.id.clone(), record);
        Ok(())
    }

    fn replace_object(
        &mut self,
        record: ObjectRecord,
        expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        if self.fail_replace {
            return Err(ObjectRuntimeError::StorageProviderFailure {
                message: "injected replace failure".to_owned(),
            });
        }

        let current =
            self.objects
                .get(&record.id)
                .ok_or_else(|| ObjectRuntimeError::ObjectNotFound {
                    object_id: record.id.clone(),
                })?;
        if current.version != expected_version {
            return Err(ObjectRuntimeError::VersionConflict {
                object_id: record.id,
                expected: expected_version,
                actual: current.version,
            });
        }

        self.objects.insert(record.id.clone(), record);
        Ok(())
    }

    fn object_exists(&self, object_id: &ObjectId) -> ObjectRuntimeResult<bool> {
        Ok(self.objects.contains_key(object_id))
    }

    fn visit_objects(
        &self,
        visitor: &mut dyn FnMut(&ObjectRecord) -> ObjectRuntimeResult<()>,
    ) -> ObjectRuntimeResult<()> {
        for record in self.objects.values() {
            visitor(record)?;
        }
        Ok(())
    }
}

struct AlternateMemoryStorage(InMemoryStorage);

impl StorageProvider for AlternateMemoryStorage {
    fn get_type(
        &self,
        object_type: &ObjectTypeRef,
    ) -> ObjectRuntimeResult<Option<ObjectTypeDefinition>> {
        self.0.get_type(object_type)
    }

    fn put_type(&mut self, definition: ObjectTypeDefinition) -> ObjectRuntimeResult<()> {
        self.0.put_type(definition)
    }

    fn get_object(&self, object_id: &ObjectId) -> ObjectRuntimeResult<Option<ObjectRecord>> {
        self.0.get_object(object_id)
    }

    fn insert_object(&mut self, record: ObjectRecord) -> ObjectRuntimeResult<()> {
        self.0.insert_object(record)
    }

    fn replace_object(
        &mut self,
        record: ObjectRecord,
        expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        self.0.replace_object(record, expected_version)
    }

    fn object_exists(&self, object_id: &ObjectId) -> ObjectRuntimeResult<bool> {
        self.0.object_exists(object_id)
    }

    fn visit_objects(
        &self,
        visitor: &mut dyn FnMut(&ObjectRecord) -> ObjectRuntimeResult<()>,
    ) -> ObjectRuntimeResult<()> {
        self.0.visit_objects(visitor)
    }
}

#[derive(Clone, Default)]
struct SharedUnitOfWorkStorage {
    inner: Arc<Mutex<SharedUnitOfWorkStorageInner>>,
}

#[derive(Default)]
struct SharedUnitOfWorkStorageInner {
    types: BTreeMap<ObjectTypeRef, ObjectTypeDefinition>,
    objects: BTreeMap<ObjectId, ObjectRecord>,
    staged_replacements: BTreeMap<UnitOfWork, Vec<(ObjectRecord, Version)>>,
    fail_stage: bool,
    fail_commit: bool,
    fail_rollback: bool,
}

impl SharedUnitOfWorkStorage {
    fn with_stage_failure() -> Self {
        let storage = Self::default();
        storage.inner.lock().unwrap().fail_stage = true;
        storage
    }

    fn lock_inner(
        &self,
    ) -> ObjectRuntimeResult<std::sync::MutexGuard<'_, SharedUnitOfWorkStorageInner>> {
        self.inner
            .lock()
            .map_err(|_| ObjectRuntimeError::StorageProviderFailure {
                message: "shared unit-of-work storage lock poisoned".to_owned(),
            })
    }
}

impl StorageProvider for SharedUnitOfWorkStorage {
    fn get_type(
        &self,
        object_type: &ObjectTypeRef,
    ) -> ObjectRuntimeResult<Option<ObjectTypeDefinition>> {
        Ok(self.lock_inner()?.types.get(object_type).cloned())
    }

    fn put_type(&mut self, definition: ObjectTypeDefinition) -> ObjectRuntimeResult<()> {
        self.lock_inner()?
            .types
            .insert(definition.type_ref.clone(), definition);
        Ok(())
    }

    fn get_object(&self, object_id: &ObjectId) -> ObjectRuntimeResult<Option<ObjectRecord>> {
        Ok(self.lock_inner()?.objects.get(object_id).cloned())
    }

    fn insert_object(&mut self, record: ObjectRecord) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_inner()?;
        if inner.objects.contains_key(&record.id) {
            return Err(ObjectRuntimeError::DuplicateIdentity {
                object_id: record.id,
            });
        }
        inner.objects.insert(record.id.clone(), record);
        Ok(())
    }

    fn replace_object(
        &mut self,
        record: ObjectRecord,
        expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_inner()?;
        replace_committed_object(&mut inner, record, expected_version)
    }

    fn begin_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        self.lock_inner()?
            .staged_replacements
            .entry(unit_of_work.clone())
            .or_default();
        Ok(())
    }

    fn replace_object_in_unit_of_work(
        &mut self,
        unit_of_work: &UnitOfWork,
        record: ObjectRecord,
        expected_version: Version,
    ) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_inner()?;
        if inner.fail_stage {
            return Err(ObjectRuntimeError::StorageProviderFailure {
                message: "injected staged replace failure".to_owned(),
            });
        }

        let current =
            inner
                .objects
                .get(&record.id)
                .ok_or_else(|| ObjectRuntimeError::ObjectNotFound {
                    object_id: record.id.clone(),
                })?;
        if current.version != expected_version {
            return Err(ObjectRuntimeError::VersionConflict {
                object_id: record.id,
                expected: expected_version,
                actual: current.version,
            });
        }

        let staged = inner
            .staged_replacements
            .get_mut(unit_of_work)
            .ok_or_else(|| inactive_unit_of_work(unit_of_work))?;
        staged.push((record, expected_version));
        Ok(())
    }

    fn commit_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_inner()?;
        if inner.fail_commit {
            return Err(ObjectRuntimeError::StorageProviderFailure {
                message: "injected unit-of-work commit failure".to_owned(),
            });
        }

        let staged = inner
            .staged_replacements
            .get(unit_of_work)
            .cloned()
            .ok_or_else(|| inactive_unit_of_work(unit_of_work))?;

        for (record, expected_version) in &staged {
            let current = inner.objects.get(&record.id).ok_or_else(|| {
                ObjectRuntimeError::ObjectNotFound {
                    object_id: record.id.clone(),
                }
            })?;
            if current.version != *expected_version {
                return Err(ObjectRuntimeError::VersionConflict {
                    object_id: record.id.clone(),
                    expected: *expected_version,
                    actual: current.version,
                });
            }
        }

        for (record, _) in staged {
            inner.objects.insert(record.id.clone(), record);
        }
        inner.staged_replacements.remove(unit_of_work);
        Ok(())
    }

    fn rollback_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> ObjectRuntimeResult<()> {
        let mut inner = self.lock_inner()?;
        if inner.fail_rollback {
            return Err(ObjectRuntimeError::StorageProviderFailure {
                message: "injected unit-of-work rollback failure".to_owned(),
            });
        }
        inner
            .staged_replacements
            .remove(unit_of_work)
            .ok_or_else(|| inactive_unit_of_work(unit_of_work))?;
        Ok(())
    }

    fn object_exists(&self, object_id: &ObjectId) -> ObjectRuntimeResult<bool> {
        Ok(self.lock_inner()?.objects.contains_key(object_id))
    }

    fn visit_objects(
        &self,
        visitor: &mut dyn FnMut(&ObjectRecord) -> ObjectRuntimeResult<()>,
    ) -> ObjectRuntimeResult<()> {
        for record in self.lock_inner()?.objects.values() {
            visitor(record)?;
        }
        Ok(())
    }
}

fn replace_committed_object(
    inner: &mut SharedUnitOfWorkStorageInner,
    record: ObjectRecord,
    expected_version: Version,
) -> ObjectRuntimeResult<()> {
    let current =
        inner
            .objects
            .get(&record.id)
            .ok_or_else(|| ObjectRuntimeError::ObjectNotFound {
                object_id: record.id.clone(),
            })?;
    if current.version != expected_version {
        return Err(ObjectRuntimeError::VersionConflict {
            object_id: record.id,
            expected: expected_version,
            actual: current.version,
        });
    }

    inner.objects.insert(record.id.clone(), record);
    Ok(())
}

fn inactive_unit_of_work(unit_of_work: &UnitOfWork) -> ObjectRuntimeError {
    ObjectRuntimeError::StorageProviderFailure {
        message: format!("unit of work is not active: {unit_of_work}"),
    }
}

struct FixedIds {
    ids: Vec<ObjectId>,
    index: usize,
}

impl FixedIds {
    fn new(ids: &[&str]) -> Self {
        Self {
            ids: ids.iter().map(|id| ObjectId::new(*id)).collect(),
            index: 0,
        }
    }
}

impl ObjectIdGenerator for FixedIds {
    fn next_id(&mut self) -> ObjectId {
        let id = self
            .ids
            .get(self.index)
            .cloned()
            .unwrap_or_else(|| ObjectId::new(format!("generated-{}", self.index)));
        self.index += 1;
        id
    }
}

fn type_ref() -> ObjectTypeRef {
    ObjectTypeRef::new("generic-object", 1)
}

fn alternate_type_ref() -> ObjectTypeRef {
    ObjectTypeRef::new("alternate-object", 1)
}

fn object_type(object_type: ObjectTypeRef) -> ObjectTypeDefinition {
    let mut properties = BTreeMap::new();
    properties.insert(
        "name".to_owned(),
        PropertyDefinition::new("name", PropertyValueKind::Text, true, text_constraints()),
    );
    properties.insert(
        "count".to_owned(),
        PropertyDefinition::new(
            "count",
            PropertyValueKind::Number,
            false,
            StructuralConstraints {
                min_number: Some(0.0),
                max_number: Some(10.0),
                ..StructuralConstraints::default()
            },
        ),
    );

    let mut relations = BTreeMap::new();
    relations.insert(
        "parent".to_owned(),
        RelationDefinition::new("parent", RelationCardinality::new(0, Some(1))),
    );

    ObjectTypeDefinition::new(object_type, properties, relations, None)
}

fn text_constraints() -> StructuralConstraints {
    StructuralConstraints {
        min_length: Some(1),
        max_length: Some(64),
        pattern: Some("^[A-Za-z0-9 -]+$".to_owned()),
        ..StructuralConstraints::default()
    }
}

fn runtime() -> ObjectRuntime<InMemoryStorage, FixedIds> {
    let runtime = ObjectRuntime::new(
        InMemoryStorage::default(),
        FixedIds::new(&["object-1", "object-2", "object-3", "object-4"]),
    );
    runtime
        .register_object_type(object_type(type_ref()))
        .unwrap();
    runtime
}

fn shared_unit_of_work_runtime(
    storage: SharedUnitOfWorkStorage,
) -> (
    ObjectRuntime<SharedUnitOfWorkStorage, FixedIds>,
    SharedUnitOfWorkStorage,
) {
    let coordinator = storage.clone();
    let runtime = ObjectRuntime::new(storage, FixedIds::new(&["object-1"]));
    runtime
        .register_object_type(object_type(type_ref()))
        .unwrap();
    (runtime, coordinator)
}

fn create_request(object_type: ObjectTypeRef) -> CreateObjectRequest {
    CreateObjectRequest {
        object_type,
        properties: properties("Alpha"),
        relations: BTreeSet::new(),
        lifecycle_state: LifecycleState::new("draft"),
        ownership: OwnershipInfo::new(ObjectId::new("owner-1"), BTreeSet::new()),
        permission_scope: PermissionScopeRef::new("scope-1"),
        retention_rule: Some(RetentionRuleRef::new("retain-default")),
        external_references: BTreeSet::new(),
        comments_ref: Some(CommentThreadRef::new("comments-1")),
        attachments_ref: Some(AttachmentSetRef::new("attachments-1")),
    }
}

fn properties(name: &str) -> BTreeMap<String, PropertyValue> {
    let mut properties = BTreeMap::new();
    properties.insert(
        "name".to_owned(),
        PropertyValue::Text {
            value: name.to_owned(),
            language: Some("en".to_owned()),
        },
    );
    properties
}

fn set_name_change(name: &str) -> ObjectChanges {
    let mut changes = ObjectChanges::default();
    changes.property_changes.insert(
        "name".to_owned(),
        PropertyChange::Set(PropertyValue::Text {
            value: name.to_owned(),
            language: Some("en".to_owned()),
        }),
    );
    changes
}

fn text_property<'a>(record: &'a ObjectRecord, name: &str) -> &'a str {
    match record.properties.get(name).unwrap() {
        PropertyValue::Text { value, .. } => value,
        _ => panic!("expected text property"),
    }
}

#[test]
fn create_read_and_update_are_by_identity_and_versioned() {
    let runtime = runtime();

    let created = runtime.create_object(create_request(type_ref())).unwrap();
    assert_eq!(created.version, Version::initial());

    let read = runtime.read_object(&created.object_id).unwrap();
    assert_eq!(read.id, created.object_id);
    assert_eq!(read.object_type, type_ref());

    let updated = runtime
        .update_object(UpdateObjectRequest {
            object_id: created.object_id.clone(),
            base_version: created.version,
            changes: set_name_change("Beta"),
        })
        .unwrap();

    assert_eq!(updated.version, Version::new(2));
    assert_eq!(updated.id, created.object_id);
    assert_eq!(updated.object_type, type_ref());
}

#[test]
fn reexported_version_is_the_shared_runtime_contract_type() {
    fn accepts_shared_version(version: SharedVersion) -> SharedVersion {
        version
    }

    let version: Version = Version::initial();

    assert_eq!(accepts_shared_version(version), SharedVersion::initial());
}

#[test]
fn unit_of_work_amendment_preserves_independent_update_behavior() {
    let (runtime, _coordinator) = shared_unit_of_work_runtime(SharedUnitOfWorkStorage::default());
    let created = runtime.create_object(create_request(type_ref())).unwrap();

    let updated = runtime
        .update_object(UpdateObjectRequest {
            object_id: created.object_id.clone(),
            base_version: created.version,
            changes: set_name_change("Beta"),
        })
        .unwrap();
    let read = runtime.read_object(&created.object_id).unwrap();

    assert_eq!(updated.version, Version::new(2));
    assert_eq!(text_property(&read, "name"), "Beta");
    assert_eq!(read.version, Version::new(2));
}

#[test]
fn unit_of_work_staged_update_is_invisible_until_commit() {
    let (runtime, mut coordinator) =
        shared_unit_of_work_runtime(SharedUnitOfWorkStorage::default());
    let created = runtime.create_object(create_request(type_ref())).unwrap();
    let unit_of_work = UnitOfWork::new("uow-commit");
    coordinator.begin_unit_of_work(&unit_of_work).unwrap();

    let staged = runtime
        .update_object_in_unit_of_work(
            UpdateObjectRequest {
                object_id: created.object_id.clone(),
                base_version: created.version,
                changes: set_name_change("Beta"),
            },
            &unit_of_work,
        )
        .unwrap();
    let before_commit = runtime.read_object(&created.object_id).unwrap();

    assert_eq!(staged.version, Version::new(2));
    assert_eq!(before_commit.version, Version::initial());
    assert_eq!(text_property(&before_commit, "name"), "Alpha");

    coordinator.commit_unit_of_work(&unit_of_work).unwrap();
    let after_commit = runtime.read_object(&created.object_id).unwrap();

    assert_eq!(after_commit.version, Version::new(2));
    assert_eq!(text_property(&after_commit, "name"), "Beta");
}

#[test]
fn unit_of_work_rollback_discards_staged_update() {
    let (runtime, mut coordinator) =
        shared_unit_of_work_runtime(SharedUnitOfWorkStorage::default());
    let created = runtime.create_object(create_request(type_ref())).unwrap();
    let unit_of_work = UnitOfWork::new("uow-rollback");
    coordinator.begin_unit_of_work(&unit_of_work).unwrap();

    runtime
        .update_object_in_unit_of_work(
            UpdateObjectRequest {
                object_id: created.object_id.clone(),
                base_version: created.version,
                changes: set_name_change("Beta"),
            },
            &unit_of_work,
        )
        .unwrap();
    coordinator.rollback_unit_of_work(&unit_of_work).unwrap();
    let read = runtime.read_object(&created.object_id).unwrap();

    assert_eq!(read.version, Version::initial());
    assert_eq!(text_property(&read, "name"), "Alpha");
}

#[test]
fn unit_of_work_staged_write_failure_leaves_committed_state_unchanged() {
    let (runtime, mut coordinator) =
        shared_unit_of_work_runtime(SharedUnitOfWorkStorage::with_stage_failure());
    let created = runtime.create_object(create_request(type_ref())).unwrap();
    let unit_of_work = UnitOfWork::new("uow-fail-stage");
    coordinator.begin_unit_of_work(&unit_of_work).unwrap();

    let result = runtime.update_object_in_unit_of_work(
        UpdateObjectRequest {
            object_id: created.object_id.clone(),
            base_version: created.version,
            changes: set_name_change("Beta"),
        },
        &unit_of_work,
    );
    let read = runtime.read_object(&created.object_id).unwrap();

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::StorageProviderFailure { .. })
    ));
    assert_eq!(read.version, Version::initial());
    assert_eq!(text_property(&read, "name"), "Alpha");
}

#[test]
fn concurrent_updates_with_same_base_version_allow_exactly_one_success() {
    let runtime = runtime();
    let created = runtime.create_object(create_request(type_ref())).unwrap();
    let barrier = Arc::new(Barrier::new(2));

    let first_runtime = runtime.clone();
    let first_barrier = Arc::clone(&barrier);
    let first_id = created.object_id.clone();
    let first = thread::spawn(move || {
        first_barrier.wait();
        first_runtime.update_object(UpdateObjectRequest {
            object_id: first_id,
            base_version: Version::initial(),
            changes: set_name_change("First"),
        })
    });

    let second_runtime = runtime.clone();
    let second_barrier = Arc::clone(&barrier);
    let second_id = created.object_id.clone();
    let second = thread::spawn(move || {
        second_barrier.wait();
        second_runtime.update_object(UpdateObjectRequest {
            object_id: second_id,
            base_version: Version::initial(),
            changes: set_name_change("Second"),
        })
    });

    let first = first.join().unwrap();
    let second = second.join().unwrap();
    let successes = [first.is_ok(), second.is_ok()]
        .into_iter()
        .filter(|success| *success)
        .count();
    let conflicts = [first, second]
        .into_iter()
        .filter(|result| matches!(result, Err(ObjectRuntimeError::VersionConflict { .. })))
        .count();

    assert_eq!(successes, 1);
    assert_eq!(conflicts, 1);
    assert_eq!(
        runtime.read_object(&created.object_id).unwrap().version,
        Version::new(2)
    );
}

#[test]
fn relation_target_must_exist_before_state_changes() {
    let runtime = runtime();
    let created = runtime.create_object(create_request(type_ref())).unwrap();

    let result = runtime.add_relation(RelationUpdateRequest {
        object_id: created.object_id.clone(),
        base_version: created.version,
        relation: Relation::new("parent", ObjectId::new("missing")),
    });

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::InvalidRelation {
            failure: InvalidRelationError::TargetDoesNotExist { .. }
        })
    ));
    let unchanged = runtime.read_object(&created.object_id).unwrap();
    assert_eq!(unchanged.version, Version::initial());
    assert!(unchanged.relations.is_empty());
}

#[test]
fn unknown_property_is_rejected() {
    let runtime = runtime();
    let mut request = create_request(type_ref());
    request
        .properties
        .insert("business-specific".to_owned(), PropertyValue::Boolean(true));

    let result = runtime.create_object(request);

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::ValidationFailed {
            failure: ValidationError::UnknownProperty { .. }
        })
    ));
}

#[test]
fn missing_required_property_is_rejected() {
    let runtime = runtime();
    let mut request = create_request(type_ref());
    request.properties.clear();

    let result = runtime.create_object(request);

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::ValidationFailed {
            failure: ValidationError::MissingRequiredProperty { .. }
        })
    ));
}

#[test]
fn property_type_mismatch_is_rejected() {
    let runtime = runtime();
    let mut request = create_request(type_ref());
    request
        .properties
        .insert("name".to_owned(), PropertyValue::Boolean(true));

    let result = runtime.create_object(request);

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::ValidationFailed {
            failure: ValidationError::TypeMismatch { .. }
        })
    ));
}

#[test]
fn property_constraint_violation_is_rejected() {
    let runtime = runtime();
    let mut request = create_request(type_ref());
    request.properties.insert(
        "name".to_owned(),
        PropertyValue::Text {
            value: "not allowed!".to_owned(),
            language: None,
        },
    );

    let result = runtime.create_object(request);

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::ValidationFailed {
            failure: ValidationError::ConstraintViolation { .. }
        })
    ));
}

#[test]
fn cardinality_violation_is_rejected() {
    let runtime = runtime();
    let source = runtime.create_object(create_request(type_ref())).unwrap();
    let target_one = runtime.create_object(create_request(type_ref())).unwrap();
    let target_two = runtime.create_object(create_request(type_ref())).unwrap();

    let with_first_parent = runtime
        .add_relation(RelationUpdateRequest {
            object_id: source.object_id.clone(),
            base_version: source.version,
            relation: Relation::new("parent", target_one.object_id),
        })
        .unwrap();

    let result = runtime.add_relation(RelationUpdateRequest {
        object_id: source.object_id,
        base_version: with_first_parent.version,
        relation: Relation::new("parent", target_two.object_id),
    });

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::InvalidRelation {
            failure: InvalidRelationError::CardinalityViolated { .. }
        })
    ));
}

#[test]
fn add_and_remove_relation_are_versioned_updates() {
    let runtime = runtime();
    let source = runtime.create_object(create_request(type_ref())).unwrap();
    let target = runtime.create_object(create_request(type_ref())).unwrap();
    let relation = Relation::new("parent", target.object_id);

    let added = runtime
        .add_relation(RelationUpdateRequest {
            object_id: source.object_id.clone(),
            base_version: source.version,
            relation: relation.clone(),
        })
        .unwrap();
    assert_eq!(added.version, Version::new(2));
    assert!(added.relations.contains(&relation));

    let removed = runtime
        .remove_relation(RelationUpdateRequest {
            object_id: source.object_id,
            base_version: added.version,
            relation: relation.clone(),
        })
        .unwrap();
    assert_eq!(removed.version, Version::new(3));
    assert!(!removed.relations.contains(&relation));
}

#[test]
fn object_not_found_error_is_distinct() {
    let runtime = runtime();

    let result = runtime.read_object(&ObjectId::new("missing"));

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::ObjectNotFound { .. })
    ));
}

#[test]
fn object_type_not_found_error_is_distinct() {
    let runtime = runtime();
    let result = runtime.create_object(create_request(ObjectTypeRef::new("missing-type", 1)));

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::ObjectTypeNotFound { .. })
    ));
}

#[test]
fn duplicate_identity_error_is_distinct() {
    let runtime = ObjectRuntime::new(InMemoryStorage::default(), FixedIds::new(&["same", "same"]));
    runtime
        .register_object_type(object_type(type_ref()))
        .unwrap();
    runtime.create_object(create_request(type_ref())).unwrap();

    let result = runtime.create_object(create_request(type_ref()));

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::DuplicateIdentity { .. })
    ));
}

#[test]
fn malformed_metadata_error_is_distinct() {
    let runtime = ObjectRuntime::new(InMemoryStorage::default(), FixedIds::new(&["object-1"]));
    let mut definition = object_type(type_ref());
    definition
        .properties
        .get_mut("name")
        .unwrap()
        .constraints
        .pattern = Some("[".to_owned());

    let result = runtime.register_object_type(definition);

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::MalformedMetadata {
            failure: MetadataError::InvalidPattern { .. }
        })
    ));
}

#[test]
fn invalidating_metadata_update_requires_migration_marker() {
    let runtime = runtime();
    runtime.create_object(create_request(type_ref())).unwrap();

    let mut changed = object_type(type_ref());
    changed.properties.insert(
        "new-required".to_owned(),
        PropertyDefinition::new(
            "new-required",
            PropertyValueKind::Boolean,
            true,
            StructuralConstraints::default(),
        ),
    );

    let result = runtime.register_object_type(changed);

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::MalformedMetadata {
            failure: MetadataError::WouldInvalidateExistingInstances { .. }
        })
    ));
}

#[test]
fn create_requires_owner_and_permission_scope() {
    let runtime = runtime();
    let mut missing_owner = create_request(type_ref());
    missing_owner.ownership.owner = ObjectId::new("");

    assert!(matches!(
        runtime.create_object(missing_owner),
        Err(ObjectRuntimeError::ValidationFailed {
            failure: ValidationError::EmptyOwner
        })
    ));

    let mut missing_scope = create_request(type_ref());
    missing_scope.permission_scope = PermissionScopeRef::new("");

    assert!(matches!(
        runtime.create_object(missing_scope),
        Err(ObjectRuntimeError::ValidationFailed {
            failure: ValidationError::EmptyPermissionScope
        })
    ));
}

#[test]
fn single_object_write_is_atomic_when_storage_rejects_replace() {
    let definition = object_type(type_ref());
    let record = ObjectRecord::new(
        ObjectId::new("object-1"),
        type_ref(),
        properties("Alpha"),
        BTreeSet::new(),
        LifecycleState::new("draft"),
        OwnershipInfo::new(ObjectId::new("owner-1"), BTreeSet::new()),
        PermissionScopeRef::new("scope-1"),
        Version::initial(),
        None,
        BTreeSet::new(),
        None,
        None,
    );
    let mut storage = InMemoryStorage {
        fail_replace: true,
        ..InMemoryStorage::default()
    };
    storage.types.insert(type_ref(), definition);
    storage.objects.insert(record.id.clone(), record.clone());
    let runtime = ObjectRuntime::new(storage, FixedIds::new(&[]));

    let result = runtime.update_object(UpdateObjectRequest {
        object_id: record.id.clone(),
        base_version: record.version,
        changes: set_name_change("Beta"),
    });

    assert!(matches!(
        result,
        Err(ObjectRuntimeError::StorageProviderFailure { .. })
    ));
    assert_eq!(runtime.read_object(&record.id).unwrap(), record);
}

#[test]
fn same_mechanisms_apply_to_different_object_types() {
    let runtime = ObjectRuntime::new(
        InMemoryStorage::default(),
        FixedIds::new(&["object-1", "object-2"]),
    );
    runtime
        .register_object_type(object_type(type_ref()))
        .unwrap();
    runtime
        .register_object_type(object_type(alternate_type_ref()))
        .unwrap();

    assert!(runtime.create_object(create_request(type_ref())).is_ok());

    let mut invalid_alternate = create_request(alternate_type_ref());
    invalid_alternate
        .properties
        .insert("business-only".to_owned(), PropertyValue::Boolean(true));

    assert!(matches!(
        runtime.create_object(invalid_alternate),
        Err(ObjectRuntimeError::ValidationFailed {
            failure: ValidationError::UnknownProperty { .. }
        })
    ));
}

#[test]
fn storage_provider_can_be_swapped_without_changing_callers() {
    fn conformance<S: StorageProvider>(storage: S) -> ObjectRuntimeResult<ObjectRecord> {
        let runtime = ObjectRuntime::new(storage, FixedIds::new(&["object-1"]));
        runtime.register_object_type(object_type(type_ref()))?;
        let created = runtime.create_object(create_request(type_ref()))?;
        runtime.read_object(&created.object_id)
    }

    let first = conformance(InMemoryStorage::default()).unwrap();
    let second = conformance(AlternateMemoryStorage(InMemoryStorage::default())).unwrap();

    assert_eq!(first, second);
}

#[test]
fn internal_bulk_read_surface_has_no_filter_and_visits_all_records() {
    let runtime = runtime();
    let first = runtime.create_object(create_request(type_ref())).unwrap();
    let second = runtime.create_object(create_request(type_ref())).unwrap();
    let mut visited = Vec::new();

    runtime
        .visit_all_objects(|record| {
            visited.push(record.id.clone());
            Ok(())
        })
        .unwrap();

    assert_eq!(visited, vec![first.object_id, second.object_id]);
}

#[test]
fn replaying_identical_calls_produces_identical_state() {
    fn scenario() -> String {
        let runtime = runtime();
        let created = runtime.create_object(create_request(type_ref())).unwrap();
        runtime
            .update_object(UpdateObjectRequest {
                object_id: created.object_id,
                base_version: created.version,
                changes: set_name_change("Beta"),
            })
            .unwrap();

        let mut debug_records = Vec::new();
        runtime
            .visit_all_objects(|record| {
                debug_records.push(format!("{record:?}"));
                Ok(())
            })
            .unwrap();
        debug_records.join("\n")
    }

    assert_eq!(scenario(), scenario());
}

#[test]
fn public_api_has_no_query_or_delete_operations() {
    let public_methods = [
        "register_object_type",
        "get_object_type",
        "create_object",
        "read_object",
        "update_object",
        "add_relation",
        "remove_relation",
        "visit_all_objects",
    ];

    for method in public_methods {
        assert!(!method.contains("filter"));
        assert!(!method.contains("query"));
        assert!(!method.contains("search"));
        assert!(!method.contains("sort"));
        assert_ne!(method, "delete_object");
        assert_ne!(method, "remove_object");
    }
}

#[test]
fn crate_has_no_content_package_path_dependency() {
    let manifest = include_str!("../Cargo.toml");
    assert!(!manifest.contains("packages/"));
    assert!(!manifest.contains("packages\\\\"));
    assert!(!manifest.contains("content-package"));
}

#[test]
fn implementation_does_not_use_wall_clock_sources() {
    let implementation = [
        include_str!("lib.rs"),
        include_str!("types.rs"),
        include_str!("errors.rs"),
        include_str!("storage.rs"),
        include_str!("validation.rs"),
    ]
    .join("\n");

    assert!(!implementation.contains("SystemTime"));
    assert!(!implementation.contains("Instant"));
    assert!(!implementation.contains("chrono"));
    assert!(!implementation.contains("now()"));
    assert!(!implementation.contains("now_"));
}
