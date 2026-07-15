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
    use super::{ObjectId, PropertyValue, PropertyValueKind};
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
}
