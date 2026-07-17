//! Dependency-neutral Runtime data contracts shared by Open-EQMS Runtime engines.
//!
//! This crate contains data contracts only. It intentionally has no dependency
//! on Object Runtime, Event Engine, storage providers, content packages,
//! plugins, validation services, registries, or business semantics.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use std::fmt;

/// Opaque globally unique identifier for a Runtime Object.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ObjectId(String);

impl ObjectId {
    /// Creates a new opaque Object identifier from a caller-supplied token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the identifier token without assigning any business meaning to it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the identifier token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Monotonic per-object version used for optimistic concurrency.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct Version(u64);

impl Version {
    /// Returns the initial version assigned to a newly created Object.
    pub fn initial() -> Self {
        Self(1)
    }

    /// Creates a version from a raw monotonic marker.
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw monotonic marker.
    pub fn value(self) -> u64 {
        self.0
    }

    /// Returns the next version after a successful update.
    pub fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

/// Opaque Unit-of-Work handle representing one physical storage-transaction boundary.
///
/// This handle carries no business meaning, does not imply ordering, and is not a
/// transaction manager. Concrete storage providers own lifecycle behavior for any
/// Unit of Work they choose to participate in.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct UnitOfWork(String);

impl UnitOfWork {
    /// Creates a new opaque Unit-of-Work handle from a caller-supplied token.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the handle token without assigning any business meaning to it.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Reports whether the handle token is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for UnitOfWork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Contract implemented by a provider that owns Unit-of-Work lifecycle behavior.
///
/// The shared contracts crate defines the shape only. It does not begin, commit,
/// roll back, store, coordinate, or globally register Units of Work.
pub trait UnitOfWorkLifecycle {
    /// Provider-specific lifecycle error type.
    type Error;

    /// Begins participation in a Unit of Work owned by the concrete provider.
    fn begin_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error>;

    /// Commits the provider-owned work staged against the supplied handle.
    fn commit_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error>;

    /// Rolls back the provider-owned work staged against the supplied handle.
    fn rollback_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error>;
}

/// Contract implemented by a provider that can stage writes into a Unit of Work.
///
/// The `Write` type is owned by the participating component or storage provider.
/// This keeps Runtime contracts neutral and prevents this crate from importing
/// engine-specific record types.
pub trait UnitOfWorkStaging<Write> {
    /// Provider-specific staging error type.
    type Error;

    /// Stages one provider-owned write against the supplied Unit-of-Work handle.
    fn stage_unit_of_work_write(
        &mut self,
        unit_of_work: &UnitOfWork,
        write: Write,
    ) -> Result<(), Self::Error>;
}

/// Primitive property or payload value kinds shared by Runtime data contracts.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PropertyValueKind {
    /// User-authored text, optionally tagged by language on the value.
    Text,
    /// Reference to a localization key.
    LocalizedTextKey,
    /// Numeric value.
    Number,
    /// Boolean value.
    Boolean,
    /// Caller-supplied date/time token.
    DateTime,
    /// Enumerated token.
    EnumValue,
    /// Reference to another Object.
    Reference,
    /// Reference to an attachment.
    AttachmentReference,
    /// Binary blob.
    BinaryBlob,
}

/// A tagged primitive Runtime value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    /// User-authored text and optional language tag.
    Text {
        /// Text value.
        value: String,
        /// Optional language tag for user-authored text.
        language: Option<String>,
    },
    /// Reference to a package-supplied localized text key.
    LocalizedTextKey(String),
    /// Numeric value.
    Number(f64),
    /// Boolean value.
    Boolean(bool),
    /// Caller-supplied date/time token.
    DateTime(String),
    /// Enumerated token.
    EnumValue(String),
    /// Object reference value.
    Reference(ObjectId),
    /// Attachment reference token.
    AttachmentReference(String),
    /// Binary blob value.
    BinaryBlob(Vec<u8>),
}

impl PropertyValue {
    /// Returns the primitive kind represented by this value.
    pub fn kind(&self) -> PropertyValueKind {
        match self {
            Self::Text { .. } => PropertyValueKind::Text,
            Self::LocalizedTextKey(_) => PropertyValueKind::LocalizedTextKey,
            Self::Number(_) => PropertyValueKind::Number,
            Self::Boolean(_) => PropertyValueKind::Boolean,
            Self::DateTime(_) => PropertyValueKind::DateTime,
            Self::EnumValue(_) => PropertyValueKind::EnumValue,
            Self::Reference(_) => PropertyValueKind::Reference,
            Self::AttachmentReference(_) => PropertyValueKind::AttachmentReference,
            Self::BinaryBlob(_) => PropertyValueKind::BinaryBlob,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ObjectId, PropertyValue, PropertyValueKind, UnitOfWork, UnitOfWorkLifecycle,
        UnitOfWorkStaging, Version,
    };
    use std::collections::BTreeSet;

    #[test]
    fn object_id_preserves_opaque_token_behavior() {
        let id = ObjectId::new("object-001");

        assert_eq!("object-001", id.as_str());
        assert_eq!("object-001", id.to_string());
        assert!(!id.is_empty());
        assert!(ObjectId::new("").is_empty());
    }

    #[test]
    fn object_id_ordering_is_deterministic() {
        let ids = BTreeSet::from([
            ObjectId::new("object-c"),
            ObjectId::new("object-a"),
            ObjectId::new("object-b"),
        ]);
        let ordered = ids.iter().map(ObjectId::as_str).collect::<Vec<_>>();

        assert_eq!(ordered, ["object-a", "object-b", "object-c"]);
    }

    #[test]
    fn version_preserves_monotonic_behavior() {
        let initial = Version::initial();
        let next = initial.next();

        assert_eq!(initial.value(), 1);
        assert_eq!(next, Version::new(2));
        assert!(next > initial);
    }

    #[test]
    fn property_value_kind_mapping_is_backward_compatible() {
        let reference_id = ObjectId::new("target-001");
        let values = [
            PropertyValue::Text {
                value: "text".to_owned(),
                language: Some("en".to_owned()),
            },
            PropertyValue::LocalizedTextKey("label.key".to_owned()),
            PropertyValue::Number(42.0),
            PropertyValue::Boolean(true),
            PropertyValue::DateTime("2026-07-15T10:00:00Z".to_owned()),
            PropertyValue::EnumValue("approved".to_owned()),
            PropertyValue::Reference(reference_id),
            PropertyValue::AttachmentReference("attachment-001".to_owned()),
            PropertyValue::BinaryBlob(vec![1, 2, 3]),
        ];
        let kinds = values.iter().map(PropertyValue::kind).collect::<Vec<_>>();

        assert_eq!(
            kinds,
            [
                PropertyValueKind::Text,
                PropertyValueKind::LocalizedTextKey,
                PropertyValueKind::Number,
                PropertyValueKind::Boolean,
                PropertyValueKind::DateTime,
                PropertyValueKind::EnumValue,
                PropertyValueKind::Reference,
                PropertyValueKind::AttachmentReference,
                PropertyValueKind::BinaryBlob,
            ]
        );
    }

    #[test]
    fn shared_values_clone_and_compare_deterministically() {
        let value = PropertyValue::Reference(ObjectId::new("object-001"));

        assert_eq!(value, value.clone());
        assert_eq!(PropertyValueKind::Reference, value.kind());
    }

    #[test]
    fn unit_of_work_preserves_opaque_token_behavior() {
        let unit_of_work = UnitOfWork::new("uow-001");

        assert_eq!("uow-001", unit_of_work.as_str());
        assert_eq!("uow-001", unit_of_work.to_string());
        assert!(!unit_of_work.is_empty());
        assert!(UnitOfWork::new("").is_empty());
    }

    #[test]
    fn unit_of_work_contracts_are_provider_owned() {
        #[derive(Default)]
        struct TestParticipant {
            lifecycle: Vec<String>,
            staged: Vec<String>,
        }

        impl UnitOfWorkLifecycle for TestParticipant {
            type Error = String;

            fn begin_unit_of_work(&mut self, unit_of_work: &UnitOfWork) -> Result<(), Self::Error> {
                self.lifecycle
                    .push(format!("begin:{}", unit_of_work.as_str()));
                Ok(())
            }

            fn commit_unit_of_work(
                &mut self,
                unit_of_work: &UnitOfWork,
            ) -> Result<(), Self::Error> {
                self.lifecycle
                    .push(format!("commit:{}", unit_of_work.as_str()));
                Ok(())
            }

            fn rollback_unit_of_work(
                &mut self,
                unit_of_work: &UnitOfWork,
            ) -> Result<(), Self::Error> {
                self.lifecycle
                    .push(format!("rollback:{}", unit_of_work.as_str()));
                Ok(())
            }
        }

        impl UnitOfWorkStaging<String> for TestParticipant {
            type Error = String;

            fn stage_unit_of_work_write(
                &mut self,
                unit_of_work: &UnitOfWork,
                write: String,
            ) -> Result<(), Self::Error> {
                self.staged
                    .push(format!("{}:{}", unit_of_work.as_str(), write));
                Ok(())
            }
        }

        let mut participant = TestParticipant::default();
        let unit_of_work = UnitOfWork::new("uow-001");

        participant.begin_unit_of_work(&unit_of_work).unwrap();
        participant
            .stage_unit_of_work_write(&unit_of_work, "write-001".to_owned())
            .unwrap();
        participant.commit_unit_of_work(&unit_of_work).unwrap();

        assert_eq!(participant.lifecycle, ["begin:uow-001", "commit:uow-001"]);
        assert_eq!(participant.staged, ["uow-001:write-001"]);
    }
}
