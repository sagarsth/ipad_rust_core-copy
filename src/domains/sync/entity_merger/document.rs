// sync/entity_merger/document.rs

use super::{DomainEntityMerger, BaseDomainMerger};
use async_trait::async_trait;
use crate::auth::AuthContext;
use crate::errors::{DomainResult, DomainError};
use crate::domains::sync::types::{ChangeLogEntry, MergeOutcome, Tombstone};
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::types::MediaDocument;
use crate::domains::core::delete_service::{DeleteService, DeleteOptions};
use sqlx::{Transaction, Sqlite, SqlitePool};
use std::sync::Arc;
use uuid::Uuid;

pub struct DocumentEntityMerger {
    repo: Arc<dyn MediaDocumentRepository + Send + Sync>,
    pool: SqlitePool,
    delete_service: Arc<dyn DeleteService<MediaDocument> + Send + Sync>,
}

impl DocumentEntityMerger {
    pub fn new(
        repo: Arc<dyn MediaDocumentRepository + Send + Sync>,
        pool: SqlitePool,
        delete_service: Arc<dyn DeleteService<MediaDocument> + Send + Sync>,
    ) -> Self {
        Self {
            repo,
            pool,
            delete_service,
        }
    }
}

#[async_trait]
impl DomainEntityMerger for DocumentEntityMerger {
    fn entity_table(&self) -> &'static str {
        "media_documents" // As per your schema
    }

    async fn apply_create(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local document create change: {}", change.operation_id);
            return Ok(());
        }
        log::info!("Applying remote CREATE for document: {}", change.entity_id);

        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => log::info!("Created document {} from remote change", id),
                    MergeOutcome::Updated(id) => log::info!("Updated existing document {} from remote CREATE change (upsert)", id),
                    MergeOutcome::NoOp(reason) => log::info!("NoOp for remote document create: {}", reason),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => {
                        log::warn!("Conflict detected for document {} during create: {}", entity_id, reason);
                        // TODO: enqueue a SyncConflict row
                    }
                    MergeOutcome::HardDeleted(id) => {
                        log::error!("Unexpected HardDeleted outcome for document {} from a CREATE operation", id);
                        let _ = tx.rollback().await;
                        return Err(DomainError::Internal(format!("CREATE operation resulted in HardDeleted for document {}", id)));
                    }
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                log::error!("Error applying remote CREATE for document {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }

    async fn apply_update(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local document update change: {}", change.operation_id);
            return Ok(());
        }
        log::info!("Applying remote UPDATE for document: {} (Field: {:?})", change.entity_id, change.field_name);

        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => {
                         log::warn!("Updated non-existent document {} from remote UPDATE change (created instead)", id);
                    }
                    MergeOutcome::Updated(id) => log::info!("Updated document {} from remote change", id),
                    MergeOutcome::NoOp(reason) => log::info!("NoOp for remote document update: {}", reason),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => {
                        log::warn!("Conflict detected for document {} during update: {}", entity_id, reason);
                        // TODO: enqueue a SyncConflict row
                    }
                    MergeOutcome::HardDeleted(id) => {
                        log::error!("Unexpected HardDeleted outcome for document {} from an UPDATE operation", id);
                        let _ = tx.rollback().await;
                        return Err(DomainError::Internal(format!("UPDATE operation resulted in HardDeleted for document {}", id)));
                    }
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                log::error!("Error applying remote UPDATE for document {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }

    async fn apply_soft_delete(&self, change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> {
        // Soft deletes for documents are usually handled by breaking links from parent entities.
        // A direct soft delete operation on a document from remote is uncommon.
        // If a document is truly marked for deletion, it should be a hard delete via tombstone.
        log::warn!("Ignoring remote soft delete for document {}. Document soft deletes are typically implicit via link removal or lead to hard deletes.", change.entity_id);
        Ok(())
    }

    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()> {
        let entity_id = tombstone.entity_id;
        log::info!("Applying remote HARD DELETE for document: {}", entity_id);

        if tombstone.entity_type != self.entity_table() {
            return Err(DomainError::Internal(format!(
                "Tombstone entity type mismatch: expected '{}', got '{}'",
                self.entity_table(), tombstone.entity_type
            )));
        }

        // Use DeleteService to handle hard delete. This ensures that:
        // 1. The media_documents record is deleted.
        // 2. The physical file is queued for deletion via PendingDeletionManager.
        // 3. Appropriate changelog/tombstone entries are made for the document itself if not already handled by a parent cascade.
        let options = DeleteOptions {
            allow_hard_delete: true,
            fallback_to_soft_delete: false, // Document hard deletes are usually definitive.
            force: true, // Force ensures cleanup as it's a sync operation.
        };

        match self.delete_service.delete(entity_id, auth, options).await {
            Ok(crate::domains::core::repository::DeleteResult::HardDeleted) => {
                log::info!("Hard deleted document {} from remote tombstone via DeleteService", entity_id);
                Ok(())
            }
            Ok(unexpected_result) => {
                log::error!(
                    "Unexpected outcome from DeleteService for document {} during hard delete: {:?}",
                    entity_id, unexpected_result
                );
                Err(DomainError::Internal(format!(
                    "Unexpected outcome from DeleteService for document hard delete: {:?}",
                    unexpected_result
                )))
            }
            Err(DomainError::EntityNotFound(_, _)) => {
                log::info!("Document {} not found locally for hard delete via DeleteService, skipping", entity_id);
                Ok(()) // It's okay if the entity is already gone
            }
            Err(e) => {
                log::error!("Error applying remote HARD DELETE for document {} via DeleteService: {:?}", entity_id, e);
                Err(e)
            }
        }
    }

    async fn apply_change_with_tx<'t>(
        &self,
        change: &ChangeLogEntry,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>
    ) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local document change (with tx): {}", change.operation_id);
            return Ok(());
        }

        match self.repo.merge_remote_change(tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => log::info!("(TX) Created document {} from remote change", id),
                    MergeOutcome::Updated(id) => log::info!("(TX) Updated document {} from remote change", id),
                    MergeOutcome::NoOp(reason) => log::info!("(TX) NoOp for remote document change: {}", reason),
                    MergeOutcome::HardDeleted(id) => {
                        // This path in merge_remote_change is less common for documents if hard deletes always go via tombstones + DeleteService.
                        // However, if a ChangeLogEntry *could* represent a hard delete directly:
                        log::info!("(TX) HardDeleted document {} from remote change via merge_remote_change", id);
                        // Ensure this path in repo.merge_remote_change correctly handles associated file cleanup if it implies that.
                    }
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => {
                        log::warn!("(TX) Conflict detected for document {} during change: {}", entity_id, reason);
                        // TODO: enqueue a SyncConflict row.
                    }
                }
                Ok(())
            },
            Err(e) => {
                log::error!("(TX) Error applying remote change for document {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }
} 