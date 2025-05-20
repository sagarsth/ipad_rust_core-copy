use super::{DomainEntityMerger, BaseDomainMerger};
use crate::domains::funding::repository::ProjectFundingRepository;
use crate::domains::sync::types::{ChangeLogEntry, MergeOutcome, Tombstone};
use crate::auth::AuthContext;
use crate::errors::{DomainResult, DomainError};
use crate::domains::core::delete_service::{DeleteService, DeleteOptions};
use sqlx::{Transaction, Sqlite, SqlitePool};
use async_trait::async_trait;
use std::sync::Arc;

/// Entity merger for `project_funding` table – parallels DonorEntityMerger.
///
/// It delegates the heavy‐lifting of Last-Write-Wins upserts to
/// `SqliteProjectFundingRepository::merge_remote_change` that we added in the
/// funding repository file.
pub struct FundingEntityMerger {
    repo: Arc<dyn ProjectFundingRepository + Send + Sync>,
    pool: SqlitePool,
    delete_service: Arc<dyn DeleteService<crate::domains::funding::types::ProjectFunding> + Send + Sync>,
}

impl FundingEntityMerger {
    pub fn new(
        repo: Arc<dyn ProjectFundingRepository + Send + Sync>,
        pool: SqlitePool,
        delete_service: Arc<dyn DeleteService<crate::domains::funding::types::ProjectFunding> + Send + Sync>,
    ) -> Self {
        Self { repo, pool, delete_service }
    }
}

#[async_trait]
impl DomainEntityMerger for FundingEntityMerger {
    fn entity_table(&self) -> &'static str { "project_funding" }

    async fn apply_create(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local funding create change: {}", change.operation_id);
            return Ok(());
        }
        log::info!("Applying remote CREATE for funding: {}", change.entity_id);

        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => log::info!("Created funding {} from remote change", id),
                    MergeOutcome::Updated(id) => log::info!("Updated existing funding {} from remote CREATE change (upsert)", id),
                    MergeOutcome::NoOp(reason) => log::info!("NoOp for remote funding create: {}", reason),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => {
                        log::warn!("Conflict detected for funding {} during create: {}", entity_id, reason);
                    }
                    MergeOutcome::HardDeleted(id) => {
                        log::error!("Unexpected HardDeleted outcome for funding {} from a CREATE operation", id);
                        let _ = tx.rollback().await;
                        return Err(DomainError::Internal(format!("CREATE operation resulted in HardDeleted for funding {}", id)));
                    }
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                log::error!("Error applying remote CREATE for funding {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }

    async fn apply_update(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local funding update change: {}", change.operation_id);
            return Ok(());
        }
        log::info!("Applying remote UPDATE for funding: {} (Field: {:?})", change.entity_id, change.field_name);

        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => log::warn!("Updated non-existent funding {} from remote UPDATE change (created instead)", id),
                    MergeOutcome::Updated(id) => log::info!("Updated funding {} from remote change", id),
                    MergeOutcome::NoOp(reason) => log::info!("NoOp for remote funding update: {}", reason),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => log::warn!("Conflict detected for funding {} during update: {}", entity_id, reason),
                    MergeOutcome::HardDeleted(id) => {
                        log::error!("Unexpected HardDeleted outcome for funding {} from an UPDATE operation", id);
                        let _ = tx.rollback().await;
                        return Err(DomainError::Internal(format!("UPDATE operation resulted in HardDeleted for funding {}", id)));
                    }
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            }
            Err(e) => {
                let _ = tx.rollback().await;
                log::error!("Error applying remote UPDATE for funding {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }

    async fn apply_soft_delete(&self, change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> {
        // Soft deletes remain local-only, mirroring donor logic.
        log::info!("Ignoring remote soft delete for funding {}. Soft deletes are local-only.", change.entity_id);
        Ok(())
    }

    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()> {
        let entity_id = tombstone.entity_id;
        log::info!("Applying remote HARD DELETE for funding: {}", entity_id);

        if tombstone.entity_type != self.entity_table() {
            return Err(DomainError::Internal(format!(
                "Tombstone entity type mismatch: expected '{}', got '{}'", self.entity_table(), tombstone.entity_type
            )));
        }

        let options = DeleteOptions {
            allow_hard_delete: true,
            fallback_to_soft_delete: false,
            force: true,
        };

        match self.delete_service.delete(entity_id, auth, options).await {
            Ok(crate::domains::core::repository::DeleteResult::HardDeleted) => {
                log::info!("Hard deleted funding {} (and its documents if any) from remote tombstone via DeleteService", entity_id);
                Ok(())
            }
            Ok(unexpected_result) => {
                log::error!("Unexpected outcome from DeleteService for funding {} during hard delete: {:?}", entity_id, unexpected_result);
                Err(DomainError::Internal(format!(
                    "Unexpected outcome from DeleteService for funding hard delete: {:?}", unexpected_result
                )))
            }
            Err(DomainError::EntityNotFound(_, _)) => {
                log::info!("Funding {} not found locally for hard delete via DeleteService, skipping", entity_id);
                Ok(())
            }
            Err(e) => {
                log::error!("Error applying remote HARD DELETE for funding {} via DeleteService: {:?}", entity_id, e);
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
            log::debug!("Skipping local funding change (with tx): {}", change.operation_id);
            return Ok(());
        }

        match self.repo.merge_remote_change(tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => log::info!("(TX) Created funding {} from remote change", id),
                    MergeOutcome::Updated(id) => log::info!("(TX) Updated funding {} from remote change", id),
                    MergeOutcome::NoOp(reason) => log::info!("(TX) NoOp for remote funding change: {}", reason),
                    MergeOutcome::HardDeleted(id) => log::info!("(TX) HardDeleted funding {} from remote change", id),
                    MergeOutcome::ConflictDetected{ entity_id, reason, .. } => log::warn!("(TX) Conflict detected for funding {} during change: {}", entity_id, reason),
                }
                Ok(())
            },
            Err(e) => {
                log::error!("(TX) Error applying remote change for funding {}: {:?}", change.entity_id, e);
                Err(e)
            }
        }
    }
} 