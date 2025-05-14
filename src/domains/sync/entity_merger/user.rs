// sync/entity_merger/user.rs

use super::{DomainEntityMerger, BaseDomainMerger};
use async_trait::async_trait;
use crate::auth::AuthContext;
use crate::errors::{DomainResult, DomainError, ValidationError};
use crate::domains::sync::types::{ChangeLogEntry, Tombstone, MergeOutcome};
use crate::domains::user::repository::UserRepository;
use crate::domains::user::types::{User, NewUser, UpdateUser};
use std::sync::Arc;
use uuid::Uuid;
use sqlx::{Transaction, Sqlite, SqlitePool};

pub struct UserEntityMerger {
    user_repo: Arc<dyn UserRepository + Send + Sync>,
    pool: SqlitePool,
}

impl UserEntityMerger {
    pub fn new(user_repo: Arc<dyn UserRepository + Send + Sync>, pool: SqlitePool) -> Self {
        Self { user_repo, pool }
    }
}

#[async_trait]
impl DomainEntityMerger for UserEntityMerger {
    fn entity_table(&self) -> &'static str {
        "users"
    }

    async fn apply_create(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        // Skip if this is a local change to avoid duplication
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local user create change: {}", change.operation_id);
            return Ok(());
        }

        // Start a transaction
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        
        // Delegate to UserRepository's merge_remote_change
        match self.user_repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => {
                        log::info!("Created user {} from remote change", id);
                    },
                    MergeOutcome::Updated(id) => {
                        log::info!("Updated existing user {} from remote create change", id);
                    },
                    MergeOutcome::NoOp(reason) => {
                        log::info!("No operation for remote user create: {}", reason);
                    },
                    MergeOutcome::HardDeleted(id) => {
                        log::warn!("Unexpected hard delete outcome for create operation on user {}", id);
                    },
                    MergeOutcome::ConflictDetected { entity_id, reason, .. } => {
                        log::warn!("Conflict detected for user {} during create: {}", entity_id, reason);
                        // Potentially return an error or handle the conflict appropriately
                    },
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            },
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn apply_update(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        // Skip if this is a local change to avoid duplication
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local user update change: {}", change.operation_id);
            return Ok(());
        }

        // Start a transaction
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        
        // Delegate to UserRepository's merge_remote_change
        match self.user_repo.merge_remote_change(&mut tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => {
                        log::warn!("Unexpected create outcome for update operation on user {}", id);
                    },
                    MergeOutcome::Updated(id) => {
                        log::info!("Updated user {} from remote change", id);
                    },
                    MergeOutcome::NoOp(reason) => {
                        log::info!("No operation for remote user update: {}", reason);
                    },
                    MergeOutcome::HardDeleted(id) => {
                        log::warn!("Unexpected hard delete outcome for update operation on user {}", id);
                    },
                    MergeOutcome::ConflictDetected { entity_id, reason, .. } => {
                        log::warn!("Conflict detected for user {} during update: {}", entity_id, reason);
                        // Potentially return an error or handle the conflict appropriately
                    },
                }
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            },
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn apply_soft_delete(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        // As per your note, soft deletes should not be synced, they should remain local
        log::info!("Ignoring remote soft delete for user {}. Soft deletes should remain local.", change.entity_id);
        Ok(())
    }

    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()> {
        // Hard deletes are applied via tombstones
        let entity_id = tombstone.entity_id;
        
        // Verify the tombstone is for a user
        if tombstone.entity_type != self.entity_table() {
            return Err(DomainError::Internal(format!(
                "Tombstone entity type mismatch: expected {}, got {}",
                self.entity_table(), tombstone.entity_type
            )));
        }
        
        // Start a transaction
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(e.into()))?;
        
        // Use hard_delete_with_tx to perform within our transaction
        match self.user_repo.hard_delete_with_tx(entity_id, auth, &mut tx).await {
            Ok(()) => {
                log::info!("Hard deleted user {} from remote tombstone", entity_id);
                tx.commit().await.map_err(|e| DomainError::Database(e.into()))?;
                Ok(())
            },
            Err(e) => {
                let _ = tx.rollback().await;
                
                // Sometimes, the entity might not exist locally, which is fine for a delete
                if let DomainError::EntityNotFound(_, _) = &e {
                    log::info!("User {} not found locally for hard delete, skipping", entity_id);
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn apply_change_with_tx<'t>(
        &self,
        change: &ChangeLogEntry,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        // Skip if this is a local change to avoid duplication
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local user change with tx: {}", change.operation_id);
            return Ok(());
        }

        // Delegate to UserRepository's merge_remote_change
        match self.user_repo.merge_remote_change(tx, change).await {
            Ok(outcome) => {
                match outcome {
                    MergeOutcome::Created(id) => {
                        log::info!("Created user {} from remote change with tx", id);
                    },
                    MergeOutcome::Updated(id) => {
                        log::info!("Updated user {} from remote change with tx", id);
                    },
                    MergeOutcome::NoOp(reason) => {
                        log::info!("No operation for remote user change with tx: {}", reason);
                    },
                    MergeOutcome::HardDeleted(id) => {
                        log::info!("Hard deleted user {} from remote change with tx", id);
                    },
                    MergeOutcome::ConflictDetected { entity_id, reason, .. } => {
                        log::warn!("Conflict detected for user {} during change with tx: {}", entity_id, reason);
                        // Potentially return an error or handle the conflict appropriately
                    },
                }
                Ok(())
            },
            Err(e) => Err(e)
        }
    }
}