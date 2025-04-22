use crate::errors::{DomainResult, DomainError, DbError};
use crate::auth::AuthContext;
use crate::types::{UserRole, PaginationParams};
use crate::domains::sync::types::{Tombstone, ChangeLogEntry, ChangeOperationType};
use crate::domains::sync::repository::{TombstoneRepository, ChangeLogRepository};
use crate::domains::core::dependency_checker::{DependencyChecker, Dependency};
use crate::domains::core::repository::{DeleteResult, HardDeletable, SoftDeletable, FindById};
use crate::domains::document::repository::MediaDocumentRepository;
use uuid::Uuid;
use chrono::Utc;
use async_trait::async_trait;
use sqlx::{SqlitePool, Transaction, Sqlite};
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use sqlx::Row;

/// Delete options for controlling deletion behavior
#[derive(Debug, Clone)]
pub struct DeleteOptions {
    /// Whether to allow hard delete
    pub allow_hard_delete: bool,
    
    /// Whether to fall back to soft delete if hard delete fails
    pub fallback_to_soft_delete: bool,
    
    /// Whether to bypass dependency checks (admin only)
    pub force: bool,
}

impl Default for DeleteOptions {
    fn default() -> Self {
        Self {
            allow_hard_delete: false,
            fallback_to_soft_delete: true,
            force: false,
        }
    }
}

/// Details about a single record that failed to delete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedDeleteDetail<E>
where 
    E: Send + Sync,
{
    pub id: Uuid,
    pub entity_data: Option<E>,
    pub entity_type: String,
    pub reason: FailureReason,
    pub dependencies: Vec<String>,
}

/// Reason why a delete operation failed for a specific record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureReason {
    /// Failed due to existing non-cascading dependencies
    DependenciesPrevented,
    /// Could be soft-deleted, but hard delete was prevented by dependencies
    SoftDeletedDueToDependencies, 
    /// Record was not found during the operation
    NotFound,
    /// User did not have permission for the requested operation (e.g., hard delete)
    AuthorizationFailed, 
    /// An unexpected database error occurred
    DatabaseError(String),
    /// Failure reason could not be determined from batch results
    Unknown, 
}

/// Result of a batch delete operation
#[derive(Debug, Clone, Default)]
pub struct BatchDeleteResult {
    /// Successfully hard deleted record IDs
    pub hard_deleted: Vec<Uuid>,
    /// Successfully soft deleted record IDs (includes those soft-deleted due to fallback)
    pub soft_deleted: Vec<Uuid>,
    /// Failed to delete record IDs (includes dependencies prevented, not found, errors)
    pub failed: Vec<Uuid>,
    /// Map of ID to dependencies that *would have* prevented hard delete
    /// (Populated for both SoftDeletedDueToDependencies and DependenciesPrevented)
    pub dependencies: HashMap<Uuid, Vec<String>>,
    /// Map of ID to specific failure errors (if captured)
    /// Optional: Requires enhancing `batch_delete` to store errors
    pub errors: HashMap<Uuid, DomainError>, 
}

/// Trait combining repository operations needed for delete service
pub trait DeleteServiceRepository<E>: FindById<E> + SoftDeletable + HardDeletable + Send + Sync {
    // Add a method to explicitly get self as a FindById reference
    fn as_find_by_id(&self) -> &dyn FindById<E>;
}

/// Implement for any type that implements all required traits
impl<T, E> DeleteServiceRepository<E> for T 
where 
    T: FindById<E> + SoftDeletable + HardDeletable + Send + Sync,
    E: Send + Sync + 'static,
{
    fn as_find_by_id(&self) -> &dyn FindById<E> {
        self
    }
}

/// Delete service for handling delete operations
#[async_trait]
pub trait DeleteService<E>: Send + Sync 
where
    E: Send + Sync + 'static,
{
    /// Get the repository
    fn repository(&self) -> &dyn FindById<E>;
    
    /// Get the tombstone repository
    fn tombstone_repository(&self) -> &dyn TombstoneRepository;
    
    /// Get the change log repository
    fn change_log_repository(&self) -> &dyn ChangeLogRepository;
    
    /// Get the dependency checker
    fn dependency_checker(&self) -> &dyn DependencyChecker;

    /// Delete an entity with specified options
    async fn delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<DeleteResult>;
    
    /// Delete multiple entities with specified options
    async fn batch_delete(
        &self,
        ids: &[Uuid],
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<BatchDeleteResult>;
    
    /// Delete multiple entities with their dependencies
    async fn delete_with_dependencies(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<DeleteResult>;

    /// Retrieve details about records that failed to delete during a batch operation
    async fn get_failed_delete_details(
        &self,
        batch_result: &BatchDeleteResult,
        auth: &AuthContext,
    ) -> DomainResult<Vec<FailedDeleteDetail<E>>>;
}

/// Base implementation of delete service
pub struct BaseDeleteService<E>
where
    E: Send + Sync + 'static,
{
    pool: SqlitePool,
    repo: Arc<dyn DeleteServiceRepository<E>>,
    tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
    dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
    media_doc_repo: Option<Arc<dyn MediaDocumentRepository>>,
    _marker: std::marker::PhantomData<E>,
}

impl<E> BaseDeleteService<E>
where
    E: Send + Sync + Clone + 'static,
{
    /// Create a new base delete service
    pub fn new(
        pool: SqlitePool,
        repo: Arc<dyn DeleteServiceRepository<E>>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        media_doc_repo: Option<Arc<dyn MediaDocumentRepository>>,
    ) -> Self {
        Self {
            pool,
            repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            media_doc_repo,
            _marker: std::marker::PhantomData,
        }
    }

    /// Helper function to handle cascading document deletion within a transaction
    async fn cascade_delete_documents<'t>(
        &self,
        parent_table_name: &str,
        parent_id: Uuid,
        hard_delete: bool, // Indicates if the parent was hard deleted
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        if let Some(media_repo) = &self.media_doc_repo {
            // Find all related documents. Use default pagination for now, might need loop for large numbers.
            // IMPORTANT: This assumes find_by_related_entity doesn't start its own transaction!
            // If it does, we need a find_by_related_entity_with_tx variant.
            // Let's assume it uses the pool directly for now, or fetch IDs first.

            // Fetch just the IDs first to avoid borrowing issues with the transaction
             // Introduce longer-lived bindings
             let parent_table_name_owned = parent_table_name.to_string();
             let parent_id_str = parent_id.to_string(); 
             
             // Use sqlx::query and bind instead of query! macro
             let doc_ids_result = sqlx::query(
                 "SELECT id FROM media_documents WHERE related_table = ? AND related_id = ? AND deleted_at IS NULL"
             )
             .bind(parent_table_name_owned) // Bind the owned string
             .bind(parent_id_str)         // Bind the owned string
             .fetch_all(&mut **tx) // Use the transaction
             .await;
             
             let doc_ids: Vec<Uuid> = match doc_ids_result {
                 Ok(rows) => rows.into_iter().filter_map(|row| {
                     // Use try_get to access the column by name
                     row.try_get::<String, _>("id").ok().map(|id_str| Uuid::parse_str(&id_str).ok()).flatten()
                 }).collect(),
                 Err(sqlx::Error::RowNotFound) => Vec::new(), // No documents found is ok
                 Err(e) => return Err(DbError::from(e).into()), // Propagate other DB errors
             };


            for doc_id in doc_ids {
                if hard_delete {
                    // --- Cascade Hard Delete ---
                    // 1. Create Tombstone for the document
                    let tombstone = Tombstone::new(doc_id, media_repo.entity_name(), auth.user_id);
                    let operation_id = tombstone.operation_id; // Capture for consistency if needed
                    self.tombstone_repo.create_tombstone_with_tx(&tombstone, tx).await?;

                    // 2. Create Change Log for the document's hard delete
                     let change_log = ChangeLogEntry {
                        operation_id, // Use tombstone's ID
                        entity_table: media_repo.entity_name().to_string(),
                        entity_id: doc_id,
                        operation_type: ChangeOperationType::HardDelete,
                        field_name: None, old_value: None, new_value: None,
                        timestamp: Utc::now(),
                        user_id: auth.user_id,
                        device_id: auth.device_id.parse().ok(),
                        sync_batch_id: None, processed_at: None, sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&change_log, tx).await?;

                    // 3. Perform the actual hard delete of the document
                    media_repo.hard_delete_with_tx(doc_id, auth, tx).await?;
                     // ON DELETE CASCADE in DB handles versions/logs implicitly here

                } else {
                    // --- Cascade Soft Delete ---
                    media_repo.soft_delete_with_tx(doc_id, auth, tx).await?;
                    // NOTE: Decide if soft-deleting a parent should HARD delete versions/logs
                    // associated with the document. If so, add calls here:
                    // self.doc_ver_repo.hard_delete_by_document_id_with_tx(doc_id, tx).await?;
                    // self.doc_log_repo.hard_delete_by_document_id_with_tx(doc_id, tx).await?;
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl<E> DeleteService<E> for BaseDeleteService<E>
where
    E: Send + Sync + Clone + 'static,
{
    fn repository(&self) -> &dyn FindById<E> {
        self.repo.as_find_by_id()
    }
    
    fn tombstone_repository(&self) -> &dyn TombstoneRepository {
        &*self.tombstone_repo
    }
    
    fn change_log_repository(&self) -> &dyn ChangeLogRepository {
        &*self.change_log_repo
    }
    
    fn dependency_checker(&self) -> &dyn DependencyChecker {
        &*self.dependency_checker
    }
    
    async fn delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<DeleteResult> {
        // 1. Permission Checks
        if options.allow_hard_delete && auth.role != UserRole::Admin {
            return Err(DomainError::AuthorizationFailed("Only admins can perform hard deletes".to_string()));
        }
        if options.force && auth.role != UserRole::Admin {
            return Err(DomainError::AuthorizationFailed("Only admins can force delete operations".to_string()));
        }

        // Check if entity exists (optional, depends if soft/hard delete check this)
        // Let's assume delete methods handle non-existent IDs gracefully or error appropriately.
        // self.repo.find_by_id(id).await?; 

        // 2. Dependency Check
        let table_name = self.repo.entity_name();
        let all_dependencies = self.dependency_checker.check_dependencies(table_name, id).await?;
        
        // Filter out cascadable dependencies, as they are handled by the DB or subsequent logic
        // We only care about non-cascadable dependencies preventing a *hard* delete
        let blocking_dependencies: Vec<String> = all_dependencies
            .iter()
            .filter(|dep| !dep.is_cascadable && dep.count > 0)
            .map(|dep| dep.table_name.clone())
            .collect();

        // 3. Decide Action: Hard Delete or Soft Delete/Prevent
        let can_hard_delete = options.allow_hard_delete 
                              && auth.role == UserRole::Admin
                              && (blocking_dependencies.is_empty() || options.force);

        if can_hard_delete {
            // --- Hard Delete Path --- 
            let mut tx = self.pool.begin().await.map_err(DbError::from)?; // Start transaction

            let hard_delete_result = async {
                // Create tombstone
                let tombstone = Tombstone::new(id, self.repo.entity_name(), auth.user_id);
                let operation_id = tombstone.operation_id; // Capture for changelog
                self.tombstone_repo.create_tombstone_with_tx(&tombstone, &mut tx).await?;

                // Create change log entry for HardDelete
                let change_log = ChangeLogEntry {
                    operation_id, // Use same ID as tombstone
                    entity_table: self.repo.entity_name().to_string(),
                    entity_id: id,
                    operation_type: ChangeOperationType::HardDelete,
                    field_name: None,
                    old_value: None, // Maybe store serialized entity before delete?
                    new_value: None,
                    timestamp: Utc::now(),
                    user_id: auth.user_id,
                    device_id: auth.device_id.parse::<Uuid>().ok(),
                    sync_batch_id: None,
                    processed_at: None,
                    sync_error: None,
                };
                self.change_log_repo.create_change_log_with_tx(&change_log, &mut tx).await?;

                // Perform the actual hard delete
                self.repo.hard_delete_with_tx(id, auth, &mut tx).await?;

                // *** Cascade Delete Documents ***
                self.cascade_delete_documents(table_name, id, true, auth, &mut tx).await?;

                Ok::<_, DomainError>(())
            }.await;

            match hard_delete_result {
                Ok(_) => {
                    tx.commit().await.map_err(DbError::from)?; // Commit on success
                    Ok(DeleteResult::HardDeleted)
                },
                Err(e @ DomainError::EntityNotFound(_, _)) => {
                    // Rollback if not found during the transaction operations
                    let _ = tx.rollback().await;
                    Err(e) // Propagate NotFound specifically
                },
                Err(e) => {
                    let _ = tx.rollback().await; 
                    Err(e) // Propagate other errors
                }
            }

        } else {
            // --- Soft Delete or Prevent Path --- 
            if !blocking_dependencies.is_empty() && !options.fallback_to_soft_delete {
                // Dependencies exist, and we are not falling back to soft delete
                return Ok(DeleteResult::DependenciesPrevented { dependencies: blocking_dependencies });
            }

            // Proceed with soft delete (either requested or fallback)
            let mut tx = self.pool.begin().await.map_err(DbError::from)?; // Start transaction

            let soft_delete_result = self.repo.soft_delete_with_tx(id, auth, &mut tx).await;

            match soft_delete_result {
                Ok(_) => {
                    tx.commit().await.map_err(DbError::from)?; // Commit on success
                    // Return dependencies that *would have* blocked hard delete
                    Ok(DeleteResult::SoftDeleted { dependencies: blocking_dependencies })
                },
                Err(e @ DomainError::EntityNotFound(_, _)) => {
                    // Rollback if not found during soft delete
                    let _ = tx.rollback().await;
                    Err(e) // Propagate NotFound
                },
                Err(e) => {
                    let _ = tx.rollback().await; 
                    Err(e) // Propagate other errors
                }
            }
        }
    }
    
    /// Delete multiple entities with specified options
    async fn batch_delete(
        &self,
        ids: &[Uuid],
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<BatchDeleteResult> {
        // Check permissions based on options
        if options.allow_hard_delete && auth.role != UserRole::Admin {
            return Err(DomainError::AuthorizationFailed("Only admins can perform hard deletes".to_string()));
        }
        
        if options.force && auth.role != UserRole::Admin {
            return Err(DomainError::AuthorizationFailed("Only admins can force delete operations".to_string()));
        }
        
        let mut result = BatchDeleteResult::default();
        
        // Process each ID
        for &id in ids {
            match self.delete(id, auth, options.clone()).await {
                Ok(DeleteResult::HardDeleted) => {
                    result.hard_deleted.push(id);
                },
                
                Ok(DeleteResult::SoftDeleted { dependencies }) => {
                    result.soft_deleted.push(id);
                    if !dependencies.is_empty() {
                        result.dependencies.insert(id, dependencies);
                    }
                },
                
                Ok(DeleteResult::DependenciesPrevented { dependencies }) => {
                    result.failed.push(id);
                    result.dependencies.insert(id, dependencies);
                },
                
                Err(e @ DomainError::AuthorizationFailed(_)) => {
                    result.failed.push(id);
                    result.errors.insert(id, e);
                },
                Err(e @ DomainError::EntityNotFound(_, _)) => {
                    result.failed.push(id);
                    result.errors.insert(id, e);
                },
                Err(e) => {
                    result.failed.push(id);
                    result.errors.insert(id, e);
                }
            }
        }
        
        Ok(result)
    }
    
    /// Delete multiple entities with their dependencies
    async fn delete_with_dependencies(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<DeleteResult> {
        // This operation is admin-only
        if auth.role != UserRole::Admin {
            return Err(DomainError::AuthorizationFailed("Only admins can delete entities with dependencies".to_string()));
        }
        
        // Check for dependencies
        let table_name = self.repo.entity_name();
        let dependencies = self.dependency_checker().check_dependencies(table_name, id).await?;
        
        // If no dependencies, just do a normal hard delete
        if dependencies.is_empty() {
            return self.delete(
                id, 
                auth, 
                DeleteOptions {
                    allow_hard_delete: true,
                    fallback_to_soft_delete: false,
                    force: false,
                }
            ).await;
        }
        
        // For each dependency, we need to handle it based on whether it's cascadable
        // This would be a more complex implementation and might require specific
        // logic per entity type to handle the dependency chain correctly
        
        // For simplicity, we'll just use the force option which will hard delete
        // regardless of dependencies (relying on database ON DELETE constraints to handle cascades)
        self.delete(
            id, 
            auth, 
            DeleteOptions {
                allow_hard_delete: true,
                fallback_to_soft_delete: false,
                force: true,
            }
        ).await
    }

    /// Retrieve details about records that failed to delete during a batch operation
    async fn get_failed_delete_details(
        &self,
        batch_result: &BatchDeleteResult,
        auth: &AuthContext,
    ) -> DomainResult<Vec<FailedDeleteDetail<E>>> {
        let mut details = Vec::new();
        let entity_type_name = self.repo.entity_name().to_string();

        for &id in &batch_result.failed {
            // Attempt to fetch the entity data (might fail if already deleted or never existed)
            // Note: This assumes find_by_id doesn't require specific permissions beyond auth context validity,
            // or that the auth context used here has sufficient read permissions.
            let entity_data_result = self.repo.find_by_id(id).await;

            let entity_data = match entity_data_result {
                 Ok(entity) => Some(entity),
                 Err(DomainError::EntityNotFound(_, _)) => None, // Expected if it was never found
                 Err(_) => None, // Other error fetching, treat as data unavailable
            };

            // Determine the failure reason
            let (reason, deps) = match batch_result.errors.get(&id) {
                Some(DomainError::EntityNotFound(_, _)) => (FailureReason::NotFound, Vec::new()),
                Some(DomainError::AuthorizationFailed(_)) => (FailureReason::AuthorizationFailed, Vec::new()),
                Some(DomainError::Database(db_err)) => (FailureReason::DatabaseError(db_err.to_string()), Vec::new()),
                // Use wildcard _ to catch any other DomainError variant or None
                Some(_) | None => { 
                    // If Conflict or no specific error, check dependencies map
                    if let Some(dep_tables) = batch_result.dependencies.get(&id) {
                         // Check if it was actually soft-deleted (meaning dependencies prevented hard delete)
                         if batch_result.soft_deleted.contains(&id) {
                              (FailureReason::SoftDeletedDueToDependencies, dep_tables.clone())
                         } else {
                             // If it's in failed *and* dependencies, it was prevented entirely
                             (FailureReason::DependenciesPrevented, dep_tables.clone())
                         }
                    } else {
                         // In failed, but no specific error and no dependencies recorded -> Unknown
                         (FailureReason::Unknown, Vec::new())
                    }
                },
                 _ => (FailureReason::Unknown, Vec::new()), // Catch-all for other DomainErrors
            };

            details.push(FailedDeleteDetail {
                id,
                entity_data,
                entity_type: entity_type_name.clone(),
                reason,
                dependencies: deps,
            });
        }

        Ok(details)
    }
}