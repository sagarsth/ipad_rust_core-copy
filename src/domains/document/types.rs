use crate::errors::{DbError, DomainError, ValidationError, DomainResult};
use crate::validation::{Validate, ValidationBuilder}; // Import validation tools
use crate::types::UserRole;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::str::FromStr;
use uuid::Uuid;
use sqlx::FromRow; // Import FromRow
use crate::types::SyncPriority;
use async_trait::async_trait;
use crate::types::{PaginatedResult, PaginationParams}; // Ensure PaginatedResult is imported

/// Document type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentType {
    pub id: Uuid,
    pub name: String,
    pub name_updated_at: Option<DateTime<Utc>>,
    pub name_updated_by: Option<Uuid>,
    pub description: Option<String>,
    pub description_updated_at: Option<DateTime<Utc>>,
    pub description_updated_by: Option<Uuid>,
    pub icon: Option<String>,
    pub icon_updated_at: Option<DateTime<Utc>>,
    pub icon_updated_by: Option<Uuid>,
    pub color: Option<String>,
    pub color_updated_at: Option<DateTime<Utc>>,
    pub color_updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
}

/// Media/Document record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDocument {
    pub id: Uuid,
    pub related_table: String,
    pub related_id: Option<Uuid>,
    pub type_id: Uuid,
    pub file_name: String,
    pub file_path: String,
    pub compressed_file_path: Option<String>,
    pub linked_field_name: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub compression_status: Option<CompressionStatus>,
    pub blob_storage_key: Option<String>,
    pub blob_sync_status: BlobSyncStatus,
    pub temp_related_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub field_identifier: Option<String>,
    pub sync_priority: SyncPriority,
}

/// Document version record for tracking file history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub document_id: Uuid,
    pub version_number: i64,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub blob_storage_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
}

/// Document access log for tracking document usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentAccessLog {
    pub id: Uuid,
    pub document_id: Uuid,
    pub user_id: Uuid,
    pub access_type: String,
    pub access_date: DateTime<Utc>,
    pub details: Option<String>,
}

/// Compression queue entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionQueueEntry {
    pub id: Uuid,
    pub document_id: Uuid,
    pub priority: Option<i64>,
    pub attempts: Option<i64>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub error_message: Option<String>,
}

/// Compression statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStats {
    pub id: String, // Always "global"
    pub total_original_size: Option<i64>,
    pub total_compressed_size: Option<i64>,
    pub space_saved: Option<i64>,
    pub compression_ratio: Option<f64>,
    pub total_files_compressed: Option<i64>,
    pub total_files_pending: Option<i64>,
    pub total_files_failed: Option<i64>,
    pub last_compression_date: Option<String>,
    pub updated_at: String,
}

/// Enum for compression status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionStatus {
    Pending,
    Compressed,
    Failed,
    Skipped,
}

impl CompressionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionStatus::Pending => "pending",
            CompressionStatus::Compressed => "compressed",
            CompressionStatus::Failed => "failed",
            CompressionStatus::Skipped => "skipped",
        }
    }
}

impl FromStr for CompressionStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(CompressionStatus::Pending),
            "compressed" => Ok(CompressionStatus::Compressed),
            "failed" => Ok(CompressionStatus::Failed),
            "skipped" | "not_needed" => Ok(CompressionStatus::Skipped),
            _ => Err(DomainError::Internal(format!("Invalid CompressionStatus string: {}", s))),
        }
    }
}

impl From<CompressionStatus> for String {
    fn from(status: CompressionStatus) -> Self {
        status.as_str().to_string()
    }
}

/// Enum for blob sync status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlobSyncStatus {
    Pending,
    InProgress,
    Synced,
    Failed,
}

impl BlobSyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlobSyncStatus::Pending => "pending",
            BlobSyncStatus::InProgress => "in_progress",
            BlobSyncStatus::Synced => "synced",
            BlobSyncStatus::Failed => "failed",
        }
    }
}

impl FromStr for BlobSyncStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(BlobSyncStatus::Pending),
            "in_progress" => Ok(BlobSyncStatus::InProgress),
            "synced" => Ok(BlobSyncStatus::Synced),
            "failed" => Ok(BlobSyncStatus::Failed),
            _ => Err(DomainError::Internal(format!("Invalid BlobSyncStatus string: {}", s))),
        }
    }
}

impl From<BlobSyncStatus> for String {
    fn from(status: BlobSyncStatus) -> Self {
        status.as_str().to_string()
    }
}

/// Enum for document access types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentAccessType {
    View,
    Download,
    EditMetadata,
    Delete,
}

impl DocumentAccessType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentAccessType::View => "view",
            DocumentAccessType::Download => "download",
            DocumentAccessType::EditMetadata => "edit_metadata",
            DocumentAccessType::Delete => "delete",
        }
    }
}

impl FromStr for DocumentAccessType {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "view" => Ok(DocumentAccessType::View),
            "download" => Ok(DocumentAccessType::Download),
            "edit_metadata" => Ok(DocumentAccessType::EditMetadata),
            "delete" => Ok(DocumentAccessType::Delete),
            _ => Err(DomainError::Internal(format!("Invalid DocumentAccessType string: {}", s))),
        }
    }
}

impl From<DocumentAccessType> for String {
    fn from(access_type: DocumentAccessType) -> Self {
        access_type.as_str().to_string()
    }
}

/// Enum for compression methods
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionMethod {
    Default,
    Lossless,
    Lossy,
    None,
}

impl CompressionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionMethod::Default => "default",
            CompressionMethod::Lossless => "lossless",
            CompressionMethod::Lossy => "lossy",
            CompressionMethod::None => "none",
        }
    }
}

impl FromStr for CompressionMethod {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(CompressionMethod::Default),
            "lossless" => Ok(CompressionMethod::Lossless),
            "lossy" => Ok(CompressionMethod::Lossy),
            "none" => Ok(CompressionMethod::None),
            _ => Err(DomainError::Internal(format!("Invalid CompressionMethod string: {}", s))),
        }
    }
}

impl From<CompressionMethod> for String {
    fn from(method: CompressionMethod) -> Self {
        method.as_str().to_string()
    }
}

/// Enum for document priority
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentPriority {
    High,
    Normal,
    Low,
    Never,
}

impl DocumentPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentPriority::High => "high",
            DocumentPriority::Normal => "normal",
            DocumentPriority::Low => "low",
            DocumentPriority::Never => "never",
        }
    }
}

impl FromStr for DocumentPriority {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "high" => Ok(DocumentPriority::High),
            "normal" => Ok(DocumentPriority::Normal),
            "low" => Ok(DocumentPriority::Low),
            "never" => Ok(DocumentPriority::Never),
            _ => Err(DomainError::Internal(format!("Invalid DocumentPriority string: {}", s))),
        }
    }
}

impl From<DocumentPriority> for String {
    fn from(priority: DocumentPriority) -> Self {
        priority.as_str().to_string()
    }
}

/// Data transfer object for creating a new document type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDocumentTypeDto {
    pub name: String,
    pub allowed_extensions: String,
    pub max_size: i64,
    pub compression_level: i64,
    pub compression_method: Option<CompressionMethod>,
    pub min_size_for_compression: Option<i64>,
    pub description: Option<String>,
    pub default_priority: DocumentPriority,
    pub icon: Option<String>,
    pub related_tables: Option<String>,
}

/// Data transfer object for updating a document type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateDocumentTypeDto {
    pub name: Option<String>,
    pub allowed_extensions: Option<String>,
    pub max_size: Option<i64>,
    pub compression_level: Option<i64>,
    pub compression_method: Option<CompressionMethod>,
    pub min_size_for_compression: Option<i64>,
    pub description: Option<String>,
    pub default_priority: Option<DocumentPriority>,
    pub icon: Option<String>,
    pub related_tables: Option<String>,
}

/// Data transfer object for creating a new media document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMediaDocumentDto {
    pub related_table: String,
    pub related_id: Uuid,
    pub title: Option<String>,
    pub type_id: Uuid,
    pub file_path: String,
    pub description: Option<String>,
    pub file_size: Option<i64>,
    pub mime_type: Option<String>,
}

/// Data transfer object for updating a media document's metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMediaDocumentDto {
    pub title: Option<String>,
    pub description: Option<String>,
}

/// Data transfer object for logging document access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogDocumentAccessDto {
    pub document_id: Uuid,
    pub user_id: Uuid,
    pub access_type: String,
    pub details: Option<String>,
}

/// Document search criteria
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentSearchCriteria {
    pub related_table: Option<String>,
    pub related_id: Option<Uuid>,
    pub type_id: Option<Uuid>,
    pub title_contains: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub include_deleted: Option<bool>,
}

/// Document type search criteria
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DocumentTypeSearchCriteria {
    pub name_contains: Option<String>,
    pub related_table: Option<String>,
    pub include_deleted: Option<bool>,
}

/// Document upload result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentUploadResult {
    pub document_id: Uuid,
    pub queued_for_compression: bool,
    pub validation_warnings: Vec<String>,
}

/// DTO for creating a new document type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentType {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

impl Validate for NewDocumentType {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(100)
            .validate()?;
        Ok(())
    }
}

/// DTO for updating an existing document type
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateDocumentType {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
}

impl Validate for UpdateDocumentType {
    fn validate(&self) -> DomainResult<()> {
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .required()
                .min_length(2)
                .max_length(100)
                .validate()?;
        }
        Ok(())
    }
}

/// DTO representing a file to be uploaded and associated with an entity
#[derive(Debug, Clone)]
pub struct NewDocumentUpload {
    pub file_name: String,
    pub mime_type: String,
    pub content: Vec<u8>,
    pub linked_field_name: Option<String>,
    pub description: Option<String>,
}

impl Validate for NewDocumentUpload {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("file_name", Some(self.file_name.clone()))
            .required()
            .max_length(255)
            .validate()?;
        ValidationBuilder::new("mime_type", Some(self.mime_type.clone()))
            .required()
            .max_length(100)
            .validate()?;
        if self.content.is_empty() {
            return Err(DomainError::Validation(ValidationError::required("content")));
        }
        Ok(())
    }
}

/// DTO for creating the MediaDocument record in the database
/// (Usually created internally by the service after saving the file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMediaDocument {
    pub id: Uuid,
    pub related_table: String,
    pub related_id: Option<Uuid>,
    pub temp_related_id: Option<Uuid>,
    pub type_id: Uuid,
    pub original_filename: String,
    pub title: Option<String>,
    pub mime_type: String,
    pub size_bytes: i64,
    pub field_identifier: Option<String>,
    pub sync_priority: SyncPriority,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewMediaDocument {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("related_table", Some(self.related_table.clone())).required().max_length(50).validate()?;
        ValidationBuilder::new("type_id", Some(self.type_id)).not_nil().validate()?;
        ValidationBuilder::new("original_filename", Some(self.original_filename.clone()))
            .required()
            .max_length(255)
            .validate()?;
        ValidationBuilder::new("mime_type", Some(self.mime_type.clone()))
            .required()
            .max_length(100)
            .validate()?;
        ValidationBuilder::new("size_bytes", Some(self.size_bytes))
            .min(0)
            .validate()?;
        Ok(())
    }
}

/// DTO for updating MediaDocument metadata
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateMediaDocument {
    pub type_id: Option<Uuid>,
    pub title: Option<String>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateMediaDocument {
    fn validate(&self) -> DomainResult<()> {
        if let Some(type_id) = self.type_id {
            ValidationBuilder::new("type_id", Some(type_id)).not_nil().validate()?;
        }
        Ok(())
    }
}

/// DTO for logging document access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentAccessLog {
    pub document_id: Uuid,
    pub user_id: Uuid,
    pub access_type: String,
    pub details: Option<String>,
}

impl Validate for NewDocumentAccessLog {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("document_id", Some(self.document_id)).not_nil().validate()?;
        ValidationBuilder::new("user_id", Some(self.user_id)).not_nil().validate()?;
        ValidationBuilder::new("access_type", Some(self.access_type.clone())).required().max_length(50).validate()?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTypeResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<DocumentType> for DocumentTypeResponse {
    fn from(entity: DocumentType) -> Self {
        Self {
            id: entity.id,
            name: entity.name,
            description: entity.description,
            icon: entity.icon,
            color: entity.color,
            created_at: entity.created_at.to_rfc3339(),
            updated_at: entity.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDocumentResponse {
    pub id: Uuid,
    pub related_table: String,
    pub related_id: Uuid,
    pub type_id: Uuid,
    pub type_name: Option<String>,
    pub file_name: String,
    pub file_path: String,
    pub compressed_file_path: Option<String>,
    pub linked_field_name: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub compression_status: Option<CompressionStatus>,
    pub blob_sync_status: BlobSyncStatus,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<Uuid>,
    pub sync_priority: SyncPriority,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<DocumentVersion>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_logs: Option<PaginatedResult<DocumentAccessLog>>,
}

impl MediaDocumentResponse {
    pub fn from_doc(doc: &MediaDocument, type_name: Option<String>) -> Self {
        let related_id = doc.related_id.unwrap_or_default();

        Self {
            id: doc.id,
            related_table: doc.related_table.clone(),
            related_id,
            type_id: doc.type_id,
            type_name,
            file_name: doc.file_name.clone(),
            file_path: doc.file_path.clone(),
            compressed_file_path: doc.compressed_file_path.clone(),
            linked_field_name: doc.linked_field_name.clone(),
            title: doc.linked_field_name.clone(),
            description: doc.description.clone(),
            mime_type: doc.mime_type.clone(),
            file_size: doc.file_size,
            compression_status: doc.compression_status,
            blob_sync_status: doc.blob_sync_status,
            created_at: doc.created_at.to_rfc3339(),
            updated_at: doc.updated_at.to_rfc3339(),
            created_by_user_id: doc.created_by_user_id,
            sync_priority: doc.sync_priority,
            versions: None,
            access_logs: None,
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct DocumentTypeRow {
    pub id: String, // Changed from Uuid
    pub name: String,
    pub name_updated_at: Option<String>, // Keep as String for RFC3339
    pub name_updated_by: Option<String>, // Changed from Option<Uuid>
    pub description: Option<String>,
    pub description_updated_at: Option<String>, // Keep as String for RFC3339
    pub description_updated_by: Option<String>, // Changed from Option<Uuid>
    pub icon: Option<String>,
    pub icon_updated_at: Option<String>, // Keep as String for RFC3339
    pub icon_updated_by: Option<String>, // Changed from Option<Uuid>
    pub color: Option<String>,
    pub color_updated_at: Option<String>, // Keep as String for RFC3339
    pub color_updated_by: Option<String>, // Changed from Option<Uuid>
    pub created_at: String, // Keep as String for RFC3339
    pub updated_at: String, // Keep as String for RFC3339
    pub created_by_user_id: Option<String>, // Changed from Option<Uuid>
    pub updated_by_user_id: Option<String>, // Changed from Option<Uuid>
    pub deleted_at: Option<String>, // Keep as String for RFC3339
    pub deleted_by_user_id: Option<String>, // Changed from Option<Uuid>
}

impl DocumentTypeRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<DocumentType> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id| {
                Uuid::parse_str(id).map_err(|_| DomainError::InvalidUuid(id.clone()))
            })
        };

        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt| {
                DateTime::parse_from_rfc3339(dt)
                    .map(|dt_fixed| dt_fixed.with_timezone(&Utc))
                    .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", dt)))
            })
        };
        
        Ok(DocumentType {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            name: self.name,
            name_updated_at: parse_datetime(&self.name_updated_at).transpose()?,
            name_updated_by: parse_uuid(&self.name_updated_by).transpose()?,
            description: self.description,
            description_updated_at: parse_datetime(&self.description_updated_at).transpose()?,
            description_updated_by: parse_uuid(&self.description_updated_by).transpose()?,
            icon: self.icon,
            icon_updated_at: parse_datetime(&self.icon_updated_at).transpose()?,
            icon_updated_by: parse_uuid(&self.icon_updated_by).transpose()?,
            color: self.color,
            color_updated_at: parse_datetime(&self.color_updated_at).transpose()?,
            color_updated_by: parse_uuid(&self.color_updated_by).transpose()?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid created_at format: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid updated_at format: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id).transpose()?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id).transpose()?,
            deleted_at: parse_datetime(&self.deleted_at).transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id).transpose()?,
        })
    }
}

/// MediaDocumentRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct MediaDocumentRow {
    pub id: String,
    pub related_table: String,
    pub related_id: Option<String>,
    pub type_id: String,
    pub original_filename: String,
    pub file_path: String,
    pub compressed_file_path: Option<String>,
    pub field_identifier: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: Option<String>,
    pub size_bytes: i64,
    pub compression_status: Option<String>,
    pub blob_storage_key: Option<String>,
    pub blob_sync_status: String,
    pub temp_related_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub sync_priority: i64,
}

impl MediaDocumentRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<MediaDocument> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id| {
                Uuid::parse_str(id).map_err(|_| DomainError::InvalidUuid(id.clone()))
            })
        };

        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt| {
                DateTime::parse_from_rfc3339(dt)
                    .map(|dt_fixed| dt_fixed.with_timezone(&Utc))
                    .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", dt)))
            })
        };

        Ok(MediaDocument {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            related_table: self.related_table,
            related_id: parse_uuid(&self.related_id).transpose()?,
            type_id: Uuid::parse_str(&self.type_id).map_err(|_| DomainError::InvalidUuid(self.type_id.clone()))?,
            file_name: self.original_filename,
            file_path: self.file_path,
            compressed_file_path: self.compressed_file_path,
            field_identifier: self.field_identifier,
            linked_field_name: self.title,
            description: self.description,
            mime_type: self.mime_type,
            file_size: Some(self.size_bytes),
            compression_status: self.compression_status.as_deref().map(CompressionStatus::from_str).transpose()?,
            blob_storage_key: self.blob_storage_key,
            blob_sync_status: BlobSyncStatus::from_str(&self.blob_sync_status)?,
            temp_related_id: parse_uuid(&self.temp_related_id).transpose()?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid created_at format: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid updated_at format: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id).transpose()?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id).transpose()?,
            deleted_at: parse_datetime(&self.deleted_at).transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id).transpose()?,
            sync_priority: SyncPriority::from_i64(self.sync_priority).ok_or_else(|| DomainError::Internal(format!("Invalid sync_priority value: {}", self.sync_priority)))?,
        })
    }
}

/// DocumentVersionRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct DocumentVersionRow {
    pub id: String, // Changed from Uuid
    pub document_id: String, // Changed from Uuid
    pub version_number: i64,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub blob_storage_key: Option<String>,
    pub created_at: String, // Keep as String for RFC3339
    pub created_by_user_id: Option<String>, // Changed from Option<Uuid>
}

impl DocumentVersionRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<DocumentVersion> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id| {
                Uuid::parse_str(id).map_err(|_| DomainError::InvalidUuid(id.clone()))
            })
        };
        
        Ok(DocumentVersion {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            document_id: Uuid::parse_str(&self.document_id).map_err(|_| DomainError::InvalidUuid(self.document_id.clone()))?,
            version_number: self.version_number,
            file_path: self.file_path,
            file_size: self.file_size,
            mime_type: self.mime_type,
            blob_storage_key: self.blob_storage_key,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid created_at format: {}", self.created_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id).transpose()?,
        })
    }
}

/// DocumentAccessLogRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct DocumentAccessLogRow {
    pub id: String, // Changed from Uuid
    pub document_id: String, // Changed from Uuid
    pub user_id: String, // Changed from Uuid
    pub access_type: String,
    pub access_date: String, // Keep as String for RFC3339
    pub details: Option<String>,
}

impl DocumentAccessLogRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<DocumentAccessLog> {
         Ok(DocumentAccessLog {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            document_id: Uuid::parse_str(&self.document_id).map_err(|_| DomainError::InvalidUuid(self.document_id.clone()))?,
            user_id: Uuid::parse_str(&self.user_id).map_err(|_| DomainError::InvalidUuid(self.user_id.clone()))?,
            access_type: self.access_type,
            access_date: DateTime::parse_from_rfc3339(&self.access_date)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid access_date format: {}", self.access_date)))?,
            details: self.details,
        })
    }
}