// sync/entity_merger/mod.rs

use async_trait::async_trait;
use uuid::Uuid;
use crate::auth::AuthContext;
use crate::errors::{DomainResult, DomainError, DbError, ValidationError};
use crate::domains::sync::types::{ChangeLogEntry, Tombstone, ChangeOperationType};
use std::sync::Arc;
use std::collections::HashMap;
use sqlx::{SqlitePool, Transaction, Sqlite};

/// Trait for domain-specific entity mergers
#[async_trait]
pub trait DomainEntityMerger: Send + Sync {
    /// Get the entity table name this merger handles
    fn entity_table(&self) -> &'static str;
    
    /// Apply a create operation
    async fn apply_create(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()>;
    
    /// Apply an update operation
    async fn apply_update(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()>;
    
    /// Apply a soft delete operation
    async fn apply_soft_delete(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()>;
    
    /// Apply a hard delete operation (from tombstone)
    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()>;
    
    /// Apply changes within a transaction (optional override)
    async fn apply_change_with_tx<'t>(
        &self,
        change: &ChangeLogEntry,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        // Default implementation: begin a new transaction for each operation if not already in one.
        // This is a fallback; individual mergers might handle transactions more specifically if needed,
        // or the SyncService might manage the overall transaction for a batch.
        // The provided logic in EntityMerger.apply_changes_batch is better for batching.
        // This default in the trait might be too simplistic or even incorrect if called directly
        // without an outer transaction manager.
        log::warn!("Default apply_change_with_tx called for table: {}. Consider overriding or ensuring outer TX.", self.entity_table());
        match change.operation_type {
            ChangeOperationType::Create => self.apply_create(change, auth).await,
            ChangeOperationType::Update => self.apply_update(change, auth).await,
            ChangeOperationType::Delete => self.apply_soft_delete(change, auth).await,
            ChangeOperationType::HardDelete => {
                // This path should ideally not be hit if hard deletes are always via tombstones.
                // If a ChangeLogEntry has HardDelete, it implies the tombstone logic might be elsewhere
                // or this is a direct command to hard delete via a ChangeLog, which is unusual.
                log::error!("HardDelete operation received in apply_change_with_tx for table: {}. This should typically be an apply_tombstone call.", self.entity_table());
                Err(DomainError::Internal("Hard delete via ChangeLogEntry in apply_change_with_tx is not standard; use apply_tombstone.".to_string()))
            }
        }
    }
}

/// Central entity merger that delegates to domain-specific mergers
pub struct EntityMerger {
    pool: SqlitePool,
    mergers: HashMap<String, Arc<dyn DomainEntityMerger>>,
}

impl EntityMerger {
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            mergers: HashMap::new(),
        }
    }
    
    /// Register a domain-specific merger
    pub fn register_merger(&mut self, merger: Arc<dyn DomainEntityMerger>) {
        self.mergers.insert(merger.entity_table().to_string(), merger);
    }
    
    /// Apply a change from the change log
    pub async fn apply_change(
        &self,
        change: &ChangeLogEntry,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let merger = self.mergers.get(&change.entity_table)
            .ok_or_else(|| DomainError::Internal(format!(
                "No merger registered for entity type: {}", 
                change.entity_table
            )))?;
        
        // Skip local changes early if the merger isn't going to handle them
        // (though individual merger methods also do this check, it can be an early exit here too)
        // However, BaseDomainMerger::is_local_change requires auth, which is passed to methods.
        // So, let the individual methods handle it for now.

        match change.operation_type {
            ChangeOperationType::Create => merger.apply_create(change, auth).await,
            ChangeOperationType::Update => merger.apply_update(change, auth).await,
            ChangeOperationType::Delete => merger.apply_soft_delete(change, auth).await,
            ChangeOperationType::HardDelete => {
                // As above, this is unusual. Hard deletes should come via tombstones.
                log::error!("HardDelete operation received in apply_change for table: {}. This should typically be an apply_tombstone call.", change.entity_table);
                Err(DomainError::Internal("Hard delete via ChangeLogEntry in apply_change is not standard; use apply_tombstone.".to_string()))
            }
        }
    }
    
    /// Apply a tombstone (hard delete)
    pub async fn apply_tombstone(
        &self,
        tombstone: &Tombstone,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let merger = self.mergers.get(&tombstone.entity_type)
            .ok_or_else(|| DomainError::Internal(format!(
                "No merger registered for entity type: {}", 
                tombstone.entity_type
            )))?;
        
        merger.apply_hard_delete(tombstone, auth).await
    }
    
    /// Apply multiple changes in a transaction
    pub async fn apply_changes_batch(
        &self,
        changes: &[ChangeLogEntry],
        auth: &AuthContext,
    ) -> DomainResult<Vec<Uuid>> { // Return IDs of successfully applied changes
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let mut applied_operation_ids = Vec::new();
        let mut first_error: Option<DomainError> = None;
        
        for change in changes {
            if first_error.is_some() {
                // If an error occurred, skip subsequent changes in this batch for this transaction.
                // The caller might decide to retry them individually or handle the error.
                log::warn!("Skipping change {} due to previous error in batch.", change.operation_id);
                continue;
            }

            match self.apply_change_with_tx_internal(change, auth, &mut tx).await {
                Ok(()) => {
                    applied_operation_ids.push(change.operation_id);
                },
                Err(e) => {
                    log::error!(
                        "Failed to apply change {} (entity_id: {}, table: {}) within batch transaction: {:?}. Will attempt rollback.", 
                        change.operation_id, change.entity_id, change.entity_table, e
                    );
                    first_error = Some(e);
                    // Don't break; continue to log skipped changes, then rollback outside loop.
                }
            }
        }
        
        if let Some(e) = first_error {
            if let Err(rollback_err) = tx.rollback().await {
                log::error!("Failed to rollback transaction after batch error: {:?}", rollback_err);
                // Return the original error, but log the rollback failure.
                // Construct a new DomainError::Internal with the combined message.
                return Err(DomainError::Internal(format!(
                    "Original batch error: {}. Also failed to rollback: {}. Original error details: {}", 
                    e, rollback_err, e
                )));
            } 
            return Err(e); // Return the first error that occurred.
        }

        tx.commit().await.map_err(DbError::from)?;
        Ok(applied_operation_ids)
    }
    
    /// Internal helper for applying a single change within an existing transaction.
    /// This is called by `apply_changes_batch`.
    async fn apply_change_with_tx_internal<'t>(
        &self,
        change: &ChangeLogEntry,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        let merger = self.mergers.get(&change.entity_table)
            .ok_or_else(|| DomainError::Internal(format!(
                "No merger registered for entity type: {} (within tx)", 
                change.entity_table
            )))?;
        
        // The DomainEntityMerger's apply_change_with_tx is called here.
        // It is responsible for calling its repo's merge_remote_change within the provided tx.
        merger.apply_change_with_tx(change, auth, tx).await
    }
}

/// Base implementation helper for domain mergers
pub struct BaseDomainMerger;

impl BaseDomainMerger {
    /// Helper to parse JSON values from change log
    pub fn parse_json_value<T: serde::de::DeserializeOwned>(
        value: &Option<String>,
        field_name: &str,
    ) -> DomainResult<Option<T>> {
        match value {
            Some(json_str) => {
                let parsed = serde_json::from_str(json_str)
                    .map_err(|e| DomainError::Validation(ValidationError::format(
                        field_name,
                        &format!("Invalid JSON for field '{}': {}", field_name, e)
                    )))?;
                Ok(Some(parsed))
            }
            None => Ok(None)
        }
    }
    
    /// Helper to extract entity ID from change
    pub fn get_entity_id(change: &ChangeLogEntry) -> DomainResult<Uuid> {
        Ok(change.entity_id)
    }
    
    /// Helper to check if change is from local device
    pub fn is_local_change(change: &ChangeLogEntry, auth: &AuthContext) -> bool {
        match &change.device_id {
            Some(change_device_uuid) => {
                if let Ok(auth_device_uuid) = Uuid::parse_str(&auth.device_id) {
                    change_device_uuid == &auth_device_uuid
                } else {
                    log::warn!("Failed to parse auth.device_id ('{}') as Uuid for local change check.", auth.device_id);
                    false // If auth.device_id is invalid, assume not local to be safe
                }
            },
            None => false, // If change.device_id is None, it's likely a very old or system-generated entry, treat as remote.
        }
    }
}

// Re-export for convenience
pub use self::user::UserEntityMerger;
pub use self::donor::DonorEntityMerger;
pub use self::document::DocumentEntityMerger;
pub use self::document_type::DocumentTypeEntityMerger;
pub use self::funding::FundingEntityMerger;
pub use self::participant::ParticipantEntityMerger;
pub use self::workshop::WorkshopEntityMerger;
pub use self::livelihood::{LivelihoodEntityMerger, SubsequentGrantEntityMerger};
pub use self::workshop_participant::WorkshopParticipantEntityMerger;
pub use self::activity::ActivityEntityMerger;
pub use self::project::ProjectEntityMerger;
pub use self::strategic_goal::StrategicGoalEntityMerger;

pub mod user;
pub mod donor;
pub mod document;
pub mod document_type;
pub mod funding;
pub mod participant;
pub mod workshop;
pub mod livelihood;
pub mod workshop_participant;
pub mod activity;
pub mod project;
pub mod strategic_goal;

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::domains::sync::types::ChangeOperationType;
    
    #[tokio::test]
    async fn test_entity_merger_registration() {
        // Test that mergers can be registered and retrieved
        // Example:
        // let pool = /* setup SqlitePool for testing */;
        // let mut merger = EntityMerger::new(pool);
        // struct MockMerger;
        // #[async_trait]
        // impl DomainEntityMerger for MockMerger {
        //     fn entity_table(&self) -> &'static str { "mock_table" }
        //     async fn apply_create(&self, _change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> { Ok(()) }
        //     async fn apply_update(&self, _change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> { Ok(()) }
        //     async fn apply_soft_delete(&self, _change: &ChangeLogEntry, _auth: &AuthContext) -> DomainResult<()> { Ok(()) }
        //     async fn apply_hard_delete(&self, _tombstone: &Tombstone, _auth: &AuthContext) -> DomainResult<()> { Ok(()) }
        // }
        // merger.register_merger(Arc::new(MockMerger));
        // assert!(merger.mergers.contains_key("mock_table"));
    }
} 