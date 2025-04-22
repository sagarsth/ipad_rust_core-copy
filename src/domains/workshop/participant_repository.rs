use crate::auth::AuthContext;
use sqlx::{Executor, Pool, Row, Sqlite, Transaction, SqlitePool, query, query_as};
use crate::domains::workshop::types::{
    NewWorkshopParticipant, UpdateWorkshopParticipant, WorkshopParticipant, WorkshopParticipantRow, ParticipantSummary
};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams}; // Might need pagination later
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

/// Trait defining workshop-participant relationship repository operations
#[async_trait]
pub trait WorkshopParticipantRepository: Send + Sync {
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
    
    // Hard delete might be needed for cleanup or admin tasks
    // async fn hard_delete_relationship(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()>;
}

/// SQLite implementation for WorkshopParticipantRepository
#[derive(Debug, Clone)]
pub struct SqliteWorkshopParticipantRepository {
    pool: SqlitePool,
}

impl SqliteWorkshopParticipantRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
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
             Self::map_row_to_entity(row)
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
        
        let result = query(
            r#"
            UPDATE workshop_participants SET 
             deleted_at = ?, 
             deleted_by_user_id = ?
            WHERE workshop_id = ? AND participant_id = ? AND deleted_at IS NULL
            "#
        )
        .bind(now)
        .bind(deleted_by)
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
        
        let result = async {
             // Fetch the link first to ensure it exists (not soft-deleted)
             let _current_link = self.find_link_by_ids_with_tx(workshop_id, participant_id, &mut tx).await?;
             
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
                  return Ok(_current_link);
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
            Ok(link) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(link)
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
                p.id, p.name, p.gender, p.age_group, p.disability
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
                Ok(ParticipantSummary {
                    id: Uuid::parse_str(&id_str)
                        .map_err(|_| DomainError::InvalidUuid(id_str))?,
                    name: row.get("name"),
                    gender: row.get("gender"),
                    age_group: row.get("age_group"),
                    disability: row.get("disability"),
                    pre_evaluation: None,  // These columns don't exist yet in DB
                    post_evaluation: None, // These columns don't exist yet in DB
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
} 