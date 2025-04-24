use crate::errors::DomainResult;
use crate::auth::AuthContext;
use uuid::Uuid;
use async_trait::async_trait;
use std::collections::HashMap;
use sqlx::{Transaction, Sqlite}; // Import transaction types

/// Result type for delete operations
#[derive(Debug, PartialEq)]
pub enum DeleteResult {
    /// Record was hard deleted
    HardDeleted,
    
    /// Record was soft deleted, with list of dependencies that prevented hard delete
    SoftDeleted {
        dependencies: Vec<String>,
    },
    
    /// Record was not deleted due to dependencies that prevented hard delete
    DependenciesPrevented {
        dependencies: Vec<String>,
    },
}

/// Result type for batch delete operations
#[derive(Debug)]
pub struct BatchDeleteResult {
    /// Successfully hard deleted record IDs
    pub hard_deleted: Vec<Uuid>,
    
    /// Successfully soft deleted record IDs
    pub soft_deleted: Vec<Uuid>,
    
    /// Failed to delete record IDs
    pub failed: Vec<Uuid>,
    
    /// Map of ID to dependencies that prevented hard delete
    pub dependencies: HashMap<Uuid, Vec<String>>,
}

/// Trait for finding entities by ID
#[async_trait]
pub trait FindById<T> {
    /// Find an entity by ID
    async fn find_by_id(&self, id: Uuid) -> DomainResult<T>;
}

/// Trait for entities that support soft deletion
#[async_trait]
pub trait SoftDeletable { // Renamed for clarity
    /// Soft delete an entity by ID (standalone)
    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()>;
    
    /// Soft delete an entity by ID within a transaction
    async fn soft_delete_with_tx(
        &self, 
        id: Uuid, 
        auth: &AuthContext, 
        tx: &mut Transaction<'_, Sqlite>
    ) -> DomainResult<()>;
}

/// Trait for entities that support hard deletion
#[async_trait]
pub trait HardDeletable { // Renamed for clarity
    /// The name of the entity table in the database (for tombstone/logging)
    fn entity_name(&self) -> &'static str;
    
    // Dependency checking might move to a separate service/checker
    // async fn check_dependencies(&self, id: Uuid) -> DomainResult<Vec<String>>; 
    
    /// Hard delete an entity by ID (standalone)
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()>;

    /// Hard delete an entity by ID within a transaction
    async fn hard_delete_with_tx(
        &self, 
        id: Uuid, 
        auth: &AuthContext, 
        tx: &mut Transaction<'_, Sqlite>
    ) -> DomainResult<()>;
    
    // Batch deletion needs careful transaction handling, omitted for now
    // async fn hard_delete_batch(&self, ids: &[Uuid], auth: &AuthContext) -> DomainResult<BatchDeleteResult>; 
    
    // This logic likely belongs in the DeleteService, not the repository trait
    // async fn delete_with_fallback(...)
}

/// Generic repository trait for basic CRUD operations
#[async_trait]
pub trait Repository<T, CreateDto, UpdateDto>: FindById<T> + SoftDeletable + HardDeletable {
    /// Find all entities
    async fn find_all(&self) -> DomainResult<Vec<T>>;
    
    /// Create a new entity (consider if this needs a _with_tx variant)
    async fn create(&self, dto: CreateDto, auth: &AuthContext) -> DomainResult<T>;
    
    /// Update an existing entity (consider if this needs a _with_tx variant)
    async fn update(&self, id: Uuid, dto: UpdateDto, auth: &AuthContext) -> DomainResult<T>;
}