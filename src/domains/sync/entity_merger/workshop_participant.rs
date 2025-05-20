use super::{DomainEntityMerger, BaseDomainMerger};
use crate::domains::workshop::participant_repository::WorkshopParticipantRepository;
use crate::domains::workshop::types::WorkshopParticipant;
use crate::domains::sync::types::{ChangeLogEntry, MergeOutcome, Tombstone};
use crate::auth::AuthContext;
use crate::errors::{DomainError, DomainResult};
use crate::domains::core::delete_service::{DeleteService, DeleteOptions};
use sqlx::{Transaction, Sqlite, SqlitePool};
use async_trait::async_trait;
use std::sync::Arc;

/// Entity merger for `workshop_participants` table – parallels other domain mergers.
pub struct WorkshopParticipantEntityMerger {
    repo: Arc<dyn WorkshopParticipantRepository + Send + Sync>,
    pool: SqlitePool,
    delete_service: Arc<dyn DeleteService<WorkshopParticipant> + Send + Sync>,
}

impl WorkshopParticipantEntityMerger {
    pub fn new(
        repo: Arc<dyn WorkshopParticipantRepository + Send + Sync>,
        pool: SqlitePool,
        delete_service: Arc<dyn DeleteService<WorkshopParticipant> + Send + Sync>,
    ) -> Self {
        Self { repo, pool, delete_service }
    }
}

#[async_trait]
impl DomainEntityMerger for WorkshopParticipantEntityMerger {
    fn entity_table(&self) -> &'static str { "workshop_participants" }

    async fn apply_create(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) { return Ok(()); }
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(_) => { tx.commit().await.map_err(|e| DomainError::Database(e.into()))?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn apply_update(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) { return Ok(()); }
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        match self.repo.merge_remote_change(&mut tx, change).await {
            Ok(_) => { tx.commit().await.map_err(|e| DomainError::Database(e.into()))?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn apply_soft_delete(&self, _change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> {
        Ok(())
    }

    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()> {
        if tombstone.entity_type != self.entity_table() {
            return Err(DomainError::Internal("Tombstone entity type mismatch".into()));
        }
        let options = DeleteOptions { allow_hard_delete: true, fallback_to_soft_delete: false, force: true };
        match self.delete_service.delete(tombstone.entity_id, auth, options).await {
            Ok(_) => Ok(()),
            Err(DomainError::EntityNotFound(_, _)) => Ok(()),
            Err(e) => Err(e)
        }
    }

    async fn apply_change_with_tx<'t>(&self, change: &ChangeLogEntry, auth: &AuthContext, tx: &mut Transaction<'t, Sqlite>) -> DomainResult<()> {
        if BaseDomainMerger::is_local_change(change, auth) { return Ok(()); }
        self.repo.merge_remote_change(tx, change).await.map(|_| ())
    }
} 