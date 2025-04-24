use crate::domains::core::file_storage_service::{FileStorageService, FileStorageError};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, DbError};
use sqlx::{SqlitePool, Row};
use uuid::Uuid;
use std::sync::Arc;
use chrono::{Utc, Duration};
use tokio::time;

/// Worker for processing file deletions in the background
pub struct FileDeletionWorker {
    pool: SqlitePool,
    file_storage_service: Arc<dyn FileStorageService>,
    shutdown_signal: Option<tokio::sync::oneshot::Receiver<()>>,
    active_files_check_enabled: bool,
}

impl FileDeletionWorker {
    pub fn new(
        pool: SqlitePool,
        file_storage_service: Arc<dyn FileStorageService>,
    ) -> Self {
        Self {
            pool,
            file_storage_service,
            shutdown_signal: None,
            active_files_check_enabled: true,
        }
    }
    
    /// Set shutdown signal receiver
    pub fn with_shutdown_signal(mut self, receiver: tokio::sync::oneshot::Receiver<()>) -> Self {
        self.shutdown_signal = Some(receiver);
        self
    }
    
    /// Disable active files check (useful for testing or cleanup mode)
    pub fn disable_active_files_check(mut self) -> Self {
        self.active_files_check_enabled = false;
        self
    }
    
    /// Start the worker loop
    pub async fn start(mut self) -> Result<(), ServiceError> {
        log::info!("Starting file deletion worker");
        
        // Run every 5 minutes
        let mut interval = time::interval(time::Duration::from_secs(5 * 60));
        
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(e) = self.process_deletion_queue().await {
                        log::error!("Error processing file deletion queue: {:?}", e);
                    }
                }
                _ = async {
                    if let Some(mut signal) = self.shutdown_signal.take() {
                        let _ = signal.await;
                        true
                    } else {
                        // Never complete if no shutdown signal
                        std::future::pending::<bool>().await
                    }
                } => {
                    log::info!("Received shutdown signal, stopping file deletion worker");
                    break;
                }
            }
        }
        
        Ok(())
    }
    
    /// Process the pending items in the file deletion queue
    async fn process_deletion_queue(&self) -> Result<(), ServiceError> {
        log::info!("Processing file deletion queue");
        
        // Get pending deletions where grace period has expired
        let pending_deletions = self.get_pending_deletions().await?;
        
        if pending_deletions.is_empty() {
            log::info!("No pending file deletions to process");
            return Ok(());
        }
        
        log::info!("Found {} pending file deletions", pending_deletions.len());
        
        for deletion in pending_deletions {
            // Skip files that are currently in use if active check is enabled
            if self.active_files_check_enabled && self.is_file_in_use(&deletion.document_id).await? {
                log::info!("Skipping file in use: document_id={}", deletion.document_id);
                
                // Update the last attempt timestamp
                let now_str = Utc::now().to_rfc3339();
                sqlx::query!(
                    r#"
                    UPDATE file_deletion_queue
                    SET 
                        last_attempt_at = ?,
                        attempts = attempts + 1
                    WHERE id = ?
                    "#,
                    now_str,
                    deletion.id
                )
                .execute(&self.pool)
                .await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                
                continue;
            }
            
            // Try to delete the original file
            let original_result = if let Some(path) = &deletion.file_path {
                self.file_storage_service.delete_file(path).await
            } else {
                Ok(()) // No path, so "success"
            };
            
            // Try to delete the compressed file if it exists
            let compressed_result = if let Some(path) = &deletion.compressed_file_path {
                self.file_storage_service.delete_file(path).await
            } else {
                Ok(()) // No compressed path, so "success"
            };
            
            // Check results and update database
            let mut error_message = None;
            
            if let Err(e) = &original_result {
                if !matches!(e, FileStorageError::NotFound(_)) {
                    // Only log real errors, not just missing files
                    error_message = Some(format!("Error deleting original file: {}", e));
                }
            }
            
            if let Err(e) = &compressed_result {
                if !matches!(e, FileStorageError::NotFound(_)) {
                    let compressed_err = format!("Error deleting compressed file: {}", e);
                    error_message = match error_message {
                        Some(msg) => Some(format!("{}; {}", msg, compressed_err)),
                        None => Some(compressed_err),
                    };
                }
            }
            
            // Update database - mark as completed if both successfully deleted or not found
            if original_result.is_ok() && compressed_result.is_ok() {
                let now_str = Utc::now().to_rfc3339();
                sqlx::query!(
                    r#"
                    UPDATE file_deletion_queue
                    SET 
                        completed_at = ?,
                        last_attempt_at = ?,
                        attempts = attempts + 1,
                        error_message = NULL
                    WHERE id = ?
                    "#,
                    now_str,
                    now_str,
                    deletion.id
                )
                .execute(&self.pool)
                .await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                
                log::info!("Successfully deleted files for document: {}", deletion.document_id);
            } else {
                // Update attempt count and error message
                let now_str = Utc::now().to_rfc3339();
                sqlx::query!(
                    r#"
                    UPDATE file_deletion_queue
                    SET 
                        last_attempt_at = ?,
                        attempts = attempts + 1,
                        error_message = ?
                    WHERE id = ?
                    "#,
                    now_str,
                    error_message,
                    deletion.id
                )
                .execute(&self.pool)
                .await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                
                log::warn!(
                    "Failed to delete files for document: {} - Error: {:?}",
                    deletion.document_id,
                    error_message
                );
            }
        }
        
        Ok(())
    }
    
    /// Get pending deletions that are ready to be processed (grace period expired)
    async fn get_pending_deletions(&self) -> Result<Vec<PendingFileDeletion>, ServiceError> {
        let rows = sqlx::query!(
            r#"
            SELECT 
                id as "id!",
                document_id as "document_id!",
                file_path as "file_path: Option<String>",
                compressed_file_path as "compressed_file_path: Option<String>",
                requested_at as "requested_at!",
                attempts
            FROM file_deletion_queue
            WHERE 
                completed_at IS NULL AND 
                datetime(requested_at) <= datetime('now', '-' || grace_period_seconds || ' seconds')
            ORDER BY
                attempts ASC, -- Try not-yet-attempted files first
                requested_at ASC -- Then oldest first
            LIMIT 100 -- Process in batches
            "#
        )
        .fetch_all(&self.pool)
        .await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        let mut result = Vec::with_capacity(rows.len());
        
        for row in rows {
            result.push(PendingFileDeletion {
                id: row.id,
                document_id: row.document_id,
                file_path: row.file_path,
                compressed_file_path: row.compressed_file_path.flatten(),
                requested_at: row.requested_at,
                attempts: row.attempts.unwrap_or(0),
            });
        }
        
        Ok(result)
    }
    
    /// Check if file is currently in use
    async fn is_file_in_use(&self, document_id: &str) -> Result<bool, ServiceError> {
        // Check active_file_usage table
        let result = sqlx::query!(
            r#"
            SELECT EXISTS(
                SELECT 1 
                FROM active_file_usage 
                WHERE 
                    document_id = ? AND 
                    last_active_at > datetime('now', '-5 minutes')
            ) as in_use
            "#,
            document_id
        )
        .fetch_one(&self.pool)
        .await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        Ok(result.in_use == 1)
    }
}

/// Struct representing a pending file deletion
struct PendingFileDeletion {
    id: String,
    document_id: String,
    file_path: Option<String>,
    compressed_file_path: Option<String>,
    requested_at: String,
    attempts: i64,
} 