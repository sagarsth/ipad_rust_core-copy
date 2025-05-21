use crate::auth::AuthContext;
use sqlx::{Executor, Pool, Row, Sqlite, Transaction, SqlitePool, query, query_as, query_scalar};
use crate::domains::workshop::types::{
    NewWorkshopParticipant, UpdateWorkshopParticipant, WorkshopParticipant, WorkshopParticipantRow, ParticipantSummary, ParticipantDetail, WorkshopSummary, ParticipantAttendance
};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams}; // Might need pagination later
use async_trait::async_trait;
use chrono::{Utc, Local}; // Added Local
use uuid::Uuid;
use std::collections::HashMap; // Added HashMap
use crate::domains::sync::repository::ChangeLogRepository; // Corrected path
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, SyncPriority}; // Corrected path and type name, removed EntityType
use serde_json; // Added serde_json for serialization
use std::sync::Arc;
use log::{debug, error, warn};
use crate::domains::user::repository::MergeableEntityRepository;
use crate::domains::sync::types::MergeOutcome;

/// Trait defining workshop-participant relationship repository operations
#[async_trait]
pub trait WorkshopParticipantRepository: Send + Sync + MergeableEntityRepository<WorkshopParticipant> {
    /// Add a participant to a workshop
    async fn add_participant(
        &self,
        new_link: &NewWorkshopParticipant,
        auth: &AuthContext,
    ) -> DomainResult<WorkshopParticipant>;

    /// Remove a participant from a workshop (soft delete)
    async fn remove_participant(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;

    /// Update evaluation details for a participant in a workshop
    async fn update_participant_evaluation(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        update_data: &UpdateWorkshopParticipant,
        auth: &AuthContext,
    ) -> DomainResult<WorkshopParticipant>;

    /// Find all participants linked to a specific workshop
    /// This version joins with participants table to get summary data.
    async fn find_participants_for_workshop(
        &self,
        workshop_id: Uuid,
        // params: PaginationParams, // Add pagination if needed
    ) -> DomainResult<Vec<ParticipantSummary>>; 
    
    /// Find a specific workshop-participant link by IDs
    async fn find_link_by_ids(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
    ) -> DomainResult<Option<WorkshopParticipant>>;
    
    /// Find all participants with detailed information for a workshop
    async fn find_participants_with_details(
        &self,
        workshop_id: Uuid,
    ) -> DomainResult<Vec<ParticipantDetail>>;
    
    /// Get evaluation completion counts for a workshop
    async fn get_evaluation_completion_counts(
        &self,
        workshop_id: Uuid,
    ) -> DomainResult<(i64, i64, i64)>; // (total, pre_eval_count, post_eval_count)
    
    /// Find all workshops a participant has attended
    async fn find_workshops_for_participant(
        &self,
        participant_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<WorkshopSummary>>;
    
    /// Get participant attendance metrics
    async fn get_participant_attendance(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<ParticipantAttendance>;
    
    /// Get evaluation statistics for a workshop
    async fn get_workshop_evaluation_stats(
        &self,
        workshop_id: Uuid,
    ) -> DomainResult<HashMap<String, i64>>; // Map of rating -> count
    
    /// Batch add participants to a workshop
    async fn batch_add_participants(
        &self,
        workshop_id: Uuid,
        participant_ids: &[Uuid],
        auth: &AuthContext,
    ) -> DomainResult<Vec<(Uuid, Result<(), DomainError>)>>; // Returns results by participant ID
    
    /// Find participants with missing evaluations
    async fn find_participants_with_missing_evaluations(
        &self,
        workshop_id: Uuid,
        eval_type: &str, // "pre" or "post"
    ) -> DomainResult<Vec<ParticipantSummary>>;
    
    /// Find a workshop-participant link by its own unique ID
    async fn find_by_id(&self, id: Uuid) -> DomainResult<WorkshopParticipant>;

    /// Hard delete a workshop-participant link by its own unique ID within a transaction
    async fn hard_delete_link_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        auth: &AuthContext, // Auth context for potential future use (logging, specific checks)
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()>;
}

/// SQLite implementation for WorkshopParticipantRepository
#[derive(Clone)]
pub struct SqliteWorkshopParticipantRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl std::fmt::Debug for SqliteWorkshopParticipantRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteWorkshopParticipantRepository")
            .field("pool", &self.pool)
            .field("change_log_repo", &"<ChangeLogRepository>")
            .finish()
    }
}

impl SqliteWorkshopParticipantRepository {
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self { // Updated constructor
        Self { pool, change_log_repo }
    }

    fn map_row_to_entity(row: WorkshopParticipantRow) -> DomainResult<WorkshopParticipant> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }
    
    /// Find link by IDs within a transaction (helper)
    async fn find_link_by_ids_with_tx<'t>(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<WorkshopParticipant> {
         let row = query_as::<_, WorkshopParticipantRow>(
            "SELECT * FROM workshop_participants WHERE workshop_id = ? AND participant_id = ? AND deleted_at IS NULL"
        )
        .bind(workshop_id.to_string())
        .bind(participant_id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(
            format!("WorkshopParticipant link between {} and {}", workshop_id, participant_id),
            workshop_id // Use one ID for the error, or create a composite key representation
        ))?;
        Self::map_row_to_entity(row)
    }

    /// Helper to log changes
    async fn log_change(
        &self,
        entity_id: Uuid,
        operation: ChangeOperationType,
        details: Option<serde_json::Value>,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let log_entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: "workshop_participants".to_string(),
            entity_id,
            operation_type: operation,
            field_name: None,
            old_value: None,
            new_value: if operation == ChangeOperationType::Create { details.as_ref().map(|d| d.to_string()) } else { None },
            document_metadata: None,
            timestamp: Utc::now(),
            user_id: auth.user_id,
            device_id: Uuid::parse_str(&auth.device_id).ok(),
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.change_log_repo.create_change_log(&log_entry).await
    }

    async fn upsert_remote_state_with_tx<'t>(&self, tx: &mut Transaction<'t, Sqlite>, remote: &WorkshopParticipant) -> DomainResult<()> {
        sqlx::query(
            r#"
INSERT OR REPLACE INTO workshop_participants (
    id, workshop_id, participant_id,
    pre_evaluation, pre_evaluation_updated_at, pre_evaluation_updated_by, pre_evaluation_updated_by_device_id,
    post_evaluation, post_evaluation_updated_at, post_evaluation_updated_by, post_evaluation_updated_by_device_id,
    created_at, updated_at, created_by_user_id, created_by_device_id, updated_by_user_id, updated_by_device_id,
    deleted_at, deleted_by_user_id, deleted_by_device_id
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(remote.id.to_string())
        .bind(remote.workshop_id.to_string())
        .bind(remote.participant_id.to_string())
        .bind(remote.pre_evaluation.as_deref())
        .bind(remote.pre_evaluation_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.pre_evaluation_updated_by.map(|u| u.to_string()))
        .bind(remote.pre_evaluation_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.post_evaluation.as_deref())
        .bind(remote.post_evaluation_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.post_evaluation_updated_by.map(|u| u.to_string()))
        .bind(remote.post_evaluation_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.created_at.to_rfc3339())
        .bind(remote.updated_at.to_rfc3339())
        .bind(remote.created_by_user_id.map(|u| u.to_string()))
        .bind(remote.created_by_device_id.map(|u| u.to_string()))
        .bind(remote.updated_by_user_id.map(|u| u.to_string()))
        .bind(remote.updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.deleted_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.deleted_by_user_id.map(|u| u.to_string()))
        .bind(remote.deleted_by_device_id.map(|u| u.to_string()))
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }
}

// SQLite raw row type for participant summary
struct ParticipantSummaryRow {
    id: String,
    name: String,
    gender: Option<String>,
    age_group: Option<String>,
    disability: bool,
    pre_evaluation: Option<String>,  // We know these are NULL in the query
    post_evaluation: Option<String>, // We know these are NULL in the query
}

#[async_trait]
impl WorkshopParticipantRepository for SqliteWorkshopParticipantRepository {
    async fn add_participant(
        &self,
        new_link: &NewWorkshopParticipant,
        auth: &AuthContext,
    ) -> DomainResult<WorkshopParticipant> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let workshop_id_str = new_link.workshop_id.to_string();
        let participant_id_str = new_link.participant_id.to_string();

        // Use INSERT OR IGNORE to handle potential UNIQUE constraint violation gracefully
        // If the link already exists (even soft-deleted), this won't insert.
        // We might need to handle undeleting an existing soft-deleted link later.
        let result = query(
            r#"
            INSERT OR IGNORE INTO workshop_participants (
                id, workshop_id, participant_id,
                pre_evaluation, pre_evaluation_updated_at, pre_evaluation_updated_by,
                post_evaluation, post_evaluation_updated_at, post_evaluation_updated_by,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, ?, 
                ?, ?, ?, ?, ?, ?, 
                ?, ?, ?, ?, 
                NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(&workshop_id_str)
        .bind(&participant_id_str)
        .bind(&new_link.pre_evaluation)
        .bind(new_link.pre_evaluation.as_ref().map(|_| &now_str)).bind(new_link.pre_evaluation.as_ref().map(|_| &user_id_str)) // pre_evaluation LWW
        .bind(&new_link.post_evaluation)
        .bind(new_link.post_evaluation.as_ref().map(|_| &now_str)).bind(new_link.post_evaluation.as_ref().map(|_| &user_id_str)) // post_evaluation LWW
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&user_id_str).bind(&user_id_str) // created_by, updated_by
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Check if insert happened or if it was ignored (already exists)
        if result.rows_affected() > 0 {
             // Fetch the newly created link by its generated ID
             let row = query_as::<_, WorkshopParticipantRow>(
                 "SELECT * FROM workshop_participants WHERE id = ?"
             )
             .bind(id.to_string())
             .fetch_one(&self.pool)
             .await
             .map_err(DbError::from)?;
             let entity = Self::map_row_to_entity(row)?;

             // Log change
             let details = serde_json::json!({
                 "workshop_id": entity.workshop_id,
                 "participant_id": entity.participant_id,
                 "pre_evaluation": entity.pre_evaluation,
                 "post_evaluation": entity.post_evaluation,
             });
             self.log_change(entity.id, ChangeOperationType::Create, Some(details), auth).await?;

             Ok(entity)
        } else {
             // Link already exists (potentially soft-deleted), try fetching it
             // Or return a specific conflict error
             match self.find_link_by_ids(new_link.workshop_id, new_link.participant_id).await {
                 Ok(Some(_existing_link)) => {
                     // If it was soft-deleted, maybe we should undelete it here?
                     // For now, just return a validation error
                     Err(DomainError::Validation(ValidationError::custom(
                         &format!("Participant {} already linked to workshop {}", new_link.participant_id, new_link.workshop_id)
                     )))
                 },
                 Ok(None) => Err(DomainError::Internal("Failed to retrieve existing workshop participant link after INSERT IGNORE".to_string())),
                 Err(e) => Err(e), // Propagate other errors
             }
        }
    }

    async fn remove_participant(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        let deleted_by_device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        
        let result = query(
            r#"
        UPDATE workshop_participants SET 
         deleted_at = ?, 
         deleted_by_user_id = ?,
         deleted_by_device_id = ?, -- Added for device ID
         updated_at = ?, -- Also update the main updated_at timestamp
         updated_by_user_id = ?, -- and who updated it (the deleter)
         updated_by_device_id = ?  -- and which device did it
        WHERE workshop_id = ? AND participant_id = ? AND deleted_at IS NULL
        "#
    )
    .bind(&now)
    .bind(&deleted_by)
    .bind(&deleted_by_device_id_str) // Bind device_id
    .bind(&now) // For updated_at
    .bind(&deleted_by) // For updated_by_user_id
    .bind(&deleted_by_device_id_str) // For updated_by_device_id
    .bind(workshop_id.to_string())
    .bind(participant_id.to_string())
    .execute(&self.pool)
    .await
    .map_err(DbError::from)?;


        if result.rows_affected() == 0 {
            // Link might not exist or was already deleted
            // Check if it exists at all first
             match self.find_link_by_ids(workshop_id, participant_id).await {
                 Ok(Some(_)) => Ok(()), // Already soft-deleted, operation is idempotent
                 Ok(None) => Err(DomainError::EntityNotFound(
                     format!("WorkshopParticipant link between {} and {}", workshop_id, participant_id),
                     workshop_id // Or composite key
                 )),
                 Err(e) => Err(e), // Propagate DB error
             }
        } else {
            //cascade in database handles the soft delete of the participant
            Ok(())
        }
    }

    async fn update_participant_evaluation(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
        update_data: &UpdateWorkshopParticipant,
        auth: &AuthContext,
    ) -> DomainResult<WorkshopParticipant> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        // Capture initial state *before* update for comparison (optional, for detailed logging)
        let initial_state = self.find_link_by_ids_with_tx(workshop_id, participant_id, &mut tx).await;
        
        let result = async {
             // Fetch the link first to ensure it exists (not soft-deleted)
             let current_link = match initial_state {
                 Ok(link) => link,
                 Err(e) => return Err(e), // Propagate error if fetch failed
             };
             
             let now = Utc::now();
             let now_str = now.to_rfc3339();
             let user_id_str = auth.user_id.to_string();
             
             let mut set_clauses = Vec::new();
             let mut params = Vec::new();
             
             // LWW for pre_evaluation
             if let Some(val) = &update_data.pre_evaluation {
                 set_clauses.push("pre_evaluation = ?");
                 set_clauses.push("pre_evaluation_updated_at = ?");
                 set_clauses.push("pre_evaluation_updated_by = ?");
                 params.push(Some(val.clone()));
                 params.push(Some(now_str.clone()));
                 params.push(Some(user_id_str.clone()));
             }
             
             // LWW for post_evaluation
              if let Some(val) = &update_data.post_evaluation {
                 set_clauses.push("post_evaluation = ?");
                 set_clauses.push("post_evaluation_updated_at = ?");
                 set_clauses.push("post_evaluation_updated_by = ?");
                 params.push(Some(val.clone()));
                 params.push(Some(now_str.clone()));
                 params.push(Some(user_id_str.clone()));
             }
             
             if set_clauses.is_empty() {
                  // No fields to update, just return current state
                  return Ok(current_link);
             }
             
             // Always update main timestamp and user
             set_clauses.push("updated_at = ?");
             params.push(Some(now_str.clone()));
             set_clauses.push("updated_by_user_id = ?");
             params.push(Some(user_id_str.clone()));
             
             let query_str = format!(
                 "UPDATE workshop_participants SET {} WHERE workshop_id = ? AND participant_id = ? AND deleted_at IS NULL",
                 set_clauses.join(", ")
             );
             
             let mut query_builder = query(&query_str);
             for param in params {
                 query_builder = query_builder.bind(param);
             }
             // Bind WHERE clause params
             query_builder = query_builder.bind(workshop_id.to_string());
             query_builder = query_builder.bind(participant_id.to_string());
             
             let update_result = query_builder.execute(&mut *tx).await.map_err(DbError::from)?;
             
             if update_result.rows_affected() == 0 {
                  // Should not happen if find_link_by_ids_with_tx succeeded, but handle defensively
                  return Err(DomainError::Internal("Failed to update workshop participant link after existence check".to_string()));
             }

            // Fetch and return the updated entity
            self.find_link_by_ids_with_tx(workshop_id, participant_id, &mut tx).await
        }.await;

        match result {
            Ok(updated_link) => {
                // Log change on successful commit
                let details = serde_json::json!({
                    "workshop_id": workshop_id,
                    "participant_id": participant_id,
                    "updated_fields": update_data, // Log what was attempted to be updated
                    // Optionally include before/after state if `initial_state` was captured successfully
                });
                self.log_change(updated_link.id, ChangeOperationType::Update, Some(details), auth).await?;
                
                tx.commit().await.map_err(DbError::from)?;
                Ok(updated_link)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn find_participants_for_workshop(
        &self,
        workshop_id: Uuid,
    ) -> DomainResult<Vec<ParticipantSummary>> {
        // Use a manual query approach instead of query_as! to avoid NULL type issues
        let rows = sqlx::query(
            r#"
            SELECT 
                p.id, p.name, p.gender, p.age_group, p.disability,
                wp.pre_evaluation, wp.post_evaluation -- <-- Also select evaluations
            FROM workshop_participants wp
            JOIN participants p ON wp.participant_id = p.id
            WHERE wp.workshop_id = ? AND wp.deleted_at IS NULL AND p.deleted_at IS NULL
            ORDER BY p.name ASC
            "#
        )
        .bind(workshop_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Manually map rows to ParticipantSummary
        let summaries = rows
            .into_iter()
            .map(|row| {
                let id_str: String = row.get("id");
                let disability_val: i64 = row.get("disability"); // Read as integer
                Ok(ParticipantSummary {
                    id: Uuid::parse_str(&id_str)
                        .map_err(|_| DomainError::InvalidUuid(id_str))?,
                    name: row.get("name"),
                    gender: row.get("gender"),
                    age_group: row.get("age_group"),
                    disability: disability_val != 0, // Convert to bool
                    pre_evaluation: row.get("pre_evaluation"), // <-- Map pre_evaluation
                    post_evaluation: row.get("post_evaluation"), // <-- Map post_evaluation
                })
            })
            .collect::<DomainResult<Vec<_>>>()?;

        Ok(summaries)
    }
    
    async fn find_link_by_ids(
        &self,
        workshop_id: Uuid,
        participant_id: Uuid,
    ) -> DomainResult<Option<WorkshopParticipant>> {
         let row = query_as::<_, WorkshopParticipantRow>(
            "SELECT * FROM workshop_participants WHERE workshop_id = ? AND participant_id = ? AND deleted_at IS NULL"
        )
        .bind(workshop_id.to_string())
        .bind(participant_id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        row.map(Self::map_row_to_entity).transpose()
    }

    async fn find_participants_with_details(
        &self,
        workshop_id: Uuid,
    ) -> DomainResult<Vec<ParticipantDetail>> {
        let workshop_id_str = workshop_id.to_string();
        
        // Join tables to get participant details with evaluation data
        let rows = sqlx::query(
            r#"
            SELECT 
                p.id, 
                p.name, 
                p.gender, 
                p.age_group, 
                p.disability,
                p.disability_type,
                wp.pre_evaluation,
                wp.post_evaluation
            FROM 
                participants p
            JOIN 
                workshop_participants wp ON p.id = wp.participant_id
            WHERE 
                wp.workshop_id = ? 
                AND wp.deleted_at IS NULL 
                AND p.deleted_at IS NULL
            ORDER BY 
                p.name ASC
            "#
        )
        .bind(&workshop_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Map rows to ParticipantDetail objects
        let mut details = Vec::new();
        for row in rows {
            let id_str: String = row.get("id");
            let id = Uuid::parse_str(&id_str)
                .map_err(|_| DomainError::Internal(format!("Invalid UUID format: {}", id_str)))?;
                
            let pre_evaluation: Option<String> = row.get("pre_evaluation");
            let post_evaluation: Option<String> = row.get("post_evaluation");
            let disability: i64 = row.get("disability");
            
            details.push(ParticipantDetail {
                id,
                name: row.get("name"),
                gender: row.get("gender"),
                age_group: row.get("age_group"),
                disability: disability != 0,
                disability_type: row.get("disability_type"),
                pre_evaluation: pre_evaluation.clone(),
                post_evaluation: post_evaluation.clone(),
                evaluation_complete: pre_evaluation.is_some() && post_evaluation.is_some(),
            });
        }
        
        Ok(details)
    }
    
    async fn get_evaluation_completion_counts(
        &self,
        workshop_id: Uuid,
    ) -> DomainResult<(i64, i64, i64)> {
        let workshop_id_str = workshop_id.to_string();
        
        let counts = query_as::<_, (i64, i64, i64)>(
            r#"
            SELECT 
                COUNT(*) as total,
                COUNT(CASE WHEN pre_evaluation IS NOT NULL THEN 1 END) as pre_eval_count,
                COUNT(CASE WHEN post_evaluation IS NOT NULL THEN 1 END) as post_eval_count
            FROM 
                workshop_participants
            WHERE 
                workshop_id = ? AND deleted_at IS NULL
            "#
        )
        .bind(&workshop_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        Ok(counts)
    }
    
    async fn find_workshops_for_participant(
        &self,
        participant_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<WorkshopSummary>> {
        let offset = (params.page - 1) * params.per_page;
        let participant_id_str = participant_id.to_string();
        
        // Get total count
        let total: i64 = query_scalar(
            r#"
            SELECT COUNT(*) 
            FROM workshop_participants wp
            JOIN workshops w ON wp.workshop_id = w.id
            WHERE wp.participant_id = ? 
            AND wp.deleted_at IS NULL 
            AND w.deleted_at IS NULL
            "#
        )
        .bind(&participant_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Fetch workshops with pagination
        let rows = query(
            r#"
            SELECT 
                w.id, 
                w.purpose, 
                w.event_date, 
                w.location, 
                w.participant_count
            FROM 
                workshop_participants wp
            JOIN 
                workshops w ON wp.workshop_id = w.id
            WHERE 
                wp.participant_id = ? 
                AND wp.deleted_at IS NULL 
                AND w.deleted_at IS NULL
            ORDER BY 
                w.event_date DESC
            LIMIT ? OFFSET ?
            "#
        )
        .bind(&participant_id_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Map results to workshop summaries
        let mut summaries = Vec::new();
        for row in rows {
            let id_str: String = row.get("id");
            let id = Uuid::parse_str(&id_str)
                .map_err(|_| DomainError::Internal(format!("Invalid UUID format: {}", id_str)))?;
                
            summaries.push(WorkshopSummary {
                id,
                purpose: row.get("purpose"),
                event_date: row.get("event_date"),
                location: row.get("location"),
                participant_count: row.get("participant_count"),
            });
        }
        
        Ok(PaginatedResult::new(
            summaries,
            total as u64,
            params,
        ))
    }
    
    async fn get_participant_attendance(
        &self,
        participant_id: Uuid,
    ) -> DomainResult<ParticipantAttendance> {
        let participant_id_str = participant_id.to_string();
        let today = Local::now().naive_local().date().format("%Y-%m-%d").to_string();
        
        // Get participant name
        let participant_name = query_scalar::<_, String>(
            "SELECT name FROM participants WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(&participant_id_str)
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Participant".to_string(), participant_id))?;
        
        // Get attendance stats
        let (workshops_attended, workshops_upcoming, total_workshops, completed_evals) = 
            query_as::<_, (i64, i64, i64, i64)>(
                r#"
                SELECT 
                    COUNT(CASE WHEN w.event_date < ? THEN 1 END) as attended,
                    COUNT(CASE WHEN w.event_date >= ? THEN 1 END) as upcoming,
                    COUNT(*) as total,
                    COUNT(CASE WHEN wp.pre_evaluation IS NOT NULL AND wp.post_evaluation IS NOT NULL THEN 1 END) as completed_evals
                FROM 
                    workshop_participants wp
                JOIN 
                    workshops w ON wp.workshop_id = w.id
                WHERE 
                    wp.participant_id = ? 
                    AND wp.deleted_at IS NULL 
                    AND w.deleted_at IS NULL
                "#
            )
            .bind(&today)
            .bind(&today)
            .bind(&participant_id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;
        
        // Calculate evaluation completion rate
        let evaluation_completion_rate = if workshops_attended > 0 {
            (completed_evals as f64 / workshops_attended as f64) * 100.0
        } else {
            0.0
        };
        
        // Get recent workshops (most recent 3)
        let recent_workshops = query(
            r#"
            SELECT 
                w.id, 
                w.purpose, 
                w.event_date, 
                w.location, 
                w.participant_count
            FROM 
                workshop_participants wp
            JOIN 
                workshops w ON wp.workshop_id = w.id
            WHERE 
                wp.participant_id = ? 
                AND wp.deleted_at IS NULL 
                AND w.deleted_at IS NULL
            ORDER BY 
                w.event_date DESC
            LIMIT 3
            "#
        )
        .bind(&participant_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Map to workshop summaries
        let mut recent_workshop_summaries = Vec::new();
        for row in recent_workshops {
            let id_str: String = row.get("id");
            let id = Uuid::parse_str(&id_str)
                .map_err(|_| DomainError::Internal(format!("Invalid UUID format: {}", id_str)))?;
                
            recent_workshop_summaries.push(WorkshopSummary {
                id,
                purpose: row.get("purpose"),
                event_date: row.get("event_date"),
                location: row.get("location"),
                participant_count: row.get("participant_count"),
            });
        }
        
        Ok(ParticipantAttendance {
            participant_id,
            participant_name,
            workshops_attended,
            workshops_upcoming,
            evaluation_completion_rate, // Rate as percentage
            recent_workshops: recent_workshop_summaries,
        })
    }

    async fn get_workshop_evaluation_stats(
        &self,
        workshop_id: Uuid,
    ) -> DomainResult<HashMap<String, i64>> {
        let workshop_id_str = workshop_id.to_string();
        // Assuming evaluations are stored as text like '1', '2', '3', '4', '5'
        // This aggregates counts for pre-evaluations. Adapt if post-eval or other fields needed.
        let rows = query_as::<_, (Option<String>, i64)>(
            r#"
            SELECT 
                pre_evaluation as rating, 
                COUNT(*) as count
            FROM 
                workshop_participants
            WHERE 
                workshop_id = ? 
                AND deleted_at IS NULL 
                AND pre_evaluation IS NOT NULL
            GROUP BY 
                pre_evaluation
            "#
        )
        .bind(&workshop_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let mut stats = HashMap::new();
        for (rating_opt, count) in rows {
            if let Some(rating) = rating_opt {
                stats.insert(rating, count);
            }
        }

        Ok(stats)
    }

    async fn batch_add_participants(
        &self,
        workshop_id: Uuid,
        participant_ids: &[Uuid],
        auth: &AuthContext,
    ) -> DomainResult<Vec<(Uuid, Result<(), DomainError>)>> {
        if participant_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let mut results = Vec::new();
        let workshop_id_str = workshop_id.to_string();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        for &participant_id in participant_ids {
            let participant_id_str = participant_id.to_string();
            let link_id = Uuid::new_v4();

            let insert_result = query(
                r#"
                INSERT INTO workshop_participants (
                    id, workshop_id, participant_id,
                    pre_evaluation, pre_evaluation_updated_at, pre_evaluation_updated_by,
                    post_evaluation, post_evaluation_updated_at, post_evaluation_updated_by,
                    created_at, updated_at, created_by_user_id, updated_by_user_id,
                    deleted_at, deleted_by_user_id
                ) VALUES (
                    ?, ?, ?, 
                    NULL, NULL, NULL, 
                    NULL, NULL, NULL, 
                    ?, ?, ?, ?, 
                    NULL, NULL
                ) ON CONFLICT(workshop_id, participant_id) DO UPDATE SET
                    deleted_at = NULL, -- Undelete if previously deleted
                    deleted_by_user_id = NULL,
                    updated_at = excluded.updated_at,
                    updated_by_user_id = excluded.updated_by_user_id
                    -- Optionally update evaluations if they were passed in batch, currently NULLed
                "#,
            )
            .bind(link_id.to_string())
            .bind(&workshop_id_str)
            .bind(&participant_id_str)
            .bind(&now_str) // created_at / updated_at for excluded
            .bind(&now_str)
            .bind(&user_id_str) // created_by / updated_by for excluded
            .bind(&user_id_str)
            .execute(&mut *tx)
            .await;

            match insert_result {
                Ok(_) => results.push((participant_id, Ok(()))),
                Err(sqlx::Error::Database(db_err)) if db_err.message().contains("UNIQUE constraint failed") => {
                    // This case should theoretically be handled by ON CONFLICT, but check just in case.
                    results.push((participant_id, Err(DbError::Conflict("Workshop participant link already exists".into()).into())))
                }
                Err(e) => results.push((participant_id, Err(DbError::from(e).into()))),
            }
        }

        match tx.commit().await {
            Ok(_) => Ok(results),
            Err(e) => {
                // Commit failed, return errors for all as uncertain
                let commit_error: DomainError = DbError::Transaction(format!("Commit failed after batch add: {}", e)).into();
                Ok(participant_ids.iter().map(|&pid| (pid, Err(commit_error.clone()))).collect())
            }
        }
    }

    async fn find_participants_with_missing_evaluations(
        &self,
        workshop_id: Uuid,
        eval_type: &str, // "pre" or "post"
    ) -> DomainResult<Vec<ParticipantSummary>> {
        let workshop_id_str = workshop_id.to_string();
        
        let condition = match eval_type {
            "pre" => "wp.pre_evaluation IS NULL",
            "post" => "wp.post_evaluation IS NULL",
            _ => return Err(DomainError::Validation(ValidationError::invalid_value(
                "eval_type", "must be 'pre' or 'post'"
            ))),
        };

        let query_str = format!(
            r#"
            SELECT 
                p.id, p.name, p.gender, p.age_group, p.disability,
                wp.pre_evaluation, wp.post_evaluation
            FROM workshop_participants wp
            JOIN participants p ON wp.participant_id = p.id
            WHERE wp.workshop_id = ? 
            AND {} 
            AND wp.deleted_at IS NULL 
            AND p.deleted_at IS NULL
            ORDER BY p.name ASC
            "#,
            condition
        );

        let rows = sqlx::query(&query_str)
            .bind(&workshop_id_str)
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Map to ParticipantSummary
        let summaries = rows
            .into_iter()
            .map(|row| {
                let id_str: String = row.get("id");
                let disability_val: i64 = row.get("disability");
                Ok(ParticipantSummary {
                    id: Uuid::parse_str(&id_str)
                        .map_err(|_| DomainError::InvalidUuid(id_str))?,
                    name: row.get("name"),
                    gender: row.get("gender"),
                    age_group: row.get("age_group"),
                    disability: disability_val != 0,
                    pre_evaluation: row.get("pre_evaluation"),
                    post_evaluation: row.get("post_evaluation"),
                })
            })
            .collect::<DomainResult<Vec<_>>>()?;

        Ok(summaries)
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<WorkshopParticipant> {
        let row = query_as::<_, WorkshopParticipantRow>(
            "SELECT * FROM workshop_participants WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("WorkshopParticipant".to_string(), id))?;
        Self::map_row_to_entity(row)
    }

    async fn hard_delete_link_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        _auth: &AuthContext, // Auth context for potential future use (logging, specific checks)
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM workshop_participants WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("WorkshopParticipant".to_string(), id))
        } else {
            Ok(())
        }
    }
}

#[async_trait]
impl MergeableEntityRepository<WorkshopParticipant> for SqliteWorkshopParticipantRepository {
    fn entity_name(&self) -> &'static str { "workshop_participants" }
    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                let state_json = remote_change.new_value.as_ref().ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for workshop_participant change")))?;
                let remote_state: WorkshopParticipant = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_workshop_participant", &format!("Invalid JSON: {}", e))))?;
                let local_opt = match self.find_link_by_ids_with_tx(remote_state.workshop_id, remote_state.participant_id, tx).await {
                    Ok(ent) => Some(ent),
                    Err(DomainError::EntityNotFound(_, _)) => None,
                    Err(e) => return Err(e),
                };
                if let Some(local) = local_opt.clone() {
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