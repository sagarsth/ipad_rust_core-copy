use crate::auth::AuthContext;
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::livelihood::types::{Livelihood, LivelihoodRow, NewLivelihood, SubsequentGrant, SubsequentGrantRow, UpdateLivelihood, NewSubsequentGrant, UpdateSubsequentGrant};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::PaginatedResult;
use crate::types::PaginationParams;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite, Transaction, query_as, query, Row, QueryBuilder};
use uuid::Uuid;
use crate::validation::Validate;
use async_trait::async_trait;

/// Repository for livelihood entities and their subsequent grants
#[async_trait]
pub trait LivehoodRepository:
    DeleteServiceRepository<Livelihood> + Send + Sync
{
    /// Create a new livelihood
    async fn create(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood>;
    
    /// Create a new livelihood with transaction
    async fn create_with_tx<'t>(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood>;

    /// Update an existing livelihood
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood>;
    
    /// Update an existing livelihood with transaction
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood>;

    /// Find all livelihoods, optionally filtered by project or participant
    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
    ) -> DomainResult<PaginatedResult<Livelihood>>;
    
    /// Find livelihoods for a specific project
    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;
    
    /// Find livelihoods for a specific participant
    async fn find_by_participant_id(
        &self,
        participant_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>>;

    /// Set a document reference for a livelihood
    async fn set_document_reference(
        &self,
        livelihood_id: Uuid,
        field_name: &str, // e.g., "business_plan"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
}

/// Repository for subsequent grants
#[async_trait]
pub trait SubsequentGrantRepository: Send + Sync {
    /// Create a new subsequent grant
    async fn create(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Create a new subsequent grant with transaction
    async fn create_with_tx<'t>(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Update an existing subsequent grant
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Find a subsequent grant by ID
    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> DomainResult<SubsequentGrant>;
    
    /// Find all subsequent grants for a livelihood
    async fn find_by_livelihood_id(
        &self,
        livelihood_id: Uuid,
    ) -> DomainResult<Vec<SubsequentGrant>>;
    
    /// Soft delete a subsequent grant
    async fn soft_delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
    
    /// Hard delete a subsequent grant
    async fn hard_delete(
        &self,
        id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;

    /// Set a document reference for a subsequent grant
    async fn set_document_reference(
        &self,
        grant_id: Uuid,
        field_name: &str, // e.g., "grant_agreement"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
}

/// SQLite implementation of the livelihood repository
pub struct SqliteLivelihoodRepository {
    pool: Pool<Sqlite>,
}

impl SqliteLivelihoodRepository {
    /// Create a new SQLite livelihood repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
    
    /// Map a database row to a domain entity
    fn map_row_to_entity(row: LivelihoodRow) -> DomainResult<Livelihood> {
        row.into_entity()
    }
    
    /// Get the entity name for this repository
    fn entity_name(&self) -> &'static str {
        "livelihood"
    }
    
    /// Find a livelihood by ID using a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood> {
        let row = query_as::<_, LivelihoodRow>(
            "SELECT * FROM livelihoods WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<Livelihood> for SqliteLivelihoodRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Livelihood> {
        let row = query_as::<_, LivelihoodRow>(
            "SELECT * FROM livelihoods WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteLivelihoodRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        // First check if the record exists and is not already deleted
        let existing = self.find_by_id_with_tx(id, tx).await?;
        
        if existing.is_deleted() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        
        // Update the record with deleted_at and deleted_by
        let rows_affected = query(
            r#"
            UPDATE livelihoods 
            SET deleted_at = ?, deleted_by_user_id = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#
        )
        .bind(now.to_rfc3339())
        .bind(auth.user_id.to_string())
        .bind(now.to_rfc3339())
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?
        .rows_affected();
        
        if rows_affected == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        Ok(())
    }
    
    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
}

#[async_trait]
impl HardDeletable for SqliteLivelihoodRepository {
    fn entity_name(&self) -> &'static str {
        "livelihood"
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        // Check if the record exists first
        let _existing = self.find_by_id_with_tx(id, tx).await?;
        
        // Delete the record permanently
        let rows_affected = query(
            "DELETE FROM livelihoods WHERE id = ?"
        )
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?
        .rows_affected();
        
        if rows_affected == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        Ok(())
    }
    
    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
}

#[async_trait]
impl LivehoodRepository for SqliteLivelihoodRepository {
    async fn create(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.create_with_tx(new_livelihood, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
    
    async fn create_with_tx<'t>(
        &self,
        new_livelihood: &NewLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood> {
        // Validate the input
        new_livelihood.validate()?;
        
        let id = Uuid::new_v4();
        let now = Utc::now();
        
        // Get created by from auth context or from dto
        let created_by = new_livelihood.created_by_user_id.unwrap_or(auth.user_id);
        
        // Insert the new livelihood
        query(
            r#"
            INSERT INTO livelihoods (
                id, 
                participant_id, 
                project_id, 
                grant_amount, 
                grant_amount_updated_at,
                grant_amount_updated_by,
                purpose, 
                purpose_updated_at,
                purpose_updated_by,
                created_at, 
                updated_at,
                created_by_user_id,
                updated_by_user_id
            ) 
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(id.to_string())
        .bind(new_livelihood.participant_id.map(|id| id.to_string()))
        .bind(new_livelihood.project_id.map(|id| id.to_string()))
        .bind(new_livelihood.grant_amount)
        .bind(new_livelihood.grant_amount.map(|_| now.to_rfc3339()))
        .bind(new_livelihood.grant_amount.map(|_| auth.user_id.to_string()))
        .bind(&new_livelihood.purpose)
        .bind(new_livelihood.purpose.as_ref().map(|_| now.to_rfc3339()))
        .bind(new_livelihood.purpose.as_ref().map(|_| auth.user_id.to_string()))
        .bind(now.to_rfc3339())
        .bind(now.to_rfc3339())
        .bind(created_by.to_string())
        .bind(auth.user_id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        
        // Fetch the newly created livelihood
        self.find_by_id_with_tx(id, tx).await
    }
    
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
    ) -> DomainResult<Livelihood> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
    
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateLivelihood,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Livelihood> {
        // Validate the input
        update_data.validate()?;
        
        // Fetch the existing record to ensure it exists and isn't deleted
        let existing = self.find_by_id_with_tx(id, tx).await?;
        
        if existing.is_deleted() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        
        // Use QueryBuilder to build the update query
        let mut builder = QueryBuilder::new("UPDATE livelihoods SET ");
        let mut separated = builder.separated(", ");
        
        // Flag to track if any actual fields are updated
        let mut fields_updated = false;
        
        // Macro to simplify adding LWW fields
        macro_rules! add_lww {
            ($field_name:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.to_string());

                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());

                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    fields_updated = true;
                }
            };
            ($field_name:ident, $field_sql:literal, $value:expr, $is_optional:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone());

                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());

                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    fields_updated = true;
                }
            };
        }
        
        // Add grant amount
        add_lww!(grant_amount, "grant_amount", update_data.grant_amount);
        
        // Add purpose
        add_lww!(purpose, "purpose", &update_data.purpose, true);
        
        // Add progress1
        add_lww!(progress1, "progress1", &update_data.progress1, true);
        
        // Add progress2
        add_lww!(progress2, "progress2", &update_data.progress2, true);
        
        // Add outcome
        add_lww!(outcome, "outcome", &update_data.outcome, true);
        
        // If no actual fields were updated, return the existing record
        if !fields_updated {
            return Ok(existing);
        }
        
        // Add common update fields
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str);
        separated.push(" updated_by_user_id = ");
        separated.push_bind_unseparated(update_data.updated_by_user_id.to_string());
        
        // Finish the query with WHERE clause
        builder.push(" WHERE id = ");
        builder.push_bind(id.to_string());
        builder.push(" AND deleted_at IS NULL");
        
        // Execute the query
        let query = builder.build();
        let result = query
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;
            
        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        // Return the updated record
        self.find_by_id_with_tx(id, tx).await
    }
    
    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        // --- Helper closure for WHERE clause --- 
        let build_where_clause = |builder: &mut QueryBuilder<'_, Sqlite>, proj_id: Option<Uuid>, part_id: Option<Uuid>| {
            builder.push(" WHERE deleted_at IS NULL");
            if let Some(pid) = proj_id {
                builder.push(" AND project_id = ");
                builder.push_bind(pid.to_string());
            }
            if let Some(pid) = part_id {
                builder.push(" AND participant_id = ");
                builder.push_bind(pid.to_string());
            }
        };

        // --- Count Query --- 
        let mut count_builder = QueryBuilder::new("SELECT COUNT(*) as count FROM livelihoods");
        build_where_clause(&mut count_builder, project_id, participant_id); // Apply WHERE logic
        
        let count_query = count_builder.build(); // Build the final count query
        
        let total: i64 = count_query
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?
            .try_get("count")
            .map_err(|e| DbError::Query(format!("Failed to get count: {}", e)))?;
            
        // --- Main Query --- 
        let mut query_builder = QueryBuilder::new("SELECT * FROM livelihoods");
        build_where_clause(&mut query_builder, project_id, participant_id); // Apply WHERE logic again
        
        query_builder.push(" ORDER BY created_at DESC LIMIT ");
        query_builder.push_bind(params.per_page as i64);
        query_builder.push(" OFFSET ");
        query_builder.push_bind((params.page as i64 - 1) * params.per_page as i64);
        
        // Build and execute the query_as directly
        let rows = query_builder.build_query_as::<LivelihoodRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;
            
        // Convert rows to entities
        let mut items = Vec::new();
        for row in rows {
            items.push(Self::map_row_to_entity(row)?);
        }
        
        // Calculate total pages
        let total_pages = (total as f64 / params.per_page as f64).ceil() as u32;
        
        Ok(PaginatedResult {
            items,
            total: total as u64,
            page: params.page,
            per_page: params.per_page,
            total_pages,
        })
    }
    
    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        self.find_all(params, Some(project_id), None).await
    }
    
    async fn find_by_participant_id(
        &self,
        participant_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        self.find_all(params, None, Some(participant_id)).await
    }

    /// Set a document reference for a livelihood
    async fn set_document_reference(
        &self,
        livelihood_id: Uuid,
        field_name: &str, // e.g., "business_plan"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let column_name = format!("{}_ref", field_name); 
        
        // Validate the field name
        if !Livelihood::field_metadata().iter().any(|m| m.field_name == field_name && m.is_document_reference_only) {
             return Err(DomainError::Validation(ValidationError::custom(&format!("Invalid document reference field for Livelihood: {}", field_name))));
        }

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let document_id_str = document_id.to_string();
        
        let mut builder = sqlx::QueryBuilder::new("UPDATE livelihoods SET ");
        builder.push(&column_name);
        builder.push(" = ");
        builder.push_bind(document_id_str);
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        builder.push(" WHERE id = ");
        builder.push_bind(livelihood_id.to_string());
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Livelihood".to_string(), livelihood_id))
        } else {
            Ok(())
        }
    }
}

/// SQLite implementation of the subsequent grant repository
pub struct SqliteSubsequentGrantRepository {
    pool: Pool<Sqlite>,
}

impl SqliteSubsequentGrantRepository {
    /// Create a new SQLite subsequent grant repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
    
    /// Map a database row to a domain entity
    fn map_row_to_entity(row: SubsequentGrantRow) -> DomainResult<SubsequentGrant> {
        row.into_entity()
    }
    
    /// Get the entity name for this repository
    fn entity_name(&self) -> &'static str {
        "subsequent_grant"
    }
    
    /// Find a subsequent grant by ID within a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<SubsequentGrant> {
        let row = query_as::<_, SubsequentGrantRow>(
            "SELECT * FROM subsequent_grants WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SubsequentGrantRepository for SqliteSubsequentGrantRepository {
    async fn create(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        let result = self.create_with_tx(new_grant, auth, &mut tx).await;
        
        if result.is_ok() {
            tx.commit().await.map_err(DbError::from)?;
        } else {
            tx.rollback().await.map_err(DbError::from)?;
        }
        
        result
    }
    
    async fn create_with_tx<'t>(
        &self,
        new_grant: &NewSubsequentGrant,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<SubsequentGrant> {
        new_grant.validate()?;
        let id = Uuid::new_v4();
        let now = Utc::now();
        let created_by = new_grant.created_by_user_id.unwrap_or(auth.user_id);
        
        query(
             r#"
             INSERT INTO subsequent_grants (
                 id, 
                 livelihood_id, 
                 amount, 
                 amount_updated_at,
                 amount_updated_by,
                 purpose, 
                 purpose_updated_at,
                 purpose_updated_by,
                 grant_date,
                 grant_date_updated_at,
                 grant_date_updated_by,
                 created_at, 
                 updated_at,
                 created_by_user_id,
                 updated_by_user_id
             ) 
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             "#
         )
         .bind(id.to_string())
         .bind(new_grant.livelihood_id.to_string())
         .bind(new_grant.amount)
         .bind(new_grant.amount.map(|_| now.to_rfc3339()))
         .bind(new_grant.amount.map(|_| auth.user_id.to_string()))
         .bind(&new_grant.purpose)
         .bind(new_grant.purpose.as_ref().map(|_| now.to_rfc3339()))
         .bind(new_grant.purpose.as_ref().map(|_| auth.user_id.to_string()))
         .bind(&new_grant.grant_date)
         .bind(new_grant.grant_date.as_ref().map(|_| now.to_rfc3339()))
         .bind(new_grant.grant_date.as_ref().map(|_| auth.user_id.to_string()))
         .bind(now.to_rfc3339())
         .bind(now.to_rfc3339())
         .bind(created_by.to_string())
         .bind(auth.user_id.to_string())
         .execute(&mut **tx)
         .await
         .map_err(DbError::from)?;
         
        self.find_by_id_with_tx(id, tx).await
    }
    
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant> {
        update_data.validate()?;
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let existing = self.find_by_id_with_tx(id, &mut tx).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        
        let mut builder = QueryBuilder::new("UPDATE subsequent_grants SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;
        
        macro_rules! add_lww {
            ($field_name:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.to_string());
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    fields_updated = true;
                }
            };
            ($field_name:ident, $field_sql:literal, $value:expr, $is_optional:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());
                    separated.push(concat!(" ", $field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    fields_updated = true;
                }
            };
        }
        
        add_lww!(amount, "amount", update_data.amount);
        add_lww!(purpose, "purpose", &update_data.purpose, true);
        add_lww!(grant_date, "grant_date", &update_data.grant_date, true);
        
        if !fields_updated {
            return Ok(existing);
        }
        
        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str);
        separated.push(" updated_by_user_id = ");
        separated.push_bind_unseparated(update_data.updated_by_user_id.to_string());
        
        builder.push(" WHERE id = ");
        builder.push_bind(id.to_string());
        builder.push(" AND deleted_at IS NULL");
        
        let query = builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
            
        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(DbError::from)?;
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        let updated_grant = self.find_by_id_with_tx(id, &mut tx).await?;
        tx.commit().await.map_err(DbError::from)?;
        Ok(updated_grant)
    }
    
    async fn find_by_id(
        &self, 
        id: Uuid
    ) -> DomainResult<SubsequentGrant> {
        let row = query_as::<_, SubsequentGrantRow>(
            "SELECT * FROM subsequent_grants WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound(self.entity_name().to_string(), id))?;
        
        Self::map_row_to_entity(row)
    }
    
    async fn find_by_livelihood_id(
        &self, 
        livelihood_id: Uuid
    ) -> DomainResult<Vec<SubsequentGrant>> {
        let rows = query_as::<_, SubsequentGrantRow>(
            "SELECT * FROM subsequent_grants WHERE livelihood_id = ? AND deleted_at IS NULL ORDER BY created_at ASC"
        )
        .bind(livelihood_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let mut grants = Vec::new();
        for row in rows {
            grants.push(Self::map_row_to_entity(row)?);
        }
        
        Ok(grants)
    }
    
    async fn soft_delete(
        &self, 
        id: Uuid, 
        auth: &AuthContext
    ) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let existing = self.find_by_id_with_tx(id, &mut tx).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        
        let mut builder = QueryBuilder::new("UPDATE subsequent_grants SET ");
        builder.push("deleted_at = ");
        builder.push_bind(now.to_rfc3339());
        builder.push(", deleted_by_user_id = ");
        builder.push_bind(auth.user_id.to_string());
        builder.push(", updated_at = ");
        builder.push_bind(now.to_rfc3339());
        
        builder.push(" WHERE id = ");
        builder.push_bind(id.to_string());
        builder.push(" AND deleted_at IS NULL");
        
        let query = builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
            
        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(DbError::from)?;
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        tx.commit().await.map_err(DbError::from)?;
        Ok(())
    }
    
    async fn hard_delete(
        &self, 
        id: Uuid, 
        _auth: &AuthContext
    ) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let existing = self.find_by_id_with_tx(id, &mut tx).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let mut builder = QueryBuilder::new("DELETE FROM subsequent_grants ");
        builder.push("WHERE id = ");
        builder.push_bind(id.to_string());
        
        let query = builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
            
        if result.rows_affected() == 0 {
            tx.rollback().await.map_err(DbError::from)?;
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        tx.commit().await.map_err(DbError::from)?;
        Ok(())
    }

    /// Set a document reference for a subsequent grant
    async fn set_document_reference(
        &self,
        grant_id: Uuid,
        field_name: &str, // e.g., "grant_agreement"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let column_name = format!("{}_ref", field_name);
        
        // Validate the field name
        if !SubsequentGrant::field_metadata().iter().any(|m| m.field_name == field_name && m.is_document_reference_only) {
             return Err(DomainError::Validation(ValidationError::custom(&format!("Invalid document reference field for SubsequentGrant: {}", field_name))));
        }

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let document_id_str = document_id.to_string();
        let grant_id_str = grant_id.to_string();
        
        let mut builder = sqlx::QueryBuilder::new("UPDATE subsequent_grants SET ");
        builder.push(&column_name);
        builder.push(" = ");
        builder.push_bind(document_id_str);
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        builder.push(" WHERE id = ");
        builder.push_bind(grant_id_str);
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("SubsequentGrant".to_string(), grant_id))
        } else {
            Ok(())
        }
    }
}