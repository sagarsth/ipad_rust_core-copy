use crate::auth::AuthContext;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::funding::types::{ProjectFunding, NewProjectFunding, UpdateProjectFunding, ProjectFundingRow, ProjectFundingSummary};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::{Utc, DateTime, Local};
use sqlx::{Pool, Sqlite, Transaction, query, query_as, query_scalar, Row};
use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use serde_json;
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType};
use crate::domains::user::repository::MergeableEntityRepository;
use crate::domains::sync::types::{MergeOutcome};
use sqlx::QueryBuilder;

/// Trait defining funding repository operations
#[async_trait]
pub trait ProjectFundingRepository: 
    DeleteServiceRepository<ProjectFunding> + MergeableEntityRepository<ProjectFunding> + Send + Sync 
{
    async fn create(
        &self,
        new_funding: &NewProjectFunding,
        auth: &AuthContext,
    ) -> DomainResult<ProjectFunding>;
    
    async fn create_with_tx<'t>(
        &self,
        new_funding: &NewProjectFunding,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<ProjectFunding>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateProjectFunding,
        auth: &AuthContext,
    ) -> DomainResult<ProjectFunding>;
    
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateProjectFunding,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<ProjectFunding>;

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;
    
    async fn find_by_project(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;
    
    async fn find_by_donor(
        &self,
        donor_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;
    
    async fn get_project_funding_stats(
        &self,
        project_id: Uuid,
    ) -> DomainResult<(i64, f64)>; // Returns (count, total_amount)
    
    async fn get_donor_funding_stats(
        &self,
        donor_id: Uuid,
    ) -> DomainResult<(i64, f64)>; // Returns (active_count, total_amount)

    /// Count fundings by status
    async fn count_by_status(&self) -> DomainResult<Vec<(Option<String>, i64)>>;

    /// Count fundings by currency
    async fn count_by_currency(&self) -> DomainResult<Vec<(String, i64)>>;

    /// Get comprehensive funding summary statistics
    async fn get_funding_summary(&self) -> DomainResult<(i64, f64, f64, HashMap<String, f64>)>;

    /// Find fundings by status
    async fn find_by_status(
        &self,
        status: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;

    /// Find upcoming fundings (start date in the future, not cancelled)
    async fn find_upcoming_fundings(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;

    /// Find overdue fundings (end date in the past, not completed/cancelled)
    async fn find_overdue_fundings(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;

    /// Get detailed funding stats for a specific donor
    async fn get_donor_detailed_funding_stats(
        &self,
        donor_id: Uuid,
    ) -> DomainResult<(i64, i64, f64, f64, f64, f64, HashMap<String, f64>)>;

    /// Get recent fundings for a donor
    async fn get_recent_fundings_for_donor(
        &self,
        donor_id: Uuid,
        limit: i64,
    ) -> DomainResult<Vec<ProjectFundingSummary>>;

    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;

    /// Find project funding within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;

    /// Find funding records by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>>;
}

/// SQLite implementation for ProjectFundingRepository
#[derive(Clone)]
pub struct SqliteProjectFundingRepository {
    pool: Pool<Sqlite>,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteProjectFundingRepository {
    pub fn new(pool: Pool<Sqlite>, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
    }

    fn map_row_to_entity(row: ProjectFundingRow) -> DomainResult<ProjectFunding> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map funding row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "project_funding"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<ProjectFunding> {
        let row = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Project Funding".to_string(), id))?;

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
impl FindById<ProjectFunding> for SqliteProjectFundingRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<ProjectFunding> {
        let row = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Project Funding".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteProjectFundingRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
      

        let result = query(
            "UPDATE project_funding SET deleted_at = ?, deleted_by_user_id = ?, deleted_by_device_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now_str)
        .bind(user_id_str)
        .bind(device_id_str)
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Project Funding".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.soft_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }
}

#[async_trait]
impl HardDeletable for SqliteProjectFundingRepository {
    fn entity_name(&self) -> &'static str {
        SqliteProjectFundingRepository::entity_name(self)
    }

    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM project_funding WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Project Funding".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.hard_delete_with_tx(id, auth, &mut tx).await {
            Ok(()) => { tx.commit().await.map_err(DbError::from)?; Ok(()) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }
}

// Blanket implementation in core::delete_service handles DeleteServiceRepository

#[async_trait]
impl ProjectFundingRepository for SqliteProjectFundingRepository {
    async fn create(
        &self,
        new_funding: &NewProjectFunding,
        auth: &AuthContext,
    ) -> DomainResult<ProjectFunding> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.create_with_tx(new_funding, auth, &mut tx).await {
            Ok(funding) => { tx.commit().await.map_err(DbError::from)?; Ok(funding) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_funding: &NewProjectFunding,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<ProjectFunding> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let project_id_str = new_funding.project_id.to_string();
        let donor_id_str = new_funding.donor_id.to_string();
        
        let currency = new_funding.currency.clone().unwrap_or_else(|| "AUD".to_string());

        query(
            r#"
            INSERT INTO project_funding (
                id, project_id, project_id_updated_at, project_id_updated_by, project_id_updated_by_device_id,
                donor_id, donor_id_updated_at, donor_id_updated_by, donor_id_updated_by_device_id,
                grant_id, grant_id_updated_at, grant_id_updated_by, grant_id_updated_by_device_id,
                amount, amount_updated_at, amount_updated_by, amount_updated_by_device_id,
                currency, currency_updated_at, currency_updated_by, currency_updated_by_device_id,
                start_date, start_date_updated_at, start_date_updated_by, start_date_updated_by_device_id,
                end_date, end_date_updated_at, end_date_updated_by, end_date_updated_by_device_id,
                status, status_updated_at, status_updated_by, status_updated_by_device_id,
                reporting_requirements, reporting_requirements_updated_at, reporting_requirements_updated_by, reporting_requirements_updated_by_device_id,
                notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
            )
            "#
        )
        .bind(id.to_string())
        .bind(&project_id_str).bind(&now_str).bind(&user_id_str).bind(device_id_str.clone())
        .bind(&donor_id_str).bind(&now_str).bind(&user_id_str).bind(device_id_str.clone())
        .bind(&new_funding.grant_id).bind(new_funding.grant_id.as_ref().map(|_| &now_str)).bind(new_funding.grant_id.as_ref().map(|_| &user_id_str)).bind(new_funding.grant_id.as_ref().map(|_| &device_id_str))
        .bind(new_funding.amount).bind(new_funding.amount.map(|_| &now_str)).bind(new_funding.amount.map(|_| &user_id_str)).bind(new_funding.amount.map(|_| &device_id_str))
        .bind(&currency).bind(&now_str).bind(&user_id_str).bind(device_id_str.clone())
        .bind(&new_funding.start_date).bind(new_funding.start_date.as_ref().map(|_| &now_str)).bind(new_funding.start_date.as_ref().map(|_| &user_id_str)).bind(new_funding.start_date.as_ref().map(|_| &device_id_str))
        .bind(&new_funding.end_date).bind(new_funding.end_date.as_ref().map(|_| &now_str)).bind(new_funding.end_date.as_ref().map(|_| &user_id_str)).bind(new_funding.end_date.as_ref().map(|_| &device_id_str))
        .bind(&new_funding.status).bind(new_funding.status.as_ref().map(|_| &now_str)).bind(new_funding.status.as_ref().map(|_| &user_id_str)).bind(new_funding.status.as_ref().map(|_| &device_id_str))
        .bind(&new_funding.reporting_requirements).bind(new_funding.reporting_requirements.as_ref().map(|_| &now_str)).bind(new_funding.reporting_requirements.as_ref().map(|_| &user_id_str)).bind(new_funding.reporting_requirements.as_ref().map(|_| &device_id_str))
        .bind(&new_funding.notes).bind(new_funding.notes.as_ref().map(|_| &now_str)).bind(new_funding.notes.as_ref().map(|_| &user_id_str)).bind(new_funding.notes.as_ref().map(|_| &device_id_str))
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(new_funding.created_by_user_id.as_ref().map(|id| id.to_string()).unwrap_or_else(||user_id_str.clone()))
        .bind(&user_id_str) // updated_by_user_id
        .bind(device_id_str.clone()) // created_by_device_id
        .bind(device_id_str.clone()) // updated_by_device_id
        .bind(Option::<String>::None) // deleted_at
        .bind(Option::<String>::None) // deleted_by_user_id
        .bind(Option::<String>::None) // deleted_by_device_id
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
            new_value: None, // Or serialize new_funding if needed
            timestamp: now, // Use the DateTime<Utc>
            user_id: user_id,
            device_id: device_uuid,
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.log_change_entry(entry, tx).await?;

        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateProjectFunding,
        auth: &AuthContext,
    ) -> DomainResult<ProjectFunding> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.update_with_tx(id, update_data, auth, &mut tx).await {
            Ok(funding) => { tx.commit().await.map_err(DbError::from)?; Ok(funding) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateProjectFunding,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<ProjectFunding> {
        let old_entity = self.find_by_id_with_tx(id, tx).await?;
        
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let id_str = id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());

        let mut builder = sqlx::QueryBuilder::new("UPDATE project_funding SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        macro_rules! add_lww_option {($field_name:ident, $field_sql:literal, $value:expr) => {
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
        };}

        macro_rules! add_lww_uuid {($field_name:ident, $field_sql:literal, $value:expr) => {
            if let Some(val) = $value { 
                separated.push(concat!($field_sql, " = "));
                separated.push_bind_unseparated(val.to_string()); 
                separated.push(concat!(" ", $field_sql, "_updated_at = "));
                separated.push_bind_unseparated(now_str.clone()); 
                separated.push(concat!(" ", $field_sql, "_updated_by = "));
                separated.push_bind_unseparated(user_id_str.clone()); 
                separated.push(concat!(" ", $field_sql, "_updated_by_device_id = "));
                separated.push_bind_unseparated(device_id_str.clone());
                fields_updated = true; 
            }
        };}

        add_lww_uuid!(project_id, "project_id", &update_data.project_id);
        add_lww_uuid!(donor_id, "donor_id", &update_data.donor_id);
        add_lww_option!(grant_id, "grant_id", &update_data.grant_id);
        add_lww_option!(amount, "amount", &update_data.amount);
        add_lww_option!(currency, "currency", &update_data.currency);
        add_lww_option!(start_date, "start_date", &update_data.start_date);
        add_lww_option!(end_date, "end_date", &update_data.end_date);
        add_lww_option!(status, "status", &update_data.status);
        add_lww_option!(reporting_requirements, "reporting_requirements", &update_data.reporting_requirements);
        add_lww_option!(notes, "notes", &update_data.notes);

        if !fields_updated {
            return Ok(old_entity); 
        }

        separated.push("updated_at = ");
        separated.push_bind_unseparated(now_str.clone());
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str.clone());
        separated.push("updated_by_device_id = ");
        separated.push_bind_unseparated(device_id_str.clone());

        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound("Project Funding".to_string(), id));
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

        log_if_changed!(project_id, "project_id");
        log_if_changed!(donor_id, "donor_id");
        log_if_changed!(grant_id, "grant_id");
        log_if_changed!(amount, "amount");
        log_if_changed!(currency, "currency");
        log_if_changed!(start_date, "start_date");
        log_if_changed!(end_date, "end_date");
        log_if_changed!(status, "status");
        log_if_changed!(reporting_requirements, "reporting_requirements");
        log_if_changed!(notes, "notes");

        Ok(new_entity)
    }

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar("SELECT COUNT(*) FROM project_funding WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding WHERE deleted_at IS NULL ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }
    
    async fn find_by_project(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;
        let project_id_str = project_id.to_string();

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM project_funding WHERE project_id = ? AND deleted_at IS NULL"
         )
         .bind(&project_id_str)
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding WHERE project_id = ? AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(project_id_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }
    
    async fn find_by_donor(
        &self,
        donor_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;
        let donor_id_str = donor_id.to_string();

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM project_funding WHERE donor_id = ? AND deleted_at IS NULL"
         )
         .bind(&donor_id_str)
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding WHERE donor_id = ? AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(donor_id_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }
    
    async fn get_project_funding_stats(
        &self,
        project_id: Uuid,
    ) -> DomainResult<(i64, f64)> {
        let result = query_as::<_, (Option<i64>, Option<f64>)>(
            "SELECT COUNT(*), SUM(amount) 
             FROM project_funding 
             WHERE project_id = ? AND deleted_at IS NULL"
        )
        .bind(project_id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Handle potential NULL results from COUNT/SUM if no records match
        let count = result.0.unwrap_or(0);
        let total_amount = result.1.unwrap_or(0.0);

        Ok((count, total_amount))
    }
    
    async fn get_donor_funding_stats(
        &self,
        donor_id: Uuid,
    ) -> DomainResult<(i64, f64)> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        let result = query_as::<_, (Option<i64>, Option<f64>)>(
            r#"
            SELECT 
                COUNT(CASE 
                    WHEN (status IS NULL OR status NOT IN ('completed', 'cancelled'))
                         AND (start_date IS NULL OR DATE(start_date) <= ?)
                         AND (end_date IS NULL OR DATE(end_date) >= ?) 
                    THEN 1 
                    ELSE NULL 
                END) as active_count, 
                SUM(amount) as total_amount
            FROM project_funding 
            WHERE donor_id = ? AND deleted_at IS NULL
            "#
        )
        .bind(&today)
        .bind(&today)
        .bind(donor_id.to_string())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let active_count = result.0.unwrap_or(0);
        let total_amount = result.1.unwrap_or(0.0);

        Ok((active_count, total_amount))
    }

    async fn count_by_status(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT status, COUNT(*) 
             FROM project_funding 
             WHERE deleted_at IS NULL 
             GROUP BY status"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }

    async fn count_by_currency(&self) -> DomainResult<Vec<(String, i64)>> {
        let counts = query_as::<_, (String, i64)>(
            "SELECT currency, COUNT(*) 
             FROM project_funding 
             WHERE deleted_at IS NULL 
             GROUP BY currency"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }

    async fn get_funding_summary(&self) -> DomainResult<(i64, f64, f64, HashMap<String, f64>)> {
        let today = Local::now().format("%Y-%m-%d").to_string();
        // Get active funding count, total amount, and average amount
        let (active_count, total_amount, avg_amount) = query_as::<_, (i64, f64, f64)>(
            r#"
            WITH active_funding AS (
                SELECT * FROM project_funding
                WHERE deleted_at IS NULL
                AND (status IS NULL OR status NOT IN ('completed', 'cancelled'))
                AND (start_date IS NULL OR DATE(start_date) <= ?)
                AND (end_date IS NULL OR DATE(end_date) >= ?)
            )
            SELECT
                COUNT(*) as active_count,
                COALESCE(SUM(amount), 0) as total_amount,
                CASE WHEN COUNT(*) > 0 THEN COALESCE(AVG(amount), 0) ELSE 0 END as avg_amount
            FROM active_funding
            "#
        )
        .bind(&today)
        .bind(&today)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?; 

        // Get distribution by currency
        let currency_rows = query_as::<_, (String, f64)>(
            r#"
            SELECT 
                currency,
                COALESCE(SUM(amount), 0) as total_amount
            FROM project_funding
            WHERE deleted_at IS NULL
            AND amount IS NOT NULL
            GROUP BY currency
            "#
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let mut funding_by_currency = HashMap::new();
        for (currency, amount) in currency_rows {
            funding_by_currency.insert(currency, amount);
        }

        Ok((active_count, total_amount, avg_amount, funding_by_currency))
    }

    async fn find_by_status(
        &self,
        status: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM project_funding WHERE status = ? AND deleted_at IS NULL"
        )
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding 
             WHERE status = ? AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(status)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn find_upcoming_fundings(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;
        let today = Local::now().format("%Y-%m-%d").to_string();

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM project_funding 
             WHERE deleted_at IS NULL 
             AND (status IS NULL OR status != 'cancelled')
             AND start_date > ?"
        )
        .bind(&today)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding 
             WHERE deleted_at IS NULL 
             AND (status IS NULL OR status != 'cancelled')
             AND start_date > ?
             ORDER BY start_date ASC LIMIT ? OFFSET ?"
        )
        .bind(&today)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn find_overdue_fundings(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;
        let today = Local::now().format("%Y-%m-%d").to_string();

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM project_funding 
             WHERE deleted_at IS NULL 
             AND (status IS NULL OR status NOT IN ('completed', 'cancelled'))
             AND end_date < ?"
        )
        .bind(&today)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding 
             WHERE deleted_at IS NULL 
             AND (status IS NULL OR status NOT IN ('completed', 'cancelled'))
             AND end_date < ?
             ORDER BY end_date ASC LIMIT ? OFFSET ?"
        )
        .bind(&today)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    async fn get_donor_detailed_funding_stats(
        &self,
        donor_id: Uuid,
    ) -> DomainResult<(i64, i64, f64, f64, f64, f64, HashMap<String, f64>)> {
        let donor_id_str = donor_id.to_string();
        let today = Local::now().format("%Y-%m-%d").to_string();
        
        // Get comprehensive stats for donor
        let (active_count, total_count, total_amount, active_amount, avg_amount, largest_amount) = 
            query_as::<_, (i64, i64, f64, f64, f64, f64)>(
                r#"
                SELECT
                    (SELECT COUNT(*) FROM project_funding 
                     WHERE donor_id = ? 
                     AND deleted_at IS NULL 
                     AND (status IS NULL OR status NOT IN ('completed', 'cancelled'))
                     AND (start_date IS NULL OR DATE(start_date) <= ?)
                     AND (end_date IS NULL OR DATE(end_date) >= ?)) as active_count,
                     
                    COUNT(*) as total_count,
                    
                    COALESCE(SUM(amount), 0) as total_amount,
                    
                    (SELECT COALESCE(SUM(amount), 0) FROM project_funding 
                     WHERE donor_id = ? 
                     AND deleted_at IS NULL 
                     AND (status IS NULL OR status NOT IN ('completed', 'cancelled'))
                     AND (start_date IS NULL OR DATE(start_date) <= ?)
                     AND (end_date IS NULL OR DATE(end_date) >= ?)) as active_amount,
                     
                    CASE WHEN COUNT(*) > 0 THEN COALESCE(AVG(amount), 0) ELSE 0 END as avg_amount,
                    
                    COALESCE(MAX(amount), 0) as largest_amount
                FROM 
                    project_funding
                WHERE 
                    donor_id = ? 
                    AND deleted_at IS NULL
                "#
            )
            .bind(&donor_id_str)
            .bind(&today)
            .bind(&today)
            .bind(&donor_id_str)
            .bind(&today)
            .bind(&today)
            .bind(&donor_id_str)
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Get currency distribution for this donor
        let currency_rows = query_as::<_, (String, f64)>(
            r#"
            SELECT 
                currency,
                COALESCE(SUM(amount), 0) as total_amount
            FROM project_funding
            WHERE donor_id = ?
            AND deleted_at IS NULL
            AND amount IS NOT NULL
            GROUP BY currency
            "#
        )
        .bind(&donor_id_str)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let mut currency_distribution = HashMap::new();
        for (currency, amount) in currency_rows {
            currency_distribution.insert(currency, amount);
        }

        Ok((
            active_count,
            total_count,
            total_amount,
            active_amount,
            avg_amount,
            largest_amount,
            currency_distribution
        ))
    }

    async fn get_recent_fundings_for_donor(
        &self,
        donor_id: Uuid,
        limit: i64,
    ) -> DomainResult<Vec<ProjectFundingSummary>> {
        let donor_id_str = donor_id.to_string();
        
        // Join with projects to get project name
        let rows = query(
            r#"
            SELECT 
                pf.id, 
                pf.project_id, 
                p.name as project_name,
                pf.amount, 
                pf.currency, 
                pf.status,
                pf.start_date, 
                pf.end_date
            FROM 
                project_funding pf
            JOIN 
                projects p ON pf.project_id = p.id
            WHERE 
                pf.donor_id = ? 
                AND pf.deleted_at IS NULL
                AND p.deleted_at IS NULL
            ORDER BY 
                pf.updated_at DESC
            LIMIT ?
            "#
        )
        .bind(&donor_id_str)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let today = Local::now().naive_local().date(); // Use naive local date
        
        let mut summaries = Vec::new();
        for row in rows {
            let id_str: String = row.get("id");
            let project_id_str: String = row.get("project_id");
            
            let id = Uuid::parse_str(&id_str).map_err(|_| {
                DomainError::Internal(format!("Invalid UUID format in funding id: {}", id_str))
            })?;
            
            let project_id = Uuid::parse_str(&project_id_str).map_err(|_| {
                DomainError::Internal(format!("Invalid UUID format in project_id: {}", project_id_str))
            })?;
            
            let status: Option<String> = row.get("status");
            let start_date: Option<String> = row.get("start_date");
            let end_date: Option<String> = row.get("end_date");
            
            // Determine if funding is active
            let is_active = match &status {
                Some(s) if s == "completed" || s == "cancelled" => false,
                _ => {
                    let start_check = match &start_date {
                        Some(date) => {
                            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                                .map(|d| d <= today)
                                .unwrap_or(true) // Consider error case as true?
                        },
                        None => true, // No start date means it's considered started
                    };
                    
                    let end_check = match &end_date {
                        Some(date) => {
                            chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
                                .map(|d| d >= today)
                                .unwrap_or(true) // Consider error case as true?
                        },
                        None => true, // No end date means it's not ended
                    };
                    
                    start_check && end_check
                }
            };
            
            summaries.push(ProjectFundingSummary {
                id,
                project_id,
                project_name: row.get("project_name"),
                amount: row.get("amount"),
                currency: row.get("currency"),
                status,
                start_date,
                end_date,
                is_active,
            });
        }

        Ok(summaries)
    }

    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;
        let project_id_str = project_id.to_string();

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM project_funding WHERE project_id = ? AND deleted_at IS NULL"
         )
         .bind(&project_id_str)
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding WHERE project_id = ? AND deleted_at IS NULL 
             ORDER BY updated_at DESC LIMIT ? OFFSET ?"
        )
        .bind(project_id_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }

    /// Find project funding within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM project_funding WHERE deleted_at IS NULL AND updated_at BETWEEN ? AND ?"
        )
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectFundingRow>(
            "SELECT * FROM project_funding WHERE deleted_at IS NULL AND updated_at BETWEEN ? AND ? ORDER BY updated_at DESC LIMIT ? OFFSET ?"
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
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(entities, total as u64, params))
    }

    /// Find funding records by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<ProjectFunding>> {
        if ids.is_empty() {
            return Ok(PaginatedResult::new(Vec::new(), 0, params));
        }

        let offset = (params.page - 1) * params.per_page;

        // Build COUNT query with dynamic placeholders
        let count_placeholders = vec!["?"; ids.len()].join(", ");
        let count_query = format!(
            "SELECT COUNT(*) FROM project_funding WHERE id IN ({}) AND deleted_at IS NULL",
            count_placeholders
        );

        let mut count_builder = QueryBuilder::new(&count_query);
        for id in ids {
            count_builder.push_bind(id.to_string());
        }

        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Build SELECT query with dynamic placeholders
        let select_placeholders = vec!["?"; ids.len()].join(", ");
        let select_query = format!(
            "SELECT * FROM project_funding WHERE id IN ({}) AND deleted_at IS NULL ORDER BY start_date ASC LIMIT ? OFFSET ?",
            select_placeholders
        );

        let mut select_builder = QueryBuilder::new(&select_query);
        for id in ids {
            select_builder.push_bind(id.to_string());
        }
        select_builder.push_bind(params.per_page as i64);
        select_builder.push_bind(offset as i64);

        let rows = select_builder
            .build_query_as::<ProjectFundingRow>()
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<ProjectFunding>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
}

// === Sync Merge Implementation ===
#[async_trait]
impl MergeableEntityRepository<ProjectFunding> for SqliteProjectFundingRepository {
    fn entity_name(&self) -> &'static str {
        "project_funding"
    }

    /// Merge a remote change (CREATE / UPDATE) coming from sync into the local database.
    ///
    /// Strategy:
    /// 1. Deserialize the remote `new_value` into a full `ProjectFunding` state.
    /// 2. If the record does not exist locally, we INSERT it (treat as CREATE).
    /// 3. If it exists, compare `updated_at`.  If the remote timestamp is **newer**
    ///    than the local one, we perform a full UPSERT ("REPLACE INTO") of the row
    ///    using the remote values (LWW at row-level).  Otherwise we `NoOp`.
    /// 4. Soft-Delete operations are ignored (handled only locally).  Hard-Deletes
    ///    are executed by higher-level entity mergers via the DeleteService.
    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        // We only act on CREATE or UPDATE operations
        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                // --- 1. Parse remote state JSON ---
                let state_json = remote_change.new_value.as_ref().ok_or_else(|| {
                    DomainError::Validation(ValidationError::custom("Missing new_value for funding change"))
                })?;

                let remote_state: ProjectFunding = serde_json::from_str(state_json).map_err(|e| {
                    DomainError::Validation(ValidationError::format(
                        "new_value_funding",
                        &format!("Invalid JSON: {}", e),
                    ))
                })?;

                // --- 2. Fetch local state (if any) ---
                let local_opt = match self.find_by_id_with_tx(remote_state.id, tx).await {
                    Ok(entity) => Some(entity),
                    Err(DomainError::EntityNotFound(_, _)) => None,
                    Err(e) => return Err(e),
                };

                // Determine if we INSERT or UPDATE
                if let Some(local) = local_opt {
                    if remote_state.updated_at <= local.updated_at {
                        return Ok(MergeOutcome::NoOp("Local copy is newer or equal".to_string()));
                    }
                    // Remote is newer – perform full upsert (replace)
                    self.upsert_remote_state_with_tx(tx, &remote_state).await?;
                    Ok(MergeOutcome::Updated(remote_state.id))
                } else {
                    // Not present locally – insert
                    self.upsert_remote_state_with_tx(tx, &remote_state).await?;
                    Ok(MergeOutcome::Created(remote_state.id))
                }
            }
            ChangeOperationType::Delete => {
                // Soft deletes are handled locally only
                Ok(MergeOutcome::NoOp("Remote soft deletes are ignored".to_string()))
            }
            ChangeOperationType::HardDelete => {
                // Higher-level merger handles HardDelete via DeleteService; report back
                Ok(MergeOutcome::HardDeleted(remote_change.entity_id))
            }
        }
    }
}

impl SqliteProjectFundingRepository {
    /// Helper: performs a full UPSERT (replace) of `remote` into `project_funding` using a
    /// single `INSERT OR REPLACE` statement.  All columns are bound in the exact order
    /// they appear in the table schema to avoid mismatch errors.
    async fn upsert_remote_state_with_tx<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote: &ProjectFunding,
    ) -> DomainResult<()> {
        use sqlx::query;

        query(
            r#"
            INSERT OR REPLACE INTO project_funding (
                id, project_id, project_id_updated_at, project_id_updated_by, project_id_updated_by_device_id,
                donor_id, donor_id_updated_at, donor_id_updated_by, donor_id_updated_by_device_id,
                grant_id, grant_id_updated_at, grant_id_updated_by, grant_id_updated_by_device_id,
                amount, amount_updated_at, amount_updated_by, amount_updated_by_device_id,
                currency, currency_updated_at, currency_updated_by, currency_updated_by_device_id,
                start_date, start_date_updated_at, start_date_updated_by, start_date_updated_by_device_id,
                end_date, end_date_updated_at, end_date_updated_by, end_date_updated_by_device_id,
                status, status_updated_at, status_updated_by, status_updated_by_device_id,
                reporting_requirements, reporting_requirements_updated_at, reporting_requirements_updated_by, reporting_requirements_updated_by_device_id,
                notes, notes_updated_at, notes_updated_by, notes_updated_by_device_id,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (
                ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?,?, ?,?,?,?
            )
            "#
        )
        .bind(remote.id.to_string())
        .bind(remote.project_id.to_string())
        .bind(remote.project_id_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.project_id_updated_by.map(|u| u.to_string()))
        .bind(remote.project_id_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.donor_id.to_string())
        .bind(remote.donor_id_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.donor_id_updated_by.map(|u| u.to_string()))
        .bind(remote.donor_id_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.grant_id)
        .bind(remote.grant_id_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.grant_id_updated_by.map(|u| u.to_string()))
        .bind(remote.grant_id_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.amount)
        .bind(remote.amount_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.amount_updated_by.map(|u| u.to_string()))
        .bind(remote.amount_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.currency)
        .bind(remote.currency_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.currency_updated_by.map(|u| u.to_string()))
        .bind(remote.currency_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.start_date)
        .bind(remote.start_date_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.start_date_updated_by.map(|u| u.to_string()))
        .bind(remote.start_date_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.end_date)
        .bind(remote.end_date_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.end_date_updated_by.map(|u| u.to_string()))
        .bind(remote.end_date_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.status)
        .bind(remote.status_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.status_updated_by.map(|u| u.to_string()))
        .bind(remote.status_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.reporting_requirements)
        .bind(remote.reporting_requirements_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.reporting_requirements_updated_by.map(|u| u.to_string()))
        .bind(remote.reporting_requirements_updated_by_device_id.map(|u| u.to_string()))
        .bind(&remote.notes)
        .bind(remote.notes_updated_at.map(|d| d.to_rfc3339()))
        .bind(remote.notes_updated_by.map(|u| u.to_string()))
        .bind(remote.notes_updated_by_device_id.map(|u| u.to_string()))
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