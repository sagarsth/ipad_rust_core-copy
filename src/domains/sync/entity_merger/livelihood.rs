use super::{DomainEntityMerger, BaseDomainMerger};
use crate::domains::livelihood::repository::LivehoodRepository;
use crate::domains::livelihood::types::Livelihood;
use crate::domains::sync::types::{ChangeLogEntry, MergeOutcome, Tombstone};
use crate::auth::AuthContext;
use crate::errors::{DomainError, DomainResult};
use crate::domains::core::delete_service::{DeleteService, DeleteOptions};
use sqlx::{Transaction, Sqlite, SqlitePool};
use async_trait::async_trait;
use std::sync::Arc;
use crate::domains::livelihood::repository::SubsequentGrantRepository;
use crate::domains::livelihood::types::SubsequentGrant;

/// Entity merger for `livelihoods` table – parallels other domain mergers.
pub struct LivelihoodEntityMerger {
    repo: Arc<dyn LivehoodRepository + Send + Sync>,
    pool: SqlitePool,
    delete_service: Arc<dyn DeleteService<Livelihood> + Send + Sync>,
}

impl LivelihoodEntityMerger {
    pub fn new(
        repo: Arc<dyn LivehoodRepository + Send + Sync>,
        pool: SqlitePool,
        delete_service: Arc<dyn DeleteService<Livelihood> + Send + Sync>,
    ) -> Self {
        Self { repo, pool, delete_service }
    }
}

#[async_trait]
impl DomainEntityMerger for LivelihoodEntityMerger {
    fn entity_table(&self) -> &'static str { "livelihoods" }

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
        Ok(()) // ignore remote soft delete
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

// === SubsequentGrantEntityMerger ===
/// Entity merger for `subsequent_grants` table – parallels other domain mergers.
pub struct SubsequentGrantEntityMerger {
    repo: Arc<dyn SubsequentGrantRepository + Send + Sync>,
    pool: SqlitePool,
    delete_service: Arc<dyn DeleteService<SubsequentGrant> + Send + Sync>,
}

impl SubsequentGrantEntityMerger {
    pub fn new(
        repo: Arc<dyn SubsequentGrantRepository + Send + Sync>,
        pool: SqlitePool,
        delete_service: Arc<dyn DeleteService<SubsequentGrant> + Send + Sync>,
    ) -> Self {
        Self { repo, pool, delete_service }
    }
}

#[async_trait]
impl DomainEntityMerger for SubsequentGrantEntityMerger {
    fn entity_table(&self) -> &'static str { "subsequent_grants" }

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