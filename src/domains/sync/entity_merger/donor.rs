use super::{DomainEntityMerger, BaseDomainMerger};
use crate::domains::donor::repository::DonorRepository;
use crate::domains::sync::types::{ChangeLogEntry, MergeOutcome, Tombstone};
use crate::auth::AuthContext;
use crate::errors::{DomainResult, DomainError};
use crate::domains::core::delete_service::{DeleteService, DeleteOptions};
use sqlx::{Transaction, Sqlite, SqlitePool};
use async_trait::async_trait;
use std::sync::Arc;

pub struct DonorEntityMerger {
    repo: Arc<dyn DonorRepository + Send + Sync>,
    pool: SqlitePool,
    delete_service: Arc<dyn DeleteService<crate::domains::donor::Donor> + Send + Sync>,
}

impl DonorEntityMerger {
    pub fn new(
        repo: Arc<dyn DonorRepository + Send + Sync>,
        pool: SqlitePool,
        delete_service: Arc<dyn DeleteService<crate::domains::donor::Donor> + Send + Sync>,
    ) -> Self {
        Self {
            repo,
            pool,
            delete_service,
        }
    }
}

#[async_trait]
impl DomainEntityMerger for DonorEntityMerger {
    fn entity_table(&self) -> &'static str { "donors" }

    async fn apply_create(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local donor create change: {}", change.operation_id);
            return Ok(());
        }
        log::info!("Applying remote CREATE for donor: {}", change.entity_id);

        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => log::info!("Created donor {} from remote change", id),
                    MergeOutcome::Updated(id) => log::info!("Updated existing donor {} from remote CREATE change (upsert)", id),
                    MergeOutcome::NoOp(reason) => log::info!("NoOp for remote donor create: {}", reason),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => {
                        log::warn!("Conflict detected for donor {} during create: {}", entity_id, reason);
                    }
                    MergeOutcome::HardDeleted(id) => {
                        log::error!("Unexpected HardDeleted outcome for donor {} from a CREATE operation", id);
                        let _ = tx.rollback().await;
                        return Err(DomainError::Internal(format!("CREATE operation resulted in HardDeleted for donor {}", id)));
                    }
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                log::error!("Error applying remote CREATE for donor {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }

    async fn apply_update(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local donor update change: {}", change.operation_id);
            return Ok(());
        }
        log::info!("Applying remote UPDATE for donor: {} (Field: {:?})", change.entity_id, change.field_name);

        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => {
                         log::warn!("Updated non-existent donor {} from remote UPDATE change (created instead)", id);
                    }
                    MergeOutcome::Updated(id) => log::info!("Updated donor {} from remote change", id),
                    MergeOutcome::NoOp(reason) => log::info!("NoOp for remote donor update: {}", reason),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => {
                        log::warn!("Conflict detected for donor {} during update: {}", entity_id, reason);
                    }
                    MergeOutcome::HardDeleted(id) => {
                        log::error!("Unexpected HardDeleted outcome for donor {} from an UPDATE operation", id);
                        let _ = tx.rollback().await;
                        return Err(DomainError::Internal(format!("UPDATE operation resulted in HardDeleted for donor {}", id)));
                    }
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                log::error!("Error applying remote UPDATE for donor {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }

    async fn apply_soft_delete(&self, change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> {
        log::info!("Ignoring remote soft delete for donor {}. Soft deletes are local-only.", change.entity_id);
        Ok(())
    }

    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()> {
        let entity_id = tombstone.entity_id;
        log::info!("Applying remote HARD DELETE for donor: {}", entity_id);

        if tombstone.entity_type != self.entity_table() {
            return Err(DomainError::Internal(format!(
                "Tombstone entity type mismatch: expected '{}', got '{}'",
                self.entity_table(), tombstone.entity_type
            )));
        }

        let options = DeleteOptions {
            allow_hard_delete: true,
            fallback_to_soft_delete: false,
            force: true,
        };

        match self.delete_service.delete(entity_id, auth, options).await {
            Ok(crate::domains::core::repository::DeleteResult::HardDeleted) => {
                log::info!("Hard deleted donor {} (and its documents if any) from remote tombstone via DeleteService", entity_id);
                Ok(())
            }
            Ok(unexpected_result) => {
                log::error!(
                    "Unexpected outcome from DeleteService for donor {} during hard delete: {:?}",
                    entity_id, unexpected_result
                );
                Err(DomainError::Internal(format!(
                    "Unexpected outcome from DeleteService for donor hard delete: {:?}",
                    unexpected_result
                )))
            }
            Err(DomainError::EntityNotFound(_, _)) => {
                log::info!("Donor {} not found locally for hard delete via DeleteService, skipping", entity_id);
                Ok(())
            }
            Err(e) => {
                log::error!("Error applying remote HARD DELETE for donor {} via DeleteService: {:?}", entity_id, e);
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
            log::debug!("Skipping local donor change (with tx): {}", change.operation_id);
            return Ok(());
        }

        match self.repo.merge_remote_change(tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => log::info!("(TX) Created donor {} from remote change", id),
                    MergeOutcome::Updated(id) => log::info!("(TX) Updated donor {} from remote change", id),
                    MergeOutcome::NoOp(reason) => log::info!("(TX) NoOp for remote donor change: {}", reason),
                    MergeOutcome::HardDeleted(id) => log::info!("(TX) HardDeleted donor {} from remote change", id),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => {
                        log::warn!("(TX) Conflict detected for donor {} during change: {}", entity_id, reason);
                    }
                }
                Ok(())
            },
            Err(e) => {
                log::error!("(TX) Error applying remote change for donor {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }
} 