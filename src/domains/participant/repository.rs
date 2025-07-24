use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, QueryBuilder};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::participant::types::{
    NewParticipant, Participant, ParticipantRow, UpdateParticipant, ParticipantDemographics, 
    WorkshopSummary, LivelihoodSummary, ParticipantFilter, ParticipantDocumentReference, ParticipantWithEnrichment
};
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::validation::{common};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::{Utc, Local, DateTime};
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;
use crate::domains::sync::types::SyncPriority;
use std::collections::HashMap;
use std::sync::Arc;
use serde_json;
use std::str::FromStr;
use crate::domains::user::repository::MergeableEntityRepository;
/// Trait defining participant repository operations
#[async_trait]
pub trait ParticipantRepository: DeleteServiceRepository<Participant> + MergeableEntityRepository<Participant> + Send + Sync {
    async fn create(
        &self,
        new_participant: &NewParticipant,
        auth: &AuthContext,
    ) -> DomainResult<Participant>;
    async fn create_with_tx<'t>(
        &self,
        new_participant: &NewParticipant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Participant>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateParticipant,
        auth: &AuthContext,
    ) -> DomainResult<Participant>;
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateParticipant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Participant>;

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Find participants by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Find participant IDs by complex filter criteria - enables bulk operations
    async fn find_ids_by_filter(
        &self,
        filter: &ParticipantFilter,
    ) -> DomainResult<Vec<Uuid>>;
    
    /// Find participants by complex filter criteria with pagination
    async fn find_by_filter(
        &self,
        filter: &ParticipantFilter,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Bulk update sync priority for participants matching filter criteria
    async fn bulk_update_sync_priority_by_filter(
        &self,
        filter: &ParticipantFilter,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;
    
    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;
    
    /// Count participants by gender
    async fn count_by_gender(&self) -> DomainResult<Vec<(Option<String>, i64)>>;
    
    /// Count participants by age group
    async fn count_by_age_group(&self) -> DomainResult<Vec<(Option<String>, i64)>>;
    
    /// Count participants by location
    async fn count_by_location(&self) -> DomainResult<Vec<(Option<String>, i64)>>;
    
    /// Count participants by disability status
    async fn count_by_disability(&self) -> DomainResult<Vec<(bool, i64)>>;
    
    /// Count participants by disability type
    async fn count_by_disability_type(&self) -> DomainResult<Vec<(Option<String>, i64)>>;
    
    /// Get comprehensive participant demographics
    async fn get_participant_demographics(&self) -> DomainResult<ParticipantDemographics>;
    
    /// Get all available disability types for UI filtering
    async fn get_available_disability_types(&self) -> DomainResult<Vec<String>>;
    
    /// Find participants by gender
    async fn find_by_gender(
        &self,
        gender: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Find participants by age group
    async fn find_by_age_group(
        &self,
        age_group: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Find participants by location
    async fn find_by_location(
        &self,
        location: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Find participants by disability status
    async fn find_by_disability(
        &self,
        has_disability: bool,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Find participants by disability type
    async fn find_by_disability_type(
        &self,
        disability_type: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Get workshop participants for a specific workshop
    async fn find_workshop_participants(
        &self,
        workshop_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Get workshops for a participant
    async fn get_participant_workshops(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<Vec<WorkshopSummary>>;
    
    /// Get livelihoods for a participant
    async fn get_participant_livelihoods(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<Vec<LivelihoodSummary>>;
    
    /// Count workshops for a participant
    async fn count_participant_workshops(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<(i64, i64, i64)>; // (total, completed, upcoming)
    
    /// Count livelihoods for a participant
    async fn count_participant_livelihoods(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<(i64, i64)>; // (total, active)
    
    /// Get document counts by type for a participant
    async fn get_participant_document_counts_by_type(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<HashMap<String, i64>>;
    
    /// Count total documents for a participant
    async fn count_participant_documents(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<i64>;
    


    /// Find participants within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
    
    /// Find participant by name (case insensitive) - used for duplicate checking
    async fn find_by_name_case_insensitive(
        &self,
        name: &str,
    ) -> DomainResult<Participant>;

    /// Find all participants by name (case insensitive) - used for duplicate detection
    async fn find_all_by_name_case_insensitive(
        &self,
        name: &str,
    ) -> DomainResult<Vec<Participant>>;

    /// **ADVANCED QUERY: Get participant document references with JOIN optimization**
    /// Matches project domain's get_project_document_references pattern exactly
    async fn get_participant_document_references(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<Vec<crate::domains::participant::types::ParticipantDocumentReference>>;
    
    /// **ADVANCED QUERY: Get participant with enriched relationship data using JOINs**
    /// Optimized query that fetches participant + related counts in minimal DB calls
    async fn get_participant_with_enrichment(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<crate::domains::participant::types::ParticipantWithEnrichment>;
    
    /// **ADVANCED QUERY: Search participants with relationship JOIN optimization**
    /// Enhanced search that includes related entity matching for comprehensive results
    async fn search_participants_with_relationships(
        &self,
        search_query: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;

    /// **BATCH PROCESSING: Memory-efficient participant statistics computation**
    /// Cache-friendly aggregation that processes large datasets efficiently
    async fn get_participant_statistics(&self) -> DomainResult<crate::domains::participant::types::ParticipantStatistics>;
    
    /// **BATCH PROCESSING: Efficient bulk update with streaming and memory optimization**
    /// Processes large batches without loading all data into memory at once
    async fn bulk_update_participants_streaming(
        &self,
        updates: Vec<(Uuid, UpdateParticipant)>,
        auth: &AuthContext,
    ) -> DomainResult<crate::domains::participant::types::ParticipantBulkOperationResult>;
    
    /// **PERFORMANCE ANALYSIS: Get database index suggestions for participant queries**
    /// Analyzes query patterns and suggests optimal indexes for performance
    async fn get_index_optimization_suggestions(&self) -> DomainResult<Vec<String>>;
    
    /// **PERFORMANCE: Optimized filter query with compound index utilization**
    /// Enhanced version of find_ids_by_filter that leverages compound indexes for maximum performance
    async fn find_ids_by_filter_optimized(
        &self,
        filter: &ParticipantFilter,
    ) -> DomainResult<Vec<Uuid>>;
}

/// SQLite implementation for ParticipantRepository
#[derive(Clone)]
pub struct SqliteParticipantRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteParticipantRepository {
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }

    fn map_row_to_entity(row: ParticipantRow) -> DomainResult<Participant> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "participants"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Participant> {
        let row = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Transaction find error for {}: {}", id, e);
            DbError::from(e)
        })?
        .ok_or_else(|| DomainError::EntityNotFound("Participant".to_string(), id))?;

        Self::map_row_to_entity(row)
    }

    // Helper to log changes consistently
    async fn log_change_entry<'t>(
        &self,
        entry: ChangeLogEntry,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await
    }
}

#[async_trait]
impl FindById<Participant> for SqliteParticipantRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Participant> {
        println!("🔍 [PARTICIPANT_REPO] Finding participant by ID: {}", id);
        
        let row = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Database error finding participant {}: {}", id, e);
            DbError::from(e)
        })?
        .ok_or_else(|| {
            println!("🚨 [PARTICIPANT_REPO] Participant {} not found", id);
            DomainError::EntityNotFound("Participant".to_string(), id)
        })?;

        let participant = Self::map_row_to_entity(row)?;
        println!("✅ [PARTICIPANT_REPO] Found participant: {}", participant.name);
        Ok(participant)
    }
}

#[async_trait]
impl SoftDeletable for SqliteParticipantRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let deleted_by = auth.user_id;
        let deleted_by_str = deleted_by.to_string();
        let deleted_by_device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        
        println!("🗑️ [PARTICIPANT_REPO] Soft deleting participant {}", id);
        
        let result = query(
            "UPDATE participants SET deleted_at = ?, deleted_by_user_id = ?, deleted_by_device_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now_str)
        .bind(deleted_by_str)
        .bind(deleted_by_device_id_str)
        .bind(id.to_string())
        .execute(&mut **tx) 
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            println!("🚨 [PARTICIPANT_REPO] Soft delete failed - participant {} not found or already deleted", id);
            Err(DomainError::EntityNotFound("Participant".to_string(), id))
        } else {
            println!("✅ [PARTICIPANT_REPO] Soft deleted participant {}", id);
            Ok(())
        }
    }

    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.soft_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(())
            }
            Err(e) => {
                println!("🚨 [PARTICIPANT_REPO] Soft delete transaction failed for {}: {}", id, e);
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteParticipantRepository {
    fn entity_name(&self) -> &'static str {
        "participants"
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, // Auth context for future logging/auditing
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        println!("💀 [PARTICIPANT_REPO] Hard deleting participant {}", id);
        
        // **ROBUST: Hard delete with cascade considerations**
        // Note: Any related document cleanup should be handled by calling service
        let result = query("DELETE FROM participants WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            println!("🚨 [PARTICIPANT_REPO] Hard delete failed - participant {} not found", id);
            Err(DomainError::EntityNotFound("Participant".to_string(), id))
        } else {
            println!("✅ [PARTICIPANT_REPO] Hard deleted participant {}", id);
            Ok(())
        }
    }

    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.hard_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(())
            }
            Err(e) => {
                println!("🚨 [PARTICIPANT_REPO] Hard delete transaction failed for {}: {}", id, e);
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
}

// Blanket implementation in core::delete_service handles DeleteServiceRepository

#[async_trait]
impl ParticipantRepository for SqliteParticipantRepository {
    async fn create(
        &self,
        new_participant: &NewParticipant,
        auth: &AuthContext,
    ) -> DomainResult<Participant> {
        let max_retries = 3;
        let mut retry_count = 0;
        
        loop {
            let mut tx = self.pool.begin().await.map_err(DbError::from)?;
            let result = self.create_with_tx(new_participant, auth, &mut tx).await;
            
            match result {
                Ok(participant) => {
                    match tx.commit().await {
                        Ok(_) => return Ok(participant),
                        Err(commit_err) => {
                            let error_str = commit_err.to_string();
                            if error_str.contains("database is locked") && retry_count < max_retries {
                                retry_count += 1;
                                println!("⚠️ [PARTICIPANT_REPO] Database locked during commit for '{}', retrying ({}/{})", 
                                         new_participant.name, retry_count, max_retries);
                                tokio::time::sleep(tokio::time::Duration::from_millis(100 * retry_count)).await;
                                continue;
                            } else {
                                return Err(DbError::from(commit_err).into());
                            }
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.rollback().await; // Best effort rollback
                    
                    // Check if it's a database lock error and we can retry
                    let error_str = e.to_string();
                    if error_str.contains("database is locked") && retry_count < max_retries {
                        retry_count += 1;
                        println!("⚠️ [PARTICIPANT_REPO] Database locked during creation for '{}', retrying ({}/{})", 
                                 new_participant.name, retry_count, max_retries);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100 * retry_count)).await;
                        continue;
                    } else {
                        // Log the error before returning
                        println!("🚨 [PARTICIPANT_REPO] Creation failed for '{}': {}", new_participant.name, e);
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_participant: &NewParticipant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Participant> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let created_by_id_str = new_participant.created_by_user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| user_id_str.clone());
        let id_str = id.to_string();

        // **ENHANCED: Pre-validate business constraints to provide better error messages**
        // Note: Removed strict name validation to allow duplicate names with smart duplicate detection
        // The frontend will handle duplicate detection and user choice

        // **ROBUST: Comprehensive insert with detailed error handling**
        let insert_result = query(
            r#"INSERT INTO participants (
                id, name, name_updated_at, name_updated_by, name_updated_by_device_id,
                gender, gender_updated_at, gender_updated_by, gender_updated_by_device_id,
                disability, disability_updated_at, disability_updated_by, disability_updated_by_device_id,
                disability_type, disability_type_updated_at, disability_type_updated_by, disability_type_updated_by_device_id,
                age_group, age_group_updated_at, age_group_updated_by, age_group_updated_by_device_id,
                location, location_updated_at, location_updated_by, location_updated_by_device_id,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#
        )
        .bind(&id_str)
        .bind(&new_participant.name)
        .bind(&now_str).bind(&user_id_str).bind(device_id_str.as_ref()) // Name LWW
        .bind(&new_participant.gender)
        .bind(new_participant.gender.as_ref().map(|_| &now_str)).bind(new_participant.gender.as_ref().map(|_| &user_id_str)).bind(new_participant.gender.as_ref().map(|_| device_id_str.as_ref())) // Gender LWW
        .bind(new_participant.disability.unwrap_or(false))
        .bind(new_participant.disability.map(|_| &now_str)).bind(new_participant.disability.map(|_| &user_id_str)).bind(new_participant.disability.map(|_| device_id_str.as_ref())) // Disability LWW
        .bind(&new_participant.disability_type)
        .bind(new_participant.disability_type.as_ref().map(|_| &now_str)).bind(new_participant.disability_type.as_ref().map(|_| &user_id_str)).bind(new_participant.disability_type.as_ref().map(|_| device_id_str.as_ref())) // Disability Type LWW
        .bind(&new_participant.age_group)
        .bind(new_participant.age_group.as_ref().map(|_| &now_str)).bind(new_participant.age_group.as_ref().map(|_| &user_id_str)).bind(new_participant.age_group.as_ref().map(|_| device_id_str.as_ref())) // Age Group LWW
        .bind(&new_participant.location)
        .bind(new_participant.location.as_ref().map(|_| &now_str)).bind(new_participant.location.as_ref().map(|_| &user_id_str)).bind(new_participant.location.as_ref().map(|_| device_id_str.as_ref())) // Location LWW
        .bind(new_participant.sync_priority.unwrap_or_default().as_str()) // sync_priority as TEXT
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&created_by_id_str).bind(&user_id_str) // created_by, updated_by
        .bind(device_id_str.as_ref()).bind(device_id_str.as_ref()) // created_by_device_id, updated_by_device_id
        .execute(&mut **tx)
        .await;

        // **ENHANCED: Detailed error analysis for better user feedback**
        match insert_result {
            Ok(_) => {
                println!("✅ [PARTICIPANT_REPO] Created participant '{}' with ID {}", new_participant.name, id);
            }
            Err(sqlx::Error::Database(db_err)) => {
                // **ROBUST: Translate database errors to meaningful user messages**
                let error_msg = if let Some(constraint) = db_err.constraint() {
                    match constraint {
                        "participants_name_unique" => {
                            format!("A participant with the name '{}' already exists", new_participant.name)
                        }
                        "participants_created_by_user_id_fkey" => {
                            "Invalid user ID: The specified user does not exist".to_string()
                        }
                        _ => {
                            format!("Database constraint violation: {}", constraint)
                        }
                    }
                } else {
                    format!("Database error during participant creation: {}", db_err.message())
                };
                
                println!("🚨 [PARTICIPANT_REPO] Database error for '{}': {}", new_participant.name, error_msg);
                
                return Err(DomainError::Internal(error_msg));
            }
            Err(e) => {
                println!("🚨 [PARTICIPANT_REPO] Unexpected error for '{}': {}", new_participant.name, e);
                return Err(DomainError::Database(DbError::from(e)));
            }
        }

        // **ROBUST: Log creation with comprehensive change tracking**
        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: self.entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create,
            field_name: None,
            old_value: None,
            new_value: serde_json::to_string(new_participant).ok(),
            timestamp: now,
            user_id: user_id,
            device_id: device_uuid,
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        
        self.log_change_entry(entry, tx).await?;

        // **ROBUST: Fetch the created participant with comprehensive error handling**
        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateParticipant,
        auth: &AuthContext,
    ) -> DomainResult<Participant> {
        // **OPTIMIZATION: Pre-validate business constraints BEFORE starting transaction**
        // Note: Removed strict name validation to allow duplicate names with smart duplicate detection
        // The frontend will handle duplicate detection and user choice

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(participant) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(participant)
            }
            Err(e) => {
                println!("🚨 [PARTICIPANT_REPO] Update failed for participant {}: {}", id, e);
                let _ = tx.rollback().await; // Best effort rollback
                Err(e)
            }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateParticipant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Participant> {
        // **ROBUST: Fetch old state for change tracking and validation**
        let old_entity = self.find_by_id_with_tx(id, tx).await?;
        
        // **CONCURRENCY: Check for name duplicates within transaction to avoid database locks**
        if let Some(new_name) = &update_data.name {
            if new_name != &old_entity.name {
                // Use the same transaction to avoid lock conflicts
                let existing_participant = query_as::<_, (String, String)>(
                    "SELECT id, name FROM participants WHERE LOWER(name) = LOWER(?) AND deleted_at IS NULL AND id != ?"
                )
                .bind(new_name)
                .bind(id.to_string())
                .fetch_optional(&mut **tx)
                .await
                .map_err(DbError::from)?;
                
                if let Some((existing_id_str, existing_name)) = existing_participant {
                    println!("ℹ️ [PARTICIPANT_REPO] Found existing participant '{}' (ID: {}) with same name as update target. Allowing update but logging for potential duplicate detection.", 
                             existing_name, existing_id_str);
                }
            }
        }
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = update_data.updated_by_user_id;
        let user_id_str = user_id.to_string();
        let id_str = id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());

        let mut builder = QueryBuilder::new("UPDATE participants SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        // **OPTIMIZATION: Last Write Wins macro for efficient field updates**
        macro_rules! add_lww {
            ($field_name:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                    separated.push_bind_unseparated(device_id_str.clone());
                    fields_updated = true;
                }
            };
        }
        
        // **ENHANCED: Special handling for disability boolean field**
        if let Some(val) = update_data.disability {
            separated.push("disability = ");
            separated.push_bind_unseparated(val);
            separated.push(" disability_updated_at = ");
            separated.push_bind_unseparated(now_str.clone());
            separated.push(" disability_updated_by = ");
            separated.push_bind_unseparated(user_id_str.clone());
            separated.push(" disability_updated_by_device_id = ");
            separated.push_bind_unseparated(device_id_str.clone());
            fields_updated = true;
        }

        // **OPTIMIZATION: Apply updates using LWW macro for consistent handling**
        add_lww!(name, "name", &update_data.name.as_ref());
        add_lww!(gender, "gender", &update_data.gender.as_ref());
        // **SPECIAL HANDLING: disability_type can be explicitly cleared**
        if let Some(disability_type_opt) = &update_data.disability_type {
            separated.push("disability_type = ");
            separated.push_bind_unseparated(disability_type_opt.clone());
            separated.push(" disability_type_updated_at = ");
            separated.push_bind_unseparated(now_str.clone());
            separated.push(" disability_type_updated_by = ");
            separated.push_bind_unseparated(user_id_str.clone());
            separated.push(" disability_type_updated_by_device_id = ");
            separated.push_bind_unseparated(device_id_str.clone());
            fields_updated = true;
        }
        add_lww!(age_group, "age_group", &update_data.age_group.as_ref());
        add_lww!(location, "location", &update_data.location.as_ref());
        
        // **ENHANCED: Handle sync priority updates**
        if let Some(priority) = &update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority.as_str());
            fields_updated = true;
        }

        if !fields_updated {
            println!("ℹ️ [PARTICIPANT_REPO] No fields to update for participant {}", id);
            return Ok(old_entity); // No fields present in DTO, return old state
        }

        // **ROBUST: Always update main timestamps for audit trail**
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str.clone());
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str.clone());
        separated.push("updated_by_device_id = ");
        separated.push_bind_unseparated(device_id_str.clone());

        // **ROBUST: Finalize and execute SQL with comprehensive error handling**
        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");
        
        let query = builder.build();
        println!("🔧 [PARTICIPANT_REPO] Executing update for participant {}", id);
        
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;
        println!("🔧 [PARTICIPANT_REPO] Update executed. Rows affected: {}", result.rows_affected());
        
        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }

        // **ROBUST: Fetch new state and log changes with comprehensive change tracking**
        let new_entity = self.find_by_id_with_tx(id, tx).await?;

        // **OPTIMIZATION: Batch change log entries for reduced transaction overhead**
        let mut change_entries = Vec::new();

        // **ENHANCED: Helper macro to collect change log entries efficiently**
        macro_rules! log_if_changed {
            ($field_name:ident, $field_sql:literal) => {
                if old_entity.$field_name != new_entity.$field_name {
                    change_entries.push(ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some($field_sql.to_string()),
                        old_value: serde_json::to_string(&old_entity.$field_name).ok(),
                        new_value: serde_json::to_string(&new_entity.$field_name).ok(),
                        timestamp: now,
                        user_id: user_id,
                        device_id: device_uuid,
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    });
                }
            };
        }

        log_if_changed!(name, "name");
        log_if_changed!(gender, "gender");
        log_if_changed!(disability, "disability");
        log_if_changed!(disability_type, "disability_type");
        log_if_changed!(age_group, "age_group");
        log_if_changed!(location, "location");
        log_if_changed!(sync_priority, "sync_priority");

        // **OPTIMIZATION: Batch insert all change log entries to reduce database operations**
        if !change_entries.is_empty() {
            println!("📝 [PARTICIPANT_REPO] Logging {} field changes for participant {}", change_entries.len(), id);
            for entry in change_entries {
                self.log_change_entry(entry, tx).await?;
            }
        }
        
        Ok(new_entity)
    }

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;
        
        println!("📋 [PARTICIPANT_REPO] Finding participants - page {}, per_page {}", params.page, params.per_page);

        // **OPTIMIZATION: Use separate queries to avoid lock escalation**
        let total: i64 = query_scalar("SELECT COUNT(*) FROM participants WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                println!("🚨 [PARTICIPANT_REPO] Error counting participants: {}", e);
                DbError::from(e)
            })?;

        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?",
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Error fetching participants: {}", e);
            DbError::from(e)
        })?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        println!("✅ [PARTICIPANT_REPO] Found {} participants (total: {})", entities.len(), total);

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        if ids.is_empty() {
            return Ok(PaginatedResult::new(Vec::new(), 0, params));
        }

        let offset = (params.page - 1) * params.per_page;
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();

        // Build dynamic count query
        let count_query = format!(
            "SELECT COUNT(*) FROM participants WHERE id IN ({}) AND deleted_at IS NULL",
            vec!["?"; ids.len()].join(", ")
        );
        let mut count_builder = query_scalar::<_, i64>(&count_query);
        for id_str in &id_strings {
            count_builder = count_builder.bind(id_str);
        }
        let total = count_builder.fetch_one(&self.pool).await.map_err(DbError::from)?;

        // Build dynamic select query
        let select_query = format!(
            "SELECT * FROM participants WHERE id IN ({}) AND deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?",
            vec!["?"; ids.len()].join(", ")
        );
        let mut select_builder = query_as::<_, ParticipantRow>(&select_query);
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        let rows = select_builder
            .bind(params.per_page as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() { 
            println!("ℹ️ [PARTICIPANT_REPO] No participants to update sync priority");
            return Ok(0); 
        }
        
        println!("🔄 [PARTICIPANT_REPO] Updating sync priority for {} participants", ids.len());
        
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        // **OPTIMIZATION: Fetch old priorities for change logging**
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        let select_query = format!(
            "SELECT id, sync_priority FROM participants WHERE id IN ({}) AND deleted_at IS NULL",
            vec!["?"; ids.len()].join(", ")
        );
        
        let mut select_builder = query_as::<_, (String, String)>(&select_query);
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        
        let old_priorities: HashMap<Uuid, SyncPriority> = select_builder
            .fetch_all(&mut *tx)
            .await
            .map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, prio_text)| {
                match Uuid::parse_str(&id_str) {
                    Ok(id) => Some((id, SyncPriority::from_str(&prio_text).unwrap_or_default())),
                    Err(_) => {
                        println!("⚠️ [PARTICIPANT_REPO] Invalid UUID in sync priority update: {}", id_str);
                        None
                    }
                }
            }).collect();

        // **ROBUST: Perform bulk update with comprehensive error handling**
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let priority_str = priority.as_str();

        let mut update_builder = QueryBuilder::new("UPDATE participants SET ");
        update_builder.push("sync_priority = "); 
        update_builder.push_bind(priority_str);
        update_builder.push(", updated_at = "); 
        update_builder.push_bind(now_str.clone());
        update_builder.push(", updated_by_user_id = "); 
        update_builder.push_bind(user_id_str.clone());
        update_builder.push(" WHERE id IN (");
        
        let mut id_separated = update_builder.separated(",");
        for id in ids { 
            id_separated.push_bind(id.to_string()); 
        }
        update_builder.push(") AND deleted_at IS NULL");

        let query = update_builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
        let rows_affected = result.rows_affected();
        
        println!("🔄 [PARTICIPANT_REPO] Updated {} participants with new sync priority", rows_affected);

        // **OPTIMIZATION: Batch log changes for participants that actually changed**
        for id in ids {
            if let Some(old_priority) = old_priorities.get(id) {
                if *old_priority != priority {
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: *id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some("sync_priority".to_string()),
                        old_value: serde_json::to_string(old_priority.as_str()).ok(),
                        new_value: serde_json::to_string(priority_str).ok(),
                        timestamp: now,
                        user_id: auth.user_id,
                        device_id: auth.device_id.parse::<Uuid>().ok(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;
                }
            }
        }

        tx.commit().await.map_err(DbError::from)?;
        Ok(rows_affected)
    }
    
    async fn count_by_gender(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT gender, COUNT(*) 
             FROM participants 
             WHERE deleted_at IS NULL 
             GROUP BY gender"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn count_by_age_group(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT age_group, COUNT(*) 
             FROM participants 
             WHERE deleted_at IS NULL 
             GROUP BY age_group"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn count_by_location(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT location, COUNT(*) 
             FROM participants 
             WHERE deleted_at IS NULL 
             GROUP BY location"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn count_by_disability(&self) -> DomainResult<Vec<(bool, i64)>> {
        let counts = query_as::<_, (i64, i64)>(
            "SELECT disability, COUNT(*) 
             FROM participants 
             WHERE deleted_at IS NULL 
             GROUP BY disability"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Convert i64 to bool for the first element
        let bool_counts: Vec<(bool, i64)> = counts
            .into_iter()
            .map(|(disability, count)| (disability != 0, count))
            .collect();

        Ok(bool_counts)
    }
    
    async fn count_by_disability_type(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT disability_type, COUNT(*) 
             FROM participants 
             WHERE deleted_at IS NULL AND disability = 1
             GROUP BY disability_type"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn get_available_disability_types(&self) -> DomainResult<Vec<String>> {
        let rows = query_as::<_, (Option<String>,)>(
            "SELECT DISTINCT disability_type FROM participants 
             WHERE disability_type IS NOT NULL 
             AND disability_type != '' 
             AND deleted_at IS NULL
             ORDER BY disability_type ASC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let types: Vec<String> = rows
            .into_iter()
            .filter_map(|(disability_type,)| disability_type)
            .collect();
            
        Ok(types)
    }
    
    /// Comprehensive statistics endpoint matching project domain's robustness
    async fn get_participant_demographics(&self) -> DomainResult<ParticipantDemographics> {
        let mut demographics = ParticipantDemographics::new();
        
        // === Basic Counts ===
        demographics.total_participants = query_scalar(
            "SELECT COUNT(*) FROM participants"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        demographics.active_participants = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        demographics.deleted_participants = demographics.total_participants - demographics.active_participants;
        
        // === Demographic Breakdowns ===
        // Gender distribution
        let gender_counts = self.count_by_gender().await?;
        for (gender_opt, count) in gender_counts {
            let gender_name = gender_opt.unwrap_or_else(|| "Unspecified".to_string());
            demographics.by_gender.insert(gender_name, count);
        }
        
        // Age group distribution
        let age_group_counts = self.count_by_age_group().await?;
        for (age_group_opt, count) in age_group_counts {
            let age_group_name = age_group_opt.unwrap_or_else(|| "Unspecified".to_string());
            demographics.by_age_group.insert(age_group_name, count);
        }
        
        // Location distribution
        let location_counts = self.count_by_location().await?;
        for (location_opt, count) in location_counts {
            let location_name = location_opt.unwrap_or_else(|| "Unspecified".to_string());
            demographics.by_location.insert(location_name, count);
        }
        
        // Disability distribution
        let disability_counts = self.count_by_disability().await?;
        for (has_disability, count) in disability_counts {
            let disability_name = if has_disability { "Yes" } else { "No" }.to_string();
            demographics.by_disability.insert(disability_name, count);
        }
        
        // Disability type distribution
        let disability_type_counts = self.count_by_disability_type().await?;
        for (disability_type_opt, count) in disability_type_counts {
            let disability_type_name = disability_type_opt.unwrap_or_else(|| "Unspecified".to_string());
            demographics.by_disability_type.insert(disability_type_name, count);
        }
        
        // === Engagement Statistics ===
        demographics.participants_with_workshops = query_scalar(
            "SELECT COUNT(DISTINCT wp.participant_id) 
             FROM workshop_participants wp 
             JOIN participants p ON wp.participant_id = p.id 
             WHERE wp.deleted_at IS NULL AND p.deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        demographics.participants_with_livelihoods = query_scalar(
            "SELECT COUNT(DISTINCT l.participant_id) 
             FROM livelihoods l 
             JOIN participants p ON l.participant_id = p.id 
             WHERE l.deleted_at IS NULL AND p.deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        demographics.participants_with_documents = query_scalar(
            "SELECT COUNT(DISTINCT md.related_id) 
             FROM media_documents md 
             JOIN participants p ON md.related_id = p.id 
             WHERE md.related_table = 'participants' AND md.deleted_at IS NULL AND p.deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Calculate participants with no engagement
        let engaged_participants = std::cmp::max(
            std::cmp::max(demographics.participants_with_workshops, demographics.participants_with_livelihoods),
            demographics.participants_with_documents
        );
        demographics.participants_with_no_engagement = demographics.active_participants - engaged_participants;
        
        // === Workshop Engagement Metrics ===
        let workshop_stats_rows = query("
            SELECT 
                COUNT(wp.workshop_id) as workshop_count,
                COUNT(DISTINCT wp.participant_id) as participant_count
            FROM workshop_participants wp 
            JOIN participants p ON wp.participant_id = p.id 
            WHERE wp.deleted_at IS NULL AND p.deleted_at IS NULL
            GROUP BY wp.participant_id
        ")
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        if !workshop_stats_rows.is_empty() {
            let total_workshops: i64 = workshop_stats_rows.iter()
                .map(|row| row.get::<i64, _>("workshop_count"))
                .sum();
            let total_participants_with_workshops = workshop_stats_rows.len() as i64;
            
            demographics.avg_workshops_per_participant = if total_participants_with_workshops > 0 {
                total_workshops as f64 / total_participants_with_workshops as f64
            } else {
                0.0
            };
            
            demographics.max_workshops_per_participant = workshop_stats_rows.iter()
                .map(|row| row.get::<i64, _>("workshop_count"))
                .max()
                .unwrap_or(0);
            
            // Distribution by workshop count
            for row in &workshop_stats_rows {
                let workshop_count = row.get::<i64, _>("workshop_count");
                *demographics.participants_by_workshop_count.entry(workshop_count).or_insert(0) += 1;
            }
        }
        
        // === Document Type Usage ===
        let doc_type_rows = query("
            SELECT dt.name, COUNT(md.id) as usage_count
            FROM document_types dt
            LEFT JOIN media_documents md ON dt.id = md.type_id 
                AND md.related_table = 'participants' 
                AND md.deleted_at IS NULL
            WHERE dt.deleted_at IS NULL
            GROUP BY dt.id, dt.name
        ")
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        for row in doc_type_rows {
            let doc_type_name: String = row.get("name");
            let usage_count: i64 = row.get("usage_count");
            demographics.document_types_usage.insert(doc_type_name, usage_count);
        }
        
        // === Temporal Statistics ===
        let now = Utc::now();
        let month_start = now.format("%Y-%m-01 00:00:00").to_string();
        let year_start = now.format("%Y-01-01 00:00:00").to_string();
        
        demographics.participants_added_this_month = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE created_at >= ?"
        )
        .bind(&month_start)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        demographics.participants_added_this_year = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE created_at >= ?"
        )
        .bind(&year_start)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Monthly registration trend (last 12 months)
        let monthly_trend_rows = query("
            SELECT 
                strftime('%Y-%m', created_at) as month,
                COUNT(*) as count
            FROM participants 
            WHERE created_at >= date('now', '-12 months')
            GROUP BY strftime('%Y-%m', created_at)
            ORDER BY month
        ")
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        for row in monthly_trend_rows {
            let month: String = row.get("month");
            let count: i64 = row.get("count");
            demographics.monthly_registration_trend.insert(month, count);
        }
        
        // === Data Quality Metrics ===
        demographics.participants_missing_gender = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE (gender IS NULL OR gender = '') AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        demographics.participants_missing_age_group = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE (age_group IS NULL OR age_group = '') AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        demographics.participants_missing_location = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE (location IS NULL OR location = '') AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Calculate data completeness
        demographics.calculate_data_completeness();
        
        Ok(demographics)
    }
    
    async fn find_by_gender(
        &self,
        gender: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE gender = ? AND deleted_at IS NULL"
        )
        .bind(gender)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE gender = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(gender)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_age_group(
        &self,
        age_group: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE age_group = ? AND deleted_at IS NULL"
        )
        .bind(age_group)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE age_group = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(age_group)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_location(
        &self,
        location: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE location = ? AND deleted_at IS NULL"
        )
        .bind(location)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE location = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(location)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_disability(
        &self,
        has_disability: bool,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;
        let disability_val = if has_disability { 1 } else { 0 };

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE disability = ? AND deleted_at IS NULL"
        )
        .bind(disability_val)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE disability = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(disability_val)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_disability_type(
        &self,
        disability_type: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE disability_type = ? AND deleted_at IS NULL"
        )
        .bind(disability_type)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE disability_type = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(disability_type)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_ids_by_filter(
        &self,
        filter: &ParticipantFilter,
    ) -> DomainResult<Vec<Uuid>> {
        let mut query_builder = QueryBuilder::new("SELECT id FROM participants");
        let mut has_conditions = false;
        
        // Base condition for deletion status
        if filter.exclude_deleted {
            query_builder.push(" WHERE deleted_at IS NULL");
            has_conditions = true;
        }
        
        // Gender filter (multiple values)
        if let Some(genders) = &filter.genders {
            if !genders.is_empty() {
                if has_conditions {
                    query_builder.push(" AND ");
                } else {
                    query_builder.push(" WHERE ");
                    has_conditions = true;
                }
                query_builder.push("gender IN (");
                let mut separated = query_builder.separated(",");
                for gender in genders {
                    separated.push_bind(gender);
                }
                separated.push_unseparated(")");
            }
        }
        
        // Age groups filter (multiple values)
        if let Some(age_groups) = &filter.age_groups {
            if !age_groups.is_empty() {
                if has_conditions {
                    query_builder.push(" AND ");
                } else {
                    query_builder.push(" WHERE ");
                    has_conditions = true;
                }
                query_builder.push("age_group IN (");
                let mut separated = query_builder.separated(",");
                for age_group in age_groups {
                    separated.push_bind(age_group);
                }
                separated.push_unseparated(")");
            }
        }
        
        // Locations filter (multiple values)
        if let Some(locations) = &filter.locations {
            if !locations.is_empty() {
                if has_conditions {
                    query_builder.push(" AND ");
                } else {
                    query_builder.push(" WHERE ");
                    has_conditions = true;
                }
                query_builder.push("location IN (");
                let mut separated = query_builder.separated(",");
                for location in locations {
                    separated.push_bind(location);
                }
                separated.push_unseparated(")");
            }
        }
        
        // Disability filtering - enhanced logic for grouped UI approach
        if let Some(disability_types) = &filter.disability_types {
            if !disability_types.is_empty() && disability_types.len() < 10 {
                // Specific disability types selected - this implies disability = true
                query_builder.push(" AND (disability = 1 AND disability_type IN (");
                let mut separated = query_builder.separated(", ");
                for disability_type in disability_types {
                    separated.push_bind(disability_type);
                }
                separated.push_unseparated("))");
            }
        } else if let Some(disability) = filter.disability {
            // General disability filter (only applied if no specific types selected)
            query_builder.push(" AND disability = ").push_bind(disability);
        }
        
        // Search text filter (searches name, disability_type, location)
        if let Some(search_text) = &filter.search_text {
            if !search_text.trim().is_empty() {
                if has_conditions {
                    query_builder.push(" AND ");
                } else {
                    query_builder.push(" WHERE ");
                    has_conditions = true;
                }
                let search_pattern = format!("%{}%", search_text.trim());
                query_builder.push("(name LIKE ");
                query_builder.push_bind(search_pattern.clone());
                query_builder.push(" OR disability_type LIKE ");
                query_builder.push_bind(search_pattern.clone());
                query_builder.push(" OR location LIKE ");
                query_builder.push_bind(search_pattern);
                query_builder.push(")");
            }
        }
        
        // Date range filter
        if let Some((start_date, end_date)) = &filter.date_range {
            if has_conditions {
                query_builder.push(" AND ");
            } else {
                query_builder.push(" WHERE ");
                has_conditions = true;
            }
            query_builder.push("created_at BETWEEN ");
            query_builder.push_bind(start_date);
            query_builder.push(" AND ");
            query_builder.push_bind(end_date);
        }
        
        // Created by user filter
        if let Some(user_ids) = &filter.created_by_user_ids {
            if !user_ids.is_empty() {
                if has_conditions {
                    query_builder.push(" AND ");
                } else {
                    query_builder.push(" WHERE ");
                    has_conditions = true;
                }
                query_builder.push("created_by_user_id IN (");
                let mut separated = query_builder.separated(",");
                for user_id in user_ids {
                    separated.push_bind(user_id.to_string());
                }
                separated.push_unseparated(")");
            }
        }
        
        // Workshop participation filter
        if let Some(workshop_ids) = &filter.workshop_ids {
            if !workshop_ids.is_empty() {
                if has_conditions {
                    query_builder.push(" AND ");
                } else {
                    query_builder.push(" WHERE ");
                    has_conditions = true;
                }
                query_builder.push("id IN (SELECT DISTINCT participant_id FROM workshop_participants WHERE workshop_id IN (");
                let mut separated = query_builder.separated(",");
                for workshop_id in workshop_ids {
                    separated.push_bind(workshop_id.to_string());
                }
                separated.push_unseparated(") AND deleted_at IS NULL)");
            }
        }
        
        // Document existence filter
        if let Some(has_documents) = filter.has_documents {
            if has_conditions {
                query_builder.push(" AND ");
            } else {
                query_builder.push(" WHERE ");
                has_conditions = true;
            }
            if has_documents {
                query_builder.push("id IN (SELECT DISTINCT entity_id FROM media_documents WHERE entity_table = 'participants' AND deleted_at IS NULL)");
            } else {
                query_builder.push("id NOT IN (SELECT DISTINCT entity_id FROM media_documents WHERE entity_table = 'participants' AND deleted_at IS NULL)");
            }
        }
        
        // Document linked fields filter
        if let Some(linked_fields) = &filter.document_linked_fields {
            if !linked_fields.is_empty() {
                if has_conditions {
                    query_builder.push(" AND ");
                } else {
                    query_builder.push(" WHERE ");
                    has_conditions = true;
                }
                query_builder.push("id IN (SELECT DISTINCT entity_id FROM media_documents WHERE entity_table = 'participants' AND linked_field IN (");
                let mut separated = query_builder.separated(",");
                for field in linked_fields {
                    separated.push_bind(field);
                }
                separated.push_unseparated(") AND deleted_at IS NULL)");
            }
        }

        let query = query_builder.build_query_as::<(String,)>();
        let rows = query.fetch_all(&self.pool).await.map_err(DbError::from)?;
        
        rows.into_iter()
            .map(|(id_str,)| {
                Uuid::parse_str(&id_str)
                    .map_err(|_| DomainError::Internal("Failed to parse UUID from row".to_string()))
            })
            .collect::<DomainResult<Vec<Uuid>>>()
    }


    
    async fn find_workshop_participants(
        &self,
        workshop_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;
        let workshop_id_str = workshop_id.to_string();

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(p.id) 
             FROM participants p
             JOIN workshop_participants wp ON p.id = wp.participant_id
             WHERE wp.workshop_id = ? 
             AND p.deleted_at IS NULL
             AND wp.deleted_at IS NULL"
        )
        .bind(&workshop_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ParticipantRow>(
            "SELECT p.* 
             FROM participants p
             JOIN workshop_participants wp ON p.id = wp.participant_id
             WHERE wp.workshop_id = ?
             AND p.deleted_at IS NULL
             AND wp.deleted_at IS NULL
             ORDER BY p.name ASC
             LIMIT ? OFFSET ?"
        )
        .bind(&workshop_id_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn get_participant_workshops(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<Vec<WorkshopSummary>> {
        let participant_id_str = participant_id.to_string();
        let today = Local::now().naive_local().date().format("%Y-%m-%d").to_string();
        
        // Query workshops for this participant with pre/post evaluation data
        let rows = query(
            r#"
            SELECT 
                w.id, 
                w.name, 
                w.event_date as date, 
                w.location,
                CASE 
                    WHEN w.event_date IS NULL THEN 0
                    WHEN date(w.event_date) < date(?) THEN 1
                    ELSE 0
                END as has_completed,
                wp.pre_evaluation,
                wp.post_evaluation
            FROM 
                workshops w
            JOIN 
                workshop_participants wp ON w.id = wp.workshop_id
            WHERE 
                wp.participant_id = ? 
                AND w.deleted_at IS NULL
                AND wp.deleted_at IS NULL
            ORDER BY 
                w.event_date DESC
            "#
        )
        .bind(&today)
        .bind(&participant_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut workshop_summaries = Vec::new();
        for row in rows {
            let id_str: String = row.get("id");
            let id = Uuid::parse_str(&id_str).map_err(|_| {
                DomainError::Internal(format!("Invalid UUID format in workshop id: {}", id_str))
            })?;
            
            let has_completed: i64 = row.get("has_completed");
            
            workshop_summaries.push(WorkshopSummary {
                id,
                name: row.get("name"),
                date: row.get("date"),
                location: row.get("location"),
                has_completed: has_completed != 0,
                pre_evaluation: row.get("pre_evaluation"),
                post_evaluation: row.get("post_evaluation"),
            });
        }
        
        Ok(workshop_summaries)
    }
    
    async fn get_participant_livelihoods(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<Vec<LivelihoodSummary>> {
        let participant_id_str = participant_id.to_string();
        
        // Query livelihoods for this participant
        let rows = query(
            r#"
            SELECT 
                l.id, 
                p.name, -- Get participant name (or adjust if livelihood has its own name field)
                l.type as type_, 
                s.value as status, -- Join with status_types
                l.initial_grant_date as start_date -- Map initial_grant_date to start_date
            FROM 
                livelihoods l
            JOIN 
                participants p ON l.participant_id = p.id
            LEFT JOIN 
                status_types s ON l.status_id = s.id
            WHERE 
                l.participant_id = ? 
                AND l.deleted_at IS NULL
                AND p.deleted_at IS NULL
            ORDER BY 
                l.initial_grant_date DESC
            "#
        )
        .bind(&participant_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut livelihood_summaries = Vec::new();
        for row in rows {
             let id_str: String = row.get("id");
             let id = Uuid::parse_str(&id_str).map_err(|_| {
                DomainError::Internal(format!("Invalid UUID format in livelihood id: {}", id_str))
            })?;
            
            livelihood_summaries.push(LivelihoodSummary {
                id,
                name: row.get("name"),
                type_: row.get("type_"),
                status: row.get("status"),
                start_date: row.get("start_date"),
            });
        }
        
        Ok(livelihood_summaries)
    }
    
    async fn count_participant_workshops(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<(i64, i64, i64)> { // (total, completed, upcoming)
        let participant_id_str = participant_id.to_string();
        let today = Local::now().naive_local().date().format("%Y-%m-%d").to_string();
        
        // Count total, completed, and upcoming workshops
        let (total, completed, upcoming) = query_as::<_, (i64, i64, i64)>(
            r#"
            SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN w.event_date IS NOT NULL AND date(w.event_date) < date(?) THEN 1 END) as completed,
                COUNT(CASE WHEN w.event_date IS NOT NULL AND date(w.event_date) >= date(?) THEN 1 END) as upcoming
            FROM 
                workshops w
            JOIN 
                workshop_participants wp ON w.id = wp.workshop_id
            WHERE 
                wp.participant_id = ? 
                AND w.deleted_at IS NULL
                AND wp.deleted_at IS NULL
            "#
        )
        .bind(&today)
        .bind(&today)
        .bind(&participant_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok((total, completed, upcoming))
    }
    
    async fn count_participant_livelihoods(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<(i64, i64)> { // (total, active)
        let participant_id_str = participant_id.to_string();
        
        // Count total and active livelihoods (assuming 'active' or 'ongoing' means active)
        let (total, active) = query_as::<_, (i64, i64)>(
            r#"
            SELECT 
                COUNT(*) as total,
                COUNT(CASE 
                    WHEN l.status_id IN (SELECT id FROM status_types WHERE value IN ('active', 'ongoing')) 
                    THEN 1 
                    ELSE NULL 
                END) as active
            FROM 
                livelihoods l
            WHERE 
                l.participant_id = ? 
                AND l.deleted_at IS NULL
            "#
        )
        .bind(&participant_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok((total, active))
    }
    
    /// Get document counts by type for a participant
    async fn get_participant_document_counts_by_type(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<HashMap<String, i64>> {
        let participant_id_str = participant_id.to_string();
        
        let rows = query_as::<_, (String, i64)>(
            "SELECT dt.name, COUNT(md.id) as count
             FROM document_types dt
             LEFT JOIN media_documents md ON dt.id = md.type_id 
                AND md.related_table = 'participants' 
                AND md.related_id = ?
                AND md.deleted_at IS NULL
             WHERE dt.deleted_at IS NULL
             GROUP BY dt.id, dt.name
             ORDER BY dt.name"
        )
        .bind(&participant_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut counts = HashMap::new();
        for (doc_type_name, count) in rows {
            counts.insert(doc_type_name, count);
        }
        
        Ok(counts)
    }
    
    /// Count total documents for a participant
    async fn count_participant_documents(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<i64> {
        let participant_id_str = participant_id.to_string();
        
        let count: i64 = query_scalar(
            "SELECT COUNT(*) FROM media_documents 
             WHERE entity_table = 'participants' 
             AND entity_id = ? 
             AND deleted_at IS NULL"
        )
        .bind(&participant_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok(count)
    }
    


    /// Find participants within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE updated_at >= ? AND updated_at <= ? AND deleted_at IS NULL"
        )
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE updated_at >= ? AND updated_at <= ? AND deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    /// Find participants by complex filter criteria with pagination and robust error handling
    async fn find_by_filter(
        &self,
        filter: &ParticipantFilter,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        println!("🔍 [PARTICIPANT_REPO] Finding participants with filter - page {}, per_page {}", params.page, params.per_page);
        
        // **CONCURRENT SAFETY: Get the IDs first using our optimized filter logic**
        let filtered_ids = self.find_ids_by_filter(filter).await.map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Filter query failed: {}", e);
            e
        })?;
        
        if filtered_ids.is_empty() {
            println!("ℹ️ [PARTICIPANT_REPO] No participants match filter criteria");
            return Ok(PaginatedResult::new(Vec::new(), 0, params));
        }
        
        // **OPTIMIZATION: Apply pagination to the filtered IDs to reduce memory usage**
        let offset = (params.page - 1) * params.per_page;
        let total = filtered_ids.len() as u64;
        
        // Get the IDs for this page
        let page_ids: Vec<Uuid> = filtered_ids
            .into_iter()
            .skip(offset as usize)
            .take(params.per_page as usize)
            .collect();
            
        if page_ids.is_empty() {
            println!("ℹ️ [PARTICIPANT_REPO] No participants on page {} for filter", params.page);
            return Ok(PaginatedResult::new(Vec::new(), total, params));
        }
        
        println!("📋 [PARTICIPANT_REPO] Fetching {} participants for current page", page_ids.len());
        
        // **ROBUST: Fetch the actual entities for this page with comprehensive error handling**
        let mut query_builder = QueryBuilder::new("SELECT * FROM participants WHERE id IN (");
        let mut separated = query_builder.separated(",");
        for id in &page_ids {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(") ORDER BY name ASC");
        
        let query = query_builder.build_query_as::<ParticipantRow>();
        let rows = query.fetch_all(&self.pool).await.map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Error fetching filtered participants: {}", e);
            DbError::from(e)
        })?;
        
        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;
            
        println!("✅ [PARTICIPANT_REPO] Found {} participants matching filter (total: {})", entities.len(), total);
        Ok(PaginatedResult::new(entities, total, params))
    }
    
    /// Bulk update sync priority with full error recovery and transaction safety
    async fn bulk_update_sync_priority_by_filter(
        &self,
        filter: &ParticipantFilter,
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        println!("🔄 [PARTICIPANT_REPO] Bulk updating sync priority via filter to: {}", priority.as_str());
        
        // **CONCURRENT SAFETY: Get IDs matching the filter first**
        let filtered_ids = self.find_ids_by_filter(filter).await.map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Filter query failed for bulk update: {}", e);
            e
        })?;
        
        if filtered_ids.is_empty() {
            println!("ℹ️ [PARTICIPANT_REPO] No participants match filter for bulk sync priority update");
            return Ok(0);
        }
        
        println!("🔄 [PARTICIPANT_REPO] Updating sync priority for {} filtered participants", filtered_ids.len());
        
        // **ROBUST: Delegate to our proven bulk update method**
        self.update_sync_priority(&filtered_ids, priority, auth).await.map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Bulk sync priority update failed: {}", e);
            e
        })
    }
    
    /// Find participant by name (case insensitive) - used for duplicate checking
    async fn find_by_name_case_insensitive(
        &self,
        name: &str,
    ) -> DomainResult<Participant> {
        println!("🔍 [PARTICIPANT_REPO] Finding participant by name: '{}'", name);
        
        let row = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE LOWER(name) = LOWER(?) AND deleted_at IS NULL"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Database error finding participant by name '{}': {}", name, e);
            DbError::from(e)
        })?
        .ok_or_else(|| {
            println!("ℹ️ [PARTICIPANT_REPO] No participant found with name: '{}'", name);
            DomainError::EntityNotFound("Participant".to_string(), Uuid::nil())
        })?;

        let participant = Self::map_row_to_entity(row)?;
        println!("✅ [PARTICIPANT_REPO] Found participant by name: {} (ID: {})", participant.name, participant.id);
        Ok(participant)
    }

    /// Find all participants by name (case insensitive) - used for duplicate detection
    async fn find_all_by_name_case_insensitive(
        &self,
        name: &str,
    ) -> DomainResult<Vec<Participant>> {
        println!("🔍 [PARTICIPANT_REPO] Finding all participants with name: '{}'", name);
        
        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE LOWER(name) = LOWER(?) AND deleted_at IS NULL ORDER BY created_at DESC"
        )
        .bind(name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Database error finding participants by name '{}': {}", name, e);
            DbError::from(e)
        })?;

        let mut participants = Vec::with_capacity(rows.len());
        for row in rows {
            let participant = Self::map_row_to_entity(row)?;
            participants.push(participant);
        }
        
        println!("✅ [PARTICIPANT_REPO] Found {} participants with name '{}'", participants.len(), name);
        Ok(participants)
    }

    /// **ADVANCED QUERY: Get participant document references with JOIN optimization**
    /// Matches project domain's get_project_document_references pattern exactly
    async fn get_participant_document_references(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<Vec<crate::domains::participant::types::ParticipantDocumentReference>> {
        println!("🔍 [PARTICIPANT_REPO] Getting document references for participant {}", participant_id);
        
        let participant_id_str = participant_id.to_string();
        let mut references = Vec::new();
        
        // **OPTIMIZATION: Get all document-linkable fields for participants**
        let doc_ref_fields: Vec<_> = Participant::field_metadata()
            .into_iter()
            .filter(|field| field.supports_documents)
            .collect();
            
        for field in doc_ref_fields {
            // **PERFORMANCE: Use LEFT JOIN to fetch document details in single query**
            let query_str = format!(
                "SELECT md.id as doc_id, md.original_filename, md.created_at, md.size_bytes as file_size 
                 FROM participants p 
                 LEFT JOIN media_documents md ON md.related_id = p.id 
                   AND md.related_table = 'participants' 
                   AND md.field_identifier = ? 
                   AND md.deleted_at IS NULL
                 WHERE p.id = ? AND p.deleted_at IS NULL"
            );
            
            let row = query(&query_str)
                .bind(&field.field_name)
                .bind(&participant_id_str)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| {
                    println!("🚨 [PARTICIPANT_REPO] Document reference query failed for field '{}': {}", field.field_name, e);
                    DbError::from(e)
                })?;
                
            if let Some(row) = row {
                let doc_id_str: Option<String> = row.get("doc_id");
                let doc_id = doc_id_str.map(|id_str| 
                    Uuid::parse_str(&id_str)
                        .map_err(|_| DomainError::Internal(format!("Invalid document UUID: {}", id_str)))
                ).transpose()?;
                
                let (filename, upload_date, file_size) = if doc_id.is_some() {
                    (
                        row.get("original_filename"),
                        row.get::<Option<String>, _>("created_at").map(|dt_str| 
                            DateTime::parse_from_rfc3339(&dt_str)
                                .map_err(|_| DomainError::Internal(format!("Invalid datetime: {}", dt_str)))
                                .map(|dt| dt.with_timezone(&Utc))
                        ).transpose()?,
                        row.get::<Option<i64>, _>("file_size").map(|fs| fs as u64),
                    )
                } else {
                    (None, None, None)
                };
                
                references.push(crate::domains::participant::types::ParticipantDocumentReference {
                    field_name: field.field_name.to_string(),
                    display_name: field.display_name.to_string(),
                    document_id: doc_id,
                    filename,
                    upload_date,
                    file_size,
                });
            } else {
                // **ROBUST: Add entry even if no document exists**
                references.push(crate::domains::participant::types::ParticipantDocumentReference {
                    field_name: field.field_name.to_string(),
                    display_name: field.display_name.to_string(),
                    document_id: None,
                    filename: None,
                    upload_date: None,
                    file_size: None,
                });
            }
        }
        
        println!("✅ [PARTICIPANT_REPO] Found {} document references for participant {}", references.len(), participant_id);
        Ok(references)
    }
    
    /// **ADVANCED QUERY: Get participant with enriched relationship data using JOINs**
    /// Optimized query that fetches participant + related counts in minimal DB calls
    async fn get_participant_with_enrichment(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<crate::domains::participant::types::ParticipantWithEnrichment> {
        println!("🔍 [PARTICIPANT_REPO] Getting enriched participant data for {}", participant_id);
        
        // **OPTIMIZATION: Single complex query with multiple LEFT JOINs for performance**
        let query_str = r#"
            SELECT 
                p.*,
                COUNT(DISTINCT w.id) as workshop_count,
                COUNT(DISTINCT l.id) as livelihood_count,
                COUNT(DISTINCT l.id) FILTER (WHERE l.status = 'active') as active_livelihood_count,
                COUNT(DISTINCT md.id) as document_count,
                COUNT(DISTINCT CASE WHEN md.created_at >= date('now', '-30 days') THEN md.id END) as recent_document_count
            FROM participants p
            LEFT JOIN workshop_participants wp ON wp.participant_id = p.id
            LEFT JOIN workshops w ON w.id = wp.workshop_id AND w.deleted_at IS NULL
            LEFT JOIN livelihoods l ON l.participant_id = p.id AND l.deleted_at IS NULL
            LEFT JOIN media_documents md ON md.related_id = p.id 
                AND md.related_table = 'participants' AND md.deleted_at IS NULL
            WHERE p.id = ? AND p.deleted_at IS NULL
            GROUP BY p.id
        "#;
        
        let row = query(query_str)
            .bind(participant_id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| {
                println!("🚨 [PARTICIPANT_REPO] Enrichment query failed for participant {}: {}", participant_id, e);
                DbError::from(e)
            })?
            .ok_or_else(|| {
                println!("🚨 [PARTICIPANT_REPO] Participant {} not found during enrichment", participant_id);
                DomainError::EntityNotFound("Participant".to_string(), participant_id)
            })?;
        
        // **PERFORMANCE: Parse participant data from row**
        let participant_row = ParticipantRow {
            id: row.get("id"),
            name: row.get("name"),
            name_updated_at: row.get("name_updated_at"),
            name_updated_by: row.get("name_updated_by"),
            name_updated_by_device_id: row.get("name_updated_by_device_id"),
            gender: row.get("gender"),
            gender_updated_at: row.get("gender_updated_at"),
            gender_updated_by: row.get("gender_updated_by"),
            gender_updated_by_device_id: row.get("gender_updated_by_device_id"),
            disability: row.get("disability"),
            disability_updated_at: row.get("disability_updated_at"),
            disability_updated_by: row.get("disability_updated_by"),
            disability_updated_by_device_id: row.get("disability_updated_by_device_id"),
            disability_type: row.get("disability_type"),
            disability_type_updated_at: row.get("disability_type_updated_at"),
            disability_type_updated_by: row.get("disability_type_updated_by"),
            disability_type_updated_by_device_id: row.get("disability_type_updated_by_device_id"),
            age_group: row.get("age_group"),
            age_group_updated_at: row.get("age_group_updated_at"),
            age_group_updated_by: row.get("age_group_updated_by"),
            age_group_updated_by_device_id: row.get("age_group_updated_by_device_id"),
            location: row.get("location"),
            location_updated_at: row.get("location_updated_at"),
            location_updated_by: row.get("location_updated_by"),
            location_updated_by_device_id: row.get("location_updated_by_device_id"),
            sync_priority: row.get("sync_priority"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            created_by_user_id: row.get("created_by_user_id"),
            updated_by_user_id: row.get("updated_by_user_id"),
            created_by_device_id: row.get("created_by_device_id"),
            updated_by_device_id: row.get("updated_by_device_id"),
            deleted_at: row.get("deleted_at"),
            deleted_by_user_id: row.get("deleted_by_user_id"),
            deleted_by_device_id: row.get("deleted_by_device_id"),
        };
        
        let participant = Self::map_row_to_entity(participant_row)?;
        
        // **OPTIMIZATION: Extract aggregated counts from single query**
        let workshop_count: i64 = row.get("workshop_count");
        let livelihood_count: i64 = row.get("livelihood_count");
        let active_livelihood_count: i64 = row.get("active_livelihood_count");
        let document_count: i64 = row.get("document_count");
        let recent_document_count: i64 = row.get("recent_document_count");
        
        println!("✅ [PARTICIPANT_REPO] Enriched participant {}: {} workshops, {} livelihoods, {} documents", 
                 participant_id, workshop_count, livelihood_count, document_count);
        
        Ok(crate::domains::participant::types::ParticipantWithEnrichment {
            participant,
            workshop_count,
            livelihood_count,
            active_livelihood_count,
            document_count,
            recent_document_count,
        })
    }
    
    /// **ADVANCED QUERY: Search participants with relationship JOIN optimization**
    /// Enhanced search that includes related entity matching for comprehensive results
    async fn search_participants_with_relationships(
        &self,
        search_query: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        println!("🔍 [PARTICIPANT_REPO] Searching participants with relationships: '{}'", search_query);
        
        let offset = (params.page - 1) * params.per_page;
        let search_term = format!("%{}%", search_query);

        // **ADVANCED: Search across participant and related entity data**
        let total_query = r#"
            SELECT COUNT(DISTINCT p.id) 
            FROM participants p
            LEFT JOIN workshop_participants wp ON wp.participant_id = p.id
            LEFT JOIN workshops w ON w.id = wp.workshop_id AND w.deleted_at IS NULL
            LEFT JOIN livelihoods l ON l.participant_id = p.id AND l.deleted_at IS NULL
            WHERE p.deleted_at IS NULL 
            AND (
                p.name LIKE ? 
                OR p.gender LIKE ? 
                OR p.age_group LIKE ? 
                OR p.location LIKE ?
                OR p.disability_type LIKE ?
                OR w.name LIKE ?
                OR l.business_name LIKE ?
            )
        "#;

        let total: i64 = query_scalar(total_query)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                println!("🚨 [PARTICIPANT_REPO] Search count query failed: {}", e);
                DbError::from(e)
            })?;

        // **PERFORMANCE: Paginated search with DISTINCT to avoid duplicates**
        let search_query = r#"
            SELECT DISTINCT p.*
            FROM participants p
            LEFT JOIN workshop_participants wp ON wp.participant_id = p.id
            LEFT JOIN workshops w ON w.id = wp.workshop_id AND w.deleted_at IS NULL
            LEFT JOIN livelihoods l ON l.participant_id = p.id AND l.deleted_at IS NULL
            WHERE p.deleted_at IS NULL 
            AND (
                p.name LIKE ? 
                OR p.gender LIKE ? 
                OR p.age_group LIKE ? 
                OR p.location LIKE ?
                OR p.disability_type LIKE ?
                OR w.name LIKE ?
                OR l.business_name LIKE ?
            )
            ORDER BY p.name ASC 
            LIMIT ? OFFSET ?
        "#;

        let rows = query_as::<_, ParticipantRow>(search_query)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(&search_term)
            .bind(params.per_page as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                println!("🚨 [PARTICIPANT_REPO] Search results query failed: {}", e);
                DbError::from(e)
            })?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Participant>>>()?;
            
        println!("✅ [PARTICIPANT_REPO] Found {} participants matching '{}' (total: {})", entities.len(), search_query, total);

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// **BATCH PROCESSING: Memory-efficient participant statistics computation**
    /// Cache-friendly aggregation that processes large datasets efficiently
    async fn get_participant_statistics(&self) -> DomainResult<crate::domains::participant::types::ParticipantStatistics> {
        println!("📊 [PARTICIPANT_REPO] Computing comprehensive participant statistics");
        
        // **OPTIMIZATION: Use single complex query to minimize database round trips**
        let stats_query = r#"
            SELECT 
                COUNT(*) as total_participants,
                COUNT(*) FILTER (WHERE deleted_at IS NULL) as active_participants,
                COUNT(*) FILTER (WHERE disability = true AND deleted_at IS NULL) as participants_with_disabilities,
                COUNT(*) FILTER (WHERE gender IS NOT NULL AND gender != '' AND deleted_at IS NULL) as gender_completeness,
                COUNT(*) FILTER (WHERE age_group IS NOT NULL AND age_group != '' AND deleted_at IS NULL) as age_completeness,
                COUNT(*) FILTER (WHERE location IS NOT NULL AND location != '' AND deleted_at IS NULL) as location_completeness
            FROM participants
        "#;
        
        let stats_row = query(stats_query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| {
                println!("🚨 [PARTICIPANT_REPO] Statistics query failed: {}", e);
                DbError::from(e)
            })?;
            
        let total_participants: i64 = stats_row.get("total_participants");
        let active_participants: i64 = stats_row.get("active_participants");
        let participants_with_disabilities: i64 = stats_row.get("participants_with_disabilities");
        
        // **PERFORMANCE: Parallel aggregation queries for demographic breakdowns**
        let gender_query = "SELECT gender, COUNT(*) as count FROM participants WHERE deleted_at IS NULL AND gender IS NOT NULL GROUP BY gender";
        let age_query = "SELECT age_group, COUNT(*) as count FROM participants WHERE deleted_at IS NULL AND age_group IS NOT NULL GROUP BY age_group";
        let location_query = "SELECT location, COUNT(*) as count FROM participants WHERE deleted_at IS NULL AND location IS NOT NULL GROUP BY location";
        let disability_type_query = "SELECT disability_type, COUNT(*) as count FROM participants WHERE deleted_at IS NULL AND disability_type IS NOT NULL GROUP BY disability_type";
        
        // **CONCURRENT: Execute all demographic queries in parallel**
        let (gender_rows, age_rows, location_rows, disability_type_rows) = tokio::try_join!(
            query_as::<_, (Option<String>, i64)>(gender_query).fetch_all(&self.pool),
            query_as::<_, (Option<String>, i64)>(age_query).fetch_all(&self.pool),
            query_as::<_, (Option<String>, i64)>(location_query).fetch_all(&self.pool),
            query_as::<_, (Option<String>, i64)>(disability_type_query).fetch_all(&self.pool)
        ).map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Demographic aggregation failed: {}", e);
            DbError::from(e)
        })?;
        
        // **OPTIMIZATION: Convert to HashMaps for efficient lookups**
        let by_gender: HashMap<String, i64> = gender_rows.into_iter()
            .filter_map(|(gender_opt, count)| gender_opt.map(|g| (g, count)))
            .collect();
            
        let by_age_group: HashMap<String, i64> = age_rows.into_iter()
            .filter_map(|(age_opt, count)| age_opt.map(|a| (a, count)))
            .collect();
            
        let by_location: HashMap<String, i64> = location_rows.into_iter()
            .filter_map(|(location_opt, count)| location_opt.map(|l| (l, count)))
            .collect();
            
        let by_disability_type: HashMap<String, i64> = disability_type_rows.into_iter()
            .filter_map(|(type_opt, count)| type_opt.map(|t| (t, count)))
            .collect();
            
        // **ANALYTICS: Engagement distribution based on activity metrics**
        let engagement_query = r#"
            WITH participant_activity AS (
                SELECT 
                    p.id,
                    COUNT(DISTINCT wp.workshop_id) as workshop_count,
                    COUNT(DISTINCT l.id) as livelihood_count,
                    COUNT(DISTINCT md.id) as document_count,
                    CASE 
                        WHEN COUNT(DISTINCT wp.workshop_id) >= 3 AND COUNT(DISTINCT l.id) >= 1 AND COUNT(DISTINCT md.id) >= 5 THEN 'High'
                        WHEN COUNT(DISTINCT wp.workshop_id) >= 1 OR COUNT(DISTINCT l.id) >= 1 OR COUNT(DISTINCT md.id) >= 2 THEN 'Medium'
                        ELSE 'Low'
                    END as engagement_level
                FROM participants p
                LEFT JOIN workshop_participants wp ON wp.participant_id = p.id
                LEFT JOIN livelihoods l ON l.participant_id = p.id AND l.deleted_at IS NULL
                LEFT JOIN media_documents md ON md.related_id = p.id AND md.related_table = 'participants' AND md.deleted_at IS NULL
                WHERE p.deleted_at IS NULL
                GROUP BY p.id
            )
            SELECT engagement_level, COUNT(*) as count
            FROM participant_activity
            GROUP BY engagement_level
        "#;
        
        let engagement_rows = query_as::<_, (String, i64)>(engagement_query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                println!("🚨 [PARTICIPANT_REPO] Engagement analysis failed: {}", e);
                DbError::from(e)
            })?;
            
        let engagement_distribution: HashMap<String, i64> = engagement_rows.into_iter().collect();
        
        // **TRENDS: Monthly registration patterns for growth analysis**
        let monthly_trends_query = r#"
            SELECT 
                strftime('%Y-%m', created_at) as month,
                COUNT(*) as count
            FROM participants 
            WHERE deleted_at IS NULL 
            AND created_at >= date('now', '-12 months')
            GROUP BY strftime('%Y-%m', created_at)
            ORDER BY month
        "#;
        
        let monthly_rows = query_as::<_, (String, i64)>(monthly_trends_query)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                println!("🚨 [PARTICIPANT_REPO] Monthly trends analysis failed: {}", e);
                DbError::from(e)
            })?;
            
        let monthly_registration_trends: HashMap<String, i64> = monthly_rows.into_iter().collect();
        
        // **QUALITY: Data completeness calculation**
        let total_fields = 6.0; // name, gender, age_group, location, disability, disability_type
        let gender_completeness: i64 = stats_row.get("gender_completeness");
        let age_completeness: i64 = stats_row.get("age_completeness");
        let location_completeness: i64 = stats_row.get("location_completeness");
        
        let data_completeness = if active_participants > 0 {
            let total_possible = active_participants * total_fields as i64;
            let total_complete = active_participants + gender_completeness + age_completeness + location_completeness;
            (total_complete as f64 / total_possible as f64) * 100.0
        } else {
            0.0
        };
        
        println!("✅ [PARTICIPANT_REPO] Computed statistics for {} participants with {:.1}% data completeness", 
                 active_participants, data_completeness);
        
        Ok(crate::domains::participant::types::ParticipantStatistics {
            total_participants,
            active_participants,
            participants_with_disabilities,
            by_gender,
            by_age_group,
            by_location,
            by_disability_type,
            engagement_distribution,
            monthly_registration_trends,
            data_completeness,
        })
    }
    
    /// **BATCH PROCESSING: Efficient bulk update with streaming and memory optimization**
    /// Processes large batches without loading all data into memory at once
    async fn bulk_update_participants_streaming(
        &self,
        updates: Vec<(Uuid, UpdateParticipant)>,
        auth: &AuthContext,
    ) -> DomainResult<crate::domains::participant::types::ParticipantBulkOperationResult> {
        let start_time = std::time::Instant::now();
        println!("🔄 [PARTICIPANT_REPO] Starting bulk update for {} participants", updates.len());
        
        let total_requested = updates.len();
        let mut successful = 0;
        let mut failed = 0;
        let mut skipped = 0;
        let mut error_details = Vec::new();
        
        // **OPTIMIZATION: Process in chunks to avoid overwhelming the database**
        const CHUNK_SIZE: usize = 50;
        let chunks: Vec<_> = updates.chunks(CHUNK_SIZE).collect();
        
        for (chunk_idx, chunk) in chunks.iter().enumerate() {
            println!("📦 [PARTICIPANT_REPO] Processing chunk {} of {} ({} items)", 
                     chunk_idx + 1, chunks.len(), chunk.len());
            
            // **TRANSACTION SAFETY: Each chunk in its own transaction for better concurrency**
            let mut tx = self.pool.begin().await.map_err(DbError::from)?;
            
            for (participant_id, update_data) in chunk.iter() {
                match self.update_with_tx(*participant_id, update_data, auth, &mut tx).await {
                    Ok(_) => {
                        successful += 1;
                        if successful % 10 == 0 {
                            println!("✅ [PARTICIPANT_REPO] Processed {} participants successfully", successful);
                        }
                    }
                    Err(DomainError::EntityNotFound(_, _)) => {
                        skipped += 1;
                        println!("⏭️ [PARTICIPANT_REPO] Skipped non-existent participant: {}", participant_id);
                    }
                    Err(e) => {
                        failed += 1;
                        let error_msg = format!("Update failed: {}", e);
                        error_details.push((*participant_id, error_msg.clone()));
                        println!("🚨 [PARTICIPANT_REPO] Failed to update participant {}: {}", participant_id, error_msg);
                    }
                }
            }
            
            // **ROBUST: Commit chunk transaction**
            match tx.commit().await {
                Ok(_) => {
                    println!("✅ [PARTICIPANT_REPO] Committed chunk {} successfully", chunk_idx + 1);
                }
                Err(e) => {
                    println!("🚨 [PARTICIPANT_REPO] Failed to commit chunk {}: {}", chunk_idx + 1, e);
                    // Mark all items in this chunk as failed
                    for (participant_id, _) in chunk.iter() {
                        failed += 1;
                        error_details.push((*participant_id, format!("Transaction commit failed: {}", e)));
                    }
                    successful -= chunk.len(); // Subtract since they didn't actually commit
                }
            }
        }
        
        let operation_duration_ms = start_time.elapsed().as_millis() as u64;
        
        println!("🏁 [PARTICIPANT_REPO] Bulk update completed in {}ms: {} successful, {} failed, {} skipped", 
                 operation_duration_ms, successful, failed, skipped);
        
        Ok(crate::domains::participant::types::ParticipantBulkOperationResult {
            total_requested,
            successful,
            failed,
            skipped,
            error_details,
            operation_duration_ms,
        })
    }
    
    /// **PERFORMANCE ANALYSIS: Get database index suggestions for participant queries**
    /// Analyzes query patterns and suggests optimal indexes for performance
    async fn get_index_optimization_suggestions(&self) -> DomainResult<Vec<String>> {
                 println!("🔍 [PARTICIPANT_REPO] Analyzing query patterns for index optimization");
        
        let mut suggestions = Vec::new();
        
        // **ANALYSIS: Check if recommended indexes exist**
        let index_check_queries = vec![
            ("idx_participants_name_lower", "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_participants_name_lower'"),
            ("idx_participants_filter_composite", "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_participants_filter_composite'"),
            ("idx_participants_demographics", "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_participants_demographics'"),
            ("idx_participants_dates", "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_participants_dates'"),
            ("idx_participants_search_text", "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_participants_search_text'"),
        ];
        
        for (index_name, check_query) in index_check_queries {
            let exists = query_scalar::<_, String>(check_query)
                .fetch_optional(&self.pool)
                .await
                .map_err(DbError::from)?
                .is_some();
                
            if !exists {
                let suggestion = match index_name {
                    "idx_participants_name_lower" => {
                        "CREATE INDEX idx_participants_name_lower ON participants(LOWER(name)) WHERE deleted_at IS NULL;".to_string()
                    },
                    "idx_participants_filter_composite" => {
                        "CREATE INDEX idx_participants_filter_composite ON participants(deleted_at, gender, age_group, location, disability) WHERE deleted_at IS NULL;".to_string()
                    },
                    "idx_participants_demographics" => {
                        "CREATE INDEX idx_participants_demographics ON participants(gender, age_group, location, disability_type) WHERE deleted_at IS NULL;".to_string()
                    },
                    "idx_participants_dates" => {
                        "CREATE INDEX idx_participants_dates ON participants(created_at, updated_at) WHERE deleted_at IS NULL;".to_string()
                    },
                    "idx_participants_search_text" => {
                        "CREATE INDEX idx_participants_search_text ON participants(name, gender, age_group, location, disability_type) WHERE deleted_at IS NULL;".to_string()
                    },
                    _ => continue,
                };
                suggestions.push(suggestion);
            }
        }
        
        // **ANALYSIS: Check for relationship table indexes**
        let relationship_indexes = vec![
            ("idx_workshop_participants_participant", "CREATE INDEX idx_workshop_participants_participant ON workshop_participants(participant_id);"),
            ("idx_livelihoods_participant", "CREATE INDEX idx_livelihoods_participant ON livelihoods(participant_id) WHERE deleted_at IS NULL;"),
            ("idx_media_documents_participant", "CREATE INDEX idx_media_documents_participant ON media_documents(related_entity_id, related_table) WHERE related_table = 'participants' AND deleted_at IS NULL;"),
        ];
        
        for (index_name, create_sql) in relationship_indexes {
                    let exists = query_scalar::<_, String>(&format!("SELECT name FROM sqlite_master WHERE type='index' AND name='{}'", index_name))
            .fetch_optional(&self.pool)
            .await
            .map_err(DbError::from)?
            .is_some();
                
            if !exists {
                suggestions.push(create_sql.to_string());
            }
        }
        
        if suggestions.is_empty() {
            suggestions.push("✅ All recommended indexes are already in place for optimal participant query performance.".to_string());
        }
        
        println!("📊 [PARTICIPANT_REPO] Generated {} index optimization suggestions", suggestions.len());
        Ok(suggestions)
    }
    
    /// **PERFORMANCE: Optimized filter query with compound index utilization**
    /// Enhanced version of find_ids_by_filter that leverages compound indexes for maximum performance
    async fn find_ids_by_filter_optimized(
        &self,
        filter: &ParticipantFilter,
    ) -> DomainResult<Vec<Uuid>> {
        println!("🚀 [PARTICIPANT_REPO] Executing optimized filter query");
        
        // **OPTIMIZATION: Use query builder with index-aware ordering of WHERE clauses**
        let mut query_builder = QueryBuilder::new("SELECT id FROM participants WHERE 1=1");
        
        // **INDEX OPTIMIZATION: Order conditions by selectivity (most selective first)**
        
        // 1. Most selective: excluded deleted records (uses primary filter)
        if filter.exclude_deleted {
            query_builder.push(" AND deleted_at IS NULL");
        }
        
        // 2. High selectivity: disability filter - enhanced logic for grouped UI approach
        if let Some(disability_types) = &filter.disability_types {
            if !disability_types.is_empty() && disability_types.len() < 10 {
                // Specific disability types selected - this implies disability = true
                query_builder.push(" AND (disability = 1 AND disability_type IN (");
                let mut separated = query_builder.separated(", ");
                for disability_type in disability_types {
                    separated.push_bind(disability_type);
                }
                separated.push_unseparated("))");
            }
        } else if let Some(disability) = filter.disability {
            // General disability filter (only applied if no specific types selected)
            query_builder.push(" AND disability = ").push_bind(disability);
        }
        
        // 3. Medium-high selectivity: specific field filters
        if let Some(genders) = &filter.genders {
            if !genders.is_empty() && genders.len() < 10 { // Only use IN if reasonable list size
                query_builder.push(" AND gender IN (");
                let mut separated = query_builder.separated(", ");
                for gender in genders {
                    separated.push_bind(gender);
                }
                separated.push_unseparated(")");
            }
        }
        
        if let Some(age_groups) = &filter.age_groups {
            if !age_groups.is_empty() && age_groups.len() < 10 {
                query_builder.push(" AND age_group IN (");
                let mut separated = query_builder.separated(", ");
                for age_group in age_groups {
                    separated.push_bind(age_group);
                }
                separated.push_unseparated(")");
            }
        }
        
        if let Some(locations) = &filter.locations {
            if !locations.is_empty() && locations.len() < 20 {
                query_builder.push(" AND location IN (");
                let mut separated = query_builder.separated(", ");
                for location in locations {
                    separated.push_bind(location);
                }
                separated.push_unseparated(")");
            }
        }
        
        // 4. Lower selectivity: text search (most expensive, do last)
        if let Some(search_text) = &filter.search_text {
            if !search_text.trim().is_empty() {
                let search_pattern = format!("%{}%", search_text.trim());
                // **OPTIMIZATION: Use LOWER() with functional index for case-insensitive search**
                query_builder.push(" AND (LOWER(name) LIKE LOWER(")
                    .push_bind(search_pattern.clone())
                    .push(") OR LOWER(gender) LIKE LOWER(")
                    .push_bind(search_pattern.clone())
                    .push(") OR LOWER(location) LIKE LOWER(")
                    .push_bind(search_pattern)
                    .push("))");
            }
        }
        
        // 5. Date range filters (can leverage date indexes)
        if let Some((start_date, end_date)) = &filter.date_range {
            if let (Ok(start), Ok(end)) = (
                DateTime::parse_from_rfc3339(start_date),
                DateTime::parse_from_rfc3339(end_date)
            ) {
                let start_utc = start.with_timezone(&Utc);
                let end_utc = end.with_timezone(&Utc);
                
                query_builder.push(" AND updated_at BETWEEN ")
                    .push_bind(start_utc.to_rfc3339())
                    .push(" AND ")
                    .push_bind(end_utc.to_rfc3339());
            }
        }
        
        // **PERFORMANCE: Add index hint for complex queries**
        query_builder.push(" ORDER BY id"); // Force index usage for consistent performance
        
        let query = query_builder.build_query_as::<(String,)>();
        let start_time = std::time::Instant::now();
        
        let rows = query.fetch_all(&self.pool).await.map_err(|e| {
            println!("🚨 [PARTICIPANT_REPO] Optimized filter query failed: {}", e);
            DbError::from(e)
        })?;
        
        let query_duration = start_time.elapsed();
        let results: Vec<Uuid> = rows.into_iter()
            .map(|(id_str,)| Uuid::parse_str(&id_str).map_err(|e| DomainError::InvalidUuid(e.to_string())))
            .collect::<DomainResult<Vec<Uuid>>>()?;
        
        println!("⚡ [PARTICIPANT_REPO] Optimized filter found {} participants in {:?}", results.len(), query_duration);
        
        if query_duration.as_millis() > 100 {
            println!("⚠️ [PARTICIPANT_REPO] Query took {}ms - consider adding indexes", query_duration.as_millis());
        }
        
        Ok(results)
    }
}

// === Sync Merge Implementation ===
#[async_trait]
impl MergeableEntityRepository<Participant> for SqliteParticipantRepository {
    fn entity_name(&self) -> &'static str { "participants" }

    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                // Parse JSON state
                let state_json = remote_change.new_value.as_ref().ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for participant change")))?;
                let remote_state: Participant = serde_json::from_str(state_json).map_err(|e| DomainError::Validation(ValidationError::format("new_value_participant", &format!("Invalid JSON: {}", e))))?;

                // Fetch local if exists
                let local_opt = match self.find_by_id_with_tx(remote_state.id, tx).await {
                    Ok(ent) => Some(ent),
                    Err(DomainError::EntityNotFound(_, _)) => None,
                    Err(e) => return Err(e),
                };

                if let Some(local) = local_opt {
                    if remote_state.updated_at <= local.updated_at {
                        return Ok(MergeOutcome::NoOp("Local copy newer or equal".into()));
                    }
                    self.upsert_remote_state_with_tx(tx, &remote_state).await?;
                    Ok(MergeOutcome::Updated(remote_state.id))
                } else {
                    self.upsert_remote_state_with_tx(tx, &remote_state).await?;
                    Ok(MergeOutcome::Created(remote_state.id))
                }
            }
            ChangeOperationType::Delete => Ok(MergeOutcome::NoOp("Remote soft delete ignored".into())),
            ChangeOperationType::HardDelete => Ok(MergeOutcome::HardDeleted(remote_change.entity_id)),
        }
    }
}

impl SqliteParticipantRepository {
    async fn upsert_remote_state_with_tx<'t>(&self, tx: &mut Transaction<'t, Sqlite>, remote: &Participant) -> DomainResult<()> {
        use sqlx::query;
        query(r#"
            INSERT OR REPLACE INTO participants (
                id, name, name_updated_at, name_updated_by, name_updated_by_device_id,
                gender, gender_updated_at, gender_updated_by, gender_updated_by_device_id,
                disability, disability_updated_at, disability_updated_by, disability_updated_by_device_id,
                disability_type, disability_type_updated_at, disability_type_updated_by, disability_type_updated_by_device_id,
                age_group, age_group_updated_at, age_group_updated_by, age_group_updated_by_device_id,
                location, location_updated_at, location_updated_by, location_updated_by_device_id,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?
            )
        "#)
        .bind(remote.id.to_string())
        .bind(&remote.name)
        .bind(remote.name_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.name_updated_by.map(|u| u.to_string()))
        .bind(remote.name_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.gender)
        .bind(remote.gender_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.gender_updated_by.map(|u| u.to_string()))
        .bind(remote.gender_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.disability as i64)
        .bind(remote.disability_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.disability_updated_by.map(|u| u.to_string()))
        .bind(remote.disability_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.disability_type)
        .bind(remote.disability_type_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.disability_type_updated_by.map(|u| u.to_string()))
        .bind(remote.disability_type_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.age_group)
        .bind(remote.age_group_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.age_group_updated_by.map(|u| u.to_string()))
        .bind(remote.age_group_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.location)
        .bind(remote.location_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.location_updated_by.map(|u| u.to_string()))
        .bind(remote.location_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.sync_priority.unwrap_or_default().as_str())
        .bind(remote.created_at.to_rfc3339())
        .bind(remote.updated_at.to_rfc3339())
        .bind(remote.created_by_user_id.map(|u| u.to_string()))
        .bind(remote.updated_by_user_id.map(|u| u.to_string()))
        .bind(remote.created_by_device_id.map(|u| u.to_string()))
        .bind(remote.updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.deleted_at.map(|d| d.to_rfc3339()))
        .bind(remote.deleted_by_user_id.map(|u| u.to_string()))
        .bind(remote.deleted_by_device_id.map(|u| u.to_string()))
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }
}
