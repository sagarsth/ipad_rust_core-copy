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
    DonorWithDocumentTimeline, DonorFundingStats, UserDonorRole,
    DashboardTrendAnalysis, DonorActivityTimeline, DonorEngagementMetrics,
    DonorDocumentTimeline, DonorDocumentSummary, DocumentComplianceStatus,
    DonorFilter, DonorBulkOperationResult, DonorDuplicateInfo, DonorType,
    DonorStatsSummary, DonorTrendAnalysis
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
use sqlx::query_scalar;
use std::str::FromStr;

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

    /// Get donor statistics including counts and funding summaries
    async fn get_donor_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<DonorDashboardStats>;
    
    /// Find donors within a date range (created_at or updated_at)
    /// Expects RFC3339 format timestamps (e.g., "2024-01-01T00:00:00Z")
    async fn find_donors_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>>;

    // === ADVANCED FILTERING ===
    /// Find donor IDs by complex filter criteria - enables bulk operations
    async fn find_donor_ids_by_filter(
        &self,
        filter: DonorFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>>;

    /// Find donors by complex filter criteria with pagination
    async fn find_donors_by_filter(
        &self,
        filter: DonorFilter,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>>;

    /// Performance-optimized version of find_donor_ids_by_filter
    async fn find_donor_ids_by_filter_optimized(
        &self,
        filter: DonorFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>>;

    // === BULK OPERATIONS ===
    /// Bulk update sync priority for donors matching filter criteria
    async fn bulk_update_sync_priority_by_filter(
        &self,
        filter: DonorFilter,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64>;

    /// Bulk update sync priority for specific donor IDs
    async fn bulk_update_donor_sync_priority(
        &self,
        ids: Vec<Uuid>,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64>;

    /// Memory-efficient bulk update with streaming processing
    async fn bulk_update_donors_streaming(
        &self,
        updates: Vec<(Uuid, UpdateDonor)>,
        chunk_size: usize,
        auth: &AuthContext,
    ) -> ServiceResult<DonorBulkOperationResult>;

    // === ENRICHMENT & ANALYTICS ===
    /// Get donor engagement metrics
    async fn get_donor_engagement_metrics(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorEngagementMetrics>;

    /// Get comprehensive donor trend analysis
    async fn get_donor_trend_analysis(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorTrendAnalysis>;

    /// Get donor funding analytics
    async fn get_donor_funding_analytics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, f64>>;

    /// Get donor growth trends over time periods
    async fn get_donor_growth_trends(
        &self,
        months: Option<i32>,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Get data quality report for dashboard alerts
    async fn get_donor_data_quality_report(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, f64>>;

    // === DUPLICATE DETECTION ===
    /// Check for potential duplicate donors by name and return their details
    async fn check_potential_duplicates(
        &self,
        name: &str,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<DonorDuplicateInfo>>;

    // === PERFORMANCE OPTIMIZATION ===
    /// Get database index optimization suggestions
    async fn get_index_optimization_suggestions(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<String>>;

    /// Enhanced donor creation with documents
    async fn create_donor_with_documents_enhanced(
        &self,
        new_donor: NewDonor,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(DonorResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;
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
                        response = response.with_funding_stats(active_count, total_amount, 0, 0.0, None);
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

    /// Comprehensive business rule validation for new donors
    async fn validate_donor_business_rules(
        &self,
        new_donor: &NewDonor,
        _current_state: Option<&Donor>,
    ) -> ServiceResult<()> {
        // 1. Log potential duplicates for information (but don't block creation)
        if let Ok(existing_donors) = self.repo.find_by_type(&new_donor.type_.clone().unwrap_or_default(), 
                                                           PaginationParams { page: 1, per_page: 1 }).await {
            if !existing_donors.items.is_empty() {
                log::info!("Found {} existing donors with same type '{}'. Allowing creation but logging for potential analysis.",
                         existing_donors.total, new_donor.type_.as_ref().unwrap_or(&"Unknown".to_string()));
            }
        }

        // 2. Validate donor type consistency
        if let Some(donor_type) = &new_donor.type_ {
            match DonorType::from_str(donor_type) {
                Some(_) => {}, // Valid type
                None => {
                    return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                        "type_",
                        &format!("Invalid donor type '{}'. Must be one of: {}", 
                            donor_type, DonorType::all_variants().join(", "))
                    ))));
                }
            }
        }

        // 3. Check name quality (business rule for data integrity)
        if new_donor.name.len() < 2 {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "name",
                "Donor name is too short. Please provide a valid organization or person name."
            ))));
        }

        // 4. Validate email format if provided
        if let Some(email) = &new_donor.email {
            if !email.contains('@') || !email.contains('.') {
                return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                    "email",
                    "Invalid email format. Please provide a valid email address."
                ))));
            }
        }

        // 5. Validate funding amount if this is a corporate donor
        if let Some(donor_type) = &new_donor.type_ {
            if donor_type == "Corporate" && new_donor.contact_person.is_none() {
                log::warn!("Corporate donor '{}' created without contact person. Consider adding contact information.", new_donor.name);
            }
        }

        Ok(())
    }

    /// Comprehensive business rule validation for donor updates
    async fn validate_donor_business_rules_for_update(
        &self,
        update_data: &UpdateDonor,
        current_donor: &Donor,
    ) -> ServiceResult<()> {
        // 1. Log potential name duplicates if name is being changed (but don't block)
        if let Some(new_name) = &update_data.name {
            if new_name != &current_donor.name {
                log::info!("Updating donor {} name from '{}' to '{}'. Duplicate check will be performed within transaction.",
                         current_donor.id, current_donor.name, new_name);
            }
        }

        // 2. Validate type changes
        if let Some(new_type) = &update_data.type_ {
            if let Some(current_type) = &current_donor.type_ {
                if new_type != current_type {
                    log::warn!("Donor {} type changed from '{}' to '{}' by user {}. Consider reviewing associated funding records.",
                             current_donor.id, current_type, new_type, update_data.updated_by_user_id);
                }
            }
        }

        // 3. Validate sync priority changes
        if let Some(new_priority) = &update_data.sync_priority {
            if new_priority == "never" {
                log::warn!("Donor {} sync priority changed to Never by user {}. This will prevent synchronization.",
                         current_donor.id, update_data.updated_by_user_id);
            } else if new_priority == "high" {
                log::info!("Donor {} sync priority changed to High by user {}",
                         current_donor.id, update_data.updated_by_user_id);
            }
        }

        // 4. Business rule: Email format validation
        if let Some(new_email) = &update_data.email {
            if !new_email.contains('@') || !new_email.contains('.') {
                return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                    "email",
                    "Invalid email format. Please provide a valid email address."
                ))));
            }
        }

        Ok(())
    }

    /// Comprehensive document upload validation for donors
    async fn validate_document_upload(
        &self,
        file_data: &[u8],
        filename: &str,
        linked_field: &Option<String>,
    ) -> ServiceResult<()> {
        // 1. File size validation (business rule)
        const MAX_FILE_SIZE: usize = 100 * 1024 * 1024; // 100MB for donor documents
        if file_data.len() > MAX_FILE_SIZE {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "file_size",
                &format!("File size ({:.2} MB) exceeds maximum allowed size of 100 MB. Please compress or reduce the file size.",
                    file_data.len() as f64 / 1024.0 / 1024.0)
            ))));
        }

        // 2. Minimum file size validation
        if file_data.len() < 100 {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "file_size",
                "File is too small to be a valid document. Please check the file and try again."
            ))));
        }

        // 3. Filename validation
        if filename.trim().is_empty() {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "filename",
                "Filename cannot be empty. Please provide a valid filename."
            ))));
        }

        // 4. File extension validation
        let extension = filename.split('.').last().unwrap_or("").to_lowercase();
        let allowed_extensions = ["pdf", "jpg", "jpeg", "png", "doc", "docx", "txt", "rtf", "xls", "xlsx"];
        if !allowed_extensions.contains(&extension.as_str()) {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "file_extension",
                &format!("File type '.{}' is not allowed. Supported formats: {}",
                    extension, allowed_extensions.join(", "))
            ))));
        }

        // 5. Linked field specific validation for donors
        if let Some(field) = linked_field {
            match field.as_str() {
                "donor_agreement_ref" => {
                    // Agreement documents should be PDF
                    if extension != "pdf" {
                        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                            "file_extension",
                            "Donor agreements must be PDF files for legal compliance."
                        ))));
                    }
                }
                "due_diligence_ref" => {
                    // Due diligence documents should be PDF or Excel
                    if !["pdf", "xls", "xlsx"].contains(&extension.as_str()) {
                        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                            "file_extension",
                            "Due diligence documents must be PDF or Excel files."
                        ))));
                    }
                }
                "tax_information_ref" => {
                    // Tax documents should be PDF
                    if !["pdf", "jpg", "jpeg", "png"].contains(&extension.as_str()) {
                        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                            "file_extension",
                            "Tax documents must be PDF or image files."
                        ))));
                    }
                }
                _ => {
                    // Other fields accept all allowed formats
                }
            }
        }

        Ok(())
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
        // 1. Check permissions first
        auth.authorize(Permission::CreateDonors)?;
        
        // 2. Enhanced input validation
        new_donor.validate().map_err(ServiceError::Domain)?;
        
        // 3. Additional business rule validation
        self.validate_donor_business_rules(&new_donor, None).await?;
        
        // 4. Create the donor
        let donor = self.repo.create(&new_donor, auth).await.map_err(ServiceError::Domain)?;
        
        // 5. Enrich and return the response
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
        // 1. Check permissions first
        auth.authorize(Permission::EditDonors)?;
        
        // 2. Set updated_by_user_id
        update_data.updated_by_user_id = auth.user_id;
        
        // 3. Enhanced input validation
        update_data.validate().map_err(ServiceError::Domain)?;
        
        // 4. Fetch current state BEFORE transaction for validation
        let current_donor = self.repo.find_by_id(id).await.map_err(ServiceError::Domain)?;
        
        // 5. Enhanced business rule validation with current state
        self.validate_donor_business_rules_for_update(&update_data, &current_donor).await?;
        
        // 6. Proceed with update
        let updated_donor = self.repo.update(id, &update_data, auth).await.map_err(ServiceError::Domain)?;
        
        // 7. Enrich and return the response
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
        // 1. Verify donor exists BEFORE starting document operations
        let _donor = self.repo.find_by_id(donor_id).await.map_err(ServiceError::Domain)?;
        
        // 2. Check permissions
        auth.authorize(Permission::ViewDonors)?; // Need to view donor to link document
        auth.authorize(Permission::UploadDocuments)?; // Need permission to upload

        // 3. Enhanced document upload validation
        self.validate_document_upload(&file_data, &original_filename, &linked_field).await?;

        // 4. Validate linked_field using DocumentLinkable trait
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

        // 5. Use the injected document service
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
            donors_by_engagement: HashMap::new(), // TODO: Implement engagement tracking
            recent_donors_count, // Count based on recent *donations* as per repo method name
            at_risk_donors_count: 0, // TODO: Implement risk calculation
            trend_analysis: DashboardTrendAnalysis {
                new_donors_trend: Vec::new(),
                funding_amount_trend: Vec::new(),
                engagement_trend: Vec::new(),
                retention_trend: Vec::new(),
            },
            top_donors: Vec::new(), // TODO: Calculate top donors
            alerts: Vec::new(), // TODO: Generate alerts
            funding_summary: FundingSummaryStats {
                total_active_fundings,
                total_funding_amount: total_amount,
                avg_funding_amount: avg_amount,
                median_funding_amount: avg_amount, // Using avg as placeholder
                funding_by_currency,
                funding_by_status: HashMap::new(),
                monthly_funding_trend: Vec::new(),
                top_funding_countries: Vec::new(),
                funding_concentration: 0.0,
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
                    completed_fundings_count: 0, // TODO: Get from repository
                    pending_fundings_count: 0, // TODO: Get from repository
                    total_funding_amount: total_amount,
                    active_funding_amount: active_amount,
                    avg_funding_amount: avg_amount,
                    median_funding_amount: avg_amount, // Using avg as placeholder
                    largest_funding_amount: largest_amount,
                    smallest_funding_amount: 0.0, // TODO: Get from repository
                    currency_distribution: currency_dist,
                    funding_frequency: 0.0, // TODO: Calculate frequency
                    retention_rate: 0.0, // TODO: Calculate retention
                    funding_trend: Vec::new(), // TODO: Get trend data
                    project_success_rate: 0.0, // TODO: Calculate success rate
                }
            },
            Err(e) => {
                 eprintln!("Failed to get detailed funding stats for donor {}: {:?}", id, e);
                 // Provide default stats on error
                 DonorFundingStats {
                    active_fundings_count: 0,
                    total_fundings_count: 0,
                    completed_fundings_count: 0,
                    pending_fundings_count: 0,
                    total_funding_amount: 0.0,
                    active_funding_amount: 0.0,
                    avg_funding_amount: 0.0,
                    median_funding_amount: 0.0,
                    largest_funding_amount: 0.0,
                    smallest_funding_amount: 0.0,
                    currency_distribution: HashMap::new(),
                    funding_frequency: 0.0,
                    retention_rate: 0.0,
                    funding_trend: Vec::new(),
                    project_success_rate: 0.0,
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
            activity_timeline: DonorActivityTimeline {
                donor_id: id,
                funding_activities: Vec::new(), // TODO: Get from repository
                communication_activities: Vec::new(), // TODO: Get from repository
                document_activities: Vec::new(), // TODO: Get from repository
                agreement_activities: Vec::new(), // TODO: Get from repository
                profile_changes: Vec::new(), // TODO: Get from repository
            },
            engagement_metrics: DonorEngagementMetrics {
                donor_id: id,
                engagement_score: 0.0, // TODO: Calculate
                funding_retention_rate: 0.0, // TODO: Calculate
                avg_donation_frequency_months: 0.0, // TODO: Calculate
                last_donation_date: None, // TODO: Get from repository
                last_communication_date: None, // TODO: Get from repository
                communication_frequency_score: 0.0, // TODO: Calculate
                project_success_correlation: 0.0, // TODO: Calculate
                responsiveness_score: 0.0, // TODO: Calculate
                relationship_strength: crate::domains::donor::types::RelationshipStrength::Weak, // TODO: Calculate
                risk_indicators: Vec::new(), // TODO: Calculate
            },
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
            document_timeline: DonorDocumentTimeline {
                donor_id: id,
                agreements_by_year: HashMap::new(), // TODO: Organize documents by year
                communications_by_quarter: documents_by_month, // Reuse the monthly organization
                due_diligence_docs: Vec::new(), // TODO: Filter by document type
                tax_documents: Vec::new(), // TODO: Filter by document type
                legal_documents: Vec::new(), // TODO: Filter by document type
                total_document_count: total_document_count as usize,
                document_completeness_score: 0.0, // TODO: Calculate completeness
            },
            document_summary: DonorDocumentSummary {
                agreement_count: 0, // TODO: Count by type
                due_diligence_count: 0, // TODO: Count by type
                tax_document_count: 0, // TODO: Count by type
                communication_count: 0, // TODO: Count by type
                legal_document_count: 0, // TODO: Count by type
                financial_statement_count: 0, // TODO: Count by type
                total_size_mb: 0.0, // TODO: Calculate total size
                last_document_upload: None, // TODO: Get latest upload time
                document_counts_by_type: HashMap::new(), // TODO: Group counts by type
            },
            compliance_status: DocumentComplianceStatus {
                has_required_documents: false, // TODO: Check requirements
                missing_documents: Vec::new(), // TODO: Identify missing docs
                expired_documents: Vec::new(), // TODO: Check for expired docs
                compliance_score: 0.0, // TODO: Calculate compliance score
            },
        })
    }

    /// Find donors within a date range (created_at or updated_at)
    /// Expects RFC3339 format timestamps (e.g., "2024-01-01T00:00:00Z")
    async fn find_donors_by_date_range(
        &self,
        start_rfc3339: &str, // RFC3339 format datetime string
        end_rfc3339: &str,   // RFC3339 format datetime string
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewDonors)?;

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

        // 4. Get donors in date range
        let paginated_result = self.repo
            .find_by_date_range(start_datetime, end_datetime, params)
            .await
            .map_err(ServiceError::Domain)?;

        // 5. Convert to response DTOs and enrich
        let mut enriched_items = Vec::new();
        for donor in paginated_result.items {
            let response = DonorResponse::from(donor);
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

    // === ADVANCED FILTERING ===
    async fn find_donor_ids_by_filter(
        &self,
        filter: DonorFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>> {
        auth.authorize(Permission::ViewDonors)?;
        filter.validate().map_err(ServiceError::Domain)?;
        self.repo.find_ids_by_filter(&filter).await.map_err(ServiceError::Domain)
    }

    async fn find_donors_by_filter(
        &self,
        filter: DonorFilter,
        params: PaginationParams,
        include: Option<&[DonorInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<DonorResponse>> {
        auth.authorize(Permission::ViewDonors)?;
        filter.validate().map_err(ServiceError::Domain)?;
        
        let paginated_result = self.repo.find_by_filter(&filter, params).await.map_err(ServiceError::Domain)?;
        let mut enriched_items = Vec::with_capacity(paginated_result.items.len());
        
        for item in paginated_result.items {
            let response = DonorResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }
        
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn find_donor_ids_by_filter_optimized(
        &self,
        filter: DonorFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>> {
        auth.authorize(Permission::ViewDonors)?;
        filter.validate().map_err(ServiceError::Domain)?;
        self.repo.find_ids_by_filter_optimized(&filter).await.map_err(ServiceError::Domain)
    }

    // === BULK OPERATIONS ===
    async fn bulk_update_sync_priority_by_filter(
        &self,
        filter: DonorFilter,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64> {
        auth.authorize(Permission::EditDonors)?;
        filter.validate().map_err(ServiceError::Domain)?;
        self.repo.bulk_update_sync_priority_by_filter(&filter, priority, auth).await.map_err(ServiceError::Domain)
    }

    async fn bulk_update_donor_sync_priority(
        &self,
        ids: Vec<Uuid>,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64> {
        auth.authorize(Permission::EditDonors)?;
        
        if ids.is_empty() {
            return Ok(0);
        }

        let rows_affected = self.repo.update_sync_priority(&ids, priority, auth).await
            .map_err(ServiceError::Domain)?;

        Ok(rows_affected)
    }

    async fn bulk_update_donors_streaming(
        &self,
        updates: Vec<(Uuid, UpdateDonor)>,
        _chunk_size: usize, // Repository handles chunking internally
        auth: &AuthContext,
    ) -> ServiceResult<DonorBulkOperationResult> {
        auth.authorize(Permission::EditDonors)?;
        
        let bulk_result = self.repo.bulk_update_donors_streaming(updates, auth).await
            .map_err(ServiceError::Domain)?;
        Ok(bulk_result)
    }

    // === ENRICHMENT & ANALYTICS ===
    async fn get_donor_engagement_metrics(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorEngagementMetrics> {
        auth.authorize(Permission::ViewDonors)?;
        let metrics = self.repo.get_donor_engagement_metrics(id).await.map_err(ServiceError::Domain)?;
        Ok(metrics)
    }

    async fn get_donor_trend_analysis(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<DonorTrendAnalysis> {
        auth.authorize(Permission::ViewDonors)?;
        
        // Get basic donor info
        let donor = self.repo.find_by_id(id).await.map_err(ServiceError::Domain)?;
        
        // Create trend analysis (placeholder implementation)
        Ok(DonorTrendAnalysis {
            donor_id: id,
            donation_patterns: HashMap::new(),
            funding_amount_trend: Vec::new(),
            communication_trend: Vec::new(),
            engagement_trend: Vec::new(),
            seasonal_patterns: HashMap::new(),
            prediction_confidence: 0.0,
        })
    }

    async fn get_donor_funding_analytics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, f64>> {
        auth.authorize(Permission::ViewDonors)?;
        
        let stats = self.repo.get_donation_stats().await.map_err(ServiceError::Domain)?;
        
        let mut analytics = HashMap::new();
        analytics.insert("total_donors".to_string(), stats.total_donors as f64);
        analytics.insert("active_donors".to_string(), stats.active_donors as f64);
        analytics.insert("inactive_donors".to_string(), stats.inactive_donors as f64);
        analytics.insert("total_donation_amount".to_string(), stats.total_donation_amount.unwrap_or(0.0));
        analytics.insert("avg_donation_amount".to_string(), stats.avg_donation_amount.unwrap_or(0.0));
        analytics.insert("data_completeness_avg".to_string(), stats.data_completeness_avg);
        
        Ok(analytics)
    }

    async fn get_donor_growth_trends(
        &self,
        months: Option<i32>,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        auth.authorize(Permission::ViewDonors)?;
        
        let period_months = months.unwrap_or(12);
        let now = Utc::now();
        
        let mut trends = HashMap::new();
        
        // Calculate monthly growth for the specified period
        for i in 0..period_months {
            let month_start = now - chrono::Duration::days((i + 1) as i64 * 30);
            let month_end = now - chrono::Duration::days(i as i64 * 30);
            
            let month_key = month_start.format("%Y-%m").to_string();
            
            // Get count for this month
            let month_count = query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM donors WHERE created_at >= ? AND created_at < ? AND deleted_at IS NULL"
            )
            .bind(month_start.to_rfc3339())
            .bind(month_end.to_rfc3339())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
            
            trends.insert(month_key, month_count);
        }
        
        Ok(trends)
    }

    async fn get_donor_data_quality_report(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, f64>> {
        auth.authorize(Permission::ViewDonors)?;
        
        let stats = self.repo.get_donation_stats().await.map_err(ServiceError::Domain)?;
        
        let mut quality_report = HashMap::new();
        
        // Overall completeness
        quality_report.insert("overall_completeness".to_string(), stats.data_completeness_avg);
        
        // Type coverage
        let total_with_type = stats.donor_count_by_type.values().sum::<i64>();
        if stats.total_donors > 0 {
            let type_completeness = (total_with_type as f64 / stats.total_donors as f64) * 100.0;
            quality_report.insert("type_completeness".to_string(), type_completeness);
        }
        
        // Country coverage
        let total_with_country = stats.donor_count_by_country.values().sum::<i64>();
        if stats.total_donors > 0 {
            let country_completeness = (total_with_country as f64 / stats.total_donors as f64) * 100.0;
            quality_report.insert("country_completeness".to_string(), country_completeness);
        }
        
        // Engagement rate
        if stats.total_donors > 0 {
            let engagement_rate = (stats.active_donors as f64 / stats.total_donors as f64) * 100.0;
            quality_report.insert("engagement_rate".to_string(), engagement_rate);
        }
        
        Ok(quality_report)
    }

    // === DUPLICATE DETECTION ===
    async fn check_potential_duplicates(
        &self,
        name: &str,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<DonorDuplicateInfo>> {
        auth.authorize(Permission::ViewDonors)?;
        
        // Use case-insensitive search for potential duplicates
        let search_filter = DonorFilter::new()
            .with_search_text(name.to_string());
        
        let similar_donors = self.repo.find_by_filter(&search_filter, 
                                                    PaginationParams { page: 1, per_page: 10 }).await
            .map_err(ServiceError::Domain)?;
        
        let mut duplicate_infos = Vec::new();
        
        for donor in similar_donors.items {
            // Calculate similarity score (simple implementation)
            let similarity_score = if donor.name.to_lowercase() == name.to_lowercase() {
                1.0
            } else if donor.name.to_lowercase().contains(&name.to_lowercase()) {
                0.8
            } else {
                0.5
            };
            
            let duplicate_info = DonorDuplicateInfo {
                id: donor.id,
                name: donor.name,
                type_: donor.type_.and_then(|t| DonorType::from_str(&t)),
                contact_person: donor.contact_person,
                email: donor.email,
                phone: donor.phone,
                country: donor.country,
                similarity_score,
                matching_fields: vec!["name".to_string()], // Could be enhanced
                confidence_level: if similarity_score > 0.9 {
                    crate::domains::donor::types::DuplicateConfidence::High
                } else if similarity_score > 0.7 {
                    crate::domains::donor::types::DuplicateConfidence::Medium
                } else {
                    crate::domains::donor::types::DuplicateConfidence::Low
                },
                document_similarity: None, // Could be enhanced
                created_at: donor.created_at,
            };
            
            duplicate_infos.push(duplicate_info);
        }
        
        Ok(duplicate_infos)
    }

    // === PERFORMANCE OPTIMIZATION ===
    async fn get_index_optimization_suggestions(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<String>> {
        auth.authorize(Permission::ViewDonors)?;
        let suggestions = self.repo.get_index_optimization_suggestions().await.map_err(ServiceError::Domain)?;
        Ok(suggestions)
    }

    async fn create_donor_with_documents_enhanced(
        &self,
        new_donor: NewDonor,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(DonorResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        // 1. Check Permissions
        auth.authorize(Permission::CreateDonors)?;
        
        if !documents.is_empty() {
            auth.authorize(Permission::UploadDocuments)?;
        }

        // 2. Enhanced validation
        new_donor.validate().map_err(ServiceError::Domain)?;
        self.validate_donor_business_rules(&new_donor, None).await?;
        
        // 3. Begin transaction for donor creation
        let mut tx = self.pool.begin().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 4. Create the donor first (within transaction)
        let created_donor = match self.repo.create_with_tx(&new_donor, auth, &mut tx).await {
            Ok(donor) => donor,
            Err(e) => {
                let _ = tx.rollback().await; // Rollback on error
                return Err(ServiceError::Domain(e));
            }
        };
        
        // 5. Commit transaction to ensure donor is created before attaching docs
        if let Err(e) = tx.commit().await {
             return Err(ServiceError::Domain(DomainError::Database(DbError::from(e))));
        }
        
        // 6. Now upload documents (outside transaction)
        let document_results = if !documents.is_empty() {
            // Enhanced validation for each document
            for (ref file_data, ref filename, ref linked_field) in &documents {
                self.validate_document_upload(file_data, filename, linked_field).await?;
            }
            
            self.upload_documents_for_entity(
                created_donor.id,
                "donors",
                documents,
                document_type_id,
                                 new_donor.sync_priority.as_ref()
                     .and_then(|p| SyncPriority::from_str(p).ok())
                     .unwrap_or(SyncPriority::Normal),
                None,
                auth,
            ).await
        } else {
            Vec::new()
        };
        
        // 7. Convert to Response DTO and return with document results
        let response = self.enrich_response(DonorResponse::from(created_donor), None, auth).await?;
        Ok((response, document_results))
    }
}