use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use crate::domains::permission::Permission;
use crate::domains::donor::repository::DonorRepository;
use crate::domains::donor::types::{
    Donor, NewDonor, DonorResponse, UpdateDonor, DonorInclude, DonorSummary, 
    DonorDashboardStats, FundingSummaryStats, DonorWithFundingDetails, 
    DonorWithDocumentTimeline, DonorFundingStats, UserDonorRole
};
use crate::domains::funding::repository::ProjectFundingRepository;
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::errors::{DomainError, DomainResult, ServiceError, ServiceResult, ValidationError, DbError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use std::collections::HashMap;
use chrono::{self, Utc, DateTime, Duration, Datelike};

// Import necessary document types and services
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;
use crate::domains::core::delete_service::PendingDeletionManager;
/// Trait defining donor service operations
#[async_trait]
pub trait DonorService: DeleteService<Donor> + Send + Sync {
    async fn create_donor(
        &self,
        new_donor: NewDonor,
        auth: &AuthContext,
    ) -> ServiceResult<DonorResponse>;

    async fn create_donor_with_documents(
        &self,
        new_donor: NewDonor,
        documents: Vec<(Vec<u8>, String, Option<String>)>, // (file_data, filename, linked_field)
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(DonorResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;

    async fn get_donor_by_id(
        &self,
        id: Uuid,
        include: Option<&[DonorInclude]>, // Used for enrichment
        auth: &AuthContext,
    ) -> ServiceResult<DonorResponse>;

    async fn list_donors(
        &self,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>>;

    async fn update_donor(
        &self,
        id: Uuid,
        update_data: UpdateDonor,
        auth: &AuthContext,
    ) -> ServiceResult<DonorResponse>;

    async fn delete_donor(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
    
    // Get summary information for dropdowns, etc.
    async fn get_donor_summary(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorSummary>;
    
    // Document integration methods
    async fn upload_document_for_donor(
        &self,
        donor_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_donor(
        &self,
        donor_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;

    /// Get comprehensive donor statistics for dashboard
    async fn get_donor_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<DonorDashboardStats>;

    /// Get distribution of donors by type
    async fn get_type_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Get distribution of donors by country
    async fn get_country_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Find donors by type
    async fn find_donors_by_type(
        &self,
        donor_type: &str,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>>;

    /// Find donors by country
    async fn find_donors_by_country(
        &self,
        country: &str,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>>;

    /// Find donors with recent donations in the past number of days
    async fn find_donors_with_recent_donations(
        &self,
        days_ago: u32,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>>;

    /// Get donor with detailed funding information
    async fn get_donor_with_funding_details(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorWithFundingDetails>;

    /// Get donor with document timeline
    async fn get_donor_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorWithDocumentTimeline>;
}

/// Implementation of the donor service
#[derive(Clone)]
pub struct DonorServiceImpl {
    pool: SqlitePool,
    repo: Arc<dyn DonorRepository + Send + Sync>,
    funding_repo: Arc<dyn ProjectFundingRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Donor>>,
    document_service: Arc<dyn DocumentService>,
    
}

impl DonorServiceImpl {
    pub fn new(
        pool: SqlitePool,
        donor_repo: Arc<dyn DonorRepository + Send + Sync>,
        funding_repo: Arc<dyn ProjectFundingRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        media_doc_repo: Arc<dyn MediaDocumentRepository>,
        document_service: Arc<dyn DocumentService>,
        deletion_manager: Arc<PendingDeletionManager>,
    ) -> Self {
        // --- Adapter setup for BaseDeleteService ---
        struct RepoAdapter(Arc<dyn DonorRepository + Send + Sync>);

        #[async_trait]
        impl FindById<Donor> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<Donor> {
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
                 "donors"
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

        let adapted_repo: Arc<dyn DeleteServiceRepository<Donor>> =
            Arc::new(RepoAdapter(donor_repo.clone()));

        let delete_service = Arc::new(BaseDeleteService::new(
            pool.clone(),
            adapted_repo,
            tombstone_repo,
            change_log_repo,
            dependency_checker,
            Some(media_doc_repo), // Pass media repo for file handling
            deletion_manager,
        ));

        Self {
            pool,
            repo: donor_repo,
            funding_repo,
            delete_service,
            document_service,
        }
    }

    /// Helper to enrich DonorResponse with included data
    async fn enrich_response(
        &self,
        mut response: DonorResponse,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<DonorResponse> {
        if let Some(includes) = include {
            // Check if we need to include funding stats
            let include_funding_stats = includes.contains(&DonorInclude::All) || 
                                      includes.contains(&DonorInclude::FundingStats);
                                
            // Include funding stats if requested
            if include_funding_stats && response.active_fundings_count.is_none() {
                match self.funding_repo.get_donor_funding_stats(response.id).await {
                    Ok((active_count, total_amount)) => {
                        response = response.with_funding_stats(active_count, total_amount);
                    },
                    Err(_) => {
                        // Stats calculation failed, but we shouldn't fail the overall response
                        // Just leave stats as None
                    }
                }
            }
            
            // Add more includes in the future as needed...
        }
        
        Ok(response)
    }
    
    /// Helper method to upload documents for entity and handle errors individually
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
            // Use the injected document_service
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

// Implement DeleteService<Donor> by delegating to delete_service
#[async_trait]
impl DeleteService<Donor> for DonorServiceImpl {
    fn repository(&self) -> &dyn FindById<Donor> { self.delete_service.repository() }
    fn tombstone_repository(&self) -> &dyn TombstoneRepository { self.delete_service.tombstone_repository() }
    fn change_log_repository(&self) -> &dyn ChangeLogRepository { self.delete_service.change_log_repository() }
    fn dependency_checker(&self) -> &dyn DependencyChecker { self.delete_service.dependency_checker() }

    // Note: Permission checks for delete operations are handled within BaseDeleteService
    // based on the DeleteOptions provided (e.g., hard_delete requires specific permission).
    async fn delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
        options: DeleteOptions
    ) -> DomainResult<DeleteResult> {
        // Permission check happens inside BaseDeleteService based on options
        self.delete_service.delete(id, auth, options).await
    }

    async fn batch_delete(
        &self,
        ids: &[Uuid],
        auth: &AuthContext,
        options: DeleteOptions
    ) -> DomainResult<crate::domains::core::delete_service::BatchDeleteResult> {
         // Permission check happens inside BaseDeleteService based on options
        self.delete_service.batch_delete(ids, auth, options).await
    }

    async fn delete_with_dependencies(
        &self,
        id: Uuid,
        auth: &AuthContext
    ) -> DomainResult<DeleteResult> {
        // Requires HardDeleteRecordWithDependencies, checked by BaseDeleteService
        self.delete_service.delete_with_dependencies(id, auth).await
    }

    async fn get_failed_delete_details(
        &self,
        batch_result: &crate::domains::core::delete_service::BatchDeleteResult,
        auth: &AuthContext
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Donor>>> {
        // Assuming View permission is sufficient to see delete details
        auth.authorize(Permission::ViewDonors).map_err(|e| match e {
            ServiceError::PermissionDenied(msg) => DomainError::AuthorizationFailed(msg),
            // Handle other potential ServiceErrors if necessary, or map to a generic DomainError
            _ => DomainError::Internal("Authorization check failed".to_string()), 
        })?;
        self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

#[async_trait]
impl DonorService for DonorServiceImpl {
    async fn create_donor(
        &self,
        new_donor: NewDonor,
        auth: &AuthContext,
    ) -> ServiceResult<DonorResponse> {
        auth.authorize(Permission::CreateDonors)?;
        
        // Validate the input data
        new_donor.validate()?;
        
        // Call the repository to create the donor
        let donor = self.repo.create(&new_donor, auth).await?;
        
        // Enrich and return the response
        self.enrich_response(donor.into(), None, auth).await
    }

    async fn create_donor_with_documents(
        &self,
        new_donor: NewDonor,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(DonorResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        auth.authorize(Permission::CreateDonors)?;
        auth.authorize(Permission::UploadDocuments)?;
        
        // Validate the input data
        new_donor.validate()?;

        // Start a transaction
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        let donor = match self.repo.create_with_tx(&new_donor, auth, &mut tx).await {
            Ok(d) => d,
            Err(e) => {
                tx.rollback().await.ok(); // Ignore rollback error
                return Err(e.into());
            }
        };

        // Commit transaction before potentially long-running document uploads
        tx.commit().await.map_err(|e| DomainError::Database(DbError::from(e)))?;

        // Now upload documents (outside the transaction)
        // Use reasonable defaults for sync/compression priority if needed
        let sync_priority = SyncPriority::Normal; 
        let compression_priority = Some(CompressionPriority::Normal); 

        let upload_results = self.upload_documents_for_entity(
            donor.id,
            "donors", // Entity type for linking
            documents,
            document_type_id,
            sync_priority,
            compression_priority,
            auth
        ).await;

        // Enrich and return the donor response along with document results
        let enriched_donor = self.enrich_response(donor.into(), None, auth).await?;
        Ok((enriched_donor, upload_results))
    }
    
    async fn get_donor_by_id(
        &self,
        id: Uuid,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<DonorResponse> {
        auth.authorize(Permission::ViewDonors)?;
        
        let donor = self.repo.find_by_id(id).await?;
        
        // Enrich and return the response
        self.enrich_response(donor.into(), include, auth).await
    }

    async fn list_donors(
        &self,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>> {
        auth.authorize(Permission::ViewDonors)?;
        
        let paginated_result = self.repo.find_all(params).await?;
        
        let mut enriched_items = Vec::with_capacity(paginated_result.items.len());
        for donor in paginated_result.items {
            let enriched = self.enrich_response(donor.into(), include, auth).await?;
            enriched_items.push(enriched);
        }
        
        // Calculate total_pages
        let total_pages = if paginated_result.per_page > 0 {
            (paginated_result.total as f64 / paginated_result.per_page as f64).ceil() as u32
        } else {
            0 // Avoid division by zero
        };
        
        Ok(PaginatedResult {
            items: enriched_items,
            total: paginated_result.total,
            page: paginated_result.page,
            per_page: paginated_result.per_page,
            total_pages, // Add total_pages (now u32)
        })
    }

    async fn update_donor(
        &self,
        id: Uuid,
        mut update_data: UpdateDonor,
        auth: &AuthContext,
    ) -> ServiceResult<DonorResponse> {
        auth.authorize(Permission::EditDonors)?;
        
        // Removed incorrect ID comparison logic as UpdateDonor has no id field
        
        // Set updated_by_user_id if it exists on the struct
        // Assuming UpdateDonor has `updated_by_user_id: Option<Uuid>` or similar
        // update_data.updated_by_user_id = Some(auth.user_id);
        // If not, the repository might handle this based on AuthContext.
        // Let's assume repo handles it for now.
        
        // Validate the input data
        update_data.validate()?;

        // Call the repository to update the donor
        let updated_donor = self.repo.update(id, &update_data, auth).await?;
        
        // Enrich and return the response
        self.enrich_response(updated_donor.into(), None, auth).await
    }

    async fn delete_donor(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult> {
        // Construct delete options based on hard_delete flag
        let options = DeleteOptions { // Use direct construction
            allow_hard_delete: hard_delete, 
            ..Default::default() 
        };

        // Use the DeleteService trait method, which handles permission internally
        let result = self.delete(id, auth, options).await?;
        Ok(result)
    }
    
    async fn get_donor_summary(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorSummary> {
        auth.authorize(Permission::ViewDonors)?; // Permission check ok
        
        // DonorRepository has no get_summary method, remove this functionality for now
        // let summary = self.repo.get_summary(id).await?;
        // Ok(summary)
        Err(ServiceError::ServiceUnavailable("get_donor_summary is not implemented yet".to_string()))
    }
    
    // --- Document integration methods ---

    async fn upload_document_for_donor(
        &self,
        donor_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>, // Specific field on donor if applicable
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        auth.authorize(Permission::ViewDonors)?; // Need to view donor to link document
        auth.authorize(Permission::UploadDocuments)?; // Need permission to upload

        // Validate linked_field using DocumentLinkable trait
        if let Some(field_name) = &linked_field {
            let is_valid_linkable_field = Donor::field_metadata().iter().any(|meta| {
                meta.field_name == field_name && meta.supports_documents
            });
            if !is_valid_linkable_field {
                 // Convert ValidationError to DomainError first, then into ServiceError
                 let validation_error = ValidationError::Custom(
                    format!("Invalid or non-linkable field for Donor: {}", field_name)
                 );
                 return Err(DomainError::Validation(validation_error).into()); 
            }
        }
        
        // Check if donor exists (implicitly done by document service's link validation)
        // Alternatively, explicitly check: self.repo.find_by_id(donor_id).await?;

        // Use the injected document service
        let result = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            donor_id,
            "donors".to_string(),
            linked_field,
            sync_priority,
            compression_priority,
            None // temp_upload_id - not needed here
        ).await?;

        Ok(result)
    }

    async fn bulk_upload_documents_for_donor(
        &self,
        donor_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        auth.authorize(Permission::ViewDonors)?;
        auth.authorize(Permission::UploadDocuments)?;

        // Check if donor exists
        self.repo.find_by_id(donor_id).await?; // Fail early if donor doesn't exist

        let mut results = Vec::with_capacity(files.len());
        for (file_data, original_filename) in files {
            // Use the injected document service for each file
            let result = self.document_service.upload_document(
                auth,
                file_data,
                original_filename.clone(),
                title.clone(),
                document_type_id,
                donor_id,
                "donors".to_string(),
                None,
                sync_priority,
                compression_priority,
                None // temp_upload_id
            ).await;

            // Collect results, including potential errors for individual uploads
            match result {
                Ok(doc_response) => results.push(doc_response),
                Err(e) => {
                    // Log the error for the specific file?
                    eprintln!("Error uploading file {} for donor {}: {:?}", original_filename, donor_id, e);
                    // Optionally, you could return a Vec<ServiceResult<MediaDocumentResponse>> instead
                    // For now, we just skip adding the error to the success list.
                    // Consider if one failed upload should fail the whole batch? Current: No.
                }
            }
        }
        
        Ok(results) // Returns only the successfully uploaded documents
    }

    async fn get_donor_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<DonorDashboardStats> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?; // Maybe ViewStatistics permission?

        // 2. Get basic donor statistics
        let donor_stats = self.repo.get_donation_stats().await?;

        // 3. Get funding summary information
        // Assume funding_repo.get_funding_summary() returns Ok<(i64, f64, f64, HashMap<String, f64>)> 
        // representing (active_count, total_amount, avg_amount, currency_distribution)
        let (total_active_fundings, total_amount, avg_amount, funding_by_currency) = 
            match self.funding_repo.get_funding_summary().await {
                Ok(summary) => summary,
                Err(e) => {
                    // Log error but provide defaults
                    eprintln!("Failed to get funding summary for donor stats: {:?}", e);
                    (0, 0.0, 0.0, HashMap::new())
                },
            };

        // 4. Calculate recent donors (e.g., created in last 30 days - adjust as needed)
        // This requires a dedicated repository method for efficiency, e.g., count_recent_donors
        // Using find_with_recent_donations count is inefficient if list is large
        let thirty_days_ago = (Utc::now() - Duration::days(30)).to_rfc3339();
        let recent_donors_count = match self.repo.find_with_recent_donations(&thirty_days_ago, PaginationParams { page: 1, per_page: 1 }).await {
             Ok(result) => result.total as i64, // Assuming find_with_recent_donations correctly counts
             Err(e) => {
                 eprintln!("Failed to count recent donors: {:?}", e);
                 0 // Default to 0 on error
             }
        };

        // 5. Create and return the dashboard stats
        Ok(DonorDashboardStats {
            total_donors: donor_stats.total_donors,
            donors_by_type: donor_stats.donor_count_by_type,
            donors_by_country: donor_stats.donor_count_by_country,
            recent_donors_count, // Count based on recent *donations* as per repo method name
            funding_summary: FundingSummaryStats {
                total_active_fundings,
                total_funding_amount: total_amount,
                avg_funding_amount: avg_amount,
                funding_by_currency,
            },
        })
    }

    async fn get_type_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?; // Or ViewStatistics?

        // 2. Get type counts from repository
        let type_counts = self.repo.count_by_type().await?;

        // 3. Convert to a more user-friendly HashMap
        let mut distribution = HashMap::new();
        for (type_opt, count) in type_counts {
            let type_name = type_opt.unwrap_or_else(|| "Unspecified".to_string());
            distribution.insert(type_name, count);
        }

        Ok(distribution)
    }

    async fn get_country_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?; // Or ViewStatistics?

        // 2. Get country counts from repository
        let country_counts = self.repo.count_by_country().await?;

        // 3. Convert to a more user-friendly HashMap
        let mut distribution = HashMap::new();
        for (country_opt, count) in country_counts {
            let country_name = country_opt.unwrap_or_else(|| "Unspecified".to_string());
            distribution.insert(country_name, count);
        }

        Ok(distribution)
    }

    async fn find_donors_by_type(
        &self,
        donor_type: &str,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?;

        // 2. Find donors by type
        let paginated_result = self.repo.find_by_type(donor_type, params).await?;

        // 3. Convert and enrich each donor
        let mut enriched_items = Vec::new();
        for donor in paginated_result.items {
            let response = DonorResponse::from(donor);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 4. Return the paginated result with enriched donors
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_donors_by_country(
        &self,
        country: &str,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?;

        // 2. Find donors by country
        let paginated_result = self.repo.find_by_country(country, params).await?;

        // 3. Convert and enrich each donor
        let mut enriched_items = Vec::new();
        for donor in paginated_result.items {
            let response = DonorResponse::from(donor);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 4. Return the paginated result with enriched donors
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_donors_with_recent_donations(
        &self,
        days_ago: u32,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?;

        // 2. Calculate the cutoff date based on days_ago
        let cutoff_date = (Utc::now() - Duration::days(days_ago as i64)).to_rfc3339();

        // 3. Find donors with recent donations using the repository method
        let paginated_result = self.repo.find_with_recent_donations(&cutoff_date, params).await?;

        // 4. Convert and enrich each donor
        let mut enriched_items = Vec::new();
        for donor in paginated_result.items {
            let response = DonorResponse::from(donor);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }

        // 5. Return the paginated result with enriched donors
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn get_donor_with_funding_details(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorWithFundingDetails> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?;
        auth.authorize(Permission::ViewFunding)?; // Ensure user can view funding details

        // 2. Get the donor (include basic stats for the DonorResponse part)
        let donor_response = self.get_donor_by_id(id, Some(&[DonorInclude::FundingStats]), auth).await?;

        // 3. Get detailed funding statistics
        // Assuming funding_repo.get_donor_detailed_funding_stats returns Ok((i64, i64, f64, f64, f64, f64, HashMap<String, f64>))
        // representing (active_count, total_count, total_amount, active_amount, avg_amount, largest_amount, currency_dist)
        let stats = match self.funding_repo.get_donor_detailed_funding_stats(id).await {
            Ok((active_count, total_count, total_amount, active_amount, avg_amount, largest_amount, currency_dist)) => {
                DonorFundingStats {
                    active_fundings_count: active_count,
                    total_fundings_count: total_count,
                    total_funding_amount: total_amount,
                    active_funding_amount: active_amount,
                    avg_funding_amount: avg_amount,
                    largest_funding_amount: largest_amount,
                    currency_distribution: currency_dist,
                }
            },
            Err(e) => {
                 eprintln!("Failed to get detailed funding stats for donor {}: {:?}", id, e);
                 // Provide default stats on error
                 DonorFundingStats {
                    active_fundings_count: 0,
                    total_fundings_count: 0,
                    total_funding_amount: 0.0,
                    active_funding_amount: 0.0,
                    avg_funding_amount: 0.0,
                    largest_funding_amount: 0.0,
                    currency_distribution: HashMap::new(),
                }
            }
        };

        // 4. Get recent funding entries (limit to, say, 5)
        // Assuming funding_repo.get_recent_fundings_for_donor returns Ok<Vec<ProjectFundingSummary>>
        let recent_fundings = match self.funding_repo.get_recent_fundings_for_donor(id, 5).await {
            Ok(fundings) => fundings,
            Err(e) => {
                eprintln!("Failed to get recent fundings for donor {}: {:?}", id, e);
                Vec::new()
            },
        };

        // 5. Combine into response
        Ok(DonorWithFundingDetails {
            donor: donor_response,
            funding_stats: stats,
            recent_fundings,
        })
    }

    async fn get_donor_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorWithDocumentTimeline> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?;
        auth.authorize(Permission::ViewDocuments)?;

        // 2. Get the donor base response
        let donor_response = self.get_donor_by_id(id, None, auth).await?;

        // 3. Get all documents for this donor (handle potential pagination if necessary)
        // Fetching all might be inefficient for donors with many docs; consider adding date range filter?
        let docs_result = self.document_service.list_media_documents_by_related_entity(
            auth,
            "donors",
            id,
            PaginationParams { page: 1, per_page: 500 }, // Set a reasonable limit, adjust as needed
            None,
        ).await?;
        
        let documents = docs_result.items;
        let total_document_count = docs_result.total; // Use total from pagination

        // 4. Organize documents by month (YYYY-MM)
        let mut documents_by_month: HashMap<String, Vec<MediaDocumentResponse>> = HashMap::new();
        for doc in documents {
            // Use created_at timestamp for grouping
            if let Ok(created_at_dt) = DateTime::parse_from_rfc3339(&doc.created_at) {
                let month_key = format!("{}-{:02}", created_at_dt.year(), created_at_dt.month());
                documents_by_month
                    .entry(month_key)
                    .or_default()
                    .push(doc);
            } else {
                 eprintln!("Failed to parse document created_at date: {}", doc.created_at);
                 // Optionally group unparsable dates under a special key
            }
        }

        // 5. Create and return the response
        Ok(DonorWithDocumentTimeline {
            donor: donor_response,
            documents_by_month,
            total_document_count,
        })
    }
}