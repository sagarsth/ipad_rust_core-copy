use std::collections::HashMap;
use sqlx::{SqlitePool, Sqlite, Transaction, query, query_as, QueryBuilder, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use async_trait::async_trait;
use serde_json::json;

use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::auth::AuthContext;
use crate::domains::sync::types::{
    SyncPriority, SyncBatchStatus, SyncDirection, SyncBatch, SyncConfig, SyncStatus,
    DeviceSyncState, ChangeLogEntry, ChangeOperationType, Tombstone, SyncConflict,
};

/// Repository for sync-related operations and tracking
#[async_trait]
pub trait SyncRepository: Send + Sync {
    /// Create a new sync batch for tracking a sync operation
    async fn create_sync_batch(&self, batch: &SyncBatch) -> DomainResult<()>;

    /// Update a sync batch's status
    async fn update_sync_batch_status(
        &self,
        batch_id: &str,
        status: SyncBatchStatus,
        error_message: Option<&str>
    ) -> DomainResult<()>;

    /// Update batch stats within a transaction
    async fn update_batch_stats<'t>(
        &self,
        batch_id: &str,
        processed: u32,
        conflicts: u32,
        errors: u32,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()>;

    /// Finalize a sync batch with overall results
    async fn finalize_sync_batch(
        &self,
        batch_id: &str,
        status: SyncBatchStatus,
        error_message: Option<&str>,
        total_processed: u32
    ) -> DomainResult<()>;

    /// Get a user's sync configuration
    async fn get_sync_config(&self, user_id: Uuid) -> DomainResult<SyncConfig>;

    /// Update a user's sync configuration
    async fn update_sync_config(&self, config: &SyncConfig) -> DomainResult<()>;

    /// Get a user's sync status overview
    async fn get_sync_status(&self, user_id: Uuid) -> DomainResult<SyncStatus>;

    /// Update the sync state token (server_token in sync_configs)
    async fn update_sync_state_token(&self, user_id: Uuid, token: Option<String>) -> DomainResult<()>;

    /// Get device sync state
    async fn get_device_sync_state(&self, device_id: Uuid) -> DomainResult<DeviceSyncState>;

    /// Update device sync state
    async fn update_device_sync_state(&self, state: &DeviceSyncState) -> DomainResult<()>;

    /// Log a sync conflict
    async fn log_sync_conflict<'t>(
        &self,
        conflict: &SyncConflict,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()>;

    /// Find conflicts for a batch
    async fn find_conflicts_for_batch(&self, batch_id: &str) -> DomainResult<Vec<ChangeLogEntry>>;
}

/// Repository for change log operations
#[async_trait]
pub trait ChangeLogRepository: Send + Sync {
    /// Create a new change log entry
    async fn create_change_log(&self, entry: &ChangeLogEntry) -> DomainResult<()>;

    /// Create a new change log entry within a transaction
    async fn create_change_log_with_tx<'t>(
        &self,
        entry: &ChangeLogEntry,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()>;

    /// Find unprocessed changes by priority
    async fn find_unprocessed_changes_by_priority(
        &self,
        priority: SyncPriority,
        limit: u32
    ) -> DomainResult<Vec<ChangeLogEntry>>;

    /// Mark a change log entry as processed
    async fn mark_as_processed<'t>(
        &self,
        operation_id: Uuid,
        batch_id: &str,
        timestamp: DateTime<Utc>,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()>;

    /// Get changes for a specific entity
    async fn get_changes_for_entity(
        &self,
        entity_table: &str,
        entity_id: Uuid,
        limit: u32
    ) -> DomainResult<Vec<ChangeLogEntry>>;

    /// Get last change timestamp for an entity field
    async fn get_last_field_change_timestamp(
        &self,
        entity_table: &str,
        entity_id: Uuid,
        field_name: &str
    ) -> DomainResult<Option<DateTime<Utc>>>;
}

/// Repository for tombstone operations
#[async_trait]
pub trait TombstoneRepository: Send + Sync {
    /// Create a new tombstone
    async fn create_tombstone(&self, tombstone: &Tombstone) -> DomainResult<()>;

    /// Create a new tombstone within a transaction
    async fn create_tombstone_with_tx<'t>(
        &self,
        tombstone: &Tombstone,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()>;

    /// Find unpushed tombstones for sync
    async fn find_unpushed_tombstones(&self, limit: u32) -> DomainResult<Vec<Tombstone>>;

    /// Mark a tombstone as pushed (by marking its change_log entry)
    async fn mark_as_pushed<'t>(
        &self,
        tombstone_id: Uuid,
        batch_id: &str,
        timestamp: DateTime<Utc>,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()>;

    /// Check if entity was already tombstoned
    async fn check_entity_tombstoned(
        &self,
        entity_type: &str,
        entity_id: Uuid
    ) -> DomainResult<bool>;

    /// Find tombstones since a specific date
    async fn find_tombstones_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
        table_filter: Option<&str>
    ) -> DomainResult<Vec<Tombstone>>;
}

/// SQLite implementation of the SyncRepository
pub struct SqliteSyncRepository {
    pool: SqlitePool,
}

impl SqliteSyncRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SyncRepository for SqliteSyncRepository {
    async fn create_sync_batch(&self, batch: &SyncBatch) -> DomainResult<()> {
        let mut builder = QueryBuilder::new(
            "INSERT INTO sync_batches (batch_id, device_id, direction, status, "
        );
        builder.push("item_count, total_size, priority, attempts, last_attempt_at, error_message, created_at, completed_at) VALUES (");
        
        let batch_id = &batch.batch_id;
        let device_id = batch.device_id.to_string();
        let direction = batch.direction.as_str();
        let status = batch.status.as_str();
        let created_at = batch.created_at.to_rfc3339();
        
        builder.push_bind(batch_id);
        builder.push(", ");
        builder.push_bind(device_id);
        builder.push(", ");
        builder.push_bind(direction);
        builder.push(", ");
        builder.push_bind(status);
        builder.push(", ");
        
        // Handle optional fields
        if let Some(item_count) = batch.item_count {
            builder.push_bind(item_count);
        } else {
            builder.push("NULL");
        }
        builder.push(", ");
        
        if let Some(total_size) = batch.total_size {
            builder.push_bind(total_size);
        } else {
            builder.push("NULL");
        }
        builder.push(", ");
        
        if let Some(priority) = batch.priority {
            builder.push_bind(priority);
        } else {
            builder.push("NULL");
        }
        builder.push(", ");
        
        if let Some(attempts) = batch.attempts {
            builder.push_bind(attempts);
        } else {
            builder.push("NULL");
        }
        builder.push(", ");
        
        if let Some(last_attempt) = batch.last_attempt_at {
            let last_attempt_str = last_attempt.to_rfc3339();
            builder.push_bind(last_attempt_str);
        } else {
            builder.push("NULL");
        }
        builder.push(", ");
        
        if let Some(error_msg) = &batch.error_message {
            builder.push_bind(error_msg);
        } else {
            builder.push("NULL");
        }
        builder.push(", ");
        
        builder.push_bind(created_at);
        builder.push(", ");
        
        if let Some(completed_at) = batch.completed_at {
            let completed_at_str = completed_at.to_rfc3339();
            builder.push_bind(completed_at_str);
        } else {
            builder.push("NULL");
        }
        
        builder.push(")");
        
        let query = builder.build();
        query.execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }

    async fn update_sync_batch_status(
        &self, 
        batch_id: &str, 
        status: SyncBatchStatus,
        error_message: Option<&str>
    ) -> DomainResult<()> {
        let mut query_builder = QueryBuilder::new(
            "UPDATE sync_batches SET status = "
        );
        
        let status_str = status.as_str();
        query_builder.push_bind(status_str);
        query_builder.push(", attempts = attempts + 1, last_attempt_at = ");
        query_builder.push("strftime('%Y-%m-%dT%H:%M:%fZ', 'now')");
        
        query_builder.push(", error_message = ");
        if let Some(msg) = error_message {
            query_builder.push_bind(msg);
        } else {
            query_builder.push("NULL");
        }
        
        query_builder.push(" WHERE batch_id = ");
        query_builder.push_bind(batch_id);
        
        let query = query_builder.build();
        query.execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }

    async fn update_batch_stats<'t>(
        &self,
        batch_id: &str,
        processed: u32,
        _conflicts: u32,
        errors: u32,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()> {
        query(
            r#"
            UPDATE sync_batches
            SET
                item_count = item_count + ?,
                status = CASE WHEN ? > 0 THEN 'partially_failed' ELSE status END
            WHERE batch_id = ?
            "#,
        )
        .bind(processed as i64)
        .bind(errors as i64)
        .bind(batch_id)
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        Ok(())
    }

    async fn finalize_sync_batch(
        &self,
        batch_id: &str,
        status: SyncBatchStatus,
        error_message: Option<&str>,
        total_processed: u32
    ) -> DomainResult<()> {
        let mut builder = QueryBuilder::new(
            "UPDATE sync_batches SET status = "
        );
        
        let status_str = status.as_str();
        builder.push_bind(status_str);
        builder.push(", error_message = ");
        
        if let Some(msg) = error_message {
            builder.push_bind(msg);
        } else {
            builder.push("NULL");
        }
        
        builder.push(", item_count = ");
        builder.push_bind(total_processed as i64);
        
        builder.push(", completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')");
        builder.push(" WHERE batch_id = ");
        builder.push_bind(batch_id);
        
        let query = builder.build();
        query.execute(&self.pool)
            .await
            .map_err(DbError::from)?;

        Ok(())
    }

    async fn get_sync_config(&self, user_id: Uuid) -> DomainResult<SyncConfig> {
        let user_id_str = user_id.to_string();
        
        let row = query!(
            r#"
            SELECT 
                user_id,
                sync_interval_minutes,
                background_sync_enabled,
                wifi_only,
                charging_only,
                sync_priority_threshold,
                document_sync_enabled,
                metadata_sync_enabled,
                server_token,
                last_sync_timestamp,
                created_at,
                updated_at
            FROM sync_configs 
            WHERE user_id = ?
            "#,
            user_id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?;

        match row {
            Some(row) => {
                let last_sync = match &row.last_sync_timestamp {
                    Some(ts) => Some(
                        DateTime::parse_from_rfc3339(ts)
                            .map_err(|_| DomainError::Validation(ValidationError::format(
                                "last_sync_timestamp", &format!("Invalid RFC3339 format: {}", ts)
                            )))?
                            .with_timezone(&Utc)
                    ),
                    None => None,
                };

                let created_at = DateTime::parse_from_rfc3339(&row.created_at)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "created_at", &format!("Invalid RFC3339 format: {}", row.created_at)
                    )))?
                    .with_timezone(&Utc);

                let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "updated_at", &format!("Invalid RFC3339 format: {}", row.updated_at)
                    )))?
                    .with_timezone(&Utc);
                
                Ok(SyncConfig {
                    user_id,
                    sync_interval_minutes: row.sync_interval_minutes,
                    background_sync_enabled: row.background_sync_enabled == 1,
                    wifi_only: row.wifi_only == 1,
                    charging_only: row.charging_only == 1,
                    sync_priority_threshold: row.sync_priority_threshold,
                    document_sync_enabled: row.document_sync_enabled == 1,
                    metadata_sync_enabled: row.metadata_sync_enabled == 1,
                    server_token: row.server_token,
                    last_sync_timestamp: last_sync,
                    created_at,
                    updated_at,
                })
            },
            None => {
                // Create default config
                let now = Utc::now();
                let default_config = SyncConfig {
                    user_id,
                    sync_interval_minutes: 60,
                    background_sync_enabled: true,
                    wifi_only: true,
                    charging_only: false,
                    sync_priority_threshold: 1,
                    document_sync_enabled: true,
                    metadata_sync_enabled: true,
                    server_token: None,
                    last_sync_timestamp: None,
                    created_at: now,
                    updated_at: now,
                };

                let config_id = Uuid::new_v4().to_string();
                let now_str = now.to_rfc3339();
                let user_id_str = user_id.to_string();
                let bg_sync_enabled = if default_config.background_sync_enabled { 1 } else { 0 };
                let wifi_only = if default_config.wifi_only { 1 } else { 0 };
                let charging_only = if default_config.charging_only { 1 } else { 0 };
                let doc_sync_enabled = if default_config.document_sync_enabled { 1 } else { 0 };
                let meta_sync_enabled = if default_config.metadata_sync_enabled { 1 } else { 0 };

                query!(
                    r#"
                    INSERT INTO sync_configs (
                        id, user_id, sync_interval_minutes, background_sync_enabled, wifi_only,
                        charging_only, sync_priority_threshold, document_sync_enabled,
                        metadata_sync_enabled, server_token, last_sync_timestamp, created_at, updated_at
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    "#,
                    config_id,
                    user_id_str,
                    default_config.sync_interval_minutes,
                    bg_sync_enabled,
                    wifi_only,
                    charging_only,
                    default_config.sync_priority_threshold,
                    doc_sync_enabled,
                    meta_sync_enabled,
                    default_config.server_token,
                    None::<String>, // last_sync_timestamp is None initially
                    now_str,
                    now_str
                )
                .execute(&self.pool)
                .await
                .map_err(DbError::from)?;

                Ok(default_config)
            }
        }
    }

    async fn update_sync_config(&self, config: &SyncConfig) -> DomainResult<()> {
        let now_str = Utc::now().to_rfc3339();
        let last_sync_str = config.last_sync_timestamp.map(|dt| dt.to_rfc3339());
        let user_id_str = config.user_id.to_string();
        let bg_sync_enabled = if config.background_sync_enabled { 1 } else { 0 };
        let wifi_only = if config.wifi_only { 1 } else { 0 };
        let charging_only = if config.charging_only { 1 } else { 0 };
        let doc_sync_enabled = if config.document_sync_enabled { 1 } else { 0 };
        let meta_sync_enabled = if config.metadata_sync_enabled { 1 } else { 0 };

        query!(
            r#"
            UPDATE sync_configs SET
                sync_interval_minutes = ?,
                background_sync_enabled = ?,
                wifi_only = ?,
                charging_only = ?,
                sync_priority_threshold = ?,
                document_sync_enabled = ?,
                metadata_sync_enabled = ?,
                server_token = ?,
                last_sync_timestamp = ?,
                updated_at = ?
            WHERE user_id = ?
            "#,
            config.sync_interval_minutes,
            bg_sync_enabled,
            wifi_only,
            charging_only,
            config.sync_priority_threshold,
            doc_sync_enabled,
            meta_sync_enabled,
            config.server_token,
            last_sync_str,
            now_str,
            user_id_str
        )
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(())
    }

    async fn get_sync_status(&self, user_id: Uuid) -> DomainResult<SyncStatus> {
        // Get basic config
        let config = self.get_sync_config(user_id).await?;
        
        // Get device sync state with last attempt timestamp
        let user_id_str = user_id.to_string();
        let device_state_row = query!(
            r#"
            SELECT 
                device_id,
                last_sync_attempt_at
            FROM device_sync_state
            WHERE user_id = ?
            ORDER BY last_sync_attempt_at DESC
            LIMIT 1
            "#,
            user_id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let last_device_sync = match device_state_row {
            Some(row) => match &row.last_sync_attempt_at {
                Some(ts) => DateTime::parse_from_rfc3339(ts).ok().map(|dt| dt.with_timezone(&Utc)),
                None => None
            },
            None => None
        };
        
        // Get pending changes count
        let pending_changes = query!(
            r#"
            SELECT COUNT(*) as count 
            FROM change_log 
            WHERE user_id = ? AND sync_batch_id IS NULL AND processed_at IS NULL
            "#,
            user_id_str
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?
        .count;
        
        // Get pending document uploads count
        let pending_docs = query!(
            r#"
            SELECT COUNT(*) as count 
            FROM media_documents 
            WHERE blob_status = 'pending' AND deleted_at IS NULL 
            AND created_by_user_id = ?
            "#,
            user_id_str
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?
        .count;
        
        // Check for active sync
        let active_syncs = query!(
            r#"
            SELECT COUNT(*) as count 
            FROM sync_batches sb
            JOIN device_sync_state dss ON sb.device_id = dss.device_id
            WHERE dss.user_id = ? AND sb.status IN ('pending', 'processing')
            "#,
            user_id_str
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?
        .count;

        Ok(SyncStatus {
            user_id,
            last_sync_timestamp: config.last_sync_timestamp,
            last_device_sync,
            sync_enabled: config.background_sync_enabled,
            offline_mode: false,
            pending_changes,
            pending_documents: pending_docs,
            sync_in_progress: active_syncs > 0,
        })
    }

    async fn update_sync_state_token(&self, user_id: Uuid, token: Option<String>) -> DomainResult<()> {
        let user_id_str = user_id.to_string();
        
        query!(
            r#"
            UPDATE sync_configs
            SET server_token = ?,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE user_id = ?
            "#,
            token,
            user_id_str
        )
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok(())
    }

    async fn get_device_sync_state(&self, device_id: Uuid) -> DomainResult<DeviceSyncState> {
        let device_id_str = device_id.to_string();
        
        let row = query!(
            r#"
            SELECT 
                device_id,
                user_id,
                last_upload_timestamp,
                last_download_timestamp,
                last_sync_status,
                last_sync_attempt_at,
                server_version,
                sync_enabled,
                created_at,
                updated_at
            FROM device_sync_state
            WHERE device_id = ?
            "#,
            device_id_str
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        match row {
            Some(row) => {
                // Parse user_id
                let user_id = Uuid::parse_str(&row.user_id)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "user_id", &format!("Invalid UUID format: {}", row.user_id)
                    )))?;
                
                // Parse timestamps
                let last_upload = match &row.last_upload_timestamp {
                    Some(ts) => Some(
                        DateTime::parse_from_rfc3339(ts)
                            .map_err(|_| DomainError::Validation(ValidationError::format(
                                "last_upload_timestamp", &format!("Invalid RFC3339 format: {}", ts)
                            )))?
                            .with_timezone(&Utc)
                    ),
                    None => None,
                };
                
                let last_download = match &row.last_download_timestamp {
                    Some(ts) => Some(
                        DateTime::parse_from_rfc3339(ts)
                            .map_err(|_| DomainError::Validation(ValidationError::format(
                                "last_download_timestamp", &format!("Invalid RFC3339 format: {}", ts)
                            )))?
                            .with_timezone(&Utc)
                    ),
                    None => None,
                };
                
                let last_attempt = match &row.last_sync_attempt_at {
                    Some(ts) => Some(
                        DateTime::parse_from_rfc3339(ts)
                            .map_err(|_| DomainError::Validation(ValidationError::format(
                                "last_sync_attempt_at", &format!("Invalid RFC3339 format: {}", ts)
                            )))?
                            .with_timezone(&Utc)
                    ),
                    None => None,
                };
                
                let created_at = DateTime::parse_from_rfc3339(&row.created_at)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "created_at", &format!("Invalid RFC3339 format: {}", row.created_at)
                    )))?
                    .with_timezone(&Utc);
                    
                let updated_at = DateTime::parse_from_rfc3339(&row.updated_at)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "updated_at", &format!("Invalid RFC3339 format: {}", row.updated_at)
                    )))?
                    .with_timezone(&Utc);
                
                // Convert last_sync_status string to DeviceSyncStatus enum if present
                let last_sync_status = match &row.last_sync_status {
                    Some(status_str) => match status_str.as_str() {
                        "success" => Some(crate::domains::sync::types::DeviceSyncStatus::Success),
                        "partial_success" => Some(crate::domains::sync::types::DeviceSyncStatus::PartialSuccess),
                        "failed" => Some(crate::domains::sync::types::DeviceSyncStatus::Failed),
                        "in_progress" => Some(crate::domains::sync::types::DeviceSyncStatus::InProgress),
                        _ => None
                    },
                    None => None
                };
                
                Ok(DeviceSyncState {
                    device_id,
                    user_id,
                    last_upload_timestamp: last_upload,
                    last_download_timestamp: last_download,
                    last_sync_status,
                    last_sync_attempt_at: last_attempt,
                    server_version: Some(row.server_version.unwrap_or(0)),
                    sync_enabled: Some(row.sync_enabled.unwrap_or(0) == 1),
                    created_at,
                    updated_at,
                })
            },
            None => Err(DomainError::EntityNotFound("DeviceSyncState".to_string(), device_id))
        }
    }

    async fn update_device_sync_state(&self, state: &DeviceSyncState) -> DomainResult<()> {
        let now_str = Utc::now().to_rfc3339();
        let device_id_str = state.device_id.to_string();
        let user_id_str = state.user_id.to_string();
        let last_upload_str = state.last_upload_timestamp.map(|dt| dt.to_rfc3339());
        let last_download_str = state.last_download_timestamp.map(|dt| dt.to_rfc3339());
        let last_attempt_str = state.last_sync_attempt_at.map(|dt| dt.to_rfc3339());
        let created_at_str = state.created_at.to_rfc3339();
        
        // Convert DeviceSyncStatus enum to string if present
        let last_sync_status_str = state.last_sync_status.as_ref().map(|s| s.as_str());
        
        // Prepare values that would create temporary values in the query
        let server_version = state.server_version.unwrap_or(0);
        let sync_enabled = if state.sync_enabled.unwrap_or(false) { 1 } else { 0 };
        
        query!(
            r#"
            INSERT INTO device_sync_state (
                device_id, user_id, last_upload_timestamp, last_download_timestamp,
                last_sync_status, last_sync_attempt_at, server_version, sync_enabled,
                created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(device_id) DO UPDATE SET
                user_id = excluded.user_id,
                last_upload_timestamp = excluded.last_upload_timestamp,
                last_download_timestamp = excluded.last_download_timestamp,
                last_sync_status = excluded.last_sync_status,
                last_sync_attempt_at = excluded.last_sync_attempt_at,
                server_version = excluded.server_version,
                sync_enabled = excluded.sync_enabled,
                updated_at = excluded.updated_at
            "#,
            device_id_str,
            user_id_str,
            last_upload_str,
            last_download_str,
            last_sync_status_str,
            last_attempt_str,
            server_version,
            sync_enabled,
            created_at_str,
            now_str
        )
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok(())
    }

    async fn log_sync_conflict<'t>(
        &self,
        conflict: &SyncConflict,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()> {
        let mut details_map = HashMap::new();
        
        // Extract values for the JSON details
        if let Some(val) = &conflict.local_change.new_value {
            details_map.insert("local_value", val.clone());
        }
        if let Some(val) = &conflict.remote_change.new_value {
            details_map.insert("remote_value", val.clone());
        }
        
        let details_json = json!(details_map).to_string();
        
        // Prepare string values
        let conflict_id_str = conflict.conflict_id.to_string();
        let entity_id_str = conflict.entity_id.to_string();
        let local_op_id_str = conflict.local_change.operation_id.to_string();
        let remote_op_id_str = conflict.remote_change.operation_id.to_string();
        let resolution_status_str = conflict.resolution_status.as_str();
        let resolution_strategy_str = conflict.resolution_strategy.as_ref().map(|s| s.as_str());
        let resolved_by_str = conflict.resolved_by_user_id.map(|id| id.to_string());
        let resolved_at_str = conflict.resolved_at.map(|dt| dt.to_rfc3339());
        let created_at_str = conflict.created_at.to_rfc3339();
        
        query!(
            r#"
            INSERT INTO sync_conflicts (
                conflict_id, entity_table, entity_id, field_name,
                local_change_op_id, remote_change_op_id, resolution_status,
                resolution_strategy, resolved_by_user_id, resolved_at, created_at, details
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            conflict_id_str,
            conflict.entity_table,
            entity_id_str,
            conflict.field_name,
            local_op_id_str,
            remote_op_id_str,
            resolution_status_str,
            resolution_strategy_str,
            resolved_by_str,
            resolved_at_str,
            created_at_str,
            details_json
        )
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        
        Ok(())
    }

    async fn find_conflicts_for_batch(&self, batch_id: &str) -> DomainResult<Vec<ChangeLogEntry>> {
        // Helper function to safely get a &str from an Option<String>
        fn field_unwrap(opt: &Option<String>) -> &str {
            opt.as_deref().unwrap_or("")
        }
        
        let rows = query!(
            r#"
            SELECT 
                operation_id as "operation_id!", entity_table as "entity_table!",
                entity_id as "entity_id!", operation_type as "operation_type!",
                field_name, old_value, new_value, document_metadata,
                timestamp as "timestamp!", user_id as "user_id!", device_id,
                sync_batch_id, processed_at, sync_error
            FROM change_log
            WHERE sync_batch_id = ? AND sync_error IS NOT NULL
            "#,
            batch_id
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let operation_id = Uuid::parse_str(&row.operation_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "operation_id", &format!("Invalid UUID: {}", &row.operation_id)
                )))?;
                
            let entity_id = Uuid::parse_str(&row.entity_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "entity_id", &format!("Invalid UUID: {}", &row.entity_id)
                )))?;
                
            let user_id = Uuid::parse_str(&row.user_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "user_id", &format!("Invalid UUID: {}", &row.user_id)
                )))?;
                
            let device_id = match &row.device_id {
                Some(id) => Some(Uuid::parse_str(id)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "device_id", &format!("Invalid UUID: {}", id)
                    )))?),
                None => None
            };
            
            let timestamp = DateTime::parse_from_rfc3339(&row.timestamp)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "timestamp", &format!("Invalid RFC3339: {}", &row.timestamp)
                )))?
                .with_timezone(&Utc);
                
            let processed_at = match &row.processed_at {
                Some(ts) => Some(DateTime::parse_from_rfc3339(ts)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "processed_at", &format!("Invalid RFC3339: {}", ts)
                    )))?
                    .with_timezone(&Utc)),
                None => None
            };
            
            let operation_type = match row.operation_type.as_str() {
                "create" => ChangeOperationType::Create,
                "update" => ChangeOperationType::Update,
                "delete" => ChangeOperationType::Delete,
                "hard_delete" => ChangeOperationType::HardDelete,
                _ => ChangeOperationType::Update // Default
            };
            
            entries.push(ChangeLogEntry {
                operation_id,
                entity_table: row.entity_table.to_string(),
                entity_id,
                operation_type,
                field_name: row.field_name.clone(),
                old_value: row.old_value.clone(),
                new_value: row.new_value.clone(),
                timestamp,
                user_id,
                device_id,
                document_metadata: row.document_metadata.clone(),
                sync_batch_id: row.sync_batch_id.clone(),
                processed_at,
                sync_error: row.sync_error.clone(),
            });
        }
        
        Ok(entries)
    }
}

/// SQLite implementation of ChangeLogRepository
pub struct SqliteChangeLogRepository {
    pool: SqlitePool,
}

impl SqliteChangeLogRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChangeLogRepository for SqliteChangeLogRepository {
    async fn create_change_log(&self, entry: &ChangeLogEntry) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        self.create_change_log_with_tx(entry, &mut tx).await?;
        tx.commit().await.map_err(DbError::from)?;
        Ok(())
    }

    async fn create_change_log_with_tx<'t>(
        &self,
        entry: &ChangeLogEntry,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()> {
        // Generate priority value based on operation type
        let priority_value = match entry.operation_type {
            ChangeOperationType::Create => 7,
            ChangeOperationType::Update => 5,
            ChangeOperationType::Delete => 8,
            ChangeOperationType::HardDelete => 9,
        };
        
        // Convert values to strings
        let operation_id_str = entry.operation_id.to_string();
        let entity_id_str = entry.entity_id.to_string();
        let operation_type_str = entry.operation_type.as_str();
        let timestamp_str = entry.timestamp.to_rfc3339();
        let user_id_str = entry.user_id.to_string();
        let device_id_str = entry.device_id.map(|id| id.to_string());
        let processed_at_str = entry.processed_at.map(|dt| dt.to_rfc3339());
        
        query!(
            r#"
            INSERT INTO change_log (
                operation_id, entity_table, entity_id, operation_type, field_name,
                old_value, new_value, document_metadata, timestamp, user_id, device_id,
                sync_batch_id, processed_at, sync_error, priority
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            operation_id_str,
            entry.entity_table,
            entity_id_str,
            operation_type_str,
            entry.field_name,
            entry.old_value,
            entry.new_value,
            entry.document_metadata,
            timestamp_str,
            user_id_str,
            device_id_str,
            entry.sync_batch_id,
            processed_at_str,
            entry.sync_error,
            priority_value
        )
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        
        Ok(())
    }

    async fn find_unprocessed_changes_by_priority(
        &self,
        priority: SyncPriority,
        limit: u32
    ) -> DomainResult<Vec<ChangeLogEntry>> {
        // Helper function to safely get a &str from an Option<String>
        fn field_unwrap(opt: &Option<String>) -> &str {
            opt.as_deref().unwrap_or("")
        }
        
        let priority_val = match priority {
            SyncPriority::High => 8,
            SyncPriority::Normal => 5,
            SyncPriority::Low => 3,
            SyncPriority::Never => 0,
        };
        
        // Convert limit to i64 to avoid temporary value in query! macro
        let limit_val = limit as i64;
        
        let rows = query!(
            r#"
            SELECT 
                operation_id as "operation_id!", entity_table as "entity_table!", 
                entity_id as "entity_id!", operation_type as "operation_type!", 
                field_name, old_value, new_value, document_metadata, 
                timestamp as "timestamp!", user_id as "user_id!", device_id,
                sync_batch_id, processed_at, sync_error
            FROM change_log
            WHERE processed_at IS NULL AND sync_batch_id IS NULL
            AND priority >= ?
            ORDER BY priority DESC, timestamp ASC
            LIMIT ?
            "#,
            priority_val,
            limit_val
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let operation_id = Uuid::parse_str(&row.operation_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "operation_id", &format!("Invalid UUID: {}", &row.operation_id)
                )))?;
                
            let entity_id = Uuid::parse_str(&row.entity_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "entity_id", &format!("Invalid UUID: {}", &row.entity_id)
                )))?;
                
            let user_id = Uuid::parse_str(&row.user_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "user_id", &format!("Invalid UUID: {}", &row.user_id)
                )))?;
                
            let device_id = match &row.device_id {
                Some(id) => Some(Uuid::parse_str(id)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "device_id", &format!("Invalid UUID: {}", id)
                    )))?),
                None => None
            };
            
            let timestamp = DateTime::parse_from_rfc3339(&row.timestamp)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "timestamp", &format!("Invalid RFC3339: {}", &row.timestamp)
                )))?
                .with_timezone(&Utc);
                
            let processed_at = match &row.processed_at {
                Some(ts) => Some(DateTime::parse_from_rfc3339(ts)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "processed_at", &format!("Invalid RFC3339: {}", ts)
                    )))?
                    .with_timezone(&Utc)),
                None => None
            };
            
            let operation_type = match row.operation_type.as_str() {
                "create" => ChangeOperationType::Create,
                "update" => ChangeOperationType::Update,
                "delete" => ChangeOperationType::Delete,
                "hard_delete" => ChangeOperationType::HardDelete,
                _ => ChangeOperationType::Update // Default
            };
            
            entries.push(ChangeLogEntry {
                operation_id,
                entity_table: row.entity_table.to_string(),
                entity_id,
                operation_type,
                field_name: row.field_name.clone(),
                old_value: row.old_value.clone(),
                new_value: row.new_value.clone(),
                timestamp,
                user_id,
                device_id,
                document_metadata: row.document_metadata.clone(),
                sync_batch_id: row.sync_batch_id.clone(),
                processed_at,
                sync_error: row.sync_error.clone(),
            });
        }
        
        Ok(entries)
    }

    async fn mark_as_processed<'t>(
        &self,
        operation_id: Uuid,
        batch_id: &str,
        timestamp: DateTime<Utc>,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()> {
        let timestamp_str = timestamp.to_rfc3339();
        let operation_id_str = operation_id.to_string();
        
        query!(
            r#"
            UPDATE change_log
            SET processed_at = ?,
                sync_batch_id = ?
            WHERE operation_id = ?
            "#,
            timestamp_str,
            batch_id,
            operation_id_str
        )
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        
        Ok(())
    }

    async fn get_changes_for_entity(
        &self,
        entity_table: &str,
        entity_id: Uuid,
        limit: u32
    ) -> DomainResult<Vec<ChangeLogEntry>> {
        let entity_id_str = entity_id.to_string();
        let limit_i64 = limit as i64;
        
        let rows = query!(
            r#"
            SELECT 
                operation_id as "operation_id!", entity_table as "entity_table!", 
                entity_id as "entity_id!", operation_type as "operation_type!", 
                field_name, old_value, new_value, document_metadata, 
                timestamp as "timestamp!", user_id as "user_id!", device_id,
                sync_batch_id, processed_at, sync_error
            FROM change_log
            WHERE entity_table = ? AND entity_id = ?
            ORDER BY timestamp DESC
            LIMIT ?
            "#,
            entity_table,
            entity_id_str,
            limit_i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut entries = Vec::with_capacity(rows.len());
        for row in rows {
            let operation_id = Uuid::parse_str(&row.operation_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "operation_id", &format!("Invalid UUID: {}", &row.operation_id)
                )))?;
                
            let row_entity_id = Uuid::parse_str(&row.entity_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "entity_id", &format!("Invalid UUID: {}", &row.entity_id)
                )))?;
                
            let user_id = Uuid::parse_str(&row.user_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "user_id", &format!("Invalid UUID: {}", &row.user_id)
                )))?;
                
            let device_id = match &row.device_id {
                Some(id) => Some(Uuid::parse_str(id)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "device_id", &format!("Invalid UUID: {}", id)
                    )))?),
                None => None
            };
            
            let timestamp = DateTime::parse_from_rfc3339(&row.timestamp)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "timestamp", &format!("Invalid RFC3339: {}", &row.timestamp)
                )))?
                .with_timezone(&Utc);
                
            let processed_at = match &row.processed_at {
                Some(ts) => Some(DateTime::parse_from_rfc3339(ts)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "processed_at", &format!("Invalid RFC3339: {}", ts)
                    )))?
                    .with_timezone(&Utc)),
                None => None
            };
            
            let operation_type = match row.operation_type.as_str() {
                "create" => ChangeOperationType::Create,
                "update" => ChangeOperationType::Update,
                "delete" => ChangeOperationType::Delete,
                "hard_delete" => ChangeOperationType::HardDelete,
                _ => ChangeOperationType::Update // Default
            };
            
            entries.push(ChangeLogEntry {
                operation_id,
                entity_table: row.entity_table.to_string(),
                entity_id: row_entity_id,
                operation_type,
                field_name: row.field_name.clone(),
                old_value: row.old_value.clone(),
                new_value: row.new_value.clone(),
                timestamp,
                user_id,
                device_id,
                document_metadata: row.document_metadata.clone(),
                sync_batch_id: row.sync_batch_id.clone(),
                processed_at,
                sync_error: row.sync_error.clone(),
            });
        }
        
        Ok(entries)
    }

    async fn get_last_field_change_timestamp(
        &self,
        entity_table: &str,
        entity_id: Uuid,
        field_name: &str
    ) -> DomainResult<Option<DateTime<Utc>>> {
        let entity_id_str = entity_id.to_string();
        
        let row = query!(
            r#"
            SELECT timestamp FROM change_log
            WHERE entity_table = ? AND entity_id = ? AND field_name = ?
            ORDER BY timestamp DESC
            LIMIT 1
            "#,
            entity_table,
            entity_id_str,
            field_name
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        match row {
            Some(row) => {
                let timestamp = DateTime::parse_from_rfc3339(&row.timestamp)
                    .map_err(|_| DomainError::Validation(ValidationError::format(
                        "timestamp", &format!("Invalid RFC3339: {}", row.timestamp)
                    )))?
                    .with_timezone(&Utc);
                
                Ok(Some(timestamp))
            },
            None => Ok(None)
        }
    }
}

/// SQLite implementation of TombstoneRepository
pub struct SqliteTombstoneRepository {
    pool: SqlitePool,
}

impl SqliteTombstoneRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TombstoneRepository for SqliteTombstoneRepository {
    async fn create_tombstone(&self, tombstone: &Tombstone) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        self.create_tombstone_with_tx(tombstone, &mut tx).await?;
        tx.commit().await.map_err(DbError::from)?;
        Ok(())
    }

    async fn create_tombstone_with_tx<'t>(&self, tombstone: &Tombstone, tx: &mut Transaction<'t, Sqlite>) -> DomainResult<()> {
        // Convert values to strings
        let id_str = tombstone.id.to_string();
        let entity_id_str = tombstone.entity_id.to_string();
        let deleted_by_str = tombstone.deleted_by.to_string();
        let deleted_at_str = tombstone.deleted_at.to_rfc3339();
        let operation_id_str = tombstone.operation_id.to_string();
        
        query!(
            r#"
            INSERT INTO tombstones (
                id, entity_id, entity_type, deleted_by, deleted_at, operation_id, additional_metadata
            ) VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
            id_str,
            entity_id_str,
            tombstone.entity_type,
            deleted_by_str,
            deleted_at_str,
            operation_id_str,
            tombstone.additional_metadata
        )
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        
        Ok(())
    }

    async fn find_unpushed_tombstones(&self, limit: u32) -> DomainResult<Vec<Tombstone>> {
        // Helper function to safely get a &str from an Option<String>
        fn field_unwrap(opt: &Option<String>) -> &str {
            opt.as_deref().unwrap_or("")
        }
        
        let limit_i64 = limit as i64;
        
        let rows = query!(
            r#"
            SELECT 
                id as "id!", entity_id as "entity_id!", entity_type as "entity_type!", 
                deleted_by as "deleted_by!", deleted_at as "deleted_at!", 
                operation_id as "operation_id!", additional_metadata
            FROM tombstones
            WHERE pushed_at IS NULL AND sync_batch_id IS NULL
            LIMIT ?
            "#,
            limit_i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut tombstones = Vec::with_capacity(rows.len());
        for row in rows {
            let id = Uuid::parse_str(&row.id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "id", &format!("Invalid UUID: {}", &row.id)
                )))?;
                
            let entity_id = Uuid::parse_str(&row.entity_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "entity_id", &format!("Invalid UUID: {}", &row.entity_id)
                )))?;
                
            let deleted_by = Uuid::parse_str(&row.deleted_by)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "deleted_by", &format!("Invalid UUID: {}", &row.deleted_by)
                )))?;
                
            let operation_id = Uuid::parse_str(&row.operation_id)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "operation_id", &format!("Invalid UUID: {}", &row.operation_id)
                )))?;
            
            let deleted_at = DateTime::parse_from_rfc3339(&row.deleted_at)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "deleted_at", &format!("Invalid RFC3339: {}", &row.deleted_at)
                )))?
                .with_timezone(&Utc);
            
            tombstones.push(Tombstone {
                id,
                entity_id,
                entity_type: row.entity_type.to_string(),
                deleted_by,
                deleted_at,
                operation_id,
                additional_metadata: row.additional_metadata.clone()
            });
        }
        
        Ok(tombstones)
    }

    async fn mark_as_pushed<'t>(
        &self,
        tombstone_id: Uuid,
        batch_id: &str,
        timestamp: DateTime<Utc>,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()> {
        let timestamp_str = timestamp.to_rfc3339();
        let tombstone_id_str = tombstone_id.to_string();
        
        query!(
            r#"
            UPDATE tombstones
            SET pushed_at = ?,
                sync_batch_id = ?
            WHERE id = ?
            "#,
            timestamp_str,
            batch_id,
            tombstone_id_str
        )
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        
        Ok(())
    }

    async fn check_entity_tombstoned(&self, entity_type: &str, entity_id: Uuid) -> DomainResult<bool> {
        let entity_id_str = entity_id.to_string();
        
        let count = query!(
            r#"
            SELECT COUNT(*) as count FROM tombstones
            WHERE entity_type = ? AND entity_id = ?
            "#,
            entity_type,
            entity_id_str
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok(count.count > 0)
    }
    
    async fn find_tombstones_since(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
        table_filter: Option<&str>
    ) -> DomainResult<Vec<Tombstone>> {
        let user_id_str = user_id.to_string();
        let since_str = since.to_rfc3339();
        
        // Build query with conditional filter
        let mut sql = String::from(
            "SELECT id, entity_id, entity_type, deleted_by, deleted_at, operation_id, additional_metadata
             FROM tombstones
             WHERE deleted_by = ? AND deleted_at >= ?"
        );
        
        if let Some(table) = table_filter {
            sql.push_str(&format!(" AND entity_type = '{}'", table));
        }
        
        sql.push_str(" ORDER BY deleted_at DESC");
        
        // We need to use query_as_with for dynamic SQL
        let query = sqlx::query(&sql)
            .bind(user_id_str)
            .bind(since_str);
            
        let rows = query.fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        let mut tombstones = Vec::with_capacity(rows.len());
        for row in rows {
            // You need to extract values from each column by name since we're not using query! macro
            let id_str: String = row.get("id");
            let entity_id_str: String = row.get("entity_id");
            let entity_type: String = row.get("entity_type");
            let deleted_by_str: String = row.get("deleted_by");
            let deleted_at_str: String = row.get("deleted_at");
            let operation_id_str: String = row.get("operation_id");
            let additional_metadata: Option<String> = row.get("additional_metadata");
            
            let id = Uuid::parse_str(&id_str)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "id", &format!("Invalid UUID: {}", &id_str)
                )))?;
                
            let entity_id = Uuid::parse_str(&entity_id_str)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "entity_id", &format!("Invalid UUID: {}", &entity_id_str)
                )))?;
                
            let deleted_by = Uuid::parse_str(&deleted_by_str)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "deleted_by", &format!("Invalid UUID: {}", &deleted_by_str)
                )))?;
                
            let operation_id = Uuid::parse_str(&operation_id_str)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "operation_id", &format!("Invalid UUID: {}", &operation_id_str)
                )))?;
            
            let deleted_at = DateTime::parse_from_rfc3339(&deleted_at_str)
                .map_err(|_| DomainError::Validation(ValidationError::format(
                    "deleted_at", &format!("Invalid RFC3339: {}", &deleted_at_str)
                )))?
                .with_timezone(&Utc);
            
            tombstones.push(Tombstone {
                id,
                entity_id,
                entity_type,
                deleted_by,
                deleted_at,
                operation_id,
                additional_metadata
            });
        }
        
        Ok(tombstones)
    }
}
