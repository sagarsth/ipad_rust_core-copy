use crate::auth::AuthContext;
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::{DocumentLinkable};
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::{MediaDocumentResponse};
use crate::domains::permission::Permission;
use crate::domains::strategic_goal::repository::StrategicGoalRepository;
use crate::domains::strategic_goal::types::{
    NewStrategicGoal, StrategicGoal, StrategicGoalResponse, UpdateStrategicGoal, StrategicGoalInclude, UserGoalRole, GoalValueSummaryResponse,
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
use std::collections::{HashMap, HashSet};
use chrono::{Utc, DateTime};
use crate::domains::core::delete_service::PendingDeletionManager;
// Add correct imports
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;

// --- ADDED: Import ProjectRepository and ProjectSummary --- 
use crate::domains::project::repository::ProjectRepository;
use crate::domains::project::types::ProjectSummary;

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

    // Add new method signatures here
    /// Find strategic goals by status ID
    async fn find_goals_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>>;

    /// Find strategic goals by responsible team
    async fn find_goals_by_responsible_team(
        &self,
        team_name: &str,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>>;

    /// Find goals created or updated by a specific user
    async fn find_goals_by_user_role(
        &self,
        user_id: Uuid,
        role: UserGoalRole,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>>;

    /// Get status distribution statistics
    async fn get_status_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Get value summary statistics
    async fn get_value_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<GoalValueSummaryResponse>;

    /// Find stale goals that haven't been updated since a specific date
    async fn find_stale_goals(
        &self,
        days_stale: u32,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>>;
}

/// Implementation of the strategic goal service
pub struct StrategicGoalServiceImpl {
    pool: SqlitePool,
    repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<StrategicGoal>>,
    document_service: Arc<dyn DocumentService>,
    // --- ADDED: Project Repository --- 
    project_repo: Arc<dyn ProjectRepository + Send + Sync>,
}

impl StrategicGoalServiceImpl {
    pub fn new(
        pool: SqlitePool,
        strategic_goal_repo: Arc<dyn StrategicGoalRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        document_service: Arc<dyn DocumentService>,
        // --- ADDED: Inject Project Repository --- 
        project_repo: Arc<dyn ProjectRepository + Send + Sync>,
        deletion_manager: Arc<PendingDeletionManager>,
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
            deletion_manager,
        ));
        
        Self {
            pool,
            repo: strategic_goal_repo, // Keep the original repo for other methods
            delete_service,
            document_service,
            // --- ADDED: Store Project Repository --- 
            project_repo,
        }
    }

    // ADDED: Enrichment helper similar to ActivityService
    async fn enrich_response(
        &self, 
        mut response: StrategicGoalResponse, 
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        if let Some(includes) = include {
            let include_set: HashSet<StrategicGoalInclude> = includes.iter().cloned().collect();
            
            // Include documents
            if include_set.contains(&StrategicGoalInclude::Documents) {
                let doc_params = PaginationParams::default();
                match self.document_service.list_media_documents_by_related_entity(
                    auth,
                    "strategic_goals",
                    response.id,
                    doc_params,
                    None
                ).await {
                    Ok(docs_result) => {
                        response.documents = Some(docs_result.items);
                    }
                    Err(e) => {
                        log::error!("Failed to fetch documents for goal {}: {}", response.id, e);
                        // Decide if this should be a hard error or just skip enrichment
                        // return Err(e); // Option 1: Return error
                        response.document_upload_errors = Some(vec![format!("Failed to fetch documents: {}", e)]); // Option 2: Report error, continue
                    }
                }
            }

            // --- ADDED: Include Project Count --- 
            if include_set.contains(&StrategicGoalInclude::ProjectCount) {
                 match self.project_repo.count_by_strategic_goal().await {
                    Ok(counts) => {
                        let count = counts.iter()
                            .find(|(sg_id, _)| sg_id.is_some() && sg_id.unwrap() == response.id)
                            .map(|(_, count)| *count)
                            .unwrap_or(0);
                        response.project_count = Some(count);
                    }
                    Err(e) => {
                        log::error!("Failed to fetch project count for goal {}: {}", response.id, e);
                        // Handle error appropriately
                    }
                }
            }

            // --- ADDED: Include Projects (Summaries) --- 
            if include_set.contains(&StrategicGoalInclude::Projects) {
                // Fetch a limited number of projects for summary view
                let project_params = PaginationParams { page: 1, per_page: 10 }; 
                match self.project_repo.find_by_strategic_goal(response.id, project_params).await {
                    Ok(paginated_projects) => {
                        let project_summaries = paginated_projects.items
                            .into_iter()
                            .map(ProjectSummary::from) // Convert Project to ProjectSummary
                            .collect::<Vec<_>>();
                        response.projects = Some(project_summaries);
                        // If project_count wasn't explicitly requested, set it from this result
                        if response.project_count.is_none() {
                            response.project_count = Some(paginated_projects.total as i64);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to fetch projects for goal {}: {}", response.id, e);
                        // Handle error appropriately
                    }
                }
            }
            
            // TODO: Implement enrichment for Status, Activities, Participants, DocumentCounts
        }
        Ok(response)
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
        // 1. Check Permissions - more explicit checks like in Activity service
        if !auth.has_permission(Permission::CreateStrategicGoals) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create strategic goals".to_string(),
            ));
        }
        
        // Separate permission check for documents
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_goal.validate()?;
        
        // 3. Begin transaction - cleaner transaction handling
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

    // UPDATED: Using enrich_response helper
    async fn get_strategic_goal_by_id(
        &self,
        id: Uuid,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<StrategicGoalResponse> {
        auth.authorize(Permission::ViewStrategicGoals)?;

        let goal = self.repo.find_by_id(id).await.map_err(ServiceError::Domain)?;
        let response = StrategicGoalResponse::from(goal);
        self.enrich_response(response, include, auth).await // Call enrich
    }

    // UPDATED: Using enrich_response helper
    async fn list_strategic_goals(
        &self,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>> {
        auth.authorize(Permission::ViewStrategicGoals)?;

        let paginated_result = self.repo.find_all(params).await.map_err(ServiceError::Domain)?;
        
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = StrategicGoalResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?; // Call enrich
            enriched_items.push(enriched);
        }

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
        // 1. Check Permissions - explicit permission check
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
        // Determine required permission - explicit permission check
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
        let _goal = self.repo.find_by_id(goal_id).await
            .map_err(|e| ServiceError::Domain(e))?;

        // 2. Check permissions - explicit permission check
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Validate the linked field if specified
        if let Some(field) = &linked_field {
            if !StrategicGoal::is_document_linkable_field(field) {
                let valid_fields: Vec<String> = StrategicGoal::document_linkable_fields()
                    .into_iter()
                    .collect();
                    
                return Err(ServiceError::Domain(ValidationError::Custom(format!(
                    "Field '{}' does not support document attachments for strategic goals. Valid fields: {}",
                    field, valid_fields.join(", ")
                )).into()));
            }
        }

        // 4. Delegate to document service
        let document = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            goal_id,
            "strategic_goals".to_string(),
            linked_field.clone(),
            sync_priority,
            compression_priority,
            None,
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
        let _goal = self.repo.find_by_id(goal_id).await
            .map_err(|e| ServiceError::Domain(e))?;

        // 2. Check permissions - explicit permission check
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
            None,
        ).await?;

        Ok(documents)
    }

    // Add implementations for new methods here
    async fn find_goals_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>> {
        auth.authorize(Permission::ViewStrategicGoals)?;

        let paginated_result = self.repo
            .find_by_status(status_id, params)
            .await
            .map_err(ServiceError::Domain)?;

        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = StrategicGoalResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?; // Call enrich
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_goals_by_responsible_team(
        &self,
        team_name: &str,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>> {
        auth.authorize(Permission::ViewStrategicGoals)?;

        let paginated_result = self.repo
            .find_by_responsible_team(team_name, params)
            .await
            .map_err(ServiceError::Domain)?;

        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = StrategicGoalResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?; // Call enrich
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_goals_by_user_role(
        &self,
        user_id: Uuid,
        role: UserGoalRole,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>> {
        auth.authorize(Permission::ViewStrategicGoals)?;

        let paginated_result = self.repo
            .find_by_user_role(user_id, role, params)
            .await
            .map_err(ServiceError::Domain)?;

        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = StrategicGoalResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?; // Call enrich
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn get_status_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        if !auth.has_permission(Permission::ViewStrategicGoals) { // Maybe a more specific permission?
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view strategic goal statistics".to_string(),
            ));
        }

        // 2. Get status counts from repository
        let status_counts = self.repo.count_by_status().await?;
        
        // 3. Convert status IDs to names 
        // TODO: Inject and use a StatusTypeRepository/Service to resolve names
        // For now, using placeholder names
        let mut distribution = HashMap::new();
        for (status_id, count) in status_counts {
            let status_name = match status_id {
                 // Example: Fetch status name using status_repo
                 // Some(id) => self.status_repo.get_status_name(id).await.unwrap_or_else(|_| format!("Status {}", id)),
                 Some(1) => "On Track".to_string(), // Placeholder based on schema seeding
                 Some(2) => "At Risk".to_string(), // Placeholder
                 Some(3) => "Delayed".to_string(), // Placeholder
                 Some(4) => "Completed".to_string(), // Placeholder
                 Some(id) => format!("Unknown Status ({})", id),
                 None => "No Status".to_string(),
            };
            distribution.insert(status_name, count);
        }
        
        Ok(distribution)
    }

    async fn get_value_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<GoalValueSummaryResponse> {
        // 1. Check permissions
        if !auth.has_permission(Permission::ViewStrategicGoals) { // Maybe a more specific permission?
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view strategic goal statistics".to_string(),
            ));
        }

        // 2. Get value summary from repository
        let summary = self.repo.get_value_summary().await?;
        
        // 3. Calculate average progress percentage
        let avg_progress = match (summary.avg_actual, summary.avg_target) {
            (Some(actual), Some(target)) if target > 0.0 => {
                Some((actual / target) * 100.0)
            },
            _ => None,
        };
        
        // 4. Convert to response DTO using the type from types.rs
        let response = GoalValueSummaryResponse {
            avg_target: summary.avg_target,
            avg_actual: summary.avg_actual,
            total_target: summary.total_target,
            total_actual: summary.total_actual,
            count: summary.count,
            avg_progress_percentage: avg_progress,
        };
        
        Ok(response)
    }

    async fn find_stale_goals(
        &self,
        days_stale: u32,
        params: PaginationParams,
        include: Option<&[StrategicGoalInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<StrategicGoalResponse>> {
        auth.authorize(Permission::ViewStrategicGoals)?;

        let cutoff_date = Utc::now() - chrono::Duration::days(days_stale as i64);
        
        let paginated_result = self.repo
            .find_stale_since(cutoff_date, params)
            .await
            .map_err(ServiceError::Domain)?;
            
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = StrategicGoalResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?; // Call enrich
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
}

// Optional: Define helper for find_goals_by_user_role if logic gets complex
// async fn fetch_and_enrich_goals_by_ids(...) -> ServiceResult<Vec<StrategicGoalResponse>> { ... }