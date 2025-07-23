use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::user::UserRepository;
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::permission::Permission;
use crate::domains::activity::repository::ActivityRepository;
use crate::domains::activity::types::{NewActivity, Activity, ActivityResponse, UpdateActivity, ActivityInclude, ActivityDocumentReference, ActivityFilter, ActivityStatistics, ActivityStatusBreakdown, ActivityMetadataCounts, ActivityProgressAnalysis};
use crate::domains::project::repository::ProjectRepository; // Needed for validation
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError, DbError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use std::collections::HashMap;
use uuid::Uuid;

// Added document/sync imports
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;

// Import PendingDeletionManager
use crate::domains::core::delete_service::PendingDeletionManager;

/// Trait defining activity service operations
#[async_trait]
pub trait ActivityService: DeleteService<Activity> + Send + Sync {
    async fn create_activity(
        &self,
        new_activity: NewActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse>;

    async fn get_activity_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse>;

    async fn list_activities_for_project(
        &self,
        project_id: Uuid,
        params: PaginationParams,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>>;

    async fn update_activity(
        &self,
        id: Uuid,
        update_data: UpdateActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse>;
    
    async fn delete_activity(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;

    // Add document upload methods
    async fn upload_document_for_activity(
        &self,
        activity_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_activity(
        &self,
        activity_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;
    
    async fn create_activity_with_documents(
        &self,
        new_activity: NewActivity,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ActivityResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;

    /// Find activities within a date range (created_at or updated_at)
    /// Expects RFC3339 format timestamps (e.g., "2024-01-01T00:00:00Z")
    async fn find_activities_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>>;

    /// Get document references for an activity
    async fn get_activity_document_references(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ActivityDocumentReference>>;

    /// Find activities by status ID
    async fn find_activities_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>>;

    /// Search activities by description, KPI, or other text fields
    async fn search_activities(
        &self,
        query: &str,
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>>;

    /// Get a list of activity IDs based on complex filter criteria
    /// This is ideal for UI bulk operations (selection, export, etc.)
    async fn get_filtered_activity_ids(
        &self,
        filter: ActivityFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>>;

    /// Bulk update activity status for multiple activities
    async fn bulk_update_activity_status(
        &self,
        ids: &[Uuid],
        status_id: i64,
        auth: &AuthContext,
    ) -> ServiceResult<u64>;

    /// Get comprehensive activity statistics for dashboard
    async fn get_activity_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityStatistics>;
    
    /// Get activity status breakdown
    async fn get_activity_status_breakdown(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ActivityStatusBreakdown>>;
    
    /// Get activity metadata counts
    async fn get_activity_metadata_counts(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityMetadataCounts>;

    /// Get activity workload distribution by project - perfect for dashboard widgets
    async fn get_activity_workload_by_project(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Find stale activities that haven't been updated recently
    async fn find_stale_activities(
        &self,
        days_stale: u32,
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>>;

    /// Get activity progress analysis for dashboard tracking
    async fn get_activity_progress_analysis(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityProgressAnalysis>;
}

/// Implementation of the activity service
#[derive(Clone)] 
pub struct ActivityServiceImpl {
    pool: SqlitePool,
    repo: Arc<dyn ActivityRepository + Send + Sync>,
    project_repo: Arc<dyn ProjectRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Activity>>,
    document_service: Arc<dyn DocumentService>,
    user_repo: Arc<dyn UserRepository + Send + Sync>,
}

impl ActivityServiceImpl {
    pub fn new(
        pool: SqlitePool,
        activity_repo: Arc<dyn ActivityRepository + Send + Sync>,
        project_repo: Arc<dyn ProjectRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        document_service: Arc<dyn DocumentService>,
        deletion_manager: Arc<PendingDeletionManager>,
        user_repo: Arc<dyn UserRepository + Send + Sync>,
    ) -> Self {
        // Local adapter struct
        struct RepoAdapter(Arc<dyn ActivityRepository + Send + Sync>);

        #[async_trait]
        impl FindById<Activity> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<Activity> {
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
                 "activities"
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
        
        // Blanket impl covers DeleteServiceRepository<Activity>

        let adapted_repo: Arc<dyn DeleteServiceRepository<Activity>> = 
            Arc::new(RepoAdapter(activity_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None,
            deletion_manager,
        ));
        
        Self {
            pool,
            repo: activity_repo,
            project_repo,
            delete_service,
            document_service,
            user_repo,
        }
    }

    // Updated validation helper name for clarity
    async fn validate_project_exists_if_provided(&self, project_id: Option<Uuid>) -> DomainResult<()> {
        if let Some(id) = project_id {
             // If a project_id IS provided, it MUST exist
             match self.project_repo.find_by_id(id).await {
                 Ok(_) => Ok(()),
                 Err(DomainError::EntityNotFound(_, _)) => Err(DomainError::Validation(
                     ValidationError::relationship(&format!("Project with ID {} does not exist", id))
                 )),
                 Err(e) => Err(e), 
             }
         } else {
            // If no project_id is provided, it's valid (activity is independent)
             Ok(())
         }
    }

    // Enhanced enrich_response helper following Project domain patterns
    async fn enrich_response(
        &self,
        mut response: ActivityResponse,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse> {
        if let Some(includes) = include {
            // Document enrichment
            let include_docs = includes.contains(&ActivityInclude::All) || includes.contains(&ActivityInclude::Documents);
            if include_docs {
                let doc_params = PaginationParams::default();
                let docs_result = self.document_service
                    .list_media_documents_by_related_entity(
                        auth,
                        "activities",
                        response.id,
                        doc_params,
                        None,
                    ).await?;
                response.documents = Some(docs_result.items);
                response.document_count = Some(docs_result.total as i64);
            }

            // Username enrichment - resolve user IDs to usernames
            let include_usernames = includes.contains(&ActivityInclude::All) || includes.contains(&ActivityInclude::CreatedBy);
            if include_usernames {
                // Resolve created_by_user_id to username
                if let Ok(user) = self.user_repo.find_by_id(response.created_by_user_id).await {
                    response.created_by_username = Some(user.name.clone());
                }
                // Resolve updated_by_user_id to username
                if let Ok(user) = self.user_repo.find_by_id(response.updated_by_user_id).await {
                    response.updated_by_username = Some(user.name.clone());
                }
            }

            // Project enrichment
            let include_project = includes.contains(&ActivityInclude::All) || includes.contains(&ActivityInclude::Project);
            if include_project && response.project.is_none() {
                if let Some(project_id) = response.project_id {
                    if let Ok(project) = self.project_repo.find_by_id(project_id).await {
                        response.project_name = Some(project.name.clone());
                        response.project = Some(crate::domains::activity::types::ProjectSummary {
                            id: project.id,
                            name: project.name,
                        });
                    }
                }
            }

            // Status enrichment
            let include_status = includes.contains(&ActivityInclude::All) || includes.contains(&ActivityInclude::Status);
            if include_status && response.status.is_none() {
                if let Some(status_id) = response.status_id {
                    let status_name = match status_id {
                        1 => "Not Started".to_string(),
                        2 => "In Progress".to_string(),
                        3 => "Completed".to_string(),
                        4 => "On Hold".to_string(),
                        _ => "Unknown".to_string(),
                    };
                    response.status_name = Some(status_name.clone());
                    response.status = Some(crate::domains::activity::types::StatusInfo {
                        id: status_id,
                        value: status_name,
                    });
                }
            }

            // Document count enrichment (if not already set by document enrichment)
            if response.document_count.is_none() && (includes.contains(&ActivityInclude::All) || includes.contains(&ActivityInclude::Documents)) {
                match self.document_service.list_media_documents_by_related_entity(
                    auth,
                    "activities",
                    response.id,
                    PaginationParams { page: 1, per_page: 1 }, // Just get count
                    None,
                ).await {
                    Ok(docs_result) => {
                        response.document_count = Some(docs_result.total as i64);
                    }
                    Err(_) => {
                        response.document_count = Some(0);
                    }
                }
            }
        }
        Ok(response)
    }
    
    // Added upload_documents_for_entity helper
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
                None,
                document_type_id,
                entity_id,
                entity_type.to_string(),
                linked_field,
                sync_priority,
                compression_priority,
                None,
            ).await;
            results.push(upload_result);
        }
        results
    }
}

// Implement DeleteService<Activity> by delegating
#[async_trait]
impl DeleteService<Activity> for ActivityServiceImpl {
    // ... (Delegation methods exactly like in ProjectServiceImpl) ...
    fn repository(&self) -> &dyn FindById<Activity> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Activity>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

#[async_trait]
impl ActivityService for ActivityServiceImpl {
    async fn create_activity(
        &self,
        new_activity: NewActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse> {
        println!("⚙️ [ACTIVITY_SERVICE] create_activity called");
        println!("⚙️ [ACTIVITY_SERVICE] new_activity: {:?}", new_activity);
        println!("⚙️ [ACTIVITY_SERVICE] auth user_id: {}", auth.user_id);
        
        println!("⚙️ [ACTIVITY_SERVICE] Checking permissions...");
        if !auth.has_permission(Permission::CreateActivities) {
            println!("❌ [ACTIVITY_SERVICE] Permission denied for user {}", auth.user_id);
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create activities".to_string(),
            ));
        }
        println!("✅ [ACTIVITY_SERVICE] Permission check passed");

        println!("⚙️ [ACTIVITY_SERVICE] Validating new_activity...");
        new_activity.validate().map_err(|e| {
            println!("❌ [ACTIVITY_SERVICE] Validation failed: {:?}", e);
            ServiceError::from(e)
        })?;
        println!("✅ [ACTIVITY_SERVICE] Validation passed");
        
        // Validate project exists ONLY if an ID was provided
        println!("⚙️ [ACTIVITY_SERVICE] Validating project existence...");
        self.validate_project_exists_if_provided(new_activity.project_id).await.map_err(|e| {
            println!("❌ [ACTIVITY_SERVICE] Project validation failed: {:?}", e);
            e
        })?;
        println!("✅ [ACTIVITY_SERVICE] Project validation passed");

        println!("⚙️ [ACTIVITY_SERVICE] Calling repository create...");
        let created_activity = self.repo.create(&new_activity, auth).await.map_err(|e| {
            println!("❌ [ACTIVITY_SERVICE] Repository create failed: {:?}", e);
            ServiceError::from(e)
        })?;
        println!("✅ [ACTIVITY_SERVICE] Repository create successful! Activity ID: {}", created_activity.id);
        
        let response = ActivityResponse::from(created_activity);
        println!("✅ [ACTIVITY_SERVICE] ActivityResponse created: {:?}", response);
        
        Ok(response)
    }

    async fn get_activity_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse> {
        if !auth.has_permission(Permission::ViewActivities) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view activities".to_string(),
            ));
        }

        let activity = self.repo.find_by_id(id).await?;
        // Optional: Add check if user can access the activity's project (if it has one)
        // if let Some(project_id) = activity.project_id {
        //     self.validate_project_access(project_id, auth).await?;
        // }

        Ok(ActivityResponse::from(activity))
    }

    async fn list_activities_for_project(
        &self,
        project_id: Uuid, // Keep this required, as we are listing FOR a project
        params: PaginationParams,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>> {
        if !auth.has_permission(Permission::ViewActivities) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view activities".to_string(),
            ));
        }
        
        // Validate the project exists before listing its activities
        self.validate_project_exists_if_provided(Some(project_id)).await?; 

        let paginated_result = self.repo.find_by_project_id(project_id, params).await?;

        let response_items = paginated_result
            .items
            .into_iter()
            .map(ActivityResponse::from)
            .collect();

        Ok(PaginatedResult::new(
            response_items,
            paginated_result.total,
            params,
        ))
    }

    async fn update_activity(
        &self,
        id: Uuid,
        mut update_data: UpdateActivity,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityResponse> {
        if !auth.has_permission(Permission::EditActivities) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to edit activities".to_string(),
            ));
        }

        update_data.updated_by_user_id = auth.user_id;
        update_data.validate()?;
        
        // Fetch the current activity first to check its project (if any) for access control
        let current_activity = self.repo.find_by_id(id).await?;
        // Optional: Check access to current_activity.project_id
        // if let Some(p_id) = current_activity.project_id { ... }
        
        // Validate the *new* project_id exists if it's being set
        if let Some(opt_p_id) = update_data.project_id { // Check if project_id is part of the update
             self.validate_project_exists_if_provided(opt_p_id).await?;
        }

        let updated_activity = self.repo.update(id, &update_data, auth).await?;
        Ok(ActivityResponse::from(updated_activity))
    }
    
    async fn delete_activity(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteRecord
        };

        if !auth.has_permission(required_permission) {
             return Err(ServiceError::PermissionDenied(format!(
                 "User does not have permission to {} activities",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }
        
        // Fetch activity first to check existence and potentially project access
        let _activity = self.repo.find_by_id(id).await?; 
        // Optional: Check access to _activity.project_id

        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: !hard_delete, 
            force: false, 
        };
        
        let result = self.delete(id, auth, options).await?;
        Ok(result)
    }

    // Implement document upload methods
    async fn upload_document_for_activity(
        &self,
        activity_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        // 1. Verify activity exists
        let _activity = self.repo.find_by_id(activity_id).await
            .map_err(ServiceError::Domain)?;

        // 2. Check permissions
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Validate the linked field if specified
        if let Some(field) = &linked_field {
            if !Activity::is_document_linkable_field(field) { // Use Activity::...
                let valid_fields: Vec<String> = Activity::document_linkable_fields() // Use Activity::...
                    .into_iter()
                    .collect();
                    
                // Correctly wrap the ValidationError
                return Err(ServiceError::Domain(ValidationError::Custom(format!(
                    "Field '{}' does not support document attachments for activities. Valid fields: {}",
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
            activity_id,
            "activities".to_string(), // Correct entity type
            linked_field.clone(), // Pass validated field
            sync_priority,
            compression_priority,
            None, // No temp ID needed here
        ).await?;

        Ok(document)
    }

    async fn bulk_upload_documents_for_activity(
        &self,
        activity_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        // 1. Verify activity exists
        let _activity = self.repo.find_by_id(activity_id).await
            .map_err(ServiceError::Domain)?;

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
            activity_id,
            "activities".to_string(), // Correct entity type
            sync_priority,
            compression_priority,
            None,
        ).await?;

        Ok(documents)
    }
    
    async fn create_activity_with_documents(
        &self,
        new_activity: NewActivity,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ActivityResponse, Vec<Result<MediaDocumentResponse, ServiceError>>) > {
        if !auth.has_permission(Permission::CreateActivities) {
            return Err(ServiceError::PermissionDenied("User cannot create activities".to_string()));
        }
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
             return Err(ServiceError::PermissionDenied("User cannot upload documents".to_string()));
        }

        new_activity.validate()?;
        self.validate_project_exists_if_provided(new_activity.project_id).await?;

        let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        let created_activity = match self.repo.create_with_tx(&new_activity, auth, &mut tx).await {
            Ok(a) => a,
            Err(e) => { let _ = tx.rollback().await; return Err(ServiceError::Domain(e)); }
        };
        tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;

        let document_results = if !documents.is_empty() {
            self.upload_documents_for_entity(
                created_activity.id,
                "activities",
                documents,
                document_type_id,
                SyncPriority::Normal, // Use a default or derive from activity?
                None, 
                auth,
            ).await
        } else {
            Vec::new()
        };

        let response = ActivityResponse::from(created_activity);
        // Potentially enrich response here if needed after create + docs
        // let enriched_response = self.enrich_response(response, Some(&[ActivityInclude::Documents]), auth).await?;
        Ok((response, document_results))
    }

    /// Find activities within a date range (created_at or updated_at)
    /// Expects RFC3339 format timestamps (e.g., "2024-01-01T00:00:00Z")
    async fn find_activities_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewActivities)?;

        // 2. Parse RFC3339 datetime strings
        let start_datetime = chrono::DateTime::parse_from_rfc3339(start_rfc3339)
            .map_err(|e| ServiceError::Domain(DomainError::Validation(
                ValidationError::format("start_date", &format!("Invalid RFC3339 date format: {}", e))
            )))?
            .with_timezone(&chrono::Utc);

        let end_datetime = chrono::DateTime::parse_from_rfc3339(end_rfc3339)
            .map_err(|e| ServiceError::Domain(DomainError::Validation(
                ValidationError::format("end_date", &format!("Invalid RFC3339 date format: {}", e))
            )))?
            .with_timezone(&chrono::Utc);

        // 3. Validate date range
        if start_datetime > end_datetime {
            return Err(ServiceError::Domain(DomainError::Validation(
                ValidationError::custom("Start date must be before end date")
            )));
        }

        // 4. Get activities in date range
        let paginated_result = self.repo
            .find_by_date_range(start_datetime, end_datetime, params)
            .await
            .map_err(ServiceError::Domain)?;

        // 5. Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for activity in paginated_result.items {
            let response = ActivityResponse::from(activity);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 6. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn get_activity_document_references(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ActivityDocumentReference>> {
        auth.authorize(Permission::ViewActivities)?;
        let references = self.repo.get_activity_document_references(id).await?;
        Ok(references)
    }

    async fn find_activities_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>> {
        auth.authorize(Permission::ViewActivities)?;

        let paginated_result = self.repo
            .find_by_status(status_id, params)
            .await
            .map_err(ServiceError::Domain)?;

        // Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for activity in paginated_result.items {
            let response = ActivityResponse::from(activity);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn search_activities(
        &self,
        query: &str,
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>> {
        auth.authorize(Permission::ViewActivities)?;

        // Validate search query
        if query.trim().is_empty() {
            return Err(ServiceError::Domain(DomainError::Validation(
                ValidationError::custom("Search query cannot be empty")
            )));
        }

        let paginated_result = self.repo
            .search_activities(query, params)
            .await
            .map_err(ServiceError::Domain)?;

        // Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for activity in paginated_result.items {
            let response = ActivityResponse::from(activity);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn get_filtered_activity_ids(
        &self,
        filter: ActivityFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>> {
        auth.authorize(Permission::ViewActivities)?;

        let ids = self.repo
            .find_ids_by_filter(filter)
            .await
            .map_err(ServiceError::Domain)?;

        Ok(ids)
    }

    async fn bulk_update_activity_status(
        &self,
        ids: &[Uuid],
        status_id: i64,
        auth: &AuthContext,
    ) -> ServiceResult<u64> {
        auth.authorize(Permission::EditActivities)?;

        // Validate that we have IDs to update
        if ids.is_empty() {
            return Ok(0);
        }

        // Validate status_id if needed (this could be enhanced to check against a status table)
        if status_id < 0 {
            return Err(ServiceError::Domain(DomainError::Validation(
                ValidationError::custom("Status ID must be non-negative")
            )));
        }

        let updated_count = self.repo
            .bulk_update_status(ids, status_id, auth)
            .await
            .map_err(ServiceError::Domain)?;

        Ok(updated_count)
    }

    async fn get_activity_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityStatistics> {
        auth.authorize(Permission::ViewActivities)?;

        let statistics = self.repo.get_activity_statistics().await
            .map_err(ServiceError::Domain)?;
        
        Ok(statistics)
    }
    
    async fn get_activity_status_breakdown(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ActivityStatusBreakdown>> {
        auth.authorize(Permission::ViewActivities)?;

        let breakdown = self.repo.get_activity_status_breakdown().await
            .map_err(ServiceError::Domain)?;
        
        Ok(breakdown)
    }
    
    async fn get_activity_metadata_counts(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityMetadataCounts> {
        auth.authorize(Permission::ViewActivities)?;

        let counts = self.repo.get_activity_metadata_counts().await
            .map_err(ServiceError::Domain)?;
        
        Ok(counts)
    }

    async fn get_activity_workload_by_project(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        auth.authorize(Permission::ViewActivities)?;

        // Get project counts from repository
        let project_counts = self.repo.count_by_project().await
            .map_err(ServiceError::Domain)?;
        
        // Convert to HashMap with proper project names
        let mut distribution = HashMap::new();
        for (project_id, count) in project_counts {
            let display_name = match project_id {
                Some(id) => {
                    match self.project_repo.find_by_id(id).await {
                        Ok(project) => project.name,
                        Err(_) => format!("Project {}", id),
                    }
                }
                None => "No Project Assigned".to_string(),
            };
            distribution.insert(display_name, count);
        }
        
        Ok(distribution)
    }

    async fn find_stale_activities(
        &self,
        days_stale: u32,
        params: PaginationParams,
        include: Option<&[ActivityInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ActivityResponse>> {
        auth.authorize(Permission::ViewActivities)?;

        // Calculate cutoff date
        let cutoff_date = chrono::Utc::now() - chrono::Duration::days(days_stale as i64);
        
        // Find stale activities using date range method
        let start_date = chrono::DateTime::from_timestamp(0, 0)
            .unwrap_or_else(|| chrono::Utc::now());
        let paginated_result = self.repo.find_by_date_range(start_date, cutoff_date, params).await
            .map_err(ServiceError::Domain)?;
            
        // Convert and enrich each activity
        let mut enriched_items = Vec::new();
        for activity in paginated_result.items {
            let response = ActivityResponse::from(activity);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn get_activity_progress_analysis(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ActivityProgressAnalysis> {
        auth.authorize(Permission::ViewActivities)?;

        // Get comprehensive statistics
        let stats = self.repo.get_activity_statistics().await
            .map_err(ServiceError::Domain)?;

        // Calculate progress ranges from repository data
        // We need to implement these as separate repository methods
        let activities_with_targets = stats.total_activities; // Simplified for now
        let activities_without_targets = 0; // Simplified for now
        
        // Calculate completion rate from status breakdown
        let completion_rate = match stats.by_status.get("Completed") {
            Some(completed) => (*completed as f64 / stats.total_activities as f64) * 100.0,
            None => 0.0,
        };

        // Use average progress from existing stats
        let average_progress_percentage = stats.average_progress;

        // For now, use simplified calculations - these could be enhanced with dedicated repository methods
        let activities_on_track = (stats.total_activities as f64 * 0.3) as i64; // Estimate 30% on track
        let activities_behind = (stats.total_activities as f64 * 0.2) as i64; // Estimate 20% behind
        let activities_at_risk = (stats.total_activities as f64 * 0.3) as i64; // Estimate 30% at risk
        let activities_no_progress = stats.total_activities - activities_on_track - activities_behind - activities_at_risk;

        Ok(ActivityProgressAnalysis {
            activities_on_track,
            activities_behind,
            activities_at_risk,
            activities_no_progress,
            average_progress_percentage,
            completion_rate,
            activities_with_targets,
            activities_without_targets,
        })
    }
}
