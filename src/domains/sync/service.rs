use std::sync::Arc;
use chrono::{DateTime, Utc};
use tokio::sync::Semaphore;
use uuid::Uuid;
use async_trait::async_trait;

use crate::auth::AuthContext;
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult};
use crate::domains::sync::types::{
    SyncStats, SyncPriority, SyncBatch, SyncBatchStatus, SyncDirection, ChangeLogEntry,
    Tombstone,
};
use crate::domains::sync::repository::{
    SyncRepository, ChangeLogRepository, TombstoneRepository,
};
use crate::domains::sync::entity_merger::EntityMerger;
use crate::domains::sync::cloud_storage::CloudStorageService;
use crate::domains::core::file_storage_service::FileStorageService;
use crate::domains::compression::service::CompressionService;
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::types::{CompressionStatus, BlobSyncStatus};

/// Maximum number of parallel uploads / downloads the sync service will run.
/// These values can later be made configurable (e.g. based on network type).
const DEFAULT_MAX_PARALLEL_UPLOADS: usize = 3;
const DEFAULT_MAX_PARALLEL_DOWNLOADS: usize = 3;

///  A small helper to construct an "empty" SyncStats instance.
fn empty_stats() -> SyncStats {
    SyncStats {
        total_uploads: 0,
        total_downloads: 0,
        failed_uploads: 0,
        failed_downloads: 0,
        conflicts_encountered: 0,
        conflicts_resolved_auto: 0,
        conflicts_resolved_manual: 0,
        conflicts_pending: 0,
        total_bytes_uploaded: 0,
        total_bytes_downloaded: 0,
        last_full_sync: None,
        avg_sync_duration_seconds: None,
    }
}

/// High-level trait for the synchronisation service.
#[async_trait]
pub trait SyncService: Send + Sync {
    /// Perform a full sync cycle (push then pull). Returns statistics for the cycle.
    async fn sync(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats>;

    /// Only push local changes (upload).
    async fn push(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats>;

    /// Only pull remote changes (download).
    async fn pull(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats>;
}

/// Implementation of the synchronisation service.
#[allow(dead_code)]
pub struct SyncServiceImpl {
    // Core repos
    sync_repo: Arc<dyn SyncRepository>,
    change_log_repo: Arc<dyn ChangeLogRepository>,
    tombstone_repo: Arc<dyn TombstoneRepository>,

    // Merge engine
    entity_merger: Arc<EntityMerger>,

    // IO / external services
    file_storage: Arc<dyn FileStorageService>,
    cloud_storage: Arc<dyn CloudStorageService>,
    compression_service: Option<Arc<dyn CompressionService>>,

    // Concurrency controls
    upload_sem: Arc<Semaphore>,
    download_sem: Arc<Semaphore>,
}

impl SyncServiceImpl {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sync_repo: Arc<dyn SyncRepository>,
        change_log_repo: Arc<dyn ChangeLogRepository>,
        tombstone_repo: Arc<dyn TombstoneRepository>,
        entity_merger: Arc<EntityMerger>,
        file_storage: Arc<dyn FileStorageService>,
        cloud_storage: Arc<dyn CloudStorageService>,
        compression_service: Option<Arc<dyn CompressionService>>,
        max_parallel_uploads: Option<usize>,
        max_parallel_downloads: Option<usize>,
    ) -> Self {
        let uploads = max_parallel_uploads.unwrap_or(DEFAULT_MAX_PARALLEL_UPLOADS);
        let downloads = max_parallel_downloads.unwrap_or(DEFAULT_MAX_PARALLEL_DOWNLOADS);

        Self {
            sync_repo,
            change_log_repo,
            tombstone_repo,
            entity_merger,
            file_storage,
            cloud_storage,
            compression_service,
            upload_sem: Arc::new(Semaphore::new(uploads)),
            download_sem: Arc::new(Semaphore::new(downloads)),
        }
    }

    /// Internal helper – build a new upload batch entry for bookkeeping.
    fn build_upload_batch(&self, device_id: Uuid) -> SyncBatch {
        SyncBatch {
            batch_id: Uuid::new_v4().to_string(),
            device_id,
            direction: SyncDirection::Upload,
            status: SyncBatchStatus::Pending,
            item_count: None,
            total_size: None,
            priority: Some(SyncPriority::High as i64),
            attempts: Some(0),
            last_attempt_at: None,
            error_message: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Internal helper – build a new download batch entry for bookkeeping.
    fn build_download_batch(&self, device_id: Uuid) -> SyncBatch {
        SyncBatch {
            batch_id: Uuid::new_v4().to_string(),
            device_id,
            direction: SyncDirection::Download,
            status: SyncBatchStatus::Pending,
            item_count: None,
            total_size: None,
            priority: Some(SyncPriority::High as i64),
            attempts: Some(0),
            last_attempt_at: None,
            error_message: None,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Push local change-log entries and tombstones to the server.
    async fn push_changes(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats> {
        let mut stats = empty_stats();
        let device_id = Uuid::parse_str(&auth.device_id).unwrap_or_else(|_| Uuid::nil());

        // 1. Collect changes & tombstones (basic strategy: gather high priority first, limit 1000)
        let changes = self.change_log_repo
            .find_unprocessed_changes_by_priority(SyncPriority::High, 1000)
            .await
            .map_err(ServiceError::Domain)?;

        let tombstones = self.tombstone_repo
            .find_unpushed_tombstones(500)
            .await
            .map_err(ServiceError::Domain)?;

        if changes.is_empty() && tombstones.is_empty() {
            return Ok(stats); // Nothing to push
        }

        // 2. Create batch record
        let mut batch = self.build_upload_batch(device_id);
        batch.item_count = Some(changes.len() as i64 + tombstones.len() as i64);
        self.sync_repo
            .create_sync_batch(&batch)
            .await
            .map_err(ServiceError::Domain)?;

        // 3. Prepare payload
        use crate::domains::sync::types::PushPayload;
        let payload = PushPayload {
            batch_id: batch.batch_id.clone(),
            device_id,
            user_id,
            changes: changes.clone(),
            tombstones: Some(tombstones.clone()),
        };

        // 4. Call remote
        let api_token = self.obtain_api_token(user_id).await?;
        let push_resp = self.cloud_storage
            .push_changes(&api_token, payload)
            .await?;

        // 5. Mark local changes/tombstones as processed (best-effort)
        {
            let db_pool = crate::globals::get_db_pool()
                .map_err(|ffi_err| ServiceError::Domain(DomainError::Internal(format!("Failed to get DB pool: {}", ffi_err))))?;
            let mut tx = db_pool.begin()
                .await
                .map_err(|e| ServiceError::Domain(DomainError::Database(e.into())))?;

            for change in &changes {
                self.change_log_repo
                    .mark_as_processed(change.operation_id, &batch.batch_id, Utc::now(), &mut tx)
                    .await
                    .map_err(ServiceError::Domain)?;
            }

            for tomb in &tombstones {
                self.tombstone_repo
                    .mark_as_pushed(tomb.id, &batch.batch_id, Utc::now(), &mut tx)
                    .await
                    .map_err(ServiceError::Domain)?;
            }

            tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(e.into())))?;
        }

        // 6. Update stats (basic)
        stats.total_uploads = push_resp.changes_accepted;
        stats.failed_uploads = push_resp.changes_rejected + push_resp.conflicts_detected;

        // TODO: update other stats fields once server returns them.

        // 7. Finalise batch
        self.sync_repo
            .finalize_sync_batch(&batch.batch_id, SyncBatchStatus::Completed, None, changes.len() as u32)
            .await
            .map_err(ServiceError::Domain)?;

        Ok(stats)
    }

    /// Pull remote changes and merge them locally.
    async fn pull_changes(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats> {
        let mut stats = empty_stats();
        let device_id = Uuid::parse_str(&auth.device_id).unwrap_or_else(|_| Uuid::nil());

        // 1. Determine sync token / last server state via sync_repo
        let config = self.sync_repo.get_sync_config(user_id).await.map_err(ServiceError::Domain)?;
        let token = config.server_token.clone();

        // 2. Create download batch
        let mut batch = self.build_download_batch(device_id);
        self.sync_repo.create_sync_batch(&batch).await.map_err(ServiceError::Domain)?;

        // 3. Fetch changes from server
        let api_token = self.obtain_api_token(user_id).await?;
        let fetch_resp = self.cloud_storage
            .get_changes_since(&api_token, device_id, token)
            .await?;

        // 4. Apply changes locally using entity merger
        let applied_ids = self.entity_merger
            .apply_changes_batch(&fetch_resp.changes, auth)
            .await
            .map_err(ServiceError::Domain)?;

        // 5. Apply tombstones
        for tomb in &fetch_resp.tombstones.clone().unwrap_or_default() {
            if let Err(e) = self.entity_merger.apply_tombstone(tomb, auth).await {
                // Log but continue (TODO: create conflict record?)
                log::error!("Failed to apply tombstone {:?}: {:?}", tomb, e);
                stats.failed_downloads += 1;
            } else {
                stats.total_downloads += 1;
            }
        }

        stats.total_downloads += applied_ids.len() as i64;

        // 6. Save new server token from response (if provided)
        self.sync_repo
            .update_sync_state_token(user_id, Some(fetch_resp.server_timestamp.to_rfc3339()))
            .await
            .map_err(ServiceError::Domain)?;

        // 7. Finalise batch
        self.sync_repo
            .finalize_sync_batch(&batch.batch_id, SyncBatchStatus::Completed, None, applied_ids.len() as u32)
            .await
            .map_err(ServiceError::Domain)?;

        Ok(stats)
    }

    /// Obtain an API token for the given user. In the current code-base this lives
    /// inside the sync configuration. Later we may query the AuthService.
    async fn obtain_api_token(&self, user_id: Uuid) -> ServiceResult<String> {
        let config = self.sync_repo.get_sync_config(user_id).await.map_err(ServiceError::Domain)?;
        config
            .server_token
            .ok_or_else(|| ServiceError::Domain(DomainError::AuthorizationFailed("No API token configured for sync".to_string())))
    }

    /// Move a single document file to the server, respecting compression rules.
    ///
    /// 1. If a compressed file exists -> upload that and mark remote key.
    /// 2. If not compressed, upload original.
    /// 3. If original already uploaded (blob_key present) – skip compressed upload.
    async fn upload_document_if_needed(&self, document_id: Uuid, auth: &AuthContext) -> ServiceResult<()> {
        // Access the global repo via globals helper
        let media_repo = crate::globals::get_media_document_repo()
            .map_err(|e| ServiceError::Domain(DomainError::Internal(format!("Failed to get media document repo: {}", e))))?;
        let doc = MediaDocumentRepository::find_by_id(&*media_repo, document_id)
            .await
            .map_err(ServiceError::Domain)?;

        // If already uploaded, nothing to do
        if doc.blob_key.is_some() {
            return Ok(());
        }

        // Determine file path (compressed vs original)
        let (path_to_upload, _is_compressed) = match self.compression_service.clone() {
            Some(cs) => {
                match cs.get_document_compression_status(document_id).await {
                    Ok(status) => {
                        if matches!(status, CompressionStatus::Completed) {
                            if let Some(cpath) = &doc.compressed_file_path {
                                (cpath.clone(), true)
                            } else {
                                (doc.file_path.clone(), false)
                            }
                        } else {
                            (doc.file_path.clone(), false)
                        }
                    }
                    Err(_) => {
                        // Fallback: upload original
                        (doc.file_path.clone(), false)
                    }
                }
            }
            None => {
                // Compression disabled → upload original
                (doc.file_path.clone(), false)
            }
        };

        // Acquire permit to limit concurrency
        let _permit = self.upload_sem.acquire().await.unwrap();

        // Stat the file for size
        let metadata = tokio::fs::metadata(&path_to_upload).await.map_err(|e| ServiceError::Domain(DomainError::File(format!("Unable to stat file: {}", e))))?;

        // Upload
        let uploader_device_id = Uuid::parse_str(&auth.device_id).unwrap_or_else(|_| Uuid::nil());
        let blob_key = self.cloud_storage
            .upload_document(uploader_device_id, document_id, &path_to_upload, &doc.mime_type, metadata.len())
            .await?;

        // Update local record with blob_key (best-effort)
        if let Err(e) = media_repo.update_blob_sync_status(document_id, BlobSyncStatus::Synced, Some(&blob_key)).await {
            log::error!("Failed to update blob status for doc {} after upload: {:?}", document_id, e);
        }

        Ok(())
    }
}

#[async_trait]
impl SyncService for SyncServiceImpl {
    async fn sync(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats> {
        // Push then pull. We could run in parallel but sequential simplifies ordering.
        let mut stats_total = empty_stats();

        let push_stats = self.push_changes(user_id, auth).await?;
        stats_total.total_uploads += push_stats.total_uploads;
        stats_total.failed_uploads += push_stats.failed_uploads;

        let pull_stats = self.pull_changes(user_id, auth).await?;
        stats_total.total_downloads += pull_stats.total_downloads;
        stats_total.failed_downloads += pull_stats.failed_downloads;

        Ok(stats_total)
    }

    async fn push(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats> {
        self.push_changes(user_id, auth).await
    }

    async fn pull(&self, user_id: Uuid, auth: &AuthContext) -> ServiceResult<SyncStats> {
        self.pull_changes(user_id, auth).await
    }
} 