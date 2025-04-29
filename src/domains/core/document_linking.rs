// src/domains/core/document_linking.rs
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FieldType {
    Text,
    Number, // Represents i64, f64
    Boolean,
    Date, // Represents NaiveDate or String YYYY-MM-DD
    Timestamp, // Represents DateTime<Utc> or String RFC3339
    Uuid,
    Decimal,
    // Special type for fields that primarily store a document reference
    DocumentRef, 
}

#[derive(Debug, Clone)]
pub struct EntityFieldMetadata {
    /// Technical name of the field (matches struct/db potentially)
    pub field_name: &'static str,
    /// User-friendly name for UI display
    pub display_name: &'static str,
    /// Can documents be logically linked to this field?
    pub supports_documents: bool,
    /// The type of the field (for UI hints, validation)
    pub field_type: FieldType,
    /// Is this field primarily just a reference to a document?
    pub is_document_reference_only: bool,
}

/// Trait for entities that allow documents to be linked to specific fields.
pub trait DocumentLinkable {
    /// Provides metadata for all fields relevant for display or linking.
    fn field_metadata() -> Vec<EntityFieldMetadata>;

    /// Get the names of fields that support document attachments.
    fn document_linkable_fields() -> HashSet<String> {
        Self::field_metadata()
            .into_iter()
            .filter(|meta| meta.supports_documents)
            .map(|meta| meta.field_name.to_string())
            .collect()
    }

    /// Check if a specific field supports document linking.
    fn is_document_linkable_field(field: &str) -> bool {
        Self::document_linkable_fields().contains(field)
    }

    /// Get metadata for a specific field by name.
    fn get_field_metadata(field_name: &str) -> Option<EntityFieldMetadata> {
         Self::field_metadata().into_iter().find(|meta| meta.field_name == field_name)
    }
}


// API Response Structure (could live in a web/api layer types mod)
#[derive(Serialize)]
pub struct FieldMetadataResponse {
    pub field_name: String,
    pub display_name: String,
    pub supports_documents: bool,
    pub field_type: FieldType, // Serialize the enum directly
    pub is_document_reference_only: bool,
}

impl From<EntityFieldMetadata> for FieldMetadataResponse {
    fn from(meta: EntityFieldMetadata) -> Self {
        FieldMetadataResponse {
            field_name: meta.field_name.to_string(),
            display_name: meta.display_name.to_string(),
            supports_documents: meta.supports_documents,
            field_type: meta.field_type,
            is_document_reference_only: meta.is_document_reference_only,
        }
    }
} 