use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, QueryBuilder};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::participant::types::{NewParticipant, Participant, ParticipantRow, UpdateParticipant};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;
use crate::domains::sync::types::SyncPriority;

/// Trait defining participant repository operations
#[async_trait]
pub trait ParticipantRepository: DeleteServiceRepository<Participant> + Send + Sync {
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
    
    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;
    
    async fn set_document_reference(
        &self,
        participant_id: Uuid,
        field_name: &str, // e.g., "profile_photo"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
}

/// SQLite implementation for ParticipantRepository
#[derive(Debug, Clone)]
pub struct SqliteParticipantRepository {
    pool: SqlitePool,
}

impl SqliteParticipantRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn map_row_to_entity(row: ParticipantRow) -> DomainResult<Participant> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
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
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        
        let result = query(
            "UPDATE participants SET deleted_at = ?, deleted_by_user_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(deleted_by)
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
        _auth: &AuthContext, 
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        // Note: Cascade delete for workshop_participants/livelihoods handled by DB
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
        let user_id_str = auth.user_id.to_string();
        // Convert bool option to i64 option (0 or 1)
        let disability_val = new_participant.disability.map(|d| if d { 1_i64 } else { 0_i64 });

        query(
            r#"
            INSERT INTO participants (
                id, name, name_updated_at, name_updated_by,
                gender, gender_updated_at, gender_updated_by,
                disability, disability_updated_at, disability_updated_by,
                disability_type, disability_type_updated_at, disability_type_updated_by,
                age_group, age_group_updated_at, age_group_updated_by,
                location, location_updated_at, location_updated_by,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(&new_participant.name).bind(&now_str).bind(&user_id_str) // name LWW
        .bind(&new_participant.gender).bind(new_participant.gender.as_ref().map(|_| &now_str)).bind(new_participant.gender.as_ref().map(|_| &user_id_str)) // gender LWW
        .bind(disability_val.unwrap_or(0)).bind(disability_val.map(|_| &now_str)).bind(disability_val.map(|_| &user_id_str)) // disability LWW (handle None -> default 0)
        .bind(&new_participant.disability_type).bind(new_participant.disability_type.as_ref().map(|_| &now_str)).bind(new_participant.disability_type.as_ref().map(|_| &user_id_str)) // disability_type LWW
        .bind(&new_participant.age_group).bind(new_participant.age_group.as_ref().map(|_| &now_str)).bind(new_participant.age_group.as_ref().map(|_| &user_id_str)) // age_group LWW
        .bind(&new_participant.location).bind(new_participant.location.as_ref().map(|_| &now_str)).bind(new_participant.location.as_ref().map(|_| &user_id_str)) // location LWW
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&user_id_str).bind(&user_id_str) // created_by, updated_by
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

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
        let _current_participant = self.find_by_id_with_tx(id, tx).await?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        let mut set_clauses = Vec::new();
        let mut params: Vec<Option<String>> = Vec::new();

        macro_rules! add_lww_update {
            ($field:ident, $value:expr, string) => {
                if let Some(val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    params.push(Some(val.to_string()));
                    params.push(Some(now_str.clone()));
                    params.push(Some(user_id_str.clone()));
                }
            };
             ($field:ident, $value:expr, bool_to_i64) => {
                 if let Some(val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    let i64_val = if *val { 1 } else { 0 };
                    params.push(Some(i64_val.to_string()));
                    params.push(Some(now_str.clone()));
                    params.push(Some(user_id_str.clone()));
                 }
             };
        }
        
        add_lww_update!(name, &update_data.name, string);
        add_lww_update!(gender, &update_data.gender, string);
        add_lww_update!(disability, &update_data.disability, bool_to_i64);
        add_lww_update!(disability_type, &update_data.disability_type, string);
        add_lww_update!(age_group, &update_data.age_group, string);
        add_lww_update!(location, &update_data.location, string);

        set_clauses.push("updated_at = ?".to_string());
        params.push(Some(now_str.clone()));
        set_clauses.push("updated_by_user_id = ?".to_string());
        params.push(Some(user_id_str.clone()));
        
        if set_clauses.len() <= 2 { 
             return Ok(_current_participant);
        }

        let query_str = format!(
            "UPDATE participants SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );
        
        let mut query_builder = query(&query_str);
        for param in params {
            query_builder = query_builder.bind(param);
        }
        query_builder = query_builder.bind(id.to_string());

        let result = query_builder
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound("Participant".to_string(), id));
        }
        
        self.find_by_id_with_tx(id, tx).await
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
    
    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() {
            return Ok(0);
        }
        
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let priority_val = priority as i64;
        
        let mut builder = QueryBuilder::new("UPDATE participants SET ");
        builder.push("sync_priority = ");
        builder.push_bind(priority_val);
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        
        // Build the WHERE clause with IN condition
        builder.push(" WHERE id IN (");
        let mut id_separated = builder.separated(",");
        for id in ids {
            id_separated.push_bind(id.to_string());
        }
        id_separated.push_unseparated(")"); // Correctly close parenthesis
        builder.push(" AND deleted_at IS NULL");
        
        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;
        
        Ok(result.rows_affected())
    }
    
    async fn set_document_reference(
        &self,
        participant_id: Uuid,
        field_name: &str, // e.g., "profile_photo"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        // Construct the database column name (e.g., "profile_photo_ref")
        let column_name = format!("{}_ref", field_name); 
        
        // Validate the field name against the metadata
        if !Participant::field_metadata().iter().any(|m| m.field_name == field_name && m.is_document_reference_only) {
             return Err(DomainError::Validation(ValidationError::custom(&format!("Invalid document reference field for Participant: {}", field_name))));
        }

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let document_id_str = document_id.to_string();
        
        // Use QueryBuilder for dynamic column name safety
        let mut builder = sqlx::QueryBuilder::new("UPDATE participants SET ");
        builder.push(&column_name); // Push the validated column name
        builder.push(" = ");
        builder.push_bind(document_id_str);
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        builder.push(" WHERE id = ");
        builder.push_bind(participant_id.to_string());
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // Participant might not exist or was deleted
            Err(DomainError::EntityNotFound("Participant".to_string(), participant_id))
        } else {
            Ok(())
        }
    }
}
