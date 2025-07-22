use crate::auth::AuthContext;
use sqlx::{SqlitePool, Transaction, Sqlite};
use crate::domains::core::dependency_checker::DependencyChecker;
use crate::domains::core::delete_service::{BaseDeleteService, DeleteOptions, DeleteService, DeleteServiceRepository};
use crate::domains::core::repository::{DeleteResult, FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::permission::Permission;
use crate::domains::participant::repository::ParticipantRepository;
use crate::domains::participant::types::{
    NewParticipant, Participant, ParticipantResponse, UpdateParticipant, ParticipantInclude,
    ParticipantDemographics, WorkshopSummary, LivelihoodSummary, ParticipantWithWorkshops,
    ParticipantWithLivelihoods, ParticipantWithDocumentTimeline, ParticipantFilter, ParticipantDocumentReference,
    ParticipantWithDocumentsByType, ParticipantActivityTimeline, ParticipantWorkshopActivity,
    ParticipantLivelihoodActivity, ParticipantDocumentActivity, ParticipantWithEnrichment,
    ParticipantEngagementMetrics, ParticipantStatistics, ParticipantBulkOperationResult,
    ParticipantSearchIndex
};
use crate::domains::sync::repository::{ChangeLogRepository, TombstoneRepository};
use crate::domains::document::service::DocumentService;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::compression::types::CompressionPriority;
use crate::errors::{DbError, DomainError, DomainResult, ServiceError, ServiceResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::validation::Validate;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use chrono::Datelike;
use crate::domains::core::delete_service::PendingDeletionManager;
use sqlx::query_scalar;
use chrono::Utc;
use chrono::DateTime;
/// Trait defining participant service operations
#[async_trait]
pub trait ParticipantService: DeleteService<Participant> + Send + Sync {
    async fn create_participant(
        &self,
        new_participant: NewParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse>;

    async fn get_participant_by_id(
        &self,
        id: Uuid,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse>;

    async fn list_participants(
        &self,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    async fn update_participant(
        &self,
        id: Uuid,
        update_data: UpdateParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse>;
    
    async fn delete_participant(
        &self,
        id: Uuid,
        hard_delete: bool,
        auth: &AuthContext,
    ) -> ServiceResult<DeleteResult>;
    
    /// Find participant IDs by complex filter criteria - enables bulk operations like project domain
    async fn find_participant_ids_by_filter(
        &self,
        filter: ParticipantFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>>;
    
    /// Find participants by complex filter criteria with pagination
    async fn find_participants_by_filter(
        &self,
        filter: ParticipantFilter,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;
    
    /// Bulk update sync priority for participants matching filter criteria
    async fn bulk_update_sync_priority_by_filter(
        &self,
        filter: ParticipantFilter,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64>;
    
    /// Bulk update sync priority for specific participant IDs - optimized like project domain
    async fn bulk_update_participant_sync_priority(
        &self,
        ids: Vec<Uuid>,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64>;

    async fn upload_document_for_participant(
        &self,
        participant_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse>;

    async fn bulk_upload_documents_for_participant(
        &self,
        participant_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>>;
    
    async fn create_participant_with_documents(
        &self,
        new_participant: NewParticipant,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ParticipantResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)>;
    
    /// Get comprehensive participant demographics for dashboards - matches project domain capabilities
    async fn get_participant_demographics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantDemographics>;
    
    /// Get participant statistics (alias for demographics for API consistency)
    async fn get_participant_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantDemographics>;
    
    /// Get engagement analytics - participants by activity level
    async fn get_participant_engagement_analytics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;
    
    /// Get growth trends over time periods
    async fn get_participant_growth_trends(
        &self,
        months: Option<i32>,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;
    
    /// Get data quality report for dashboard alerts
    async fn get_data_quality_report(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, f64>>;
    

    
    /// Get participant with document timeline - matches project domain pattern
    async fn get_participant_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithDocumentTimeline>;
    
    /// Get participant with documents organized by type - alternative view matching project pattern exactly
    async fn get_participant_with_documents_by_type(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithDocumentsByType>;
    
    /// Get participant activity timeline for engagement tracking
    async fn get_participant_activity_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantActivityTimeline>;

    /// Get distribution of participants by gender
    async fn get_gender_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Get distribution of participants by age group
    async fn get_age_group_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Get distribution of participants by location
    async fn get_location_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;

    /// Get distribution of participants by disability status
    async fn get_disability_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>>;
    
    /// Get all available disability types for UI dropdown - called on long-press of disability filter
    async fn get_available_disability_types(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<String>>;

    /// Find participants by gender
    async fn find_participants_by_gender(
        &self,
        gender: &str,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    /// Find participants by age group
    async fn find_participants_by_age_group(
        &self,
        age_group: &str,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    /// Find participants by location
    async fn find_participants_by_location(
        &self,
        location: &str,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    /// Find participants by disability status
    async fn find_participants_by_disability(
        &self,
        has_disability: bool,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    /// Get participants for a specific workshop
    async fn get_workshop_participants(
        &self,
        workshop_id: Uuid,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    /// Get participant with detailed workshop information
    async fn get_participant_with_workshops(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithWorkshops>;

    /// Get participant with detailed livelihood information
    async fn get_participant_with_livelihoods(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithLivelihoods>;

    // ---------------------------------------------------------------------------
    // ENTERPRISE ADVANCED FEATURES
    // ---------------------------------------------------------------------------

    /// Search participants with relationship data (workshops and livelihoods)
    async fn search_participants_with_relationships(
        &self,
        search_text: &str,
        params: PaginationParams,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>>;

    /// Get participant with comprehensive enrichment data
    async fn get_participant_with_enrichment(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithEnrichment>;

    /// Get comprehensive participant statistics for dashboards
    async fn get_comprehensive_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantStatistics>;

    /// Get participant document references metadata
    async fn get_participant_document_references(
        &self,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ParticipantDocumentReference>>;

    /// Bulk update participants with streaming processing
    async fn bulk_update_participants_streaming(
        &self,
        updates: Vec<(Uuid, UpdateParticipant)>,
        chunk_size: usize,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantBulkOperationResult>;

    /// Get database index optimization suggestions
    async fn get_index_optimization_suggestions(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<String>>;

    /// Find participant IDs by filter with query optimization
    async fn find_participant_ids_by_filter_optimized(
        &self,
        filter: ParticipantFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>>;

}

/// Implementation of the participant service
#[derive(Clone)] 
pub struct ParticipantServiceImpl {
    pool: SqlitePool,
    repo: Arc<dyn ParticipantRepository + Send + Sync>,
    delete_service: Arc<BaseDeleteService<Participant>>,
    document_service: Arc<dyn DocumentService>,
}

impl ParticipantServiceImpl {
    pub fn new(
        pool: SqlitePool,
        participant_repo: Arc<dyn ParticipantRepository + Send + Sync>,
        tombstone_repo: Arc<dyn TombstoneRepository + Send + Sync>,
        change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
        dependency_checker: Arc<dyn DependencyChecker + Send + Sync>,
        document_service: Arc<dyn DocumentService>,
        deletion_manager: Arc<PendingDeletionManager>,
    ) -> Self {
        // Local adapter struct
        struct RepoAdapter(Arc<dyn ParticipantRepository + Send + Sync>);

        #[async_trait]
        impl FindById<Participant> for RepoAdapter {
            async fn find_by_id(&self, id: Uuid) -> DomainResult<Participant> {
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
                 "participants"
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
        
        // Blanket impl covers DeleteServiceRepository<Participant>

        let adapted_repo: Arc<dyn DeleteServiceRepository<Participant>> = 
            Arc::new(RepoAdapter(participant_repo.clone()));

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
            repo: participant_repo,
            delete_service,
            document_service,
        }
    }
    
    /// Comprehensive enrichment system matching project domain capabilities
    async fn enrich_response(
        &self,
        mut response: ParticipantResponse,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        if let Some(includes) = include {
            // **OPTIMIZATION: Group similar data fetches to reduce database round trips**
            
            // Process include options
            let include_workshop_count = includes.contains(&ParticipantInclude::WorkshopCount) 
                || includes.contains(&ParticipantInclude::AllCounts) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_livelihood_count = includes.contains(&ParticipantInclude::LivelihoodCount) 
                || includes.contains(&ParticipantInclude::AllCounts) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_active_livelihood_count = includes.contains(&ParticipantInclude::ActiveLivelihoodCount) 
                || includes.contains(&ParticipantInclude::AllCounts) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_completed_workshop_count = includes.contains(&ParticipantInclude::CompletedWorkshopCount) 
                || includes.contains(&ParticipantInclude::AllCounts) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_upcoming_workshop_count = includes.contains(&ParticipantInclude::UpcomingWorkshopCount) 
                || includes.contains(&ParticipantInclude::AllCounts) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_document_count = includes.contains(&ParticipantInclude::DocumentCount) 
                || includes.contains(&ParticipantInclude::AllCounts) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_document_counts_by_type = includes.contains(&ParticipantInclude::DocumentCountsByType) 
                || includes.contains(&ParticipantInclude::AllCounts) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_documents = includes.contains(&ParticipantInclude::Documents) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_workshops = includes.contains(&ParticipantInclude::Workshops) 
                || includes.contains(&ParticipantInclude::All);
                
            let include_livelihoods = includes.contains(&ParticipantInclude::Livelihoods) 
                || includes.contains(&ParticipantInclude::All);

            // **OPTIMIZATION: Eager field loading - fetch workshop data once if any workshop fields needed**
            if include_workshop_count || include_completed_workshop_count || include_upcoming_workshop_count || include_workshops {
                if let Ok((total, completed, upcoming)) = self.repo.count_participant_workshops(response.id).await {
                    if include_workshop_count {
                        response.workshop_count = Some(total);
                    }
                    if include_completed_workshop_count {
                        response.completed_workshop_count = Some(completed);
                    }
                    if include_upcoming_workshop_count {
                        response.upcoming_workshop_count = Some(upcoming);
                    }
                }
                
                // Fetch full workshops if needed
                if include_workshops {
                    if let Ok(workshops) = self.repo.get_participant_workshops(response.id).await {
                        response.workshops = Some(workshops);
                    }
                }
            }

            // **OPTIMIZATION: Eager field loading - fetch livelihood data once if any livelihood fields needed**  
            if include_livelihood_count || include_active_livelihood_count || include_livelihoods {
                if let Ok((total, active)) = self.repo.count_participant_livelihoods(response.id).await {
                    if include_livelihood_count {
                        response.livelihood_count = Some(total);
                    }
                    if include_active_livelihood_count {
                        response.active_livelihood_count = Some(active);
                    }
                }
                
                // Fetch full livelihoods if needed
                if include_livelihoods {
                    if let Ok(livelihoods) = self.repo.get_participant_livelihoods(response.id).await {
                        response.livelihoods = Some(livelihoods);
                    }
                }
            }

            // **OPTIMIZATION: Eager field loading - fetch document data efficiently**
            if include_document_count || include_document_counts_by_type || include_documents {
                // Fetch document count if needed
                if include_document_count {
                    if let Ok(count) = self.repo.count_participant_documents(response.id).await {
                        response.document_count = Some(count);
                    }
                }

                // Fetch document counts by type if needed
                if include_document_counts_by_type {
                    if let Ok(counts) = self.repo.get_participant_document_counts_by_type(response.id).await {
                        response.document_counts_by_type = Some(counts);
                    }
                }

                // Fetch full documents if needed
                if include_documents {
                    let doc_params = PaginationParams::default();
                    if let Ok(docs_result) = self.document_service
                        .list_media_documents_by_related_entity(
                            auth,
                            "participants",
                            response.id,
                            doc_params,
                            None
                        ).await {
                        response.documents = Some(docs_result.items);
                    }
                }
            }
        }
        
        Ok(response)
    }
    
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
                filename.clone(),
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
    
    /// Comprehensive business rule validation for new participants
    async fn validate_participant_business_rules(
        &self,
        new_participant: &NewParticipant,
        _current_state: Option<&Participant>,
    ) -> ServiceResult<()> {
        // 1. Check for duplicate names (business rule)
        if let Some(existing_participant) = self.repo.find_by_name_case_insensitive(&new_participant.name).await.ok() {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "name",
                &format!("A participant with the name '{}' already exists. Please use a different name or check if this is a duplicate entry.", new_participant.name)
            ))));
        }
        
        // 2. Validate disability consistency
        if let Some(disability_type) = &new_participant.disability_type {
            if new_participant.disability == Some(false) {
                return Err(ServiceError::Domain(DomainError::Validation(ValidationError::custom(
                    "Cannot specify a disability type when disability is set to false. Either set disability to true or remove the disability type."
                ))));
            }
        }
        
        // 3. Check name quality (business rule for data integrity)
        if new_participant.name.len() < 3 {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "name",
                "Participant name is too short. Please provide a full name for better identification."
            ))));
        }
        
        // 4. Validate age group and disability type consistency (business logic)
        if let (Some(age_group), Some(disability_type)) = (&new_participant.age_group, &new_participant.disability_type) {
            if age_group == "child" && disability_type.to_lowercase().contains("psychosocial") {
                return Err(ServiceError::Domain(DomainError::Validation(ValidationError::custom(
                    "Psychosocial disability type requires careful assessment for child participants. Please consult with a specialist before proceeding."
                ))));
            }
        }
        
        Ok(())
    }
    
    /// Comprehensive business rule validation for participant updates
    async fn validate_participant_business_rules_for_update(
        &self,
        update_data: &UpdateParticipant,
        current_participant: &Participant,
    ) -> ServiceResult<()> {
        // 1. Check for duplicate names if name is being changed
        if let Some(new_name) = &update_data.name {
            if new_name != &current_participant.name {
                if let Some(existing_participant) = self.repo.find_by_name_case_insensitive(new_name).await.ok() {
                    if existing_participant.id != current_participant.id {
                        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                            "name",
                            &format!("A participant with the name '{}' already exists. Please use a different name.", new_name)
                        ))));
                    }
                }
            }
        }
        
        // 2. Validate disability consistency with current and new state
        let final_disability = update_data.disability.unwrap_or(current_participant.disability);
        let final_disability_type = update_data.disability_type.as_ref()
            .or(current_participant.disability_type.as_ref());
            
        if final_disability_type.is_some() && !final_disability {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::custom(
                "Cannot have a disability type when disability is false. Either set disability to true or remove the disability type."
            ))));
        }
        
        // 3. Business rule: Disability consistency validation
        if update_data.disability == Some(false) && current_participant.disability {
            // Log this significant change for audit purposes
            eprintln!(
                "WARNING: Participant {} disability status changed from true to false by user {}. Consider reviewing associated documents if any.",
                current_participant.id,
                update_data.updated_by_user_id
            );
            // Note: In the future, we can add more sophisticated document field validation here
        }
        
        // 4. Validate sync priority changes (business rule)
        if update_data.sync_priority == Some(SyncPriority::Never) {
            // Log warning for Never priority as it prevents synchronization
            eprintln!(
                "WARNING: Participant {} sync priority changed to Never by user {}. This will prevent synchronization.",
                current_participant.id,
                update_data.updated_by_user_id
            );
        } else if update_data.sync_priority == Some(SyncPriority::High) {
            // Log info for High priority changes for audit purposes
            println!(
                "INFO: Participant {} sync priority changed to High by user {}",
                current_participant.id,
                update_data.updated_by_user_id
            );
        }
        
        Ok(())
    }
    
    /// Comprehensive document upload validation
    async fn validate_document_upload(
        &self,
        file_data: &[u8],
        filename: &str,
        linked_field: &Option<String>,
    ) -> ServiceResult<()> {
        // 1. File size validation (business rule)
        const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50MB
        if file_data.len() > MAX_FILE_SIZE {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "file_size",
                &format!("File size ({:.2} MB) exceeds maximum allowed size of 50 MB. Please compress or reduce the file size.", 
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
        
        if filename.len() > 255 {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "filename",
                &format!("Filename is too long ({} characters). Maximum allowed is 255 characters.", filename.len())
            ))));
        }
        
        // 4. File extension validation
        let extension = filename.split('.').last().unwrap_or("").to_lowercase();
        let allowed_extensions = ["pdf", "jpg", "jpeg", "png", "doc", "docx", "txt", "rtf"];
        if !allowed_extensions.contains(&extension.as_str()) {
            return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                "file_extension",
                &format!("File type '.{}' is not allowed. Supported formats: {}", 
                    extension, allowed_extensions.join(", "))
            ))));
        }
        
        // 5. Basic file content validation (prevent malicious uploads)
        if extension == "pdf" {
            // Check for PDF magic number
            if file_data.len() >= 4 && &file_data[0..4] != b"%PDF" {
                return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                    "file_content",
                    "File appears to be corrupted or is not a valid PDF. Please check the file and try again."
                ))));
            }
        }
        
        // 6. Linked field specific validation
        if let Some(field) = linked_field {
            match field.as_str() {
                "profile_photo" => {
                    // Profile photos should be images
                    if !["jpg", "jpeg", "png"].contains(&extension.as_str()) {
                        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                            "file_extension",
                            "Profile photos must be image files (jpg, jpeg, png)."
                        ))));
                    }
                    
                    // Size limit for profile photos
                    const MAX_PHOTO_SIZE: usize = 5 * 1024 * 1024; // 5MB
                    if file_data.len() > MAX_PHOTO_SIZE {
                        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                            "file_size",
                            "Profile photo size exceeds 5 MB limit. Please use a smaller image."
                        ))));
                    }
                }
                "identification" => {
                    // Identification documents should be PDF or images
                    if !["pdf", "jpg", "jpeg", "png"].contains(&extension.as_str()) {
                        return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                            "file_extension",
                            "Identification documents must be PDF or image files (pdf, jpg, jpeg, png)."
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

// Implement DeleteService<Participant> by delegating
#[async_trait]
impl DeleteService<Participant> for ParticipantServiceImpl {
    fn repository(&self) -> &dyn FindById<Participant> {
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
    ) -> DomainResult<Vec<crate::domains::core::delete_service::FailedDeleteDetail<Participant>>> {
         self.delete_service.get_failed_delete_details(batch_result, auth).await
    }
}

#[async_trait]
impl ParticipantService for ParticipantServiceImpl {
    async fn create_participant(
        &self,
        new_participant: NewParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        // 1. Check permissions first
        auth.authorize(Permission::CreateParticipants)?;
        
        // 2. Enhanced input validation
        new_participant.validate().map_err(ServiceError::Domain)?;
        
        // 3. Additional business rule validation
        self.validate_participant_business_rules(&new_participant, None).await?;
        
        // 4. Create the participant
        let created_participant = self.repo.create(&new_participant, auth).await.map_err(ServiceError::Domain)?;
        let response = ParticipantResponse::from(created_participant);
        
        Ok(response)
    }

    async fn get_participant_by_id(
        &self,
        id: Uuid,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        if !auth.has_permission(Permission::ViewParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to view participants".to_string(),
            ));
        }

        let participant = self.repo.find_by_id(id).await?;
        let response = ParticipantResponse::from(participant);
        
        self.enrich_response(response, include, auth).await
    }

    async fn list_participants(
        &self,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        if !auth.has_permission(Permission::ViewParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to list participants".to_string(),
            ));
        }

        let paginated_result = self.repo.find_all(params).await?;

        let mut enriched_items = Vec::with_capacity(paginated_result.items.len());
        for item in paginated_result.items {
            let response = ParticipantResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?; 
            enriched_items.push(enriched);
        }

        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }

    async fn update_participant(
        &self,
        id: Uuid,
        mut update_data: UpdateParticipant,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantResponse> {
        // 1. Check permissions first
        auth.authorize(Permission::EditParticipants)?;
        
        // 2. Set updated_by_user_id
        update_data.updated_by_user_id = auth.user_id;
        
        // 3. Enhanced input validation
        update_data.validate().map_err(ServiceError::Domain)?;
        
        // 4. **OPTIMIZATION: Fetch current state BEFORE transaction for validation**
        let current_participant = self.repo.find_by_id(id).await.map_err(ServiceError::Domain)?;
        
        // 5. Enhanced business rule validation with current state
        self.validate_participant_business_rules_for_update(&update_data, &current_participant).await?;
        
        // 6. Proceed with update
        let updated_participant = self.repo.update(id, &update_data, auth).await.map_err(ServiceError::Domain)?;
        let response = ParticipantResponse::from(updated_participant);
        
        Ok(response)
    }
    
    async fn delete_participant(
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
                 "User does not have permission to {} participants",
                 if hard_delete { "hard delete" } else { "delete" }
             )));
        }
        
        // Fetch participant first to check existence
        let _ = self.repo.find_by_id(id).await?;

        let options = DeleteOptions {
            allow_hard_delete: hard_delete,
            // Note: Consider dependencies (workshops, livelihoods) - may need DependencyChecker
            // If hard delete is disallowed due to dependencies, it might error or soft delete instead.
            fallback_to_soft_delete: !hard_delete, 
            force: false, 
        };
        
        // Use the delete method inherited from DeleteService<Participant>
        let result = self.delete(id, auth, options).await?;
        Ok(result)
    }
    
    async fn find_participant_ids_by_filter(
        &self,
        filter: ParticipantFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>> {
        auth.authorize(Permission::ViewParticipants)?;
        filter.validate().map_err(ServiceError::Domain)?;
        self.repo.find_ids_by_filter(&filter).await.map_err(ServiceError::Domain)
    }
    
    async fn find_participants_by_filter(
        &self,
        filter: ParticipantFilter,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        auth.authorize(Permission::ViewParticipants)?;
        filter.validate().map_err(ServiceError::Domain)?;
        
        let paginated_result = self.repo.find_by_filter(&filter, params).await.map_err(ServiceError::Domain)?;
        let mut enriched_items = Vec::with_capacity(paginated_result.items.len());
        
        for item in paginated_result.items {
            let response = ParticipantResponse::from(item);
            let enriched = self.enrich_response(response, include, auth).await?;
            enriched_items.push(enriched);
        }
        
        Ok(PaginatedResult::new(
            enriched_items,
            paginated_result.total,
            params,
        ))
    }
    
    async fn bulk_update_sync_priority_by_filter(
        &self,
        filter: ParticipantFilter,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64> {
        auth.authorize(Permission::EditParticipants)?;
        filter.validate().map_err(ServiceError::Domain)?;
        self.repo.bulk_update_sync_priority_by_filter(&filter, priority, auth).await.map_err(ServiceError::Domain)
    }
    
    async fn bulk_update_participant_sync_priority(
        &self,
        ids: Vec<Uuid>,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> ServiceResult<u64> {
        // **OPTIMIZATION: Pre-validate permissions and data before repository call**
        auth.authorize(Permission::EditParticipants)?;
        
        if ids.is_empty() {
            return Ok(0);
        }

        // **OPTIMIZATION: Use repository bulk operation - exact pattern from project domain**
        let rows_affected = self.repo.update_sync_priority(&ids, priority, auth).await
            .map_err(ServiceError::Domain)?;

        Ok(rows_affected)
    }

    async fn upload_document_for_participant(
        &self,
        participant_id: Uuid,
        file_data: Vec<u8>,
        original_filename: String,
        title: Option<String>,
        document_type_id: Uuid,
        linked_field: Option<String>,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<MediaDocumentResponse> {
        // 1. **OPTIMIZATION: Verify participant exists BEFORE starting document operations**
        let _participant = self.repo.find_by_id(participant_id).await.map_err(ServiceError::Domain)?;

        // 2. Check permissions
        auth.authorize(Permission::UploadDocuments)?;

        // 3. **ENHANCED: Comprehensive document upload validation**
        self.validate_document_upload(&file_data, &original_filename, &linked_field).await?;

        // 4. Validate the linked field if specified
        if let Some(field) = &linked_field {
            let valid_fields = Participant::document_linkable_fields();
            if !valid_fields.contains(field) {
                let valid_fields_vec: Vec<String> = valid_fields.into_iter().collect();
                return Err(ServiceError::Domain(DomainError::Validation(ValidationError::format(
                    "linked_field",
                    &format!("Field '{}' does not support document attachments for participants. Valid fields: {}", 
                        field, valid_fields_vec.join(", ")
                    )
                ))));
            }
        }

        // 5. Delegate to document service
        let document = self.document_service.upload_document(
            auth,
            file_data,
            original_filename,
            title,
            document_type_id,
            participant_id,
            "participants".to_string(),
            linked_field,
            sync_priority,
            compression_priority,
            None,
        ).await?;

        Ok(document)
    }

    async fn bulk_upload_documents_for_participant(
        &self,
        participant_id: Uuid,
        files: Vec<(Vec<u8>, String)>,
        title: Option<String>,
        document_type_id: Uuid,
        sync_priority: SyncPriority,
        compression_priority: Option<CompressionPriority>,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<MediaDocumentResponse>> {
        // 1. Verify participant exists
        let _participant = self.repo.find_by_id(participant_id).await
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
            participant_id,
            "participants".to_string(),
            sync_priority,
            compression_priority,
            None,
        ).await?;

        Ok(documents)
    }
    
    async fn create_participant_with_documents(
        &self,
        new_participant: NewParticipant,
        documents: Vec<(Vec<u8>, String, Option<String>)>,
        document_type_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<(ParticipantResponse, Vec<Result<MediaDocumentResponse, ServiceError>>)> {
        // 1. Check Permissions
        if !auth.has_permission(Permission::CreateParticipants) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to create participants".to_string(),
            ));
        }
        
        if !documents.is_empty() && !auth.has_permission(Permission::UploadDocuments) {
            return Err(ServiceError::PermissionDenied(
                "User does not have permission to upload documents".to_string(),
            ));
        }

        // 2. Validate Input DTO
        new_participant.validate()?;
        
        // 3. Begin transaction for participant creation
        let mut tx = self.pool.begin().await
            .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 4. Create the participant first (within transaction)
        let created_participant = match self.repo.create_with_tx(&new_participant, auth, &mut tx).await {
            Ok(participant) => participant,
            Err(e) => {
                let _ = tx.rollback().await; // Rollback on error
                return Err(ServiceError::Domain(e));
            }
        };
        
        // 5. Commit transaction to ensure participant is created before attaching docs
        if let Err(e) = tx.commit().await {
             return Err(ServiceError::Domain(DomainError::Database(DbError::from(e))));
        }
        
        // 6. Now upload documents (outside transaction)
        let document_results = if !documents.is_empty() {
            self.upload_documents_for_entity(
                created_participant.id,
                "participants",
                documents,
                document_type_id,
                new_participant.sync_priority.unwrap_or(SyncPriority::Normal),
                None,
                auth,
            ).await
        } else {
            Vec::new()
        };
        
        // 7. Convert to Response DTO and return with document results
        let response = ParticipantResponse::from(created_participant);
        Ok((response, document_results))
    }

    async fn get_participant_demographics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantDemographics> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Get demographics from repository
        let demographics = self.repo.get_participant_demographics().await?;
        
        Ok(demographics)
    }
    
    async fn get_participant_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantDemographics> {
        // Alias for get_participant_demographics for API consistency with project domain
        self.get_participant_demographics(auth).await
    }
    
    async fn get_participant_engagement_analytics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let demographics = self.repo.get_participant_demographics().await?;
        
        let mut analytics = HashMap::new();
        
        // Engagement levels
        analytics.insert("high_engagement".to_string(), 
            demographics.participants_with_workshops.min(demographics.participants_with_livelihoods));
        analytics.insert("medium_engagement".to_string(), 
            (demographics.participants_with_workshops + demographics.participants_with_livelihoods) - 
            demographics.participants_with_workshops.min(demographics.participants_with_livelihoods));
        analytics.insert("low_engagement".to_string(), 
            demographics.participants_with_documents - demographics.participants_with_workshops - demographics.participants_with_livelihoods);
        analytics.insert("no_engagement".to_string(), 
            demographics.participants_with_no_engagement);
        
        // Workshop participation analysis
        let total_workshop_participants = demographics.participants_by_workshop_count.values().sum::<i64>();
        analytics.insert("multiple_workshops".to_string(), 
            demographics.participants_by_workshop_count.iter()
                .filter(|(count, _)| **count > 1)
                .map(|(_, participants)| participants)
                .sum());
        analytics.insert("single_workshop".to_string(), 
            demographics.participants_by_workshop_count.get(&1).copied().unwrap_or(0));
        
        Ok(analytics)
    }
    
    async fn get_participant_growth_trends(
        &self,
        months: Option<i32>,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let period_months = months.unwrap_or(12);
        let now = Utc::now();
        
        let mut trends = HashMap::new();
        
        // Calculate monthly growth for the specified period
        for i in 0..period_months {
            let month_start = now - chrono::Duration::days((i + 1) as i64 * 30);
            let month_end = now - chrono::Duration::days(i as i64 * 30);
            
            let month_key = month_start.format("%Y-%m").to_string();
            
            // Get count for this month (simplified - in production you'd want more precise date handling)
            let month_count = query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM participants WHERE created_at >= ? AND created_at < ?"
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
    
    async fn get_data_quality_report(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, f64>> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let demographics = self.repo.get_participant_demographics().await?;
        
        let mut quality_report = HashMap::new();
        
        // Overall completeness
        quality_report.insert("overall_completeness".to_string(), 
            demographics.data_completeness_percentage);
        
        // Individual field completeness
        if demographics.active_participants > 0 {
            let gender_completeness = 100.0 - 
                (demographics.participants_missing_gender as f64 / demographics.active_participants as f64 * 100.0);
            let age_group_completeness = 100.0 - 
                (demographics.participants_missing_age_group as f64 / demographics.active_participants as f64 * 100.0);
            let location_completeness = 100.0 - 
                (demographics.participants_missing_location as f64 / demographics.active_participants as f64 * 100.0);
            
            quality_report.insert("gender_completeness".to_string(), gender_completeness);
            quality_report.insert("age_group_completeness".to_string(), age_group_completeness);
            quality_report.insert("location_completeness".to_string(), location_completeness);
            
            // Engagement metrics as quality indicators
            let engagement_rate = (demographics.participants_with_workshops + 
                                 demographics.participants_with_livelihoods + 
                                 demographics.participants_with_documents) as f64 / 
                                demographics.active_participants as f64 * 100.0;
            quality_report.insert("engagement_rate".to_string(), engagement_rate.min(100.0));
        }
        
        Ok(quality_report)
    }
    

    
    async fn get_participant_with_document_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithDocumentTimeline> {
        // 1. Check permissions - matches project domain exactly
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewDocuments)?;

        // 2. Get the participant - matches project domain pattern  
        let participant = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
            
        let participant_response = ParticipantResponse::from(participant);
        
        // 3. Get all documents for this participant - exact pagination params from project
        let documents = self.document_service.list_media_documents_by_related_entity(
            auth,
            "participants",
            id,
            PaginationParams { page: 1, per_page: 100 }, // Use exact field names and values from project
            None,
        ).await?.items;
        
        // 4. Organize documents by month (participant-specific timeline approach)
        let mut documents_by_month: HashMap<String, Vec<MediaDocumentResponse>> = HashMap::new();
        let mut total_document_count = 0u64; // Exact type matching project domain
        
        for doc in documents {
            // Extract month from created_at timestamp
            let month_key: String = if let Ok(created_at) = DateTime::parse_from_rfc3339(&doc.created_at) {
                created_at.format("%Y-%m").to_string() // "YYYY-MM" format
            } else {
                "Unknown".to_string() // Fallback for invalid dates
            };
            
            documents_by_month
                .entry(month_key) // Using owned String is fine
                .or_insert_with(Vec::new)
                .push(doc);
            total_document_count += 1; // Increment as u64
        }
        
        // 5. Create and return combined response - exact return type from project domain
        Ok(ParticipantWithDocumentTimeline {
            participant: participant_response,
            documents_by_month,
            total_document_count, // Already u64 type
        })
    }
    
    async fn get_participant_with_documents_by_type(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithDocumentsByType> {
        // 1. Check permissions - matches project domain exactly
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewDocuments)?;

        // 2. Get the participant - matches project domain pattern  
        let participant = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
            
        let participant_response = ParticipantResponse::from(participant);
        
        // 3. Get all documents for this participant - exact pagination params from project
        let documents = self.document_service.list_media_documents_by_related_entity(
            auth,
            "participants",
            id,
            PaginationParams { page: 1, per_page: 100 }, // Exact values from project domain
            None,
        ).await?.items;
        
        // 4. Organize documents by type/category - exact pattern from project domain
        let mut documents_by_type: HashMap<String, Vec<MediaDocumentResponse>> = HashMap::new();
        let mut total_document_count = 0u64; // Exact type matching project domain
        
        for doc in documents {
            // Use field_identifier if available, otherwise use default category - exact logic from project
            let document_type: String = match &doc.field_identifier {
                Some(field) => field.clone(), // Clone here for owned String - exact from project
                None => "General".to_string(), // Also creates owned String - exact from project
            };
            
            documents_by_type
                .entry(document_type) // Using owned String is fine - exact from project
                .or_insert_with(Vec::new)
                .push(doc);
            total_document_count += 1; // Increment as u64 - exact from project
        }
        
        // 5. Create and return combined response - exact return type pattern
        Ok(ParticipantWithDocumentsByType {
            participant: participant_response,
            documents_by_type,
            total_document_count, // Already u64 type
        })
    }
    
    async fn get_participant_activity_timeline(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantActivityTimeline> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewWorkshops)?;
        auth.authorize(Permission::ViewDocuments)?;

        // 2. Get the participant
        let participant = self.repo.find_by_id(id).await
            .map_err(ServiceError::Domain)?;
        
        // 3. Get workshop activities
        let workshops = self.repo.get_participant_workshops(id).await
            .map_err(ServiceError::Domain)?;
        let workshop_participation: Vec<ParticipantWorkshopActivity> = workshops.into_iter()
            .map(|ws| ParticipantWorkshopActivity {
                workshop_id: ws.id,
                workshop_name: ws.name,
                participation_date: ws.date.and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now), // Fallback to now if date parsing fails
                pre_evaluation: ws.pre_evaluation,
                post_evaluation: ws.post_evaluation,
                completion_status: if ws.has_completed { "Completed".to_string() } else { "In Progress".to_string() },
            })
            .collect();
            
        // 4. Get livelihood activities  
        let livelihoods = self.repo.get_participant_livelihoods(id).await
            .map_err(ServiceError::Domain)?;
        let livelihood_progression: Vec<ParticipantLivelihoodActivity> = livelihoods.into_iter()
            .map(|lv| ParticipantLivelihoodActivity {
                livelihood_id: lv.id,
                livelihood_name: lv.name,
                start_date: lv.start_date.and_then(|d| DateTime::parse_from_rfc3339(&d).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now), // Fallback to now if date parsing fails
                status: lv.status.unwrap_or_else(|| "Unknown".to_string()),
                progression_milestones: vec![], // Could be enhanced to track actual milestones
            })
            .collect();
            
        // 5. Get document activities
        let documents = self.document_service.list_media_documents_by_related_entity(
            auth,
            "participants",
            id,
            PaginationParams { page: 1, per_page: 100 },
            None,
        ).await?.items;
        
        let document_uploads: Vec<ParticipantDocumentActivity> = documents.into_iter()
            .map(|doc| ParticipantDocumentActivity {
                document_id: doc.id,
                document_name: doc.original_filename.clone(),
                upload_date: DateTime::parse_from_rfc3339(&doc.created_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now()), // Fallback to now if date parsing fails
                document_type: doc.type_name.unwrap_or_else(|| "Unknown".to_string()),
                linked_field: doc.field_identifier,
            })
            .collect();
            
        // 6. Calculate engagement score (simple scoring algorithm)
        let engagement_score = (workshop_participation.len() as f64 * 0.4) + 
                             (livelihood_progression.len() as f64 * 0.4) + 
                             (document_uploads.len() as f64 * 0.2);
                             
        // 7. Find last activity date
        let mut last_dates = Vec::new();
        if let Some(last_workshop) = workshop_participation.last() {
            last_dates.push(last_workshop.participation_date);
        }
        if let Some(last_livelihood) = livelihood_progression.last() {
            last_dates.push(last_livelihood.start_date);
        }
        if let Some(last_document) = document_uploads.last() {
            last_dates.push(last_document.upload_date);
        }
        let last_activity_date = last_dates.into_iter().max();
        
        // 8. Return activity timeline
        Ok(ParticipantActivityTimeline {
            participant_id: participant.id,
            participant_name: participant.name,
            workshop_participation,
            livelihood_progression,
            document_uploads,
            engagement_score,
            last_activity_date,
        })
    }

    async fn get_gender_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Get gender counts from repository
        let gender_counts = self.repo.count_by_gender().await?;

        // 3. Convert to a more user-friendly HashMap
        let mut distribution = HashMap::new();
        for (gender_opt, count) in gender_counts {
            let gender_name = match gender_opt {
                Some(g) => g,
                None => "Unspecified".to_string(),
            };
            distribution.insert(gender_name, count);
        }

        Ok(distribution)
    }

    async fn get_age_group_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Get age group counts from repository
        let age_group_counts = self.repo.count_by_age_group().await?;

        // 3. Convert to a more user-friendly HashMap
        let mut distribution = HashMap::new();
        for (age_group_opt, count) in age_group_counts {
            let age_group_name = match age_group_opt {
                Some(a) => a,
                None => "Unspecified".to_string(),
            };
            distribution.insert(age_group_name, count);
        }

        Ok(distribution)
    }

    async fn get_location_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Get location counts from repository
        let location_counts = self.repo.count_by_location().await?;

        // 3. Convert to a more user-friendly HashMap
        let mut distribution = HashMap::new();
        for (location_opt, count) in location_counts {
            let location_name = match location_opt {
                Some(l) => l,
                None => "Unspecified".to_string(),
            };
            distribution.insert(location_name, count);
        }

        Ok(distribution)
    }

    async fn get_disability_distribution(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<HashMap<String, i64>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Get disability counts from repository
        let disability_counts = self.repo.count_by_disability().await?;

        // 3. Convert to a more user-friendly HashMap
        let mut distribution = HashMap::new();
        for (has_disability, count) in disability_counts {
            let disability_name = if has_disability { "Yes" } else { "No" }.to_string();
            distribution.insert(disability_name, count);
        }

        Ok(distribution)
    }

    async fn get_available_disability_types(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<String>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Get available disability types from repository
        let available_types = self.repo.get_available_disability_types().await?;

        Ok(available_types)
    }

    async fn find_participants_by_gender(
        &self,
        gender: &str,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Find participants by gender
        let paginated_result = self.repo.find_by_gender(gender, params).await?;

        // 3. Convert and enrich participants
        let mut enriched_items = Vec::new();
        for participant in paginated_result.items {
            let response = ParticipantResponse::from(participant);
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

    async fn find_participants_by_age_group(
        &self,
        age_group: &str,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Find participants by age group
        let paginated_result = self.repo.find_by_age_group(age_group, params).await?;

        // 3. Convert and enrich participants
        let mut enriched_items = Vec::new();
        for participant in paginated_result.items {
            let response = ParticipantResponse::from(participant);
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

    async fn find_participants_by_location(
        &self,
        location: &str,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Find participants by location
        let paginated_result = self.repo.find_by_location(location, params).await?;

        // 3. Convert and enrich participants
        let mut enriched_items = Vec::new();
        for participant in paginated_result.items {
            let response = ParticipantResponse::from(participant);
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

    async fn find_participants_by_disability(
        &self,
        has_disability: bool,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;

        // 2. Find participants by disability status
        let paginated_result = self.repo.find_by_disability(has_disability, params).await?;

        // 3. Convert and enrich participants
        let mut enriched_items = Vec::new();
        for participant in paginated_result.items {
            let response = ParticipantResponse::from(participant);
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

    async fn get_workshop_participants(
        &self,
        workshop_id: Uuid,
        params: PaginationParams,
        include: Option<&[ParticipantInclude]>,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Find participants for this workshop
        let paginated_result = self.repo.find_workshop_participants(workshop_id, params).await?;

        // 3. Convert and enrich participants
        let mut enriched_items = Vec::new();
        for participant in paginated_result.items {
            let response = ParticipantResponse::from(participant);
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

    async fn get_participant_with_workshops(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithWorkshops> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewWorkshops)?;

        // 2. Get participant
        let participant = self.repo.find_by_id(id).await?;
        let participant_response = ParticipantResponse::from(participant);
        
        // 3. Get workshops for participant
        let workshops = self.repo.get_participant_workshops(id).await?;
        
        // 4. Get workshop counts
        let (total_workshops, completed_workshops, upcoming_workshops) = 
            self.repo.count_participant_workshops(id).await?;
        
        // 5. Create and return combined response
        Ok(ParticipantWithWorkshops {
            participant: participant_response,
            workshops,
            total_workshops,
            completed_workshops,
            upcoming_workshops,
        })
    }

    async fn get_participant_with_livelihoods(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithLivelihoods> {
        // 1. Check permissions
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewLivelihoods)?;

        // 2. Get participant
        let participant = self.repo.find_by_id(id).await?;
        let participant_response = ParticipantResponse::from(participant);
        
        // 3. Get livelihoods for participant
        let livelihoods = self.repo.get_participant_livelihoods(id).await?;
        
        // 4. Get livelihood counts
        let (total_livelihoods, active_livelihoods) = 
            self.repo.count_participant_livelihoods(id).await?;
        
        // 5. Create and return combined response
        Ok(ParticipantWithLivelihoods {
            participant: participant_response,
            livelihoods,
            total_livelihoods,
            active_livelihoods,
        })
    }

    // ---------------------------------------------------------------------------
    // ENTERPRISE ADVANCED FEATURES IMPLEMENTATION
    // ---------------------------------------------------------------------------

    async fn search_participants_with_relationships(
        &self,
        search_text: &str,
        params: PaginationParams,
        auth: &AuthContext,
    ) -> ServiceResult<PaginatedResult<ParticipantResponse>> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let paginated_result = self.repo.search_participants_with_relationships(search_text, params).await?;
        
        // Convert to responses
        let participant_responses: Vec<ParticipantResponse> = paginated_result.items
            .into_iter()
            .map(ParticipantResponse::from)
            .collect();
        
        Ok(PaginatedResult::new(
            participant_responses,
            paginated_result.total,
            params,
        ))
    }

    async fn get_participant_with_enrichment(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantWithEnrichment> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let enriched = self.repo.get_participant_with_enrichment(id).await?;
        Ok(enriched)
    }

    async fn get_comprehensive_statistics(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantStatistics> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let statistics = self.repo.get_participant_statistics().await?;
        Ok(statistics)
    }

    async fn get_participant_document_references(
        &self,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<ParticipantDocumentReference>> {
        auth.authorize(Permission::ViewParticipants)?;
        auth.authorize(Permission::ViewDocuments)?;
        
        let document_refs = self.repo.get_participant_document_references(participant_id).await?;
        Ok(document_refs)
    }

    async fn bulk_update_participants_streaming(
        &self,
        updates: Vec<(Uuid, UpdateParticipant)>,
        chunk_size: usize,
        auth: &AuthContext,
    ) -> ServiceResult<ParticipantBulkOperationResult> {
        auth.authorize(Permission::EditParticipants)?;
        
        // Note: The repository handles chunking internally, chunk_size is handled at FFI level for batching requests
        let bulk_result = self.repo.bulk_update_participants_streaming(updates, auth).await?;
        Ok(bulk_result)
    }

    async fn get_index_optimization_suggestions(
        &self,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<String>> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let suggestions = self.repo.get_index_optimization_suggestions().await?;
        Ok(suggestions)
    }

    async fn find_participant_ids_by_filter_optimized(
        &self,
        filter: ParticipantFilter,
        auth: &AuthContext,
    ) -> ServiceResult<Vec<Uuid>> {
        auth.authorize(Permission::ViewParticipants)?;
        
        let ids = self.repo.find_ids_by_filter_optimized(&filter).await?;
        Ok(ids)
    }

}
