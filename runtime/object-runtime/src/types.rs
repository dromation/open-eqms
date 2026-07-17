//! Data contracts for the Open-EQMS Object Runtime.

use std::collections::{BTreeMap, BTreeSet};

pub use open_eqms_runtime_contracts::{ObjectId, PropertyValue, PropertyValueKind, Version};

/// Reference to a registered Object Type metadata definition.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ObjectTypeRef {
    /// Language-neutral Object Type name.
    pub name: String,
    /// Metadata version for this Object Type.
    pub version: u32,
}

impl ObjectTypeRef {
    /// Creates a new Object Type reference.
    pub fn new(name: impl Into<String>, version: u32) -> Self {
        Self {
            name: name.into(),
            version,
        }
    }
}

/// Opaque lifecycle-state token owned and interpreted by the Process Engine.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct LifecycleState(String);

impl LifecycleState {
    /// Creates a lifecycle-state token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored lifecycle-state token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the lifecycle-state token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Opaque permission-scope reference resolved by the Security component.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PermissionScopeRef(String);

impl PermissionScopeRef {
    /// Creates a permission-scope reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored permission-scope token.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the permission-scope token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Opaque retention-rule reference interpreted outside the Object Runtime.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct RetentionRuleRef(String);

impl RetentionRuleRef {
    /// Creates a retention-rule reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored retention-rule token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque reference to an external system record.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct ExternalReference {
    /// Opaque external system identifier.
    pub system: String,
    /// Opaque key inside the external system.
    pub key: String,
}

impl ExternalReference {
    /// Creates a new external reference.
    pub fn new(system: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            key: key.into(),
        }
    }
}

/// Opaque pointer to an external comment thread or comment store.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct CommentThreadRef(String);

impl CommentThreadRef {
    /// Creates a comment-thread reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored comment-thread token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque pointer to an external attachment set or attachment store.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct AttachmentSetRef(String);

impl AttachmentSetRef {
    /// Creates an attachment-set reference.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the stored attachment-set token.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Ownership and responsibility references stored on every Object.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnershipInfo {
    /// Owner Object identifier, typically referencing an identity-bearing Object.
    pub owner: ObjectId,
    /// Responsible user Object identifiers.
    pub responsible_users: BTreeSet<ObjectId>,
}

impl OwnershipInfo {
    /// Creates ownership information.
    pub fn new(owner: ObjectId, responsible_users: BTreeSet<ObjectId>) -> Self {
        Self {
            owner,
            responsible_users,
        }
    }
}

/// Structural constraints declared by Object Type metadata.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StructuralConstraints {
    /// Minimum string or blob length, where applicable.
    pub min_length: Option<usize>,
    /// Maximum string or blob length, where applicable.
    pub max_length: Option<usize>,
    /// Minimum numeric value, where applicable.
    pub min_number: Option<f64>,
    /// Maximum numeric value, where applicable.
    pub max_number: Option<f64>,
    /// Deterministic regular-expression pattern for string-like values.
    pub pattern: Option<String>,
    /// Allowed values for enum properties.
    pub allowed_enum_values: BTreeSet<String>,
}

/// Definition of one property on an Object Type.
#[derive(Clone, Debug, PartialEq)]
pub struct PropertyDefinition {
    /// Language-neutral property name.
    pub name: String,
    /// Required property value kind.
    pub kind: PropertyValueKind,
    /// Whether the property must be present on every instance.
    pub required: bool,
    /// Structural validation constraints.
    pub constraints: StructuralConstraints,
}

impl PropertyDefinition {
    /// Creates a property definition.
    pub fn new(
        name: impl Into<String>,
        kind: PropertyValueKind,
        required: bool,
        constraints: StructuralConstraints,
    ) -> Self {
        Self {
            name: name.into(),
            kind,
            required,
            constraints,
        }
    }
}

/// Cardinality limits for a relation type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationCardinality {
    /// Minimum number of targets required for this relation type.
    pub min: usize,
    /// Maximum number of targets allowed for this relation type.
    pub max: Option<usize>,
}

impl RelationCardinality {
    /// Creates a relation cardinality declaration.
    pub fn new(min: usize, max: Option<usize>) -> Self {
        Self { min, max }
    }
}

/// Definition of one relation type on an Object Type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelationDefinition {
    /// Language-neutral relation type token.
    pub relation_type: String,
    /// Structural cardinality limits.
    pub cardinality: RelationCardinality,
}

impl RelationDefinition {
    /// Creates a relation definition.
    pub fn new(relation_type: impl Into<String>, cardinality: RelationCardinality) -> Self {
        Self {
            relation_type: relation_type.into(),
            cardinality,
        }
    }
}

/// Directed typed edge from one Object to another Object.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Relation {
    /// Language-neutral relation type token.
    pub relation_type: String,
    /// Target Object identifier.
    pub target_id: ObjectId,
}

impl Relation {
    /// Creates a directed relation.
    pub fn new(relation_type: impl Into<String>, target_id: ObjectId) -> Self {
        Self {
            relation_type: relation_type.into(),
            target_id,
        }
    }
}

/// Versioned Object Type metadata definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectTypeDefinition {
    /// Name/version pair for this metadata definition.
    pub type_ref: ObjectTypeRef,
    /// Property definitions keyed by language-neutral property name.
    pub properties: BTreeMap<String, PropertyDefinition>,
    /// Relation definitions keyed by language-neutral relation type.
    pub relations: BTreeMap<String, RelationDefinition>,
    /// Optional marker showing an external migration plan accompanies an invalidating update.
    pub migration_marker: Option<String>,
}

impl ObjectTypeDefinition {
    /// Creates an Object Type definition.
    pub fn new(
        type_ref: ObjectTypeRef,
        properties: BTreeMap<String, PropertyDefinition>,
        relations: BTreeMap<String, RelationDefinition>,
        migration_marker: Option<String>,
    ) -> Self {
        Self {
            type_ref,
            properties,
            relations,
            migration_marker,
        }
    }
}

/// Full current-state Object record owned by the Object Runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectRecord {
    /// Object identity.
    pub id: ObjectId,
    /// Object Type name/version this record was last validated against.
    pub object_type: ObjectTypeRef,
    /// Current property values.
    pub properties: BTreeMap<String, PropertyValue>,
    /// Current directed relations.
    pub relations: BTreeSet<Relation>,
    /// Current opaque lifecycle state.
    pub lifecycle_state: LifecycleState,
    /// Current ownership information.
    pub ownership: OwnershipInfo,
    /// Current permission-scope reference.
    pub permission_scope: PermissionScopeRef,
    /// Current optimistic-concurrency version.
    pub version: Version,
    /// Optional retention-rule reference.
    pub retention_rule: Option<RetentionRuleRef>,
    /// Opaque external references.
    pub external_references: BTreeSet<ExternalReference>,
    /// Optional pointer to comments managed outside this SPEC.
    pub comments_ref: Option<CommentThreadRef>,
    /// Optional pointer to attachments managed outside this SPEC.
    pub attachments_ref: Option<AttachmentSetRef>,
}

impl ObjectRecord {
    /// Creates a full Object record.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ObjectId,
        object_type: ObjectTypeRef,
        properties: BTreeMap<String, PropertyValue>,
        relations: BTreeSet<Relation>,
        lifecycle_state: LifecycleState,
        ownership: OwnershipInfo,
        permission_scope: PermissionScopeRef,
        version: Version,
        retention_rule: Option<RetentionRuleRef>,
        external_references: BTreeSet<ExternalReference>,
        comments_ref: Option<CommentThreadRef>,
        attachments_ref: Option<AttachmentSetRef>,
    ) -> Self {
        Self {
            id,
            object_type,
            properties,
            relations,
            lifecycle_state,
            ownership,
            permission_scope,
            version,
            retention_rule,
            external_references,
            comments_ref,
            attachments_ref,
        }
    }
}

/// A property mutation applied during an Object update.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyChange {
    /// Set or replace a property value.
    Set(PropertyValue),
    /// Remove a property value.
    Remove,
}
