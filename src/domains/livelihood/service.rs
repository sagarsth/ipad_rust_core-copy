use crate::auth::AuthContext;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService};
use crate::domains::core::repository::{DeleteResult, FindById};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::livelihood::repository::{LivehoodRepository, SubsequentGrantRepository, SqliteLivelihoodRepository, SqliteSubsequentGrantRepository};
use crate::domains::livelihood::types::{Livelihood, LivelihoodInclude, LivelihoodResponse, NewLivelihood, NewSubsequentGrant, ParticipantSummary, ProjectSummary, SubsequentGrantResponse, SubsequentGrantSummary, UpdateLivelihood, UpdateSubsequentGrant, LivelioodStatsSummary, LivelioodWithParticipantDetails, ParticipantDetails, LivelioodWithDocumentTimeline, ParticipantOutcomeMetrics, LivelihoodDashboardMetrics};
use crate::domains::participant::repository::ParticipantRepository;
use crate::domains::permission::Permission;
use crate::domains::project::repository::ProjectRepository;
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, DbError, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, Transaction, Row};
use std::{sync::Arc, collections::HashMap};
use uuid::Uuid;
use chrono::{Utc, DateTime, Datelike};

// Add document-related imports
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;

/// Interface for the livelihood service
#[async_trait]
pub trait LivehoodService: DeleteService<Livelihood> + Send + Sync {
    /// Create a new livelihood
    async fn create_livelihood(
        &self,
        new_livelihood: NewLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse>;

    /// Create a new livelihood with documents
    async fn create_livelihood_with_documents(
        &self,
        new_livelihood: NewLivelihood,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (file_data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(LivelihoodResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;

    /// Get a livelihood by ID
    async fn get_livelihood_by_id(
        &self,
        id: Uuid,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse>;

    /// List all livelihoods with optional filtering
    async fn list_livelihoods(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>>;

    /// Update a livelihood
    async fn update_livelihood(
        &self,
        id: Uuid,
        update_data: UpdateLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse>;
    
    /// Delete a livelihood
    async fn delete_livelihood(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
    
    /// Add a subsequent grant to a livelihood
    async fn add_subsequent_grant(
        &self,
        new_grant: NewSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse>;
    
    /// Update a subsequent grant
    async fn update_subsequent_grant(
        &self,
        id: Uuid,
        update_data: UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse>;
    
    /// Get a subsequent grant by ID
    async fn get_subsequent_grant_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse>;
    
    /// Delete a subsequent grant
    async fn delete_subsequent_grant(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<()>;

    /// Upload a document for a livelihood
    async fn upload_document_for_livelihood(
        &self,
        livelihood_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    /// Bulk upload documents for a livelihood
    async fn bulk_upload_documents_for_livelihood(
        &self,
        livelihood_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;

    /// Get comprehensive livelihood statistics for dashboard
    async fn get_livelihood_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<LivelioodStatsSummary>;

    /// Get outcome status distribution for dashboard
    async fn get_outcome_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Find livelihoods with outcome documentation
    async fn find_livelihoods_with_outcome(
        &self,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>>;

    /// Find livelihoods without outcome documentation (for tracking)
    async fn find_livelihoods_without_outcome(
        &self,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>>;

    /// Find livelihoods with multiple grants
    async fn find_livelihoods_with_multiple_grants(
        &self,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>>;

    /// Get livelihood with detailed participant information
    async fn get_livelihood_with_participant_details(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<LivelioodWithParticipantDetails>;

    /// Get livelihood with document timeline
    async fn get_livelihood_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<LivelioodWithDocumentTimeline>;

    /// Get outcome metrics for a participant
    async fn get_participant_outcome_metrics(
        &self,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantOutcomeMetrics>;
    
    /// Get livelihood dashboard metrics
    async fn get_livelihood_dashboard_metrics(
        &self,
        months_back: i32,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodDashboardMetrics>;
    
    /// Find livelihoods created in date range
    async fn find_livelihoods_by_date_range(
        &self,
        start_date: &str,
        end_date: &str,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>>;
}

/// Implementation of the livelihood service
pub struct LivehoodServiceImpl {
    repo: Arc<SqliteLivelihoodRepository>,
    delete_service: Arc<BaseDeleteService<Livelihood>>,
    subsequent_grant_repo: Arc<SqliteSubsequentGrantRepository>,
    project_repo: Arc<dyn ProjectRepository>,
    participant_repo: Arc<dyn ParticipantRepository>,
    document_service: Arc<dyn DocumentService>,
    pool: Pool<Sqlite>,
}

impl LivehoodServiceImpl {
    /// Create a new livelihood service implementation
    pub fn new(
        pool: Pool<Sqlite>,
        livelihood_repo: Arc<SqliteLivelihoodRepository>,
        subsequent_grant_repo: Arc<SqliteSubsequentGrantRepository>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        project_repo: Arc<dyn ProjectRepository>,
        participant_repo: Arc<dyn ParticipantRepository>,
        document_service: Arc<dyn DocumentService>,
        media_doc_repo: Arc<dyn MediaDocumentRepository>,
    ) -> Self {
        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(),
            livelihood_repo.clone(),
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            Some(media_doc_repo) // Pass media repo for delete service to handle document cleanup
        ));
        
        Self {
            repo: livelihood_repo,
            delete_service,
            subsequent_grant_repo,
            project_repo,
            participant_repo,
            document_service,
            pool,
        }
    }
    
    /// Enrich a livelihood response with related entities
    async fn enrich_response(
        &self,
        mut response: LivelihoodResponse,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse> {
        let includes = match include {
            Some(includes) => includes,
            None => return Ok(response),
        };
        
        // Check if we need to include all relations
        let include_all = includes.contains(&LivelihoodInclude::All);
        
        // Include project if requested
        if include_all || includes.contains(&LivelihoodInclude::Project) {
            if let Some(project_id) = response.project_id {
                match self.project_repo.find_by_id(project_id).await {
                    Ok(project) => {
                        response = response.with_project(ProjectSummary {
                            id: project.id,
                            name: project.name,
                        });
                    }
                    Err(DomainError::EntityNotFound(_, _)) => {
                        // Project not found, but this is not a critical error
                    }
                    Err(e) => return Err(ServiceError::Domain(e)),
                }
            }
        }
        
        // Include participant if requested
        if include_all || includes.contains(&LivelihoodInclude::Participant) {
            if let Some(participant_id) = response.participant_id {
                match self.participant_repo.find_by_id(participant_id).await {
                    Ok(participant) => {
                        response = response.with_participant(ParticipantSummary {
                            id: participant.id,
                            name: participant.name,
                            gender: participant.gender,
                            age_group: participant.age_group,
                            disability: participant.disability,
                        });
                    }
                    Err(DomainError::EntityNotFound(_, _)) => {
                        // Participant not found, but this is not a critical error
                    }
                    Err(e) => return Err(ServiceError::Domain(e)),
                }
            }
        }
        
        // Include subsequent grants if requested
        if include_all || includes.contains(&LivelihoodInclude::SubsequentGrants) {
            match self.subsequent_grant_repo.find_by_livelihood_id(response.id).await {
                Ok(grants) => {
                    let grant_summaries = grants
                        .into_iter()
                        .map(SubsequentGrantSummary::from)
                        .collect::<Vec<_>>();
                    
                    response = response.with_subsequent_grants(grant_summaries);
                }
                Err(e) => return Err(ServiceError::Domain(e)),
            }
        }

        // Include documents if requested
        if include_all || includes.contains(&LivelihoodInclude::Documents) {
            let doc_params = PaginationParams::default();
            match self.document_service.list_media_documents_by_related_entity(
                auth,
                "livelihoods", // Entity type
                response.id,
                doc_params,
                None, // No nested includes for documents
            ).await {
                Ok(docs_result) => {
                    response.documents = Some(docs_result.items);
                }
                Err(e) => return Err(e),
            }
        }
        
        Ok(response)
    }

    /// Helper method to upload documents for a livelihood and handle errors individually
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
}

// Implement DeleteService for LivehoodServiceImpl
#[async_trait]
impl DeleteService<Livelihood> for LivehoodServiceImpl {
    fn repository(&self) -> &dyn FindById<Livelihood> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Livelihood>>> {
        self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

// Implement LivehoodService for LivehoodServiceImpl
#[async_trait]
impl LivehoodService for LivehoodServiceImpl {
    async fn create_livelihood(
        &self,
        new_livelihood: NewLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse> {
        // Check permissions - explicit permission check
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create livelihoods".to_string(),
            ));
        }
        
        // Validate the new livelihood
        new_livelihood.validate().map_err(ServiceError::Domain)?;
        
        // Create the livelihood
        let livelihood = self.repo
            .create(&new_livelihood, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response and enrich if needed
        let response = LivelihoodResponse::from(livelihood);
        Ok(response)
    }

    async fn create_livelihood_with_documents(
        &self,
        new_livelihood: NewLivelihood,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (file_data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(LivelihoodResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        // 1. Check Permissions - explicit permission checks
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create livelihoods".to_string(),
            ));
        }
        
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_livelihood.validate().map_err(ServiceError::Domain)?;
        
        // 3. Begin transaction
        let mut tx = self.pool.begin().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 4. Create the livelihood first (within transaction)
        let created_livelihood = match self.repo.create_with_tx(&new_livelihood, auth, &mut tx).await {
            Ok(livelihood) => livelihood,
            Err(e) => {
                let _ = tx.rollback().await; // Rollback on error
                return Err(ServiceError::Domain(e));
            }
        };

        // 5. Commit transaction to ensure livelihood is created
        tx.commit().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;

        // 6. Now upload documents (outside transaction, linking to created_livelihood.id)
        let document_results = if !documents.is_empty() {
            self.upload_documents_for_entity(
                created_livelihood.id,
                "livelihoods", // Entity type
                documents,
                document_type_id,
                SyncPriority::Normal, // Default priority
                None, // Use default compression priority
                auth,
            ).await
        } else {
            Vec::new()
        };

        // 7. Convert to Response DTO and return with document results
        let response = LivelihoodResponse::from(created_livelihood);
        Ok((response, document_results))
    }
    
    async fn get_livelihood_by_id(
        &self,
        id: Uuid,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse> {
        // Check permissions - explicit permission check
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }
        
        // Get the livelihood
        let livelihood = self.repo
            .find_by_id(id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response and enrich - now passing auth for document enrichment
        let response = LivelihoodResponse::from(livelihood);
        self.enrich_response(response, include, auth).await
    }
    
    async fn list_livelihoods(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>> {
        // Check permissions - explicit permission check
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }
        
        // Get the livelihoods
        let result = self.repo
            .find_all(params, project_id, participant_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Map items to responses and enrich - now passing auth for document enrichment
        let mut responses = Vec::new();
        for livelihood in result.items {
            let response = LivelihoodResponse::from(livelihood);
            let enriched = self.enrich_response(response, include, auth).await?;
            responses.push(enriched);
        }
        
        // Return paginated result with enriched responses
        Ok(PaginatedResult {
            items: responses,
            total: result.total,
            page: result.page,
            per_page: result.per_page,
            total_pages: result.total_pages,
        })
    }
    
    async fn update_livelihood(
        &self,
        id: Uuid,
        mut update_data: UpdateLivelihood,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodResponse> {
        // Check permissions - explicit permission check
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to update livelihoods".to_string(),
            ));
        }
        
        // Set the updated by user ID
        update_data.updated_by_user_id = auth.user_id;
        
        // Validate the update data
        update_data.validate().map_err(ServiceError::Domain)?;
        
        // Update the livelihood
        let livelihood = self.repo
            .update(id, &update_data, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response with subsequent grants and documents
        let response = LivelihoodResponse::from(livelihood);
        let includes = &[LivelihoodInclude::SubsequentGrants, LivelihoodInclude::Documents];
        let enriched = self.enrich_response(response, Some(includes), auth).await?;
        
        Ok(enriched)
    }
    
    async fn delete_livelihood(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        // Check permissions - explicit permission check
        if hard_delete && !auth.can_hard_delete() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to hard delete records".to_string(),
            ));
        }
        
        if !auth.can_delete_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to delete livelihoods".to_string(),
            ));
        }
        
        // Prepare delete options
        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: true,
            force: false,
        };
        
        // Delete the livelihood
        self.delete(id, auth, options)
            .await
            .map_err(ServiceError::Domain)
    }
    
    async fn add_subsequent_grant(
        &self,
        mut new_grant: NewSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse> {
        // Check permissions - explicit permission check
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to add grants".to_string(),
            ));
        }
        
        // Set the created by user ID if not set
        if new_grant.created_by_user_id.is_none() {
            new_grant.created_by_user_id = Some(auth.user_id);
        }
        
        // Validate the new grant
        new_grant.validate().map_err(ServiceError::Domain)?;
        
        // Check if the livelihood exists
        let livelihood = self.repo
            .find_by_id(new_grant.livelihood_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Create the subsequent grant
        let grant = self.subsequent_grant_repo
            .create(&new_grant, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response
        let mut response = SubsequentGrantResponse::from(grant);
        
        // Get participant name for livelihood summary
        let participant_name = if let Some(participant_id) = livelihood.participant_id {
            match self.participant_repo.find_by_id(participant_id).await {
                Ok(participant) => participant.name,
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };
        
        // Add livelihood summary
        response = response.with_livelihood(crate::domains::livelihood::types::LivelihoodSummary {
            id: livelihood.id,
            participant_id: livelihood.participant_id.unwrap_or(Uuid::nil()),
            participant_name,
            grant_amount: livelihood.grant_amount,
            purpose: livelihood.purpose,
        });
        
        Ok(response)
    }
    
    async fn update_subsequent_grant(
        &self,
        id: Uuid,
        mut update_data: UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse> {
        // Check permissions - explicit permission check
        if !auth.can_edit_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to update grants".to_string(),
            ));
        }
        
        // Set the updated by user ID
        update_data.updated_by_user_id = auth.user_id;
        
        // Validate the update data
        update_data.validate().map_err(ServiceError::Domain)?;
        
        // Update the subsequent grant
        let grant = self.subsequent_grant_repo
            .update(id, &update_data, auth)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Get the livelihood to create summary
        let livelihood = self.repo
            .find_by_id(grant.livelihood_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response
        let mut response = SubsequentGrantResponse::from(grant);
        
        // Get participant name for livelihood summary
        let participant_name = if let Some(participant_id) = livelihood.participant_id {
            match self.participant_repo.find_by_id(participant_id).await {
                Ok(participant) => participant.name,
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };
        
        // Add livelihood summary
        response = response.with_livelihood(crate::domains::livelihood::types::LivelihoodSummary {
            id: livelihood.id,
            participant_id: livelihood.participant_id.unwrap_or(Uuid::nil()),
            participant_name,
            grant_amount: livelihood.grant_amount,
            purpose: livelihood.purpose,
        });
        
        Ok(response)
    }
    
    async fn get_subsequent_grant_by_id(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<SubsequentGrantResponse> {
        // Check permissions - explicit permission check
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view grants".to_string(),
            ));
        }
        
        // Get the subsequent grant
        let grant = self.subsequent_grant_repo
            .find_by_id(id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Get the livelihood to create summary
        let livelihood = self.repo
            .find_by_id(grant.livelihood_id)
            .await
            .map_err(ServiceError::Domain)?;
        
        // Convert to response
        let mut response = SubsequentGrantResponse::from(grant);
        
        // Get participant name for livelihood summary
        let participant_name = if let Some(participant_id) = livelihood.participant_id {
            match self.participant_repo.find_by_id(participant_id).await {
                Ok(participant) => participant.name,
                Err(_) => "Unknown".to_string(),
            }
        } else {
            "Unknown".to_string()
        };
        
        // Add livelihood summary
        response = response.with_livelihood(crate::domains::livelihood::types::LivelihoodSummary {
            id: livelihood.id,
            participant_id: livelihood.participant_id.unwrap_or(Uuid::nil()),
            participant_name,
            grant_amount: livelihood.grant_amount,
            purpose: livelihood.purpose,
        });
        
        Ok(response)
    }
    
    async fn delete_subsequent_grant(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<()> {
        // Check permissions - explicit permission check
        if hard_delete && !auth.can_hard_delete() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to hard delete records".to_string(),
            ));
        }
        
        if !auth.can_delete_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to delete grants".to_string(),
            ));
        }
        
        // Delete the subsequent grant
        if hard_delete {
            self.subsequent_grant_repo
                .hard_delete(id, auth)
                .await
                .map_err(ServiceError::Domain)?;
        } else {
            self.subsequent_grant_repo
                .soft_delete(id, auth)
                .await
                .map_err(ServiceError::Domain)?;
        }
        
        Ok(())
    }

    async fn upload_document_for_livelihood(
        &self,
        livelihood_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        // 1. Verify livelihood exists
        let _livelihood = self.repo.find_by_id(livelihood_id).await
            .map_err(ServiceError::Domain)?;

        // 2. Check permissions - explicit permission check
        if !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 3. Validate the linked field if specified
        if let Some(field) = &linked_field {
            if !Livelihood::is_document_linkable_field(field) {
                let valid_fields: Vec<String> = Livelihood::document_linkable_fields()
                    .into_iter()
                    .collect();
                    
                return Err(ServiceError::Domain(ValidationError::Custom(format!(
                    "Field '{}' does not support document attachments for livelihoods. Valid fields: {}",
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
            livelihood_id,
            "livelihoods".to_string(), // Entity type
            linked_field.clone(), // Pass validated field
            sync_priority,
            compression_priority,
            None, // No temp ID for direct uploads
        ).await?;

        Ok(document)
    }

    async fn bulk_upload_documents_for_livelihood(
        &self,
        livelihood_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        // 1. Verify livelihood exists
        let _livelihood = self.repo.find_by_id(livelihood_id).await
            .map_err(ServiceError::Domain)?;

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
            livelihood_id,
            "livelihoods".to_string(), // Entity type
            sync_priority,
            compression_priority,
            None, // No temp ID for direct uploads
        ).await?;

        Ok(documents)
    }

    async fn get_livelihood_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<LivelioodStatsSummary> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihood statistics".to_string(),
            ));
        }

        // 2. Get stats from repository
        let stats = self.repo
            .get_livelihood_stats()
            .await
            .map_err(ServiceError::Domain)?;

        Ok(stats)
    }

    async fn get_outcome_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view outcome distribution".to_string(),
            ));
        }

        // 2. Get distribution from repository
        self.repo
            .get_outcome_status_distribution()
            .await
            .map_err(ServiceError::Domain)
    }

    async fn find_livelihoods_with_outcome(
        &self,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }

        // 2. Get livelihoods with outcome from repository
        let paginated_result = self.repo
            .find_with_outcome(params)
            .await
            .map_err(ServiceError::Domain)?;

        // 3. Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for livelihood in paginated_result.items {
            let response = LivelihoodResponse::from(livelihood);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_livelihoods_without_outcome(
        &self,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }

        // 2. Get livelihoods without outcome from repository
        let paginated_result = self.repo
            .find_without_outcome(params)
            .await
            .map_err(ServiceError::Domain)?;

        // 3. Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for livelihood in paginated_result.items {
            let response = LivelihoodResponse::from(livelihood);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_livelihoods_with_multiple_grants(
        &self,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }

        // 2. Get livelihoods with multiple grants from repository
        let paginated_result = self.repo
            .find_with_multiple_grants(params)
            .await
            .map_err(ServiceError::Domain)?;

        // 3. Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for livelihood in paginated_result.items {
            let response = LivelihoodResponse::from(livelihood);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn get_livelihood_with_participant_details(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<LivelioodWithParticipantDetails> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }

        // 2. Get the livelihood
        let livelihood = self.repo
            .find_by_id(id)
            .await
            .map_err(ServiceError::Domain)?;

        let participant_id = livelihood.participant_id.ok_or_else(|| 
            ServiceError::Domain(DomainError::EntityNotFound(
                "Participant".to_string(), // Entity type
                Uuid::nil() // Placeholder ID, actual participant ID unknown here
            ))
        )?;

        // 3. Get the participant details
        let participant = self.participant_repo
            .find_by_id(participant_id)
            .await
            .map_err(ServiceError::Domain)?;

        // 4. Get subsequent grants
        let subsequent_grants = self.subsequent_grant_repo
            .find_by_livelihood_id(id)
            .await
            .map_err(ServiceError::Domain)?;

        let grant_summaries = subsequent_grants
            .iter()
            .map(|g| SubsequentGrantSummary::from(g.clone()))
            .collect::<Vec<_>>();

        // 5. Calculate total grant amount
        let initial_amount = livelihood.grant_amount.unwrap_or(0.0);
        let subsequent_amount: f64 = subsequent_grants
            .iter()
            .filter_map(|g| g.amount)
            .sum();
        let total_grant_amount = initial_amount + subsequent_amount;

        // 6. Get document count
        let doc_counts = self.repo
            .get_document_counts(&[id])
            .await
            .map_err(ServiceError::Domain)?;

        let doc_count = doc_counts.get(&id).copied().unwrap_or(0);

        // 7. Create and enrich livelihood response
        let livelihood_response = LivelihoodResponse::from(livelihood);
        let enriched_response = self.enrich_response(
            livelihood_response,
            Some(&[LivelihoodInclude::Project]), // Only include project summary here
            auth
        ).await?;

        // 8. Create participant details (Set missing fields to None)
        let participant_details = ParticipantDetails {
            id: participant.id,
            name: participant.name, 
            gender: participant.gender, 
            age_group: participant.age_group, 
            disability: participant.disability,
            // --- Fields not present in Participant struct set to None ---
            address: None, 
            phone: None, 
            occupation: None, 
            family_size: None, 
            created_at: participant.created_at.to_rfc3339(),
        };

        // 9. Return the combined details
        Ok(LivelioodWithParticipantDetails {
            livelihood: enriched_response,
            participant_details,
            subsequent_grants: grant_summaries,
            total_grant_amount,
            documents_count: doc_count,
        })
    }

    async fn get_livelihood_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<LivelioodWithDocumentTimeline> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }
        
        if !auth.has_permission(Permission::ViewDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view documents".to_string(),
            ));
        }

        // 2. Get the livelihood
        let livelihood = self.repo
            .find_by_id(id)
            .await
            .map_err(ServiceError::Domain)?;

        // 3. Create and enrich livelihood response (Include necessary relations)
        let livelihood_response = LivelihoodResponse::from(livelihood);
        let enriched_response = self.enrich_response(
            livelihood_response,
            Some(&[LivelihoodInclude::Project, LivelihoodInclude::Participant, LivelihoodInclude::SubsequentGrants]), // Include base relations
            auth
        ).await?;

        // 4. Get all documents for this livelihood using correct PaginationParams
        let documents = self.document_service
            .list_media_documents_by_related_entity(
                auth,
                "livelihoods",
                id,
                PaginationParams { page: 1, per_page: 100 }, // Use struct literal
                None,
            )
            .await?
            .items;

        // 5. Organize documents by month
        let mut documents_by_month: HashMap<String, Vec<MediaDocumentResponse>> = HashMap::new();
        let mut total_document_count = 0;
        
        for doc in documents {
            if let Ok(created_at) = chrono::DateTime::parse_from_rfc3339(&doc.created_at) {
                // Format as YYYY-MM for grouping
                let month_key = format!("{}-{:02}", created_at.year(), created_at.month());
                
                documents_by_month
                    .entry(month_key)
                    .or_insert_with(Vec::new)
                    .push(doc);
                total_document_count += 1; // Increment count here
            }
        }

        // 6. Return the timeline
        Ok(LivelioodWithDocumentTimeline {
            livelihood: enriched_response,
            documents_by_month,
            total_document_count: total_document_count as u64, // Use the calculated count
        })
    }

    async fn get_participant_outcome_metrics(
        &self,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantOutcomeMetrics> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }

        // 2. Get participant details
        let participant = self.participant_repo
            .find_by_id(participant_id)
            .await
            .map_err(ServiceError::Domain)?;

        // 3. Get all livelihoods for this participant (using correct PaginationParams)
        let livelihoods = self.repo
            .find_by_participant_id(participant_id, PaginationParams { page: 1, per_page: 100 }) // Fetch up to 100
            .await
            .map_err(ServiceError::Domain)?
            .items;

        if livelihoods.is_empty() {
            return Err(ServiceError::Domain(DomainError::EntityNotFound(
                "Livelihood".to_string(), // Entity type
                participant_id // ID relates to participant's livelihoods
            )));
        }

        // 4. Extract livelihood IDs
        let livelihood_ids: Vec<Uuid> = livelihoods.iter().map(|l| l.id).collect();

        // 5. Get all subsequent grants for these livelihoods
        let mut all_subsequent_grants = Vec::new();
        for id in &livelihood_ids {
            let grants = self.subsequent_grant_repo
                .find_by_livelihood_id(*id)
                .await
                .map_err(ServiceError::Domain)?;
            all_subsequent_grants.extend(grants);
        }

        // 6. Calculate total grants received
        let total_grants_received = livelihoods.len() as i64 + all_subsequent_grants.len() as i64;

        // 7. Calculate total grant amount
        let initial_amount: f64 = livelihoods.iter().filter_map(|l| l.grant_amount).sum();
        let subsequent_amount: f64 = all_subsequent_grants.iter().filter_map(|g| g.amount).sum();
        let total_grant_amount = initial_amount + subsequent_amount;

        // 8. Find grant dates (consider both livelihood creation and subsequent grant dates)
        let mut all_dates: Vec<DateTime<Utc>> = Vec::new();
        
        // Add creation dates for livelihoods as proxy for initial grant dates
        for livelihood in &livelihoods {
            all_dates.push(livelihood.created_at);
        }
        
        // Add grant dates for subsequent grants (parse carefully)
        for grant in &all_subsequent_grants {
            if let Some(date_str) = &grant.grant_date {
                if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                    // Assume grant time is start of day UTC for consistency
                    if let Some(datetime) = date.and_hms_opt(0, 0, 0) {
                         all_dates.push(chrono::DateTime::<Utc>::from_utc(datetime, Utc));
                    }
                } else {
                    // Fall back to creation date if grant_date format is invalid
                    all_dates.push(grant.created_at);
                }
            } else {
                // Fall back to creation date if grant_date is missing
                all_dates.push(grant.created_at);
            }
        }
        
        // Sort dates to find first and last
        all_dates.sort();
        let first_grant_date = all_dates.first().map(|d| d.to_rfc3339());
        let last_grant_date = all_dates.last().map(|d| d.to_rfc3339());

        // 9. Determine outcome status (check if *any* livelihood has a non-empty outcome)
        let has_outcome = livelihoods.iter().any(|l| 
            l.outcome.as_ref().map_or(false, |o| !o.is_empty())
        );
        
        // Get the most recent outcome description (order by update time might be better)
        let outcome_description = livelihoods.iter()
            .max_by_key(|l| l.updated_at)
            .and_then(|l| l.outcome.clone())
            .filter(|o| !o.is_empty());

        // 10. Return metrics
        Ok(ParticipantOutcomeMetrics {
            participant_id,
            participant_name: participant.name, 
            gender: participant.gender, 
            total_grants_received,
            total_grant_amount,
            first_grant_date,
            last_grant_date,
            has_positive_outcome: has_outcome,
            outcome_description,
        })
    }
    
    async fn get_livelihood_dashboard_metrics(
        &self,
        months_back: i32,
        auth: &AuthContext,
    ) -> ServiceResult<LivelihoodDashboardMetrics> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihood metrics".to_string(),
            ));
        }

        // 2. Get overall statistics
        let stats = self.repo
            .get_livelihood_stats()
            .await
            .map_err(ServiceError::Domain)?;
            
        // Get unique count of participants with livelihoods
        let unique_participants_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT participant_id) 
             FROM livelihoods 
             WHERE participant_id IS NOT NULL 
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ServiceError::Domain(DbError::from(e).into()))?;

        // 3. Get monthly grant data (subsequent only)
        let monthly_subsequent_grants = self.subsequent_grant_repo
            .get_monthly_grant_stats(months_back)
            .await
            .map_err(ServiceError::Domain)?;
            
        // Get monthly initial grant data
        let monthly_query_str = format!(
            r#"
            WITH RECURSIVE months(date) AS (
                SELECT DATE(DATETIME('now', 'start of month', '{}' months)) 
                UNION ALL
                SELECT DATE(DATETIME(date, '+1 month'))
                FROM months
                WHERE date < DATE('now', 'start of month')
            )
            SELECT 
                strftime('%Y-%m', months.date) as month,
                COUNT(l.id) as grant_count,
                COALESCE(SUM(l.grant_amount), 0) as total_amount
            FROM months
            LEFT JOIN livelihoods l ON strftime('%Y-%m', l.created_at) = strftime('%Y-%m', months.date)
            WHERE l.deleted_at IS NULL OR l.deleted_at IS NULL
            GROUP BY month
            ORDER BY month
            "#,
            -months_back
        );

        let initial_grant_rows = sqlx::query(&monthly_query_str)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ServiceError::Domain(DbError::from(e).into()))?;

        // 4. Combine monthly data
        let mut grant_count_by_month = HashMap::new();
        let mut grant_amount_by_month = HashMap::new();
        
        // Process initial grants
        for row in initial_grant_rows {
            let month: String = row.get("month");
            let count: i64 = row.get("grant_count");
            let amount: f64 = row.get("total_amount");
            
            grant_count_by_month.insert(month.clone(), count);
            grant_amount_by_month.insert(month, amount);
        }
        
        // Add subsequent grants
        for (month, count, amount) in monthly_subsequent_grants {
            *grant_count_by_month.entry(month.clone()).or_insert(0) += count;
            *grant_amount_by_month.entry(month).or_insert(0.0) += amount;
        }

        // 5. Get outcome status distribution
        let outcome_status_distribution = self.repo
            .get_outcome_status_distribution()
            .await
            .map_err(ServiceError::Domain)?;

        // 6. Get gender distribution (query directly for counts)
        let gender_distribution: HashMap<String, i64> = sqlx::query(
            r#"
            SELECT 
                COALESCE(p.gender, 'Unspecified') as gender_group,
                COUNT(DISTINCT l.participant_id) as count
            FROM livelihoods l
            JOIN participants p ON l.participant_id = p.id
            WHERE l.deleted_at IS NULL AND p.deleted_at IS NULL
            GROUP BY gender_group
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter().map(|row| {
                let gender: String = row.get("gender_group");
                let count: i64 = row.get("count");
                (gender, count)
            }).collect()
        })
        .map_err(|e| ServiceError::Domain(DbError::from(e).into()))?;

        // 7. Get age group distribution (query directly for counts)
        let age_group_distribution: HashMap<String, i64> = sqlx::query(
            r#"
            SELECT 
                COALESCE(p.age_group, 'Unspecified') as age_group,
                COUNT(DISTINCT l.participant_id) as count
            FROM livelihoods l
            JOIN participants p ON l.participant_id = p.id
            WHERE l.deleted_at IS NULL AND p.deleted_at IS NULL
            GROUP BY age_group
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter().map(|row| {
                let age_group: String = row.get("age_group");
                let count: i64 = row.get("count");
                (age_group, count)
            }).collect()
        })
        .map_err(|e| ServiceError::Domain(DbError::from(e).into()))?;

        // 8. Return dashboard metrics
        Ok(LivelihoodDashboardMetrics {
            total_participants_supported: unique_participants_count,
            total_grant_amount: stats.total_grant_amount + stats.total_subsequent_grant_amount,
            grant_count_by_month,
            grant_amount_by_month,
            outcome_status_distribution,
            gender_distribution,
            age_group_distribution,
        })
    }
    
    async fn find_livelihoods_by_date_range(
        &self,
        start_date: &str,
        end_date: &str,
        params: PaginationParams,
        include: Option<&[LivelihoodInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<LivelihoodResponse>> {
        // 1. Check permissions
        if !auth.can_view_livelihoods() {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view livelihoods".to_string(),
            ));
        }

        // Validate date format - YYYY-MM-DD
        if chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d").is_err() {
            return Err(ServiceError::Domain(DomainError::Validation(
                ValidationError::format(
                    "start_date", 
                    "Invalid date format. Expected YYYY-MM-DD"
                )
            )));
        }
        
        if chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d").is_err() {
            return Err(ServiceError::Domain(DomainError::Validation(
                 ValidationError::format(
                    "end_date", 
                    "Invalid date format. Expected YYYY-MM-DD"
                )
            )));
        }

        // 2. Get livelihoods in date range
        let paginated_result = self.repo
            .find_by_date_range(start_date, end_date, params)
            .await
            .map_err(ServiceError::Domain)?;

        // 3. Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for livelihood in paginated_result.items {
            let response = LivelihoodResponse::from(livelihood);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 4. Return paginated result
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
}

// Add trait for permission checking to AuthContext
trait LivehoodPermissions {
    fn can_view_livelihoods(&self) -> bool;
    fn can_edit_livelihoods(&self) -> bool;
    fn can_delete_livelihoods(&self) -> bool;
    fn can_hard_delete(&self) -> bool;
}

impl LivehoodPermissions for AuthContext {
    fn can_view_livelihoods(&self) -> bool {
        self.has_permission(Permission::ViewLivelihoods)
    }
    
    fn can_edit_livelihoods(&self) -> bool {
        self.has_permission(Permission::EditLivelihoods) ||
        self.has_permission(Permission::CreateLivelihoods)
    }
    
    fn can_delete_livelihoods(&self) -> bool {
        self.has_permission(Permission::DeleteLivelihoods)
    }
    
    fn can_hard_delete(&self) -> bool {
        self.has_permission(Permission::HardDeleteRecord)
    }
}