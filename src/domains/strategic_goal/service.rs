use crate::auth::AuthContext;
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::{NewMediaDocument, MediaDocumentResponse};
use crate::domains::permission::Permission;
use crate::domains::strategic_goal::repository::StrategicGoalRepository;
use crate::domains::strategic_goal::types::{
    NewStrategicGoal, StrategicGoal, StrategicGoalResponse, UpdateStrategicGoal, StrategicGoalInclude,
};
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError, DbError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use sqlx::{SqlitePool, Transaction, Sqlite};
use std::sync::Arc;
use uuid::Uuid;
use std::str::FromStr;

// Add correct imports
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;

/// Trait defining strategic goal service operations
#[async_trait]
pub trait StrategicGoalService: DeleteService<StrategicGoal> + Send + Sync {
    async fn create_strategic_goal(
        &self,
        new_goal: NewStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse>;

    /// Creates a strategic goal with associated documents in a single operation
    /// Documents are attached to the created goal using temporary IDs
    /// Returns both the created goal and the results of document uploads (which may include errors)
    async fn create_strategic_goal_with_documents(
        &self,
        new_goal: NewStrategicGoal,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (file_data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(StrategicGoalResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;

    async fn get_strategic_goal_by_id(
        &self,
        id: Uuid,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse>;

    async fn list_strategic_goals(
        &self,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>>;

    async fn update_strategic_goal(
        &self,
        id: Uuid,
        update_data: UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse>;
    
    async fn delete_strategic_goal(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;

    // Document integration methods
    async fn upload_document_for_goal(
        &self,
        goal_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_goal(
        &self,
        goal_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;
    
    /// Helper method to upload documents for a strategic goal and handle errors individually
    async fn upload_documents_for_entity(
        &self,
        entity_id: Uuid,
        entity_type: &str,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> Vec<Result<MediaDocumentResponse, ServiceError>>;
}

/// Implementation of the strategic goal service
pub struct StrategicGoalServiceImpl {
    pool: SqlitePool,
    repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<StrategicGoal>>,
    document_service: Arc<dyn DocumentService>,
}

impl StrategicGoalServiceImpl {
    pub fn new(
        pool: SqlitePool,
        strategic_goal_repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        document_service: Arc<dyn DocumentService>,
    ) -> Self {
        // Define a local wrapper struct that implements DeleteServiceRepository
        struct RepoAdapter(Arc<dyn StrategicGoalRepository + Send + Sync>);

        #[async_trait]
        impl FindById<StrategicGoal> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<StrategicGoal> {
                self.0.find_by_id(id).await
            }
        }

        #[async_trait]
        impl SoftDeletable for RepoAdapter {
            async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
                 self.0.soft_delete(id, auth).await
             }
             
             async fn soft_delete_with_tx(
                 &self,
                 id: Uuid,
                 auth: &AuthContext,
                 tx: &mut Transaction<'_, Sqlite>,
             ) -> DomainResult<()> {
                 self.0.soft_delete_with_tx(id, auth, tx).await
             }
        }

        #[async_trait]
        impl HardDeletable for RepoAdapter {
            fn entity_name(&self) -> &'static str {
                 "strategic_goals"
             }
             
             async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
                 self.0.hard_delete(id, auth).await
             }
             
             async fn hard_delete_with_tx(
                 &self,
                 id: Uuid,
                 auth: &AuthContext,
                 tx: &mut Transaction<'_, Sqlite>,
             ) -> DomainResult<()> {
                 self.0.hard_delete_with_tx(id, auth, tx).await
             }
        }

        // Wrap the specific repo in the adapter
        let adapted_repo: Arc<dyn DeleteServiceRepository<StrategicGoal>> = 
            Arc::new(RepoAdapter(strategic_goal_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_repo, // Pass the adapted repo
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None,
        ));
        
        Self {
            pool,
            repo: strategic_goal_repo, // Keep the original repo for other methods
            delete_service,
            document_service,
        }
    }
}

#[async_trait]
impl DeleteService<StrategicGoal> for StrategicGoalServiceImpl {
    // Delegate DeleteService methods to the inner BaseDeleteService
    fn repository(&self) -> &dyn crate::domains::core::repository::FindById<StrategicGoal> {
        self.delete_service.repository()
    }

    fn tombstone_repository(&self) -> &dyn TombstoneRepository {
        self.delete_service.tombstone_repository()
    }

    fn change_log_repository(&self) -> &dyn ChangeLogRepository {
        self.delete_service.change_log_repository()
    }

    fn dependency_checker(&self) -> &dyn DependencyChecker {
        self.delete_service.dependency_checker()
    }

    async fn delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<DeleteResult> {
        self.delete_service.delete(id, auth, options).await
    }

    async fn batch_delete(
        &self,
        ids: &[Uuid],
        auth: &AuthContext,
        options: DeleteOptions,
    ) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> {
        self.delete_service.batch_delete(ids, auth, options).await
    }

    async fn delete_with_dependencies(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<DeleteResult> {
        self.delete_service.delete_with_dependencies(id, auth).await
    }
    
    async fn get_failed_delete_details(
        &self,
        batch_result: &crate::domains::core::delete_service::BatchDeleteResult,
        auth: &AuthContext,
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<StrategicGoal>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

#[async_trait]
impl StrategicGoalService for StrategicGoalServiceImpl {
    async fn create_strategic_goal(
        &self,
        new_goal: NewStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateStrategicGoals) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create strategic goals".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_goal.validate()?; // Propagates DomainError::Validation

        // 3. Perform Creation
        let created_goal = self.repo.create(&new_goal, auth).await?;

        // 4. Convert to Response DTO
        let response = StrategicGoalResponse::from(created_goal);
        Ok(response)
    }
    
    /// Creates a strategic goal with associated documents in a single operation
    async fn create_strategic_goal_with_documents(
        &self,
        new_goal: NewStrategicGoal,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (file_data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(StrategicGoalResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateStrategicGoals) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create strategic goals".to_string(),
            ));
        }
        
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_goal.validate()?;
        
        // 3. Begin transaction
        let mut tx = self.pool.begin().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 4. Create the strategic goal first (within transaction)
        let created_goal = match self.repo.create_with_tx(&new_goal, auth, &mut tx).await {
            Ok(goal) => goal,
            Err(e) => {
                // Rollback transaction on error
                let _ = tx.rollback().await;
                return Err(ServiceError::Domain(e));
            }
        };
        
        // 5. Commit transaction to ensure goal is created
        tx.commit().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 6. Now that we have a goal ID, upload documents (outside transaction)
        let document_results = if !documents.is_empty() {
            self.upload_documents_for_entity(
                created_goal.id,
                "strategic_goals",
                documents,
                document_type_id,
                SyncPriority::Normal,
                None, // Use default compression priority
                auth,
            ).await
        } else {
            Vec::new()
        };
        
        // 7. Convert to Response DTO and return with document results
        let response = StrategicGoalResponse::from(created_goal);
        Ok((response, document_results))
    }

    async fn get_strategic_goal_by_id(
        &self,
        id: Uuid,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::ViewStrategicGoals) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view strategic goals".to_string(),
            ));
        }

        // 2. Fetch from Repository
        let goal = self.repo.find_by_id(id).await?;

        // 3. Convert to Response DTO
        let response = StrategicGoalResponse::from(goal);
        Ok(response)
    }

    async fn list_strategic_goals(
        &self,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::ViewStrategicGoals) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list strategic goals".to_string(),
            ));
        }

        // 2. Fetch Paginated Data
        let paginated_result = self.repo.find_all(params).await?;

        // 3. Convert items to Response DTOs
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = StrategicGoalResponse::from(item);
            enriched_items.push(response);
        }

        // 4. Create Paginated Response
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn update_strategic_goal(
        &self,
        id: Uuid,
        mut update_data: UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::EditStrategicGoals) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to edit strategic goals".to_string(),
            ));
        }

        // Ensure the updater's ID is set
        update_data.updated_by_user_id = auth.user_id; 

        // 2. Validate Input DTO
        update_data.validate()?;
        
        // 3. Perform Update
        let updated_goal = self.repo.update(id, &update_data, auth).await?;

        // 4. Convert to Response DTO
        let response = StrategicGoalResponse::from(updated_goal);
        Ok(response)
    }
    
    async fn delete_strategic_goal(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        // Determine required permission
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteStrategicGoals
        };

        if !auth.has_permission(required_permission) {
             return Err(ServiceError::PermissionDenied(format!(
                 "User does not have permission to {} strategic goals",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }
        
        // Configure delete options
        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: !hard_delete,
            force: false,
        };
        
        // Delegate to the core delete method
        let result = self.delete(id, auth, options).await?;
        
        Ok(result)
    }

    // Helper method to upload documents for any entity and handle errors individually
    async fn upload_documents_for_entity(
        &self,
        entity_id: Uuid,
        entity_type: &str,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> Vec<Result<MediaDocumentResponse, ServiceError>> {
        let mut results = Vec::new();
        
        for (file_data, filename, linked_field) in documents {
            let upload_result = self.document_service.upload_document(
                auth,
                file_data,
                filename,
                None, // No title, will use filename as default
                document_type_id,
                entity_id,
                entity_type.to_string(),
                linked_field,
                sync_priority,
                compression_priority,
                None, // No temp ID needed since entity exists
            ).await;
            
            // Store the result (success or error) without failing the whole operation
            results.push(upload_result);
        }
        
        results
    }

    // Document integration methods
    async fn upload_document_for_goal(
        &self,
        goal_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        // 1. Verify goal exists
        let goal = self.repo.find_by_id(goal_id).await
            .map_err(|e| ServiceError::Domain(e))?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Delegate to document service
        let document = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            goal_id,
            "strategic_goals".to_string(),
            linked_field,
            sync_priority,
            compression_priority,
            None, // No temp ID for direct uploads
        ).await?;

        Ok(document)
    }

    async fn bulk_upload_documents_for_goal(
        &self,
        goal_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        // 1. Verify goal exists
        let goal = self.repo.find_by_id(goal_id).await
            .map_err(|e| ServiceError::Domain(e))?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Delegate to document service
        let documents = self.document_service.bulk_upload_documents(
            auth,
            files,
            title,
            document_type_id,
            goal_id,
            "strategic_goals".to_string(),
            sync_priority,
            compression_priority,
            None, // No temp ID for direct uploads
        ).await?;

        Ok(documents)
    }
}