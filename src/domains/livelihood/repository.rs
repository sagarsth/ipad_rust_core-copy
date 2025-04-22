use crate::auth::AuthContext;
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::livelihood::types::{Livelihood, LivelihoodRow, NewLivelihood, SubsequentGrant, SubsequentGrantRow, UpdateLivelihood, NewSubsequentGrant, UpdateSubsequentGrant};
use crate::errors::{DbError, DomainError, DomainResult};
use crate::types::PaginatedResult;
use crate::types::PaginationParams;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite, Transaction, query_as, query, Row};
use uuid::Uuid;
use std::collections::HashSet;
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
        
        // Prepare update parts for each field (following LWW pattern)
        let mut sets = Vec::new();
        let mut params: Vec<String> = Vec::new();
        
        // Helper function to add a set clause and parameter
        let add_set = |sets: &mut Vec<String>, params: &mut Vec<String>, field: &str, value: &str| {
            sets.push(format!("{} = ?", field));
            params.push(value.to_string());
        };
        
        // Helper for timestamp fields (LWW pattern)
        let add_timestamp_update = |
            sets: &mut Vec<String>, 
            params: &mut Vec<String>,
            field: &str, 
            value: Option<&String>,
            existing_ts: Option<&DateTime<Utc>>, 
            now: &DateTime<Utc>
        | {
            if value.is_some() {
                sets.push(format!("{}_updated_at = ?", field));
                params.push(now.to_rfc3339());
                
                sets.push(format!("{}_updated_by = ?", field));
                params.push(auth.user_id.to_string());
            }
        };
        
        // Grant amount
        if let Some(grant_amount) = update_data.grant_amount {
            add_set(&mut sets, &mut params, "grant_amount", &grant_amount.to_string());
            add_timestamp_update(&mut sets, &mut params, "grant_amount", Some(&grant_amount.to_string()), existing.grant_amount_updated_at.as_ref(), &now);
        }
        
        // Purpose
        if let Some(purpose) = &update_data.purpose {
            add_set(&mut sets, &mut params, "purpose", purpose);
            add_timestamp_update(&mut sets, &mut params, "purpose", Some(purpose), existing.purpose_updated_at.as_ref(), &now);
        }
        
        // Progress1
        if let Some(progress1) = &update_data.progress1 {
            add_set(&mut sets, &mut params, "progress1", progress1);
            add_timestamp_update(&mut sets, &mut params, "progress1", Some(progress1), existing.progress1_updated_at.as_ref(), &now);
        }
        
        // Progress2
        if let Some(progress2) = &update_data.progress2 {
            add_set(&mut sets, &mut params, "progress2", progress2);
            add_timestamp_update(&mut sets, &mut params, "progress2", Some(progress2), existing.progress2_updated_at.as_ref(), &now);
        }
        
        // Outcome
        if let Some(outcome) = &update_data.outcome {
            add_set(&mut sets, &mut params, "outcome", outcome);
            add_timestamp_update(&mut sets, &mut params, "outcome", Some(outcome), existing.outcome_updated_at.as_ref(), &now);
        }
        
        // Updated timestamps
        add_set(&mut sets, &mut params, "updated_at", &now.to_rfc3339());
        add_set(&mut sets, &mut params, "updated_by_user_id", &update_data.updated_by_user_id.to_string());
        
        // If no fields to update, return existing record
        if sets.len() <= 2 {  // Only updated_at and updated_by
            return Ok(existing);
        }
        
        // Build and execute the update query
        let query_string = format!(
            "UPDATE livelihoods SET {} WHERE id = ? AND deleted_at IS NULL",
            sets.join(", ")
        );
        
        let mut query_builder = query(&query_string);
        
        // Bind all parameters
        for param in params {
            query_builder = query_builder.bind(param);
        }
        
        // Bind the ID
        query_builder = query_builder.bind(id.to_string());
        
        // Execute the query
        query_builder
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;
        
        // Return the updated record
        self.find_by_id_with_tx(id, tx).await
    }
    
    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
        participant_id: Option<Uuid>,
    ) -> DomainResult<PaginatedResult<Livelihood>> {
        // Base query
        let mut query_string = String::from("SELECT * FROM livelihoods WHERE deleted_at IS NULL");
        let mut count_query_string = String::from("SELECT COUNT(*) as count FROM livelihoods WHERE deleted_at IS NULL");
        
        // Parameters to bind
        let mut param_values: Vec<String> = Vec::new();
        
        // Add filters
        if let Some(pid) = project_id {
            query_string.push_str(" AND project_id = ?");
            count_query_string.push_str(" AND project_id = ?");
            param_values.push(pid.to_string());
        }
        
        if let Some(pid) = participant_id {
            query_string.push_str(" AND participant_id = ?");
            count_query_string.push_str(" AND participant_id = ?");
            param_values.push(pid.to_string());
        }
        
        // Add order by, limit and offset
        query_string.push_str(" ORDER BY created_at DESC LIMIT ? OFFSET ?");
        
        // Build and execute count query
        let mut count_query = query("SELECT COUNT(*) as count FROM livelihoods WHERE deleted_at IS NULL");
        
        for value in &param_values {
            count_query = count_query.bind(value);
        }
        
        let total: i64 = count_query
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?
            .try_get("count")
            .map_err(|e| DbError::Query(format!("Failed to get count: {}", e)))?;
        
        // Build and execute main query
        let mut main_query = query_as::<_, LivelihoodRow>(&query_string);
        
        for value in param_values {
            main_query = main_query.bind(value);
        }
        
        main_query = main_query
            .bind(params.per_page as i64)
            .bind((params.page as i64 - 1) * params.per_page as i64);
        
        let rows = main_query
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
        // Validate the input
        new_grant.validate()?;
        
        let id = Uuid::new_v4();
        let now = Utc::now();
        
        // Get created by from auth context or from dto
        let created_by = new_grant.created_by_user_id.unwrap_or(auth.user_id);
        
        // Insert the new subsequent grant
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
        
        // Fetch the newly created subsequent grant
        self.find_by_id(id).await
    }
    
    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateSubsequentGrant,
        auth: &AuthContext,
    ) -> DomainResult<SubsequentGrant> {
        // Validate the input
        update_data.validate()?;
        
        // Fetch the existing record to ensure it exists and isn't deleted
        let existing = self.find_by_id(id).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        
        // Prepare update parts for each field (following LWW pattern)
        let mut sets = Vec::new();
        let mut params: Vec<String> = Vec::new();
        
        // Helper function to add a set clause and parameter
        let add_set = |sets: &mut Vec<String>, params: &mut Vec<String>, field: &str, value: &str| {
            sets.push(format!("{} = ?", field));
            params.push(value.to_string());
        };
        
        // Helper for timestamp fields (LWW pattern)
        let add_timestamp_update = |
            sets: &mut Vec<String>, 
            params: &mut Vec<String>,
            field: &str, 
            value: Option<&String>,
            existing_ts: Option<&DateTime<Utc>>, 
            now: &DateTime<Utc>
        | {
            if value.is_some() {
                sets.push(format!("{}_updated_at = ?", field));
                params.push(now.to_rfc3339());
                
                sets.push(format!("{}_updated_by = ?", field));
                params.push(auth.user_id.to_string());
            }
        };
        
        // Amount
        if let Some(amount) = update_data.amount {
            add_set(&mut sets, &mut params, "amount", &amount.to_string());
            add_timestamp_update(&mut sets, &mut params, "amount", Some(&amount.to_string()), existing.amount_updated_at.as_ref(), &now);
        }
        
        // Purpose
        if let Some(purpose) = &update_data.purpose {
            add_set(&mut sets, &mut params, "purpose", purpose);
            add_timestamp_update(&mut sets, &mut params, "purpose", Some(purpose), existing.purpose_updated_at.as_ref(), &now);
        }
        
        // Grant date
        if let Some(grant_date) = &update_data.grant_date {
            add_set(&mut sets, &mut params, "grant_date", grant_date);
            add_timestamp_update(&mut sets, &mut params, "grant_date", Some(grant_date), existing.grant_date_updated_at.as_ref(), &now);
        }
        
        // Updated timestamps
        add_set(&mut sets, &mut params, "updated_at", &now.to_rfc3339());
        add_set(&mut sets, &mut params, "updated_by_user_id", &update_data.updated_by_user_id.to_string());
        
        // If no fields to update, return existing record
        if sets.len() <= 2 {  // Only updated_at and updated_by
            return Ok(existing);
        }
        
        // Build and execute the update query
        let query_string = format!(
            "UPDATE subsequent_grants SET {} WHERE id = ? AND deleted_at IS NULL",
            sets.join(", ")
        );
        
        let mut query_builder = query(&query_string);
        
        // Bind all parameters
        for param in params {
            query_builder = query_builder.bind(param);
        }
        
        // Bind the ID
        query_builder = query_builder.bind(id.to_string());
        
        // Execute the query
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        
        query_builder
            .execute(&mut *tx)
            .await
            .map_err(DbError::from)?;
        
        tx.commit().await.map_err(DbError::from)?;
        
        // Return the updated record
        self.find_by_id(id).await
    }
    
    async fn find_by_id(&self, id: Uuid) -> DomainResult<SubsequentGrant> {
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
    
    async fn find_by_livelihood_id(&self, livelihood_id: Uuid) -> DomainResult<Vec<SubsequentGrant>> {
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
    
    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        // First check if the record exists and is not already deleted
        let existing = self.find_by_id(id).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        let now = Utc::now();
        
        // Update the record with deleted_at and deleted_by
        let rows_affected = query(
            r#"
            UPDATE subsequent_grants 
            SET deleted_at = ?, deleted_by_user_id = ?, updated_at = ?
            WHERE id = ? AND deleted_at IS NULL
            "#
        )
        .bind(now.to_rfc3339())
        .bind(auth.user_id.to_string())
        .bind(now.to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?
        .rows_affected();
        
        if rows_affected == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        Ok(())
    }
    
    async fn hard_delete(&self, id: Uuid, _auth: &AuthContext) -> DomainResult<()> {
        // Check if the record exists first
        let existing = self.find_by_id(id).await?;
        
        if existing.deleted_at.is_some() {
            return Err(DomainError::DeletedEntity(self.entity_name().to_string(), id));
        }
        
        // Delete the record permanently
        let rows_affected = query(
            "DELETE FROM subsequent_grants WHERE id = ?"
        )
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(DbError::from)?
        .rows_affected();
        
        if rows_affected == 0 {
            return Err(DomainError::EntityNotFound(self.entity_name().to_string(), id));
        }
        
        Ok(())
    }
}
