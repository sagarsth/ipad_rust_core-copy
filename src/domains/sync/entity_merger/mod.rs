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
        match change.operation_type {
            ChangeOperationType::Create => self.apply_create(change, auth).await,
            ChangeOperationType::Update => self.apply_update(change, auth).await,
            ChangeOperationType::Delete => self.apply_soft_delete(change, auth).await,
            ChangeOperationType::HardDelete => {
                Err(DomainError::Internal("Hard delete should use tombstone".to_string()))
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
        
        match change.operation_type {
            ChangeOperationType::Create => merger.apply_create(change, auth).await,
            ChangeOperationType::Update => merger.apply_update(change, auth).await,
            ChangeOperationType::Delete => merger.apply_soft_delete(change, auth).await,
            ChangeOperationType::HardDelete => {
                Err(DomainError::Internal("Hard delete should use tombstone".to_string()))
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
    ) -> DomainResult<Vec<Uuid>> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let mut applied_ids = Vec::new();
        
        for change in changes {
            match self.apply_change_with_tx(change, auth, &mut tx).await {
                Ok(()) => {
                    applied_ids.push(change.operation_id);
                },
                Err(e) => {
                    log::error!("Failed to apply change {}: {:?}", change.operation_id, e);
                }
            }
        }
        
        tx.commit().await.map_err(DbError::from)?;
        Ok(applied_ids)
    }
    
    /// Apply a change within a transaction
    async fn apply_change_with_tx<'t>(
        &self,
        change: &ChangeLogEntry,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        let merger = self.mergers.get(&change.entity_table)
            .ok_or_else(|| DomainError::Internal(format!(
                "No merger registered for entity type: {}", 
                change.entity_table
            )))?;
        
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
                    false
                }
            },
            None => false,
        }
    }
}

// Re-export for convenience
pub use self::project::ProjectEntityMerger;
pub use self::user::UserEntityMerger;
pub use self::document::DocumentEntityMerger;

mod project;
mod user;
mod document;

#[cfg(test)]
mod tests {
    // use super::*;
    // use crate::domains::sync::types::ChangeOperationType;
    
    #[tokio::test]
    async fn test_entity_merger_registration() {
        // Test that mergers can be registered and retrieved
        // Example:
        // let pool = /* setup SqlitePool */;
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