use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, QueryBuilder};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::participant::types::{
    NewParticipant, Participant, ParticipantRow, UpdateParticipant, ParticipantDemographics, 
    WorkshopSummary, LivelihoodSummary
};
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::{Utc, Local};
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

    /// Find participants within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: chrono::DateTime<chrono::Utc>,
        end_date: chrono::DateTime<chrono::Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>>;
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
        .map_err(DbError::from)?
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
        let row = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Participant".to_string(), id))?;

        Self::map_row_to_entity(row)
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
            Err(DomainError::EntityNotFound("Participant".to_string(), id))
        } else {
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
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM participants WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Participant".to_string(), id))
        } else {
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
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.create_with_tx(new_participant, auth, &mut tx).await {
            Ok(participant) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(participant)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
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

        query(
            r#"
            INSERT INTO participants (
                id, 
                name, name_updated_at, name_updated_by, name_updated_by_device_id,
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
                ?, 
                ?, ?, ?, ?, 
                ?, ?, ?, ?, 
                ?, ?, ?, ?, 
                ?, ?, ?, ?, 
                ?, ?, ?, ?, 
                ?, ?, ?, ?, 
                ?, 
                ?, ?, ?, ?, 
                ?, ?,
                NULL, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(&new_participant.name).bind(&now_str).bind(&user_id_str).bind(&device_id_str) // Name LWW
        .bind(&new_participant.gender).bind(new_participant.gender.as_ref().map(|_| &now_str)).bind(new_participant.gender.as_ref().map(|_| &user_id_str)).bind(new_participant.gender.as_ref().map(|_| &device_id_str)) // Gender LWW
        .bind(new_participant.disability.unwrap_or(false)).bind(new_participant.disability.map(|_| &now_str)).bind(new_participant.disability.map(|_| &user_id_str)).bind(new_participant.disability.map(|_| &device_id_str)) // Disability LWW
        .bind(&new_participant.disability_type).bind(new_participant.disability_type.as_ref().map(|_| &now_str)).bind(new_participant.disability_type.as_ref().map(|_| &user_id_str)).bind(new_participant.disability_type.as_ref().map(|_| &device_id_str)) // Disability Type LWW
        .bind(&new_participant.age_group).bind(new_participant.age_group.as_ref().map(|_| &now_str)).bind(new_participant.age_group.as_ref().map(|_| &user_id_str)).bind(new_participant.age_group.as_ref().map(|_| &device_id_str)) // Age Group LWW
        .bind(&new_participant.location).bind(new_participant.location.as_ref().map(|_| &now_str)).bind(new_participant.location.as_ref().map(|_| &user_id_str)).bind(new_participant.location.as_ref().map(|_| &device_id_str)) // Location LWW
        .bind(new_participant.sync_priority.unwrap_or_default().as_str()) // sync_priority as TEXT
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&created_by_id_str).bind(&user_id_str) // created_by, updated_by
        .bind(&device_id_str).bind(&device_id_str) // created_by_device_id, updated_by_device_id
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        // Log Create Operation
        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: self.entity_name().to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create,
            field_name: None,
            old_value: None,
            new_value: None, // Optionally serialize the whole new_participant
            timestamp: now,
            user_id: user_id,
            device_id: device_uuid,
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.log_change_entry(entry, tx).await?;

        // Fetch the created participant to return it
        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateParticipant,
        auth: &AuthContext,
    ) -> DomainResult<Participant> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.update_with_tx(id, update_data, auth, &mut tx).await {
            Ok(participant) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(participant)
            }
            Err(e) => {
                let _ = tx.rollback().await;
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
        // --- Fetch Old State --- 
        let old_entity = self.find_by_id_with_tx(id, tx).await?;
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let id_str = id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());

        let mut builder = QueryBuilder::new("UPDATE participants SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        // --- Update LWW Macro (No comparison here, just SQL build) --- 
        macro_rules! add_lww {
            ($field_name:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value { // Check if update DTO has this field
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone()); // Bind value
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone()); // Bind timestamp
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone()); // Bind user
                    separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                    separated.push_bind_unseparated(device_id_str.clone()); // Bind device_id
                    fields_updated = true; // Mark SQL update needed
                }
            };
        }
        
        // --- Special handle for bool (disability) --- 
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

        // --- Apply updates using LWW macro --- 
        add_lww!(name, "name", &update_data.name.as_ref());
        add_lww!(gender, "gender", &update_data.gender.as_ref());
        // disability handled above
        add_lww!(disability_type, "disability_type", &update_data.disability_type.as_ref());
        add_lww!(age_group, "age_group", &update_data.age_group.as_ref());
        add_lww!(location, "location", &update_data.location.as_ref());
        
        // --- Handle Sync Priority --- 
        if let Some(priority) = update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority.as_str()); // Bind as TEXT
            fields_updated = true;
        }

        if !fields_updated {
            return Ok(old_entity); // No fields present in DTO, return old state
        }

        // --- Always update main timestamps --- 
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str.clone());
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str.clone());
        separated.push("updated_by_device_id = ");
        separated.push_bind_unseparated(device_id_str.clone());

        // --- Finalize and Execute SQL --- 
        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");
        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;
        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }

        // --- Fetch New State & Log Field Changes --- 
        let new_entity = self.find_by_id_with_tx(id, tx).await?;

        macro_rules! log_if_changed {
            ($field_name:ident, $field_sql:literal) => {
                if old_entity.$field_name != new_entity.$field_name {
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: self.entity_name().to_string(),
                        entity_id: id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some($field_sql.to_string()),
                        old_value: serde_json::to_string(&old_entity.$field_name).ok(),
                        new_value: serde_json::to_string(&new_entity.$field_name).ok(),
                        timestamp: now,
                        user_id: user_id,
                        device_id: device_uuid.clone(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.log_change_entry(entry, tx).await?;
                }
            };
        }

        log_if_changed!(name, "name");
        log_if_changed!(gender, "gender");
        log_if_changed!(disability, "disability");
        log_if_changed!(disability_type, "disability_type");
        log_if_changed!(age_group, "age_group");
        log_if_changed!(location, "location");
        // Log sync_priority change (handle Option<SyncPriority>)
        if old_entity.sync_priority.unwrap_or_default() != new_entity.sync_priority.unwrap_or_default() {
            let entry = ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: self.entity_name().to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("sync_priority".to_string()),
                old_value: serde_json::to_string(old_entity.sync_priority.unwrap_or_default().as_str()).ok(), // Log as TEXT
                new_value: serde_json::to_string(new_entity.sync_priority.unwrap_or_default().as_str()).ok(), // Log as TEXT
                timestamp: now,
                user_id: user_id,
                device_id: device_uuid,
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            };
            self.log_change_entry(entry, tx).await?;
        }

        Ok(new_entity)
    }

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Participant>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar("SELECT COUNT(*) FROM participants WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        let rows = query_as::<_, ParticipantRow>(
            "SELECT * FROM participants WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?",
        )
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
        if ids.is_empty() { return Ok(0); }
        
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        // 1. Fetch old priorities
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        let select_query = format!(
            "SELECT id, sync_priority FROM participants WHERE id IN ({})",
            vec!["?"; ids.len()].join(", ")
        );
        let mut select_builder = query_as::<_, (String, String)>(&select_query);
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        let old_priorities: std::collections::HashMap<Uuid, SyncPriority> = select_builder
            .fetch_all(&mut *tx)
            .await.map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, prio_text)| {
                match Uuid::parse_str(&id_str) {
                    Ok(id) => Some((id, SyncPriority::from_str(&prio_text).unwrap_or_default())),
                    Err(_) => None, 
                }
            }).collect();

        // 2. Perform Update
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let priority_str = priority.as_str();
        
        let mut update_builder = QueryBuilder::new("UPDATE participants SET ");
        update_builder.push("sync_priority = "); update_builder.push_bind(priority_str);
        update_builder.push(", updated_at = "); update_builder.push_bind(now_str.clone());
        update_builder.push(", updated_by_user_id = "); update_builder.push_bind(user_id_str.clone());
        update_builder.push(", updated_by_device_id = "); update_builder.push_bind(device_id_str.clone());
        update_builder.push(" WHERE id IN (");
        let mut id_separated = update_builder.separated(",");
        for id in ids { id_separated.push_bind(id.to_string()); }
        update_builder.push(") AND deleted_at IS NULL");
        
        let query = update_builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
        let rows_affected = result.rows_affected();

        // 3. Log changes
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
                        user_id: user_id,
                        device_id: device_uuid.clone(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.log_change_entry(entry, &mut tx).await?;
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
    
    async fn get_participant_demographics(&self) -> DomainResult<ParticipantDemographics> {
        // Get total participant count
        let total_participants: i64 = query_scalar(
            "SELECT COUNT(*) FROM participants WHERE deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Get gender distribution
        let gender_counts = self.count_by_gender().await?;
        let mut by_gender = HashMap::new();
        for (gender_opt, count) in gender_counts {
            let gender_name = gender_opt.unwrap_or_else(|| "Unspecified".to_string());
            by_gender.insert(gender_name, count);
        }
        
        // Get age group distribution
        let age_group_counts = self.count_by_age_group().await?;
        let mut by_age_group = HashMap::new();
        for (age_group_opt, count) in age_group_counts {
            let age_group_name = age_group_opt.unwrap_or_else(|| "Unspecified".to_string());
            by_age_group.insert(age_group_name, count);
        }
        
        // Get location distribution
        let location_counts = self.count_by_location().await?;
        let mut by_location = HashMap::new();
        for (location_opt, count) in location_counts {
            let location_name = location_opt.unwrap_or_else(|| "Unspecified".to_string());
            by_location.insert(location_name, count);
        }
        
        // Get disability distribution
        let disability_counts = self.count_by_disability().await?;
        let mut by_disability = HashMap::new();
        for (has_disability, count) in disability_counts {
            let disability_name = if has_disability { "Yes" } else { "No" }.to_string();
            by_disability.insert(disability_name, count);
        }
        
        // Get disability type distribution
        let disability_type_counts = self.count_by_disability_type().await?;
        let mut by_disability_type = HashMap::new();
        for (disability_type_opt, count) in disability_type_counts {
            let disability_type_name = disability_type_opt.unwrap_or_else(|| "Unspecified".to_string());
            by_disability_type.insert(disability_type_name, count);
        }
        
        Ok(ParticipantDemographics {
            total_participants,
            by_gender,
            by_age_group,
            by_location,
            by_disability,
            by_disability_type,
        })
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
