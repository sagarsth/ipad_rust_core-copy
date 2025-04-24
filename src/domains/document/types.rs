use crate::errors::{DomainError, ValidationError, DomainResult};
use crate::validation::{Validate, ValidationBuilder};
use crate::types::{PaginatedResult, SyncPriority}; // Import SyncPriority
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap; // Keep for DocumentSummary if used
use std::str::FromStr;
use uuid::Uuid;
use sqlx::FromRow;
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain; // Import SyncPriority from the correct path

// --- Domain Entities ---

/// Document type definition
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub default_priority: String, // ADDED - As String matching CompressionPriority names
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
}

/// Media/Document record (Immutable after creation)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MediaDocument {
    pub id: Uuid,
    pub related_table: String,
    pub related_id: Option<Uuid>,
    pub type_id: Uuid,
    pub original_filename: String, // RENAMED from file_name
    pub file_path: String, // Path to the original uploaded file
    pub compressed_file_path: Option<String>,
    pub compressed_size_bytes: Option<i64>, // ADDED
    pub title: Option<String>,
    pub field_identifier: Option<String>, // RENAMED from linked_field_name
    pub description: Option<String>, // Keep if useful, though not updatable via API
    pub mime_type: String,           // Changed to non-optional String
    pub size_bytes: i64,             // RENAMED from file_size, changed to non-optional
    pub compression_status: String, // Changed to String (use CompressionStatus::as_str())
    pub blob_key: Option<String>,
    pub blob_status: String,   // Changed to String (use BlobSyncStatus::as_str())
    pub temp_related_id: Option<Uuid>,
    pub has_error: Option<i64>,         // RE-ADDED: 0 or 1
    pub error_type: Option<String>,     // RE-ADDED: e.g., 'storage_failure', 'compression_failure'
    pub error_message: Option<String>,  // RE-ADDED: Details of the error
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,   // Still updated internally (e.g., sync status)
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>, // Still updated internally
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub sync_priority: String,       // Changed to String (use SyncPriority::as_str())
}

/// Document version record for tracking file history (if needed)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub document_id: Uuid,
    pub version_number: i64,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub blob_key: Option<String>, // Sync key for this specific version's file
    pub created_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
}

/// Document access log for tracking document usage
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DocumentAccessLog {
    pub id: Uuid,
    pub document_id: Uuid,
    pub user_id: Uuid, // Use Uuid::nil() for system actions
    pub access_type: String, // Use DocumentAccessType::as_str()
    pub access_date: DateTime<Utc>,
    pub details: Option<String>,
}

// --- Enums ---

/// Enum for compression status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CompressionStatus {
    #[default] // Default for new records
    Pending,
    InProgress, // ADDED
    Completed,  // RENAMED from Compressed
    Failed,
    Skipped,    // e.g., file type not compressible or already small
}

impl CompressionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionStatus::Pending => "PENDING",
            CompressionStatus::InProgress => "IN_PROGRESS",
            CompressionStatus::Completed => "COMPLETED",
            CompressionStatus::Failed => "FAILED",
            CompressionStatus::Skipped => "SKIPPED",
        }
    }
}

impl FromStr for CompressionStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(CompressionStatus::Pending),
            "IN_PROGRESS" => Ok(CompressionStatus::InProgress),
            "COMPLETED" | "COMPRESSED" => Ok(CompressionStatus::Completed), // Allow old value
            "FAILED" => Ok(CompressionStatus::Failed),
            "SKIPPED" => Ok(CompressionStatus::Skipped),
            _ => Err(DomainError::Internal(format!("Invalid CompressionStatus string: {}", s))),
        }
    }
}

/// Enum for blob sync status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BlobSyncStatus {
    #[default] // Default for new records
    Pending,
    InProgress,
    Synced,
    Failed,
}

impl BlobSyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlobSyncStatus::Pending => "PENDING",
            BlobSyncStatus::InProgress => "IN_PROGRESS",
            BlobSyncStatus::Synced => "SYNCED",
            BlobSyncStatus::Failed => "FAILED",
        }
    }
}

impl FromStr for BlobSyncStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "PENDING" => Ok(BlobSyncStatus::Pending),
            "IN_PROGRESS" => Ok(BlobSyncStatus::InProgress),
            "SYNCED" => Ok(BlobSyncStatus::Synced),
            "FAILED" => Ok(BlobSyncStatus::Failed),
            _ => Err(DomainError::Internal(format!("Invalid BlobSyncStatus string: {}", s))),
        }
    }
}

/// Enum for document access types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DocumentAccessType {
    View,
    Download,
    AttemptView,        // ADDED
    AttemptDownload,    // ADDED
    RequestDownload,    // ADDED
    Delete,
    SyncStatusChange,   // ADDED
    SystemUpdate,       // ADDED
    // EditMetadata,    // REMOVED - No longer editable via API, keep if create logs needed
}

impl DocumentAccessType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentAccessType::View => "VIEW",
            DocumentAccessType::Download => "DOWNLOAD",
            DocumentAccessType::AttemptView => "ATTEMPT_VIEW",
            DocumentAccessType::AttemptDownload => "ATTEMPT_DOWNLOAD",
            DocumentAccessType::RequestDownload => "REQUEST_DOWNLOAD",
            DocumentAccessType::Delete => "DELETE",
            DocumentAccessType::SyncStatusChange => "SYNC_STATUS_CHANGE",
            DocumentAccessType::SystemUpdate => "SYSTEM_UPDATE",
        }
    }
}

impl FromStr for DocumentAccessType {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "VIEW" => Ok(DocumentAccessType::View),
            "DOWNLOAD" => Ok(DocumentAccessType::Download),
            "ATTEMPT_VIEW" => Ok(DocumentAccessType::AttemptView),
            "ATTEMPT_DOWNLOAD" => Ok(DocumentAccessType::AttemptDownload),
            "REQUEST_DOWNLOAD" => Ok(DocumentAccessType::RequestDownload),
            "DELETE" => Ok(DocumentAccessType::Delete),
            "SYNC_STATUS_CHANGE" => Ok(DocumentAccessType::SyncStatusChange),
            "SYSTEM_UPDATE" => Ok(DocumentAccessType::SystemUpdate),
            _ => Err(DomainError::Internal(format!("Invalid DocumentAccessType string: {}", s))),
        }
    }
}

/// Enum for Compression Priority (Matches service usage)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum CompressionPriority {
    Low = 1,
    Normal = 5,
    High = 10,
}

impl CompressionPriority {
     pub fn as_str(&self) -> &'static str {
        match self {
            CompressionPriority::Low => "LOW",
            CompressionPriority::Normal => "NORMAL",
            CompressionPriority::High => "HIGH",
        }
    }
     pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            1 => Some(CompressionPriority::Low),
            5 => Some(CompressionPriority::Normal),
            10 => Some(CompressionPriority::High),
            _ => None,
        }
    }
}

impl FromStr for CompressionPriority {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "LOW" => Ok(CompressionPriority::Low),
            "NORMAL" => Ok(CompressionPriority::Normal),
            "HIGH" => Ok(CompressionPriority::High),
            _ => Err(DomainError::Internal(format!("Invalid CompressionPriority string: {}", s))),
        }
    }
}

// --- Data Transfer Objects (DTOs) ---

/// DTO for creating a new document type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentType {
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub default_priority: String, // ADDED - e.g., "NORMAL"
}

impl Validate for NewDocumentType {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(100)
            .validate()?;
        // Validate default_priority is a valid CompressionPriority string
        CompressionPriority::from_str(&self.default_priority)?;
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
    pub default_priority: Option<String>, // ADDED
}

impl Validate for UpdateDocumentType {
    fn validate(&self) -> DomainResult<()> {
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .required() // If present, must not be empty
                .min_length(2)
                .max_length(100)
                .validate()?;
        }
        if let Some(prio) = &self.default_priority {
             CompressionPriority::from_str(prio)?;
        }
        Ok(())
    }
}

/// DTO for creating the MediaDocument record in the database
/// (Used internally by the service after saving the file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewMediaDocument {
    pub id: Uuid,
    pub related_table: String,
    pub related_id: Option<Uuid>,
    pub temp_related_id: Option<Uuid>,
    pub type_id: Uuid,
    pub original_filename: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: String,
    pub size_bytes: i64,
    pub field_identifier: Option<String>,
    pub sync_priority: String,
    pub created_by_user_id: Option<Uuid>,
    pub file_path: String, // Added to store the relative path
    pub compression_status: String, // Default "PENDING"
    pub blob_status: String, // Default "PENDING"
    pub compressed_file_path: Option<String>, // Default None
    pub compressed_size_bytes: Option<i64>, // Default None
    pub blob_key: Option<String>, // Default None
}

impl Validate for NewMediaDocument {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("related_table", Some(self.related_table.clone()))
            .required().max_length(50).validate()?;
            
        // Ensure either related_id or temp_related_id is set, but not both
        if self.related_id.is_none() && self.temp_related_id.is_none() {
            return Err(DomainError::Validation(ValidationError::custom("Either related_id or temp_related_id must be provided")));
        }
        if self.related_id.is_some() && self.temp_related_id.is_some() {
            return Err(DomainError::Validation(ValidationError::custom("Cannot provide both related_id and temp_related_id")));
        }
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
            .required()
            .min(0)
            .validate()?;
        ValidationBuilder::new("file_path", Some(self.file_path.clone()))
            .required()
            .max_length(1000)
            .validate()?;
        
        Ok(())
    }
}

// UpdateMediaDocument DTO is REMOVED as documents are immutable

/// DTO for logging document access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDocumentAccessLog {
    pub document_id: Uuid,
    pub user_id: Uuid,
    pub access_type: String, // Expects DocumentAccessType::as_str()
    pub details: Option<String>,
}

impl Validate for NewDocumentAccessLog {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("document_id", Some(self.document_id)).not_nil().validate()?;
        // Allow Uuid::nil() for system user
        // ValidationBuilder::new("user_id", Some(self.user_id)).not_nil().validate()?;
        ValidationBuilder::new("access_type", Some(self.access_type.clone())).required().max_length(50).validate()?;
        // Validate that access_type is a known enum variant
        DocumentAccessType::from_str(&self.access_type)?;
        Ok(())
    }
}

// --- Response DTOs ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentTypeResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub default_priority: String, // ADDED
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
            default_priority: entity.default_priority, // ADDED
            created_at: entity.created_at.to_rfc3339(),
            updated_at: entity.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaDocumentResponse {
    pub id: Uuid,
    pub related_table: String,
    pub related_id: Option<Uuid>,
    pub temp_related_id: Option<Uuid>,
    pub type_id: Uuid,
    pub type_name: Option<String>,
    pub original_filename: String,
    pub file_path: String,
    pub compressed_file_path: Option<String>,
    pub field_identifier: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub mime_type: String,
    pub size_bytes: i64,
    pub compressed_size_bytes: Option<i64>,
    pub compression_status: String,
    pub blob_status: String,
    pub sync_priority: String,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<Uuid>,
    pub is_available_locally: bool,
    // Updated fields for error handling (match MediaDocument)
    pub has_error: bool, // Use bool here for API clarity
    pub error_type: Option<String>,
    pub error_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<DocumentVersion>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_logs: Option<PaginatedResult<DocumentAccessLog>>,
}

impl MediaDocumentResponse {
    /// Create response DTO from the domain entity. Enrichment happens later.
    pub fn from_doc(doc: &MediaDocument, type_name: Option<String>) -> Self {
        // Use the has_error field directly
        let has_error_flag = doc.has_error.unwrap_or(0) == 1;
        
        Self {
            id: doc.id,
            related_table: doc.related_table.clone(),
            related_id: doc.related_id,
            temp_related_id: doc.temp_related_id,
            type_id: doc.type_id,
            type_name,
            original_filename: doc.original_filename.clone(),
            file_path: doc.file_path.clone(),
            compressed_file_path: doc.compressed_file_path.clone(),
            field_identifier: doc.field_identifier.clone(),
            title: doc.title.clone(),
            description: doc.description.clone(),
            mime_type: doc.mime_type.clone(),
            size_bytes: doc.size_bytes,
            compressed_size_bytes: doc.compressed_size_bytes,
            compression_status: doc.compression_status.clone(),
            blob_status: doc.blob_status.clone(),
            sync_priority: doc.sync_priority.clone(),
            created_at: doc.created_at.to_rfc3339(),
            updated_at: doc.updated_at.to_rfc3339(),
            created_by_user_id: doc.created_by_user_id,
            is_available_locally: false, // Will be set by enrich_response
            
            // Set error fields from MediaDocument
            has_error: has_error_flag,
            error_type: if has_error_flag { doc.error_type.clone() } else { None },
            error_message: if has_error_flag { doc.error_message.clone() } else { None },
            
            versions: None,
            access_logs: None,
        }
    }
}

// --- Structs for Internal Use (e.g., Sync Service) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub total_count: i64,
    pub unlinked_count: i64,
    pub linked_fields: HashMap<String, i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentFileInfo {
    pub id: Uuid,
    pub file_path: String, // Path potentially used for sync (could be original or compressed)
    pub absolute_path: String,
    pub is_compressed: bool, // Indicates if file_path refers to the compressed version
    pub size_bytes: i64,     // Size corresponding to file_path
    pub mime_type: String,
    pub file_exists_locally: bool,
    pub blob_status: String, // Use BlobSyncStatus::as_str()
    pub blob_key: Option<String>,
    pub sync_priority: String, // Use SyncPriority::as_str()
    pub original_file_path: String, // Always the original path
    pub original_size_bytes: i64,   // Always the original size
    pub compression_status: CompressionStatus, // Return enum for internal logic
}


// --- Database Row Mappers ---

#[derive(Debug, Clone, FromRow)]
pub struct DocumentTypeRow {
    pub id: String,
    pub name: String,
    pub name_updated_at: Option<String>,
    pub name_updated_by: Option<String>,
    pub description: Option<String>,
    pub description_updated_at: Option<String>,
    pub description_updated_by: Option<String>,
    pub icon: Option<String>,
    pub icon_updated_at: Option<String>,
    pub icon_updated_by: Option<String>,
    pub color: Option<String>,
    pub color_updated_at: Option<String>,
    pub color_updated_by: Option<String>,
    pub default_priority: String, // ADDED
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
}

impl DocumentTypeRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<DocumentType> {
        // Keep existing helper closures
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
            default_priority: self.default_priority, // ADDED
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
    pub original_filename: String, // RENAMED from file_name
    pub file_path: String,
    pub compressed_file_path: Option<String>,
    pub compressed_size_bytes: Option<i64>, // ADDED
    pub field_identifier: Option<String>, // RENAMED from linked_field_name
    pub title: Option<String>,
    pub description: Option<String>, // Keep if column exists
    pub mime_type: String, // Make non-optional in DB if possible
    pub size_bytes: i64, // RENAMED from file_size
    pub compression_status: String, // Stored as string
    pub blob_key: Option<String>,
    pub blob_status: String, // Stored as string
    pub temp_related_id: Option<String>,
    pub has_error: Option<i64>,         // RE-ADDED
    pub error_type: Option<String>,     // RE-ADDED
    pub error_message: Option<String>,  // RE-ADDED
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub sync_priority: i64, // Stored as integer in DB
}

impl MediaDocumentRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<MediaDocument> {
        // Keep existing helper closures
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
        // Helper to convert DB priority int to SyncPriority string
        let sync_priority_str = SyncPriorityFromSyncDomain::from(self.sync_priority as i32) // Cast i64 to i32
             .as_str() 
             .to_string(); 

        Ok(MediaDocument {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            related_table: self.related_table,
            related_id: parse_uuid(&self.related_id).transpose()?,
            type_id: Uuid::parse_str(&self.type_id).map_err(|_| DomainError::InvalidUuid(self.type_id.clone()))?,
            original_filename: self.original_filename,
            file_path: self.file_path,
            compressed_file_path: self.compressed_file_path,
            compressed_size_bytes: self.compressed_size_bytes,
            title: self.title,
            field_identifier: self.field_identifier,
            description: self.description,
            mime_type: self.mime_type,
            size_bytes: self.size_bytes,
            compression_status: self.compression_status,
            blob_key: self.blob_key,
            blob_status: self.blob_status,
            temp_related_id: parse_uuid(&self.temp_related_id).transpose()?,
            has_error: self.has_error,             // Mapped
            error_type: self.error_type,           // Mapped
            error_message: self.error_message,     // Mapped
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
            sync_priority: sync_priority_str, // Assign the converted string
        })
    }
}

/// DocumentVersionRow - SQLite row representation
#[derive(Debug, Clone, FromRow)]
pub struct DocumentVersionRow {
    pub id: String,
    pub document_id: String,
    pub version_number: i64,
    pub file_path: String,
    pub file_size: i64,
    pub mime_type: String,
    pub blob_key: Option<String>,
    pub created_at: String,
    pub created_by_user_id: Option<String>,
}

impl DocumentVersionRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<DocumentVersion> {
        // Keep existing helper closures
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
            blob_key: self.blob_key,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid created_at format: {}", self.created_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id).transpose()?,
        })
    }
}

/// DocumentAccessLogRow - SQLite row representation
#[derive(Debug, Clone, FromRow)]
pub struct DocumentAccessLogRow {
    pub id: String,
    pub document_id: String,
    pub user_id: String,
    pub access_type: String,
    pub access_date: String,
    pub details: Option<String>,
}

impl DocumentAccessLogRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<DocumentAccessLog> {
         // Validate access_type when reading from DB
         let _ = DocumentAccessType::from_str(&self.access_type)?;

         Ok(DocumentAccessLog {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::InvalidUuid(self.id.clone()))?,
            document_id: Uuid::parse_str(&self.document_id).map_err(|_| DomainError::InvalidUuid(self.document_id.clone()))?,
            user_id: Uuid::parse_str(&self.user_id).map_err(|_| DomainError::InvalidUuid(self.user_id.clone()))?,
            access_type: self.access_type, // Keep as string
            access_date: DateTime::parse_from_rfc3339(&self.access_date)
                 .map(|dt| dt.with_timezone(&Utc))
                 .map_err(|_| DomainError::Internal(format!("Invalid access_date format: {}", self.access_date)))?,
            details: self.details,
        })
    }
}

// --- REMOVED Unused Structs/DTOs ---
// CompressionQueueEntry, CompressionStats, CompressionMethod, DocumentPriority
// CreateDocumentTypeDto, UpdateDocumentTypeDto, CreateMediaDocumentDto, UpdateMediaDocumentDto
// LogDocumentAccessDto, DocumentSearchCriteria, DocumentTypeSearchCriteria
// DocumentUploadResult, NewDocumentUpload