use serde::{Deserialize, Serialize};
use uuid::Uuid;
use chrono::{DateTime, Utc};

// Re-export UserRole and Permission from the permission module
pub use crate::domains::permission::{UserRole, Permission};

/// Sync Priority Levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[repr(i64)] // Representation for sqlx
pub enum SyncPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Never = 3, // Explicitly never sync this item
}

impl SyncPriority {
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            0 => Some(SyncPriority::Low),
            1 => Some(SyncPriority::Normal),
            2 => Some(SyncPriority::High),
            3 => Some(SyncPriority::Never),
            _ => None,
        }
    }
}

/// Generic row ID type - can be used with SQLite's INTEGER or TEXT primary keys
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RowId {
    Int(i64),
    Uuid(Uuid),
    Text(String),
}

impl RowId {
    pub fn as_uuid(&self) -> Option<&Uuid> {
        match self {
            RowId::Uuid(uuid) => Some(uuid),
            _ => None,
        }
    }
    
    pub fn as_int(&self) -> Option<i64> {
        match self {
            RowId::Int(id) => Some(*id),
            _ => None,
        }
    }
    
    pub fn as_text(&self) -> String {
        match self {
            RowId::Int(id) => id.to_string(),
            RowId::Uuid(uuid) => uuid.to_string(),
            RowId::Text(text) => text.clone(),
        }
    }
}

/// Common timestamp types used across entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timestamps {
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

/// Common authorship types used across entities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Authorship {
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_by_user_id: Option<Uuid>,
}

/// Device sync state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSyncState {
    pub device_id: String,
    pub user_id: Uuid,
    pub last_upload_timestamp: Option<DateTime<Utc>>,
    pub last_download_timestamp: Option<DateTime<Utc>>,
    pub last_sync_status: Option<SyncStatus>,
    pub last_sync_attempt_at: Option<DateTime<Utc>>,
    pub server_version: i64,
    pub sync_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Device metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetadata {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub model: Option<String>,
    pub os_version: Option<String>,
    pub app_version: String,
    pub last_active_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Sync status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncStatus {
    Success,
    PartialSuccess,
    Failed,
    InProgress,
}

impl SyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatus::Success => "success",
            SyncStatus::PartialSuccess => "partial_success",
            SyncStatus::Failed => "failed",
            SyncStatus::InProgress => "in_progress",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "success" => Some(SyncStatus::Success),
            "partial_success" => Some(SyncStatus::PartialSuccess),
            "failed" => Some(SyncStatus::Failed),
            "in_progress" => Some(SyncStatus::InProgress),
            _ => None,
        }
    }
}

/// Connection status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    Online,
    Offline,
    Unstable,
}

/// Sync batch direction enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    Upload,
    Download,
}

impl SyncDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncDirection::Upload => "upload",
            SyncDirection::Download => "download",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "upload" => Some(SyncDirection::Upload),
            "download" => Some(SyncDirection::Download),
            _ => None,
        }
    }
}

/// Sync batch status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncBatchStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    PartiallyFailed,
}

impl SyncBatchStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncBatchStatus::Pending => "pending",
            SyncBatchStatus::Processing => "processing",
            SyncBatchStatus::Completed => "completed",
            SyncBatchStatus::Failed => "failed",
            SyncBatchStatus::PartiallyFailed => "partially_failed",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(SyncBatchStatus::Pending),
            "processing" => Some(SyncBatchStatus::Processing),
            "completed" => Some(SyncBatchStatus::Completed),
            "failed" => Some(SyncBatchStatus::Failed),
            "partially_failed" => Some(SyncBatchStatus::PartiallyFailed),
            _ => None,
        }
    }
}

/// Change log operation type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeLogOperationType {
    Create,
    Update,
    Delete,
    HardDelete,
}

impl ChangeLogOperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeLogOperationType::Create => "create",
            ChangeLogOperationType::Update => "update",
            ChangeLogOperationType::Delete => "delete",
            ChangeLogOperationType::HardDelete => "hard_delete",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "create" => Some(ChangeLogOperationType::Create),
            "update" => Some(ChangeLogOperationType::Update),
            "delete" => Some(ChangeLogOperationType::Delete),
            "hard_delete" => Some(ChangeLogOperationType::HardDelete),
            _ => None,
        }
    }
}

/// Audit log action type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditLogAction {
    Create,
    Update,
    Delete,
    HardDelete,
    LoginSuccess,
    LoginFail,
    Logout,
    SyncUploadStart,
    SyncUploadComplete,
    SyncUploadFail,
    SyncDownloadStart,
    SyncDownloadComplete,
    SyncDownloadFail,
    MergeConflictResolved,
    MergeConflictDetected,
    PermissionDenied,
    DataExport,
    DataImport,
}

impl AuditLogAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditLogAction::Create => "create",
            AuditLogAction::Update => "update",
            AuditLogAction::Delete => "delete",
            AuditLogAction::HardDelete => "hard_delete",
            AuditLogAction::LoginSuccess => "login_success",
            AuditLogAction::LoginFail => "login_fail",
            AuditLogAction::Logout => "logout",
            AuditLogAction::SyncUploadStart => "sync_upload_start",
            AuditLogAction::SyncUploadComplete => "sync_upload_complete",
            AuditLogAction::SyncUploadFail => "sync_upload_fail",
            AuditLogAction::SyncDownloadStart => "sync_download_start",
            AuditLogAction::SyncDownloadComplete => "sync_download_complete",
            AuditLogAction::SyncDownloadFail => "sync_download_fail",
            AuditLogAction::MergeConflictResolved => "merge_conflict_resolved",
            AuditLogAction::MergeConflictDetected => "merge_conflict_detected",
            AuditLogAction::PermissionDenied => "permission_denied",
            AuditLogAction::DataExport => "data_export",
            AuditLogAction::DataImport => "data_import",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "create" => Some(AuditLogAction::Create),
            "update" => Some(AuditLogAction::Update),
            "delete" => Some(AuditLogAction::Delete),
            "hard_delete" => Some(AuditLogAction::HardDelete),
            "login_success" => Some(AuditLogAction::LoginSuccess),
            "login_fail" => Some(AuditLogAction::LoginFail),
            "logout" => Some(AuditLogAction::Logout),
            "sync_upload_start" => Some(AuditLogAction::SyncUploadStart),
            "sync_upload_complete" => Some(AuditLogAction::SyncUploadComplete),
            "sync_upload_fail" => Some(AuditLogAction::SyncUploadFail),
            "sync_download_start" => Some(AuditLogAction::SyncDownloadStart),
            "sync_download_complete" => Some(AuditLogAction::SyncDownloadComplete),
            "sync_download_fail" => Some(AuditLogAction::SyncDownloadFail),
            "merge_conflict_resolved" => Some(AuditLogAction::MergeConflictResolved),
            "merge_conflict_detected" => Some(AuditLogAction::MergeConflictDetected),
            "permission_denied" => Some(AuditLogAction::PermissionDenied),
            "data_export" => Some(AuditLogAction::DataExport),
            "data_import" => Some(AuditLogAction::DataImport),
            _ => None,
        }
    }
}

/// Pagination parameters
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaginationParams {
    pub page: u32,
    pub per_page: u32,
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            per_page: 20,
        }
    }
}

/// Paginated result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub per_page: u32,
    pub total_pages: u32,
}

impl<T> PaginatedResult<T> {
    pub fn new(items: Vec<T>, total: u64, params: PaginationParams) -> Self {
        let total_pages = (total as f64 / params.per_page as f64).ceil() as u32;
        Self {
            items,
            total,
            page: params.page,
            per_page: params.per_page,
            total_pages,
        }
    }
}