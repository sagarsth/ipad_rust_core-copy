use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::permission::Permission;
use crate::domains::funding::repository::ProjectFundingRepository;
use crate::domains::funding::types::{
    ProjectFunding, NewProjectFunding, ProjectFundingResponse, UpdateProjectFunding, 
    FundingInclude, ProjectSummary, DonorSummary
};
use crate::domains::project::repository::ProjectRepository;
use crate::domains::donor::repository::DonorRepository;
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;
use crate::domains::core::document_linking::DocumentLinkable;

/// Trait defining project funding service operations
#[async_trait]
pub trait ProjectFundingService: DeleteService<ProjectFunding> + Send + Sync {
    async fn create_funding(
        &self,
        new_funding: NewProjectFunding,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectFundingResponse>;

    async fn get_funding_by_id(
        &self,
        id: Uuid,
        include: Option<&[FundingInclude]>, // Used for enrichment
        auth: &AuthContext,
    ) -> ServiceResult<ProjectFundingResponse>;

    async fn list_fundings(
        &self,
        params: PaginationParams,
        include: Option<&[FundingInclude]>, // Used for enrichment
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectFundingResponse>>;
    
    async fn list_fundings_by_project(
        &self,
        project_id: Uuid,
        params: PaginationParams,
        include: Option<&[FundingInclude]>, // Used for enrichment
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectFundingResponse>>;
    
    async fn list_fundings_by_donor(
        &self,
        donor_id: Uuid,
        params: PaginationParams,
        include: Option<&[FundingInclude]>, // Used for enrichment
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectFundingResponse>>;

    async fn update_funding(
        &self,
        id: Uuid,
        update_data: UpdateProjectFunding,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectFundingResponse>;

    async fn delete_funding(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
    
    // Get funding statistics for project
    async fn get_project_funding_stats(
        &self,
        project_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(i64, f64)>;
    
    // Get funding statistics for donor
    async fn get_donor_funding_stats(
        &self,
        donor_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(i64, f64)>;

    // Document integration methods
    async fn upload_document_for_funding(
        &self,
        funding_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_funding(
        &self,
        funding_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;
}

/// Implementation of the project funding service
#[derive(Clone)]
pub struct ProjectFundingServiceImpl {
    pool: SqlitePool,
    repo: Arc<dyn ProjectFundingRepository + Send + Sync>,
    project_repo: Arc<dyn ProjectRepository + Send + Sync>,
    donor_repo: Arc<dyn DonorRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<ProjectFunding>>,
    document_service: Arc<dyn DocumentService>,
}

impl ProjectFundingServiceImpl {
    pub fn new(
        pool: SqlitePool,
        funding_repo: Arc<dyn ProjectFundingRepository + Send + Sync>,
        project_repo: Arc<dyn ProjectRepository + Send + Sync>,
        donor_repo: Arc<dyn DonorRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        document_service: Arc<dyn DocumentService>,
    ) -> Self {
        // --- Adapter setup for BaseDeleteService ---
        struct RepoAdapter(Arc<dyn ProjectFundingRepository + Send + Sync>);

        #[async_trait]
        impl FindById<ProjectFunding> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<ProjectFunding> {
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
                 "project_funding"
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

        let adapted_repo: Arc<dyn DeleteServiceRepository<ProjectFunding>> =
            Arc::new(RepoAdapter(funding_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            None, // No media repo needed for funding
        ));

        Self {
            pool,
            repo: funding_repo,
            project_repo,
            donor_repo,
            delete_service,
            document_service,
        }
    }

    /// Helper to enrich ProjectFundingResponse with included data
    async fn enrich_response(
        &self,
        mut response: ProjectFundingResponse,
        include: Option<&[FundingInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectFundingResponse> {
        if let Some(includes) = include {
            // Check if we need to include project details
            let include_project = includes.contains(&FundingInclude::All) || 
                                includes.contains(&FundingInclude::Project);
                                
            // Check if we need to include donor details
            let include_donor = includes.contains(&FundingInclude::All) || 
                              includes.contains(&FundingInclude::Donor);
            
            // Include project if requested
            if include_project && response.project.is_none() {
                match self.project_repo.find_by_id(response.project_id).await {
                    Ok(project) => {
                        response.project = Some(ProjectSummary {
                            id: project.id,
                            name: project.name,
                        });
                    },
                    Err(_) => {
                        // Project not found, but we shouldn't fail the overall response
                        // Just leave project as None
                    }
                }
            }
            
            // Include donor if requested
            if include_donor && response.donor.is_none() {
                match self.donor_repo.find_by_id(response.donor_id).await {
                    Ok(donor) => {
                        response.donor = Some(DonorSummary {
                            id: donor.id,
                            name: donor.name,
                            type_: donor.type_,
                        });
                    },
                    Err(_) => {
                        // Donor not found, but we shouldn't fail the overall response
                        // Just leave donor as None
                    }
                }
            }
        }
        
        Ok(response)
    }
    
    /// Helper to validate existence of related entities
    async fn validate_relations(
        &self,
        project_id: Uuid,
        donor_id: Uuid,
    ) -> DomainResult<()> {
        // Validate project existence
        self.project_repo.find_by_id(project_id).await?;
        
        // Validate donor existence
        self.donor_repo.find_by_id(donor_id).await?;
        
        Ok(())
    }
}

// Implement DeleteService<ProjectFunding> by delegating to delete_service
#[async_trait]
impl DeleteService<ProjectFunding> for ProjectFundingServiceImpl {
    fn repository(&self) -> &dyn FindById<ProjectFunding> { self.delete_service.repository() }
    fn tombstone_repository(&self) -> &dyn TombstoneRepository { self.delete_service.tombstone_repository() }
    fn change_log_repository(&self) -> &dyn ChangeLogRepository { self.delete_service.change_log_repository() }
    fn dependency_checker(&self) -> &dyn DependencyChecker { self.delete_service.dependency_checker() }
    async fn delete( &self, id: Uuid, auth: &AuthContext, options: DeleteOptions ) -> DomainResult<DeleteResult> { self.delete_service.delete(id, auth, options).await }
    async fn batch_delete( &self, ids: &[Uuid], auth: &AuthContext, options: DeleteOptions ) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> { self.delete_service.batch_delete(ids, auth, options).await }
    async fn delete_with_dependencies( &self, id: Uuid, auth: &AuthContext ) -> DomainResult<DeleteResult> { self.delete_service.delete_with_dependencies(id, auth).await }
    async fn get_failed_delete_details( &self, batch_result: &crate::domains::core::delete_service::BatchDeleteResult, auth: &AuthContext ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<ProjectFunding>>> { self.delete_service.get_failed_delete_details(batch_result, auth).await }
}

#[async_trait]
impl ProjectFundingService for ProjectFundingServiceImpl {
    async fn create_funding(
        &self,
        new_funding: NewProjectFunding,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectFundingResponse> {
        // Check admin permission for funding management
        auth.authorize(Permission::CreateFunding)?;

        // Validate the DTO
        new_funding.validate()?;
        
        // Validate that project and donor exist
        self.validate_relations(new_funding.project_id, new_funding.donor_id).await
            .map_err(ServiceError::Domain)?;

        // Create funding record
        let created_funding = self.repo.create(&new_funding, auth).await
            .map_err(ServiceError::Domain)?;
            
        // Convert to response DTO
        let response = ProjectFundingResponse::from(created_funding);
        
        Ok(response)
    }

    async fn get_funding_by_id(
        &self,
        id: Uuid,
        include: Option<&[FundingInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectFundingResponse> {
        // Check admin permission for viewing funding
        auth.authorize(Permission::ViewFunding)?;

        // Get the funding record
        let funding = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
            
        // Convert to response DTO
        let response = ProjectFundingResponse::from(funding);
        
        // Enrich with included data
        self.enrich_response(response, include, auth).await
    }

    async fn list_fundings(
        &self,
        params: PaginationParams,
        include: Option<&[FundingInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectFundingResponse>> {
        // Check admin permission for viewing funding
        auth.authorize(Permission::ViewFunding)?;

        // Get paginated funding records
        let paginated_result = self.repo.find_all(params).await
            .map_err(ServiceError::Domain)?;
            
        // Convert and enrich items
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = ProjectFundingResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }
        
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
    
    async fn list_fundings_by_project(
        &self,
        project_id: Uuid,
        params: PaginationParams,
        include: Option<&[FundingInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectFundingResponse>> {
        // Check admin permission for viewing funding
        auth.authorize(Permission::ViewFunding)?;

        // Validate project exists
        self.project_repo.find_by_id(project_id).await?;

        // Get paginated funding records for this project
        let paginated_result = self.repo.find_by_project(project_id, params).await
            .map_err(ServiceError::Domain)?;
            
        // Convert and enrich items
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = ProjectFundingResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }
        
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
    
    async fn list_fundings_by_donor(
        &self,
        donor_id: Uuid,
        params: PaginationParams,
        include: Option<&[FundingInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ProjectFundingResponse>> {
        // Check admin permission for viewing funding
        auth.authorize(Permission::ViewFunding)?;

        // Validate donor exists
        self.donor_repo.find_by_id(donor_id).await?;

        // Get paginated funding records for this donor
        let paginated_result = self.repo.find_by_donor(donor_id, params).await
            .map_err(ServiceError::Domain)?;
            
        // Convert and enrich items
        let mut enriched_items = Vec::new();
        for item in paginated_result.items {
            let response = ProjectFundingResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }
        
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn update_funding(
        &self,
        id: Uuid,
        mut update_data: UpdateProjectFunding,
        auth: &AuthContext,
    ) -> ServiceResult<ProjectFundingResponse> {
        // Check admin permission for funding management
        auth.authorize(Permission::EditFunding)?;

        // Set the updated_by user ID
        update_data.updated_by_user_id = auth.user_id;
        
        // Validate the DTO
        update_data.validate()?;
        
        // Validate relations if updating project_id or donor_id
        if update_data.project_id.is_some() || update_data.donor_id.is_some() {
            // Get the current funding to get current project/donor IDs
            let current = self.repo.find_by_id(id).await
                .map_err(ServiceError::Domain)?;
                
            // Determine which project_id and donor_id to validate
            let project_id = update_data.project_id.unwrap_or(current.project_id);
            let donor_id = update_data.donor_id.unwrap_or(current.donor_id);
            
            // Validate these relations
            self.validate_relations(project_id, donor_id).await
                .map_err(ServiceError::Domain)?;
        }

        // Update the funding
        let updated_funding = self.repo.update(id, &update_data, auth).await
            .map_err(ServiceError::Domain)?;
            
        // Convert to response DTO
        let response = ProjectFundingResponse::from(updated_funding);
        
        Ok(response)
    }

    async fn delete_funding(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        // Check permissions - different permissions for soft vs hard delete
        let required_permission = if hard_delete {
            Permission::HardDeleteRecord
        } else {
            Permission::DeleteFunding
        };

        auth.authorize(required_permission)?;

        // Set up delete options
        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            fallback_to_soft_delete: !hard_delete,
            force: false,
        };

        // Delegate to delete service
        let result = self.delete(id, auth, options).await
            .map_err(ServiceError::Domain)?;
            
        Ok(result)
    }
    
    async fn get_project_funding_stats(
        &self,
        project_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(i64, f64)> {
        // Check permission for viewing funding
        auth.authorize(Permission::ViewFunding)?;
        
        // Validate project exists
        self.project_repo.find_by_id(project_id).await?;
        
        // Get funding stats from repository
        let stats = self.repo.get_project_funding_stats(project_id).await
            .map_err(ServiceError::Domain)?;
            
        Ok(stats)
    }
    
    async fn get_donor_funding_stats(
        &self,
        donor_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(i64, f64)> {
        // Check permission for viewing funding
        auth.authorize(Permission::ViewFunding)?;
        
        // Validate donor exists
        self.donor_repo.find_by_id(donor_id).await?;
        
        // Get funding stats from repository
        let stats = self.repo.get_donor_funding_stats(donor_id).await
            .map_err(ServiceError::Domain)?;
            
        Ok(stats)
    }

    // --- Document integration methods ---

    async fn upload_document_for_funding(
        &self,
        funding_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        auth.authorize(Permission::ViewFunding)?;
        auth.authorize(Permission::UploadDocuments)?;

        if let Some(field_name) = &linked_field {
             let is_valid_linkable_field = ProjectFunding::field_metadata().iter().any(|meta| {
                meta.field_name == field_name && meta.supports_documents
            });
            if !is_valid_linkable_field {
                 let validation_error = ValidationError::Custom(
                    format!("Invalid or non-linkable field for ProjectFunding: {}", field_name)
                 );
                 return Err(DomainError::Validation(validation_error).into()); 
            }
        }
        
        let result = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            funding_id,
            "project_funding".to_string(),
            linked_field,
            sync_priority,
            compression_priority,
            None
        ).await?;
        
        Ok(result)
    }

    async fn bulk_upload_documents_for_funding(
        &self,
        funding_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        auth.authorize(Permission::ViewFunding)?;
        auth.authorize(Permission::UploadDocuments)?;

        self.repo.find_by_id(funding_id).await?;

        let mut results = Vec::with_capacity(files.len());
        for (file_data, original_filename) in files {
            let result = self.document_service.upload_document(
                auth,
                file_data,
                original_filename.clone(), 
                title.clone(), 
                document_type_id,
                funding_id,
                "project_funding".to_string(),
                None,
                sync_priority,
                compression_priority,
                None
            ).await;

            match result {
                Ok(doc_response) => results.push(doc_response),
                Err(e) => {
                    eprintln!("Error uploading file {} for funding {}: {:?}", original_filename, funding_id, e);
                }
            }
        }
        
        Ok(results)
    }
}