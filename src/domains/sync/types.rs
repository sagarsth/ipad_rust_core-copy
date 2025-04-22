use crate::errors::{DomainError, ValidationError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;
use sqlx::FromRow;

/// The direction of a sync operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    Upload,
    Download,
}

impl FromStr for SyncDirection {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "upload" => Ok(SyncDirection::Upload),
            "download" => Ok(SyncDirection::Download),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid SyncDirection string: {}", s)
            )))
        }
    }
}

impl From<SyncDirection> for String {
    fn from(direction: SyncDirection) -> Self {
        match direction {
            SyncDirection::Upload => "upload".to_string(),
            SyncDirection::Download => "download".to_string(),
        }
    }
}

/// The status of a sync batch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncBatchStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    PartiallyFailed,
}

impl FromStr for SyncBatchStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(SyncBatchStatus::Pending),
            "processing" => Ok(SyncBatchStatus::Processing),
            "completed" => Ok(SyncBatchStatus::Completed),
            "failed" => Ok(SyncBatchStatus::Failed),
            "partially_failed" => Ok(SyncBatchStatus::PartiallyFailed),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid SyncBatchStatus string: {}", s)
            )))
        }
    }
}

impl From<SyncBatchStatus> for String {
    fn from(status: SyncBatchStatus) -> Self {
        match status {
            SyncBatchStatus::Pending => "pending".to_string(),
            SyncBatchStatus::Processing => "processing".to_string(),
            SyncBatchStatus::Completed => "completed".to_string(),
            SyncBatchStatus::Failed => "failed".to_string(),
            SyncBatchStatus::PartiallyFailed => "partially_failed".to_string(),
        }
    }
}

/// The status of a device sync connection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceSyncStatus {
    Success,
    PartialSuccess,
    Failed,
    InProgress,
}

impl FromStr for DeviceSyncStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "success" => Ok(DeviceSyncStatus::Success),
            "partial_success" => Ok(DeviceSyncStatus::PartialSuccess),
            "failed" => Ok(DeviceSyncStatus::Failed),
            "in_progress" => Ok(DeviceSyncStatus::InProgress),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid DeviceSyncStatus string: {}", s)
            )))
        }
    }
}

impl From<DeviceSyncStatus> for String {
    fn from(status: DeviceSyncStatus) -> Self {
        match status {
            DeviceSyncStatus::Success => "success".to_string(),
            DeviceSyncStatus::PartialSuccess => "partial_success".to_string(),
            DeviceSyncStatus::Failed => "failed".to_string(),
            DeviceSyncStatus::InProgress => "in_progress".to_string(),
        }
    }
}

/// The type of change operation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeOperationType {
    Create,
    Update,
    Delete,
    HardDelete,
}

impl ChangeOperationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeOperationType::Create => "create",
            ChangeOperationType::Update => "update",
            ChangeOperationType::Delete => "delete",
            ChangeOperationType::HardDelete => "hard_delete",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "create" => Some(ChangeOperationType::Create),
            "update" => Some(ChangeOperationType::Update),
            "delete" => Some(ChangeOperationType::Delete),
            "hard_delete" => Some(ChangeOperationType::HardDelete),
            _ => None,
        }
    }
}

impl From<ChangeOperationType> for String {
    fn from(op_type: ChangeOperationType) -> Self {
        op_type.as_str().to_string()
    }
}

/// The status of conflict resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolutionStatus {
    Resolved,
    Unresolved,
    Manual,
    Ignored,
}

/// The strategy for conflict resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolutionStrategy {
    ServerWins,
    ClientWins,
    LastWriteWins,
    MergePrioritizeServer,
    MergePrioritizeClient,
    Manual,
}

/// Sync priority for entities
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncPriority {
    Critical = 10,    // Sync immediately, retry aggressively
    High = 8,         // Sync early in the queue
    Normal = 5,       // Default priority
    Low = 3,          // Sync when convenient
    Background = 1,   // Sync only when resources available
}

impl From<i32> for SyncPriority {
    fn from(value: i32) -> Self {
        match value {
            10 => SyncPriority::Critical,
            8..=9 => SyncPriority::High,
            4..=7 => SyncPriority::Normal,
            2..=3 => SyncPriority::Low,
            _ => SyncPriority::Background,
        }
    }
}

impl From<SyncPriority> for i32 {
    fn from(priority: SyncPriority) -> Self {
        match priority {
            SyncPriority::Critical => 10,
            SyncPriority::High => 8,
            SyncPriority::Normal => 5,
            SyncPriority::Low => 3,
            SyncPriority::Background => 1,
        }
    }
}

/// Sync mode configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMode {
    /// Sync all available data (initial sync)
    Full,
    /// Sync only changes since last sync
    Incremental,
    /// Sync only essential data (metadata)
    Minimal,
    /// Sync only specific entities
    Selective,
}

/// Data purge strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataPurgeStrategy {
    /// Never purge this data
    Never,
    /// Purge when storage is low
    WhenStorageLow,
    /// Purge after specific time (e.g., 30 days)
    AfterTime,
    /// Purge after sync confirmed
    AfterSync,
    /// Purge immediately after use
    Immediate,
}

/// Represents a batch of changes for sync
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncBatch {
    pub batch_id: String,
    pub device_id: Uuid,
    pub direction: SyncDirection,
    pub status: SyncBatchStatus,
    pub item_count: Option<i64>,
    pub total_size: Option<i64>,
    pub priority: Option<i64>,
    pub attempts: Option<i64>,
    pub last_attempt_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Represents a device's sync state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSyncState {
    pub device_id: Uuid,
    pub user_id: Uuid,
    pub last_upload_timestamp: Option<DateTime<Utc>>,
    pub last_download_timestamp: Option<DateTime<Utc>>,
    pub last_sync_status: Option<DeviceSyncStatus>,
    pub last_sync_attempt_at: Option<DateTime<Utc>>,
    pub server_version: Option<i64>,
    pub sync_enabled: Option<bool>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Metadata about a device
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetadata {
    pub device_id: Uuid,
    pub name: String,
    pub platform: String,
    pub model: Option<String>,
    pub os_version: Option<String>,
    pub app_version: String,
    pub last_active_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Represents a change in the change log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeLogEntry {
    pub operation_id: Uuid,
    pub entity_table: String,
    pub entity_id: Uuid,
    pub operation_type: ChangeOperationType,
    pub field_name: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub user_id: Uuid,
    pub device_id: Option<Uuid>,
    pub sync_batch_id: Option<String>,
    pub processed_at: Option<DateTime<Utc>>,
    pub sync_error: Option<String>,
}

/// Represents a tombstone record for a hard-deleted entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tombstone {
    /// Unique ID for the tombstone
    pub id: Uuid,
    
    /// ID of the deleted entity
    pub entity_id: Uuid,
    
    /// Type of the deleted entity (table name)
    pub entity_type: String,
    
    /// User ID who performed the deletion
    pub deleted_by: Uuid,
    
    /// When the deletion occurred
    pub deleted_at: DateTime<Utc>,
    
    /// Operation ID for batch operations
    pub operation_id: Uuid,
}

impl Tombstone {
    /// Create a new tombstone with a generated operation ID
    pub fn new(
        entity_id: Uuid,
        entity_type: &str,
        deleted_by: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            entity_id,
            entity_type: entity_type.to_string(),
            deleted_by,
            deleted_at: Utc::now(),
            operation_id: Uuid::new_v4(),
        }
    }
    
    /// Create a new tombstone with a specific operation ID
    pub fn with_operation_id(
        entity_id: Uuid,
        entity_type: &str,
        deleted_by: Uuid,
        operation_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            entity_id,
            entity_type: entity_type.to_string(),
            deleted_by,
            deleted_at: Utc::now(),
            operation_id,
        }
    }
}

/// Application connection settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConnectionSettings {
    pub id: String, // Always "cloud"
    pub api_endpoint: String,
    pub api_version: Option<String>,
    pub connection_timeout: Option<i64>,
    pub offline_mode_enabled: Option<bool>,
    pub retry_count: Option<i64>,
    pub retry_delay: Option<i64>,
    pub updated_at: DateTime<Utc>,
}

/// Represents a conflict between local and remote changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    pub conflict_id: Uuid,
    pub entity_table: String,
    pub entity_id: Uuid,
    pub field_name: Option<String>,
    pub local_change: ChangeLogEntry,
    pub remote_change: ChangeLogEntry,
    pub resolution_status: ConflictResolutionStatus,
    pub resolution_strategy: Option<ConflictResolutionStrategy>,
    pub resolved_by_user_id: Option<Uuid>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Data Transfer Object for initializing a sync session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSessionInitDto {
    pub device_id: Uuid,
    pub user_id: Uuid,
    pub sync_mode: SyncMode,
    pub last_sync_timestamp: Option<DateTime<Utc>>,
    pub network_type: Option<String>,
    pub battery_level: Option<f64>,
    pub available_storage: Option<i64>,
    pub app_version: String,
}

/// Response from initializing a sync session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSessionInitResponse {
    pub session_id: String,
    pub server_time: DateTime<Utc>,
    pub last_known_client_sync: Option<DateTime<Utc>>,
    pub sync_mode_approved: SyncMode,
    pub estimated_download_size: Option<i64>,
    pub estimated_download_count: Option<i64>,
}

/// DTO for creating a new sync batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSyncBatchDto {
    pub device_id: Uuid,
    pub direction: SyncDirection,
    pub priority: Option<i64>,
}

/// DTO for uploading changes to server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadChangesDto {
    pub batch_id: String,
    pub device_id: Uuid,
    pub user_id: Uuid,
    pub changes: Vec<ChangeLogEntry>,
    pub tombstones: Option<Vec<Tombstone>>,
}

/// Response from uploading changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadChangesResponse {
    pub batch_id: String,
    pub changes_accepted: i64,
    pub changes_rejected: i64,
    pub conflicts_detected: i64,
    pub conflicts: Option<Vec<SyncConflict>>,
    pub server_timestamp: DateTime<Utc>,
}

/// DTO for downloading changes from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadChangesDto {
    pub batch_id: String,
    pub device_id: Uuid,
    pub user_id: Uuid,
    pub last_sync_timestamp: Option<DateTime<Utc>>,
    pub tables_requested: Option<Vec<String>>,
    pub entity_ids_requested: Option<HashMap<String, Vec<Uuid>>>,
    pub max_changes: Option<i64>,
    pub include_blobs: Option<bool>,
}

/// Response from downloading changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadChangesResponse {
    pub batch_id: String,
    pub changes: Vec<ChangeLogEntry>,
    pub tombstones: Option<Vec<Tombstone>>,
    pub has_more: bool,
    pub server_timestamp: DateTime<Utc>,
    pub next_batch_hint: Option<String>,
}

/// DTO for selective sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectiveSyncConfigDto {
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub table_config: HashMap<String, TableSyncConfig>,
    pub storage_quota_mb: Option<i64>,
}

/// Configuration for syncing a specific table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSyncConfig {
    pub table_name: String,
    pub enabled: bool,
    pub priority: SyncPriority,
    pub purge_strategy: DataPurgeStrategy,
    pub retention_days: Option<i64>,
    pub include_blobs: bool,
    pub sync_field_level: bool,  // Whether to sync field-level changes or just record-level
    pub filter_config: Option<TableFilterConfig>,
}

/// Filters for selective sync of a table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableFilterConfig {
    pub time_window_days: Option<i64>,
    pub related_to_user_only: Option<bool>,
    pub specific_ids: Option<Vec<Uuid>>,
    pub custom_filter: Option<String>, // JSON string with custom filter criteria
}

/// DTO for confirming changes have been processed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmChangesDto {
    pub batch_id: String,
    pub device_id: Uuid,
    pub user_id: Uuid,
    pub processed_change_ids: Vec<Uuid>,
    pub failed_change_ids: Option<Vec<Uuid>>,
    pub failure_details: Option<HashMap<Uuid, String>>,
}

/// Response from confirming changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmChangesResponse {
    pub batch_id: String,
    pub confirmation_status: String,
    pub server_timestamp: DateTime<Utc>,
}

/// Data about sync progress to report to user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub sync_in_progress: bool,
    pub current_operation: Option<String>,
    pub total_changes: i64,
    pub processed_changes: i64,
    pub pending_uploads: i64,
    pub pending_downloads: i64,
    pub last_sync_timestamp: Option<DateTime<Utc>>,
    pub last_sync_status: Option<DeviceSyncStatus>,
    pub sync_errors: Vec<String>,
    pub table_progress: Option<HashMap<String, TableSyncProgress>>,
}

/// Progress for a specific table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableSyncProgress {
    pub table_name: String,
    pub records_synced: i64,
    pub total_records: Option<i64>,
    pub bytes_transferred: Option<i64>,
    pub completed: bool,
}

/// Sync statistics for the app
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStats {
    pub total_uploads: i64,
    pub total_downloads: i64,
    pub failed_uploads: i64,
    pub failed_downloads: i64,
    pub conflicts_encountered: i64,
    pub conflicts_resolved_auto: i64,
    pub conflicts_resolved_manual: i64,
    pub conflicts_pending: i64,
    pub total_bytes_uploaded: i64,
    pub total_bytes_downloaded: i64,
    pub last_full_sync: Option<DateTime<Utc>>,
    pub avg_sync_duration_seconds: Option<f64>,
}

/// Record of a sync session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSession {
    pub session_id: String,
    pub user_id: Uuid,
    pub device_id: Uuid,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub sync_mode: SyncMode,
    pub status: DeviceSyncStatus,
    pub error_message: Option<String>,
    pub changes_uploaded: Option<i64>,
    pub changes_downloaded: Option<i64>,
    pub conflicts_encountered: Option<i64>,
    pub bytes_transferred: Option<i64>,
    pub network_type: Option<String>,
    pub duration_seconds: Option<f64>,
}

/// Configuration for sync scheduling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncScheduleConfig {
    pub auto_sync_enabled: bool,
    pub wifi_only: bool,
    pub charging_only: Option<bool>,
    pub min_battery_percentage: Option<i64>,
    pub background_sync_interval_minutes: Option<i64>,
    pub quiet_hours_start: Option<i64>, // Hour of day (0-23)
    pub quiet_hours_end: Option<i64>,   // Hour of day (0-23)
    pub max_sync_frequency_minutes: Option<i64>,
    pub allow_metered_connection: Option<bool>,
}

fn parse_uuid(uuid_str: &str, field_name: &str) -> Result<Uuid, DomainError> {
    Uuid::parse_str(uuid_str).map_err(|_| DomainError::Validation(ValidationError::format(
        field_name, &format!("Invalid UUID format: {}", uuid_str)
    )))
}

fn parse_optional_uuid(uuid_str: Option<String>, field_name: &str) -> Result<Option<Uuid>, DomainError> {
    uuid_str.map(|s| parse_uuid(&s, field_name)).transpose()
}

fn parse_datetime(dt_str: &str, field_name: &str) -> Result<DateTime<Utc>, DomainError> {
    DateTime::parse_from_rfc3339(dt_str)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| DomainError::Validation(ValidationError::format(
            field_name, &format!("Invalid RFC3339 format: {}", dt_str)
        )))
}

fn parse_optional_datetime(dt_str: Option<String>, field_name: &str) -> Result<Option<DateTime<Utc>>, DomainError> {
    dt_str.map(|s| parse_datetime(&s, field_name)).transpose()
}

#[derive(Debug, Clone, FromRow)]
pub struct SyncBatchRow {
    pub batch_id: String,
    pub device_id: String,
    pub direction: String,
    pub status: String,
    pub item_count: Option<i64>,
    pub total_size: Option<i64>,
    pub priority: Option<i64>,
    pub attempts: Option<i64>,
    pub last_attempt_at: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct DeviceSyncStateRow {
    pub device_id: String,
    pub user_id: String,
    pub last_upload_timestamp: Option<String>,
    pub last_download_timestamp: Option<String>,
    pub last_sync_status: Option<String>,
    pub last_sync_attempt_at: Option<String>,
    pub server_version: Option<i64>,
    pub sync_enabled: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct ChangeLogEntryRow {
    pub operation_id: String,
    pub entity_table: String,
    pub entity_id: String,
    pub operation_type: String,
    pub field_name: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub timestamp: String,
    pub user_id: String,
    pub device_id: Option<String>,
    pub sync_batch_id: Option<String>,
    pub processed_at: Option<String>,
    pub sync_error: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TombstoneRow {
    pub id: String,
    pub entity_id: String,
    pub entity_type: String,
    pub deleted_by: String,
    pub deleted_at: String,
    pub operation_id: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct AppConnectionSettingsRow {
    pub id: String,
    pub api_endpoint: String,
    pub api_version: Option<String>,
    pub connection_timeout: Option<i64>,
    pub offline_mode_enabled: Option<i64>,
    pub retry_count: Option<i64>,
    pub retry_delay: Option<i64>,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SyncConflictRow {
    pub conflict_id: String,
    pub entity_table: String,
    pub entity_id: String,
    pub field_name: Option<String>,
    pub local_change_op_id: String,
    pub remote_change_op_id: String,
    pub resolution_status: String,
    pub resolution_strategy: Option<String>,
    pub resolved_by_user_id: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SyncSessionRow {
    pub session_id: String,
    pub user_id: String,
    pub device_id: String,
    pub start_time: String,
    pub end_time: Option<String>,
    pub sync_mode: String,
    pub status: String,
    pub error_message: Option<String>,
    pub changes_uploaded: Option<i64>,
    pub changes_downloaded: Option<i64>,
    pub conflicts_encountered: Option<i64>,
    pub bytes_transferred: Option<i64>,
    pub network_type: Option<String>,
    pub duration_seconds: Option<f64>,
}

impl TryFrom<SyncBatchRow> for SyncBatch {
    type Error = DomainError;
    fn try_from(row: SyncBatchRow) -> Result<Self, Self::Error> {
        Ok(Self {
            batch_id: row.batch_id,
            device_id: parse_uuid(&row.device_id, "sync_batch.device_id")?,
            direction: SyncDirection::from_str(&row.direction)?,
            status: SyncBatchStatus::from_str(&row.status)?,
            item_count: row.item_count,
            total_size: row.total_size,
            priority: row.priority,
            attempts: row.attempts,
            last_attempt_at: parse_optional_datetime(row.last_attempt_at, "sync_batch.last_attempt_at")?,
            error_message: row.error_message,
            created_at: parse_datetime(&row.created_at, "sync_batch.created_at")?,
            completed_at: parse_optional_datetime(row.completed_at, "sync_batch.completed_at")?,
        })
    }
}

impl TryFrom<DeviceSyncStateRow> for DeviceSyncState {
    type Error = DomainError;
    fn try_from(row: DeviceSyncStateRow) -> Result<Self, Self::Error> {
        Ok(Self {
            device_id: parse_uuid(&row.device_id, "device_sync_state.device_id")?,
            user_id: parse_uuid(&row.user_id, "device_sync_state.user_id")?,
            last_upload_timestamp: parse_optional_datetime(row.last_upload_timestamp, "device_sync_state.last_upload_timestamp")?,
            last_download_timestamp: parse_optional_datetime(row.last_download_timestamp, "device_sync_state.last_download_timestamp")?,
            last_sync_status: row.last_sync_status.map(|s| DeviceSyncStatus::from_str(&s)).transpose()?,
            last_sync_attempt_at: parse_optional_datetime(row.last_sync_attempt_at, "device_sync_state.last_sync_attempt_at")?,
            server_version: row.server_version,
            sync_enabled: row.sync_enabled.map(|v| v == 1),
            created_at: parse_datetime(&row.created_at, "device_sync_state.created_at")?,
            updated_at: parse_datetime(&row.updated_at, "device_sync_state.updated_at")?,
        })
    }
}

impl TryFrom<ChangeLogEntryRow> for ChangeLogEntry {
    type Error = DomainError;
    fn try_from(row: ChangeLogEntryRow) -> Result<Self, Self::Error> {
        Ok(Self {
            operation_id: parse_uuid(&row.operation_id, "change_log.operation_id")?,
            entity_table: row.entity_table,
            entity_id: parse_uuid(&row.entity_id, "change_log.entity_id")?,
            operation_type: ChangeOperationType::from_str(&row.operation_type).ok_or_else(|| {
                DomainError::Validation(ValidationError::custom("Invalid ChangeOperationType"))
            })?,
            field_name: row.field_name,
            old_value: row.old_value,
            new_value: row.new_value,
            timestamp: parse_datetime(&row.timestamp, "change_log.timestamp")?,
            user_id: parse_uuid(&row.user_id, "change_log.user_id")?,
            device_id: parse_optional_uuid(row.device_id, "change_log.device_id")?,
            sync_batch_id: row.sync_batch_id,
            processed_at: parse_optional_datetime(row.processed_at, "change_log.processed_at")?,
            sync_error: row.sync_error,
        })
    }
}

impl TryFrom<TombstoneRow> for Tombstone {
    type Error = DomainError;
    fn try_from(row: TombstoneRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: parse_uuid(&row.id, "tombstone.id")?,
            entity_id: parse_uuid(&row.entity_id, "tombstone.entity_id")?,
            entity_type: row.entity_type,
            deleted_by: parse_uuid(&row.deleted_by, "tombstone.deleted_by")?,
            deleted_at: parse_datetime(&row.deleted_at, "tombstone.deleted_at")?,
            operation_id: parse_uuid(&row.operation_id, "tombstone.operation_id")?,
        })
    }
}

impl TryFrom<AppConnectionSettingsRow> for AppConnectionSettings {
    type Error = DomainError;
    fn try_from(row: AppConnectionSettingsRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            api_endpoint: row.api_endpoint,
            api_version: row.api_version,
            connection_timeout: row.connection_timeout,
            offline_mode_enabled: row.offline_mode_enabled.map(|v| v == 1),
            retry_count: row.retry_count,
            retry_delay: row.retry_delay,
            updated_at: parse_datetime(&row.updated_at, "app_connection_settings.updated_at")?,
        })
    }
}

impl TryFrom<SyncSessionRow> for SyncSession {
    type Error = DomainError;
    fn try_from(row: SyncSessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            session_id: row.session_id,
            user_id: parse_uuid(&row.user_id, "sync_session.user_id")?,
            device_id: parse_uuid(&row.device_id, "sync_session.device_id")?,
            start_time: parse_datetime(&row.start_time, "sync_session.start_time")?,
            end_time: parse_optional_datetime(row.end_time, "sync_session.end_time")?,
            sync_mode: SyncMode::from_str(&row.sync_mode)?,
            status: DeviceSyncStatus::from_str(&row.status)?,
            error_message: row.error_message,
            changes_uploaded: row.changes_uploaded,
            changes_downloaded: row.changes_downloaded,
            conflicts_encountered: row.conflicts_encountered,
            bytes_transferred: row.bytes_transferred,
            network_type: row.network_type,
            duration_seconds: row.duration_seconds,
        })
    }
}

impl FromStr for ConflictResolutionStatus {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "resolved" => Ok(Self::Resolved),
            "unresolved" => Ok(Self::Unresolved),
            "manual" => Ok(Self::Manual),
            "ignored" => Ok(Self::Ignored),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid ConflictResolutionStatus string: {}", s)
            )))
        }
    }
}

impl FromStr for ConflictResolutionStrategy {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "server_wins" => Ok(Self::ServerWins),
            "client_wins" => Ok(Self::ClientWins),
            "last_write_wins" => Ok(Self::LastWriteWins),
            "merge_prioritize_server" => Ok(Self::MergePrioritizeServer),
            "merge_prioritize_client" => Ok(Self::MergePrioritizeClient),
            "manual" => Ok(Self::Manual),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid ConflictResolutionStrategy string: {}", s)
            )))
        }
    }
}

impl FromStr for SyncMode {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "full" => Ok(Self::Full),
            "incremental" => Ok(Self::Incremental),
            "minimal" => Ok(Self::Minimal),
            "selective" => Ok(Self::Selective),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid SyncMode string: {}", s)
            )))
        }
    }
}