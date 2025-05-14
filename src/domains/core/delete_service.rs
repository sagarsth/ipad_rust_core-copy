use crate::errors::{DomainResult, DomainError, DbError};
use crate::auth::AuthContext;
use crate::types::UserRole;
use crate::domains::sync::types::{Tombstone, ChangeLogEntry, ChangeOperationType};
use crate::domains::sync::repository::{TombstoneRepository, ChangeLogRepository};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::repository::{DeleteResult, HardDeletable, SoftDeletable, FindById};
use crate::domains::document::repository::MediaDocumentRepository;
use uuid::Uuid;
use chrono::Utc;
use async_trait::async_trait;
use sqlx::{SqlitePool, Transaction, Sqlite};
use std::sync::Arc;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json; // Added for JSON manipulation
use sqlx::Row;
use tokio::sync::RwLock; // Added for RwLock

/// Helper to convert a device_id string to Uuid
fn parse_device_id(device_id_str: &str) -> Option<Uuid> {
    Uuid::parse_str(device_id_str).ok()
}

/// Holds information for a pending document file deletion.
#[derive(Debug, Clone)]
struct DocumentDeletion {
    document_id: Uuid,
    file_path: String, // file_path is NOT NULL in media_documents and file_deletion_queue
    compressed_file_path: Option<String>,
}

/// Manages document deletions that are pending based on the outcome of a main database transaction.
#[derive(Debug)]
pub struct PendingDeletionManager {
    pool: SqlitePool,
    pending_deletions: RwLock<HashMap<Uuid, Vec<DocumentDeletion>>>,
}

impl PendingDeletionManager {
    /// Creates a new PendingDeletionManager.
    pub fn new(pool: SqlitePool) -> Self {
        Self {
            pool,
            pending_deletions: RwLock::new(HashMap::new()),
        }
    }

    /// Adds a document deletion to the pending list for a given operation ID.
    pub async fn add_pending(&self, operation_id: Uuid, deletion: DocumentDeletion) {
        let mut pending = self.pending_deletions.write().await;
        pending.entry(operation_id).or_default().push(deletion);
    }

    /// Commits pending deletions for an operation ID to the file_deletion_queue.
    /// This is typically called after the main database transaction has successfully committed.
    pub async fn commit_deletions(&self, operation_id: Uuid, requested_by_user_id: Uuid) -> DomainResult<()> {
        let mut pending_map = self.pending_deletions.write().await;
        if let Some(deletions_to_commit) = pending_map.remove(&operation_id) {
            if deletions_to_commit.is_empty() {
                return Ok(());
            }

            // Start a new transaction for inserting into the file_deletion_queue
            let mut queue_tx = self.pool.begin().await.map_err(DbError::from)?;
            let requested_by_user_id_str = requested_by_user_id.to_string();

            for deletion in deletions_to_commit {
                let queue_id_str = Uuid::new_v4().to_string();
                let doc_id_str = deletion.document_id.to_string();
                let now_str = Utc::now().to_rfc3339();

                sqlx::query!(
                    r#"
                    INSERT INTO file_deletion_queue (
                        id,
                        document_id,
                        file_path,
                        compressed_file_path,
                        requested_at,
                        requested_by,
                        grace_period_seconds
                    )
                    VALUES (?, ?, ?, ?, ?, ?, ?)
                    "#,
                    queue_id_str,
                    doc_id_str,
                    deletion.file_path, // Is String, matches NOT NULL schema
                    deletion.compressed_file_path,
                    now_str,
                    requested_by_user_id_str,
                    86400 // 24 hour grace period
                )
                .execute(&mut *queue_tx) // Changed &mut **tx to &mut *queue_tx
                .await
                .map_err(DbError::from)?;
            }
            queue_tx.commit().await.map_err(DbError::from)?;
        }
        Ok(())
    }

    /// Discards pending deletions for an operation ID.
    /// This is typically called if the main database transaction has rolled back.
    pub async fn discard_deletions(&self, operation_id: Uuid) {
        let mut pending = self.pending_deletions.write().await;
        pending.remove(&operation_id);
    }
}

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
    deletion_manager: Arc<PendingDeletionManager>, // Added
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
        deletion_manager: Arc<PendingDeletionManager>, // Added
    ) -> Self {
        Self {
            pool,
            repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            media_doc_repo,
            deletion_manager, // Added
            _marker: std::marker::PhantomData,
        }
    }

    /// Enhanced helper function to handle document deletion with physical file cleanup
    async fn cascade_delete_documents<'t, T>(
        &self,
        parent_table_name: &str,
        parent_id: Uuid,
        hard_delete: bool, // Indicates if the parent was hard deleted
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
        pending_delete_operation_id: Uuid, // Added to associate with PendingDeletionManager
    ) -> DomainResult<()> 
    where 
        T: Send + Sync + Clone + 'static // Changed generic E to T to avoid conflict
    {
        if let Some(media_repo) = &self.media_doc_repo {
            // Fetch just the document IDs and paths for the related entity
            // using a direct query to avoid borrowing issues with the transaction
            let parent_id_str = parent_id.to_string(); // Store the string in a variable
            let document_data_result = sqlx::query!(
                r#"
                SELECT 
                    id, 
                    file_path, 
                    compressed_file_path
                FROM 
                    media_documents 
                WHERE 
                    related_table = ? AND 
                    related_id = ? AND 
                    deleted_at IS NULL
                "#,
                parent_table_name,
                parent_id_str // Use the variable here
            )
             .fetch_all(&mut **tx) // Use the transaction
             .await;
             
            let document_data = match document_data_result {
                Ok(data) => data,
                 Err(sqlx::Error::RowNotFound) => Vec::new(), // No documents found is ok
                 Err(e) => return Err(DbError::from(e).into()), // Propagate other DB errors
             };

            // Process each document
            for doc_data_row in document_data { // Renamed `doc` to `doc_data_row`
                // Parse the ID string to UUID
                let doc_id = match Uuid::parse_str(&doc_data_row.id) {
                    Ok(id) => id,
                    Err(_) => continue, // Skip if invalid UUID (shouldn't happen)
                };
    
                if hard_delete {
                    // --- HARD DELETE PATH ---
                    
                    // 1. Create tombstone for the document
                    let mut tombstone = Tombstone::new(doc_id, media_repo.entity_name(), auth.user_id);
                    
                    // 2. Add additional metadata for sync/cleanup tracking
                    let metadata = serde_json::json!({
                        "file_path": doc_data_row.file_path,
                        "compressed_file_path": doc_data_row.compressed_file_path,
                        "parent_table": parent_table_name,
                        "parent_id": parent_id.to_string(),
                        "deletion_type": "cascade",
                        "timestamp": Utc::now().to_rfc3339()
                    });
                    
                    tombstone.additional_metadata = Some(metadata.to_string());
                    let operation_id_for_changelog = tombstone.operation_id; // Capture for consistency for changelog
                    
                    // 3. Create the tombstone record
                    self.tombstone_repo.create_tombstone_with_tx(&tombstone, tx).await?;

                    // 4. Create change log entry for the document's hard delete
                     let change_log = ChangeLogEntry {
                        operation_id: operation_id_for_changelog, // Use tombstone's ID
                        entity_table: media_repo.entity_name().to_string(),
                        entity_id: doc_id,
                        operation_type: ChangeOperationType::HardDelete,
                        field_name: None, 
                        old_value: None, 
                        new_value: None,
                        document_metadata: Some(metadata.to_string()), // Add the same metadata
                        timestamp: Utc::now(),
                        user_id: auth.user_id,
                        device_id: parse_device_id(&auth.device_id),
                        sync_batch_id: None, 
                        processed_at: None, 
                        sync_error: None,
                    };
                    
                    self.change_log_repo.create_change_log_with_tx(&change_log, tx).await?;

                    // 5. Add to PendingDeletionManager instead of queuing directly
                    let deletion_info = DocumentDeletion {
                        document_id: doc_id,
                        file_path: doc_data_row.file_path.clone(), // file_path is String in query result
                        compressed_file_path: doc_data_row.compressed_file_path.clone(), // compressed_file_path is Option<String>
                    };
                    self.deletion_manager.add_pending(pending_delete_operation_id, deletion_info).await;
                    
                    // 6. Perform the actual hard delete of the document record
                    media_repo.hard_delete_with_tx(doc_id, auth, tx).await?;
                    
                    // DB ON DELETE CASCADE will handle document versions and access logs automatically

                } else {
                    // --- SOFT DELETE PATH ---
                    
                    // Only soft delete the document record - don't queue for file deletion yet
                    // since the document should still exist but be marked as deleted
                    media_repo.soft_delete_with_tx(doc_id, auth, tx).await?;
                    
                    // Log access for the soft delete
                    let log_id_str = Uuid::new_v4().to_string();
                    let log_doc_id_str = doc_id.to_string();
                    let log_user_id_str = auth.user_id.to_string();
                    let log_timestamp_str = Utc::now().to_rfc3339();
                    let log_details_str = format!("Cascade soft delete from parent: {}/{}", parent_table_name, parent_id);
                    sqlx::query!(
                        r#"
                        INSERT INTO document_access_logs (
                            id,
                            document_id,
                            user_id,
                            access_type,
                            access_date,
                            details
                        )
                        VALUES (?, ?, ?, ?, ?, ?)
                        "#,
                        log_id_str,      // Use variable
                        log_doc_id_str,  // Use variable
                        log_user_id_str, // Use variable
                        "delete", // soft delete
                        log_timestamp_str, // Use variable
                        log_details_str    // Use variable
                    )
                    .execute(&mut **tx)
                    .await
                    .map_err(DbError::from)?; // Propagate error instead of just logging
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

        // 2. Dependency Check
        let table_name = self.repo.entity_name();
        let all_dependencies = self.dependency_checker.check_dependencies(table_name, id).await?;
        
        let blocking_dependencies: Vec<String> = all_dependencies
            .iter()
            .filter(|dep| !dep.is_cascadable && dep.count > 0)
            .map(|dep| dep.table_name.clone())
            .collect();

        // 3. Decide Action: Hard Delete or Soft Delete/Prevent
        let can_hard_delete = options.allow_hard_delete 
                              && auth.role == UserRole::Admin
                              && (blocking_dependencies.is_empty() || options.force);

        // Unique ID for this entire delete operation, used for managing pending document deletions
        let pending_delete_operation_id = Uuid::new_v4();

        if can_hard_delete {
            // --- Hard Delete Path --- 
            let mut tx = self.pool.begin().await.map_err(DbError::from)?; // Start transaction

            let hard_delete_result = async {
                // Create tombstone
                let tombstone = Tombstone::new(id, self.repo.entity_name(), auth.user_id);
                let operation_id_for_changelog = tombstone.operation_id; // Capture for changelog
                self.tombstone_repo.create_tombstone_with_tx(&tombstone, &mut tx).await?;

                // Create change log entry for HardDelete
                let change_log = ChangeLogEntry {
                    operation_id: operation_id_for_changelog, // Use same ID as tombstone
                    entity_table: self.repo.entity_name().to_string(),
                    entity_id: id,
                    operation_type: ChangeOperationType::HardDelete,
                    field_name: None,
                    old_value: None, // Maybe store serialized entity before delete?
                    new_value: None,
                    document_metadata: None, // Not a document deletion itself
                    timestamp: Utc::now(),
                    user_id: auth.user_id,
                    device_id: parse_device_id(&auth.device_id), // Using the helper
                    sync_batch_id: None,
                    processed_at: None,
                    sync_error: None,
                };
                self.change_log_repo.create_change_log_with_tx(&change_log, &mut tx).await?;

                // Perform the actual hard delete of the main entity
                self.repo.hard_delete_with_tx(id, auth, &mut tx).await?;

                // Cascade delete documents, passing the pending_delete_operation_id
                self.cascade_delete_documents::<E>(table_name, id, true, auth, &mut tx, pending_delete_operation_id).await?;

                Ok::<_, DomainError>(())
            }.await;

            match hard_delete_result {
                Ok(_) => {
                    tx.commit().await.map_err(DbError::from)?;
                    // If DB commit is successful, commit the document deletions to the queue
                    self.deletion_manager.commit_deletions(pending_delete_operation_id, auth.user_id).await?;
                    Ok(DeleteResult::HardDeleted)
                },
                Err(e @ DomainError::EntityNotFound(_, _)) => {
                    let _ = tx.rollback().await;
                    // If DB rollback, discard pending document deletions
                    self.deletion_manager.discard_deletions(pending_delete_operation_id).await;
                    Err(e) 
                },
                Err(e) => {
                    let _ = tx.rollback().await; 
                    // If DB rollback, discard pending document deletions
                    self.deletion_manager.discard_deletions(pending_delete_operation_id).await;
                    Err(e) 
                }
            }

        } else {
            // --- Soft Delete or Prevent Path --- 
            if !blocking_dependencies.is_empty() && !options.fallback_to_soft_delete {
                return Ok(DeleteResult::DependenciesPrevented { dependencies: blocking_dependencies });
            }

            let mut tx = self.pool.begin().await.map_err(DbError::from)?; 

            let soft_delete_result = self.repo.soft_delete_with_tx(id, auth, &mut tx).await;

            match soft_delete_result {
                Ok(_) => {
                     // For soft delete, document files are not queued for physical deletion immediately.
                     // The cascade_delete_documents for soft delete path handles DB record changes.
                     // The pending_delete_operation_id is passed but won't be used by add_pending if hard_delete is false.
                     self.cascade_delete_documents::<E>(table_name, id, false, auth, &mut tx, pending_delete_operation_id).await?;

                    tx.commit().await.map_err(DbError::from)?; 
                    Ok(DeleteResult::SoftDeleted { dependencies: blocking_dependencies })
                },
                Err(e @ DomainError::EntityNotFound(_, _)) => {
                    let _ = tx.rollback().await;
                    // No pending physical deletions to discard for soft delete path
                    Err(e) 
                },
                Err(e) => {
                    let _ = tx.rollback().await; 
                    // No pending physical deletions to discard for soft delete path
                    Err(e) 
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