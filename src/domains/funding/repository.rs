use crate::auth::AuthContext;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::funding::types::{ProjectFunding, NewProjectFunding, UpdateProjectFunding, ProjectFundingRow};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{Pool, Sqlite, Transaction, query, query_as, query_scalar, Row};
use uuid::Uuid;

/// Trait defining funding repository operations
#[async_trait]
pub trait ProjectFundingRepository: 
    DeleteServiceRepository<ProjectFunding> + Send + Sync 
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

    // Add the document reference method
    async fn set_document_reference(
        &self,
        funding_id: Uuid,
        field_name: &str,
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
}

/// SQLite implementation for ProjectFundingRepository
#[derive(Debug, Clone)]
pub struct SqliteProjectFundingRepository {
    pool: Pool<Sqlite>,
}

impl SqliteProjectFundingRepository {
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
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
        let now = Utc::now().to_rfc3339();
        let result = query(
            "UPDATE project_funding SET deleted_at = ?, deleted_by_user_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(auth.user_id.to_string())
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
        _auth: &AuthContext,
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
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let project_id_str = new_funding.project_id.to_string();
        let donor_id_str = new_funding.donor_id.to_string();
        
        // Define the default currency if none is provided
        let currency = new_funding.currency.clone().unwrap_or_else(|| "AUD".to_string());

        query(
            r#"
            INSERT INTO project_funding (
                id, project_id, project_id_updated_at, project_id_updated_by,
                donor_id, donor_id_updated_at, donor_id_updated_by,
                grant_id, grant_id_updated_at, grant_id_updated_by,
                amount, amount_updated_at, amount_updated_by,
                currency, currency_updated_at, currency_updated_by,
                start_date, start_date_updated_at, start_date_updated_by,
                end_date, end_date_updated_at, end_date_updated_by,
                status, status_updated_at, status_updated_by,
                reporting_requirements, reporting_requirements_updated_at, reporting_requirements_updated_by,
                notes, notes_updated_at, notes_updated_by,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL
            )
            "#
        )
        .bind(id.to_string())
        .bind(project_id_str).bind(&now).bind(&user_id_str) // project_id with LWW metadata
        .bind(donor_id_str).bind(&now).bind(&user_id_str) // donor_id with LWW metadata
        .bind(&new_funding.grant_id).bind(new_funding.grant_id.as_ref().map(|_| &now)).bind(new_funding.grant_id.as_ref().map(|_| &user_id_str))
        .bind(new_funding.amount).bind(new_funding.amount.map(|_| &now)).bind(new_funding.amount.map(|_| &user_id_str))
        .bind(&currency).bind(&now).bind(&user_id_str) // currency with LWW metadata
        .bind(&new_funding.start_date).bind(new_funding.start_date.as_ref().map(|_| &now)).bind(new_funding.start_date.as_ref().map(|_| &user_id_str))
        .bind(&new_funding.end_date).bind(new_funding.end_date.as_ref().map(|_| &now)).bind(new_funding.end_date.as_ref().map(|_| &user_id_str))
        .bind(&new_funding.status).bind(new_funding.status.as_ref().map(|_| &now)).bind(new_funding.status.as_ref().map(|_| &user_id_str))
        .bind(&new_funding.reporting_requirements).bind(new_funding.reporting_requirements.as_ref().map(|_| &now)).bind(new_funding.reporting_requirements.as_ref().map(|_| &user_id_str))
        .bind(&new_funding.notes).bind(new_funding.notes.as_ref().map(|_| &now)).bind(new_funding.notes.as_ref().map(|_| &user_id_str))
        .bind(&now).bind(&now) // created_at, updated_at
        .bind(new_funding.created_by_user_id.as_ref().map(|id| id.to_string()).unwrap_or(user_id_str.clone())).bind(&user_id_str) // created_by, updated_by
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

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
        let _ = self.find_by_id_with_tx(id, tx).await?; // Ensure exists
        
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let id_str = id.to_string();

        let mut builder = sqlx::QueryBuilder::new("UPDATE project_funding SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        macro_rules! add_lww_option {($field_name:ident, $field_sql:literal, $value:expr) => {
            if let Some(val) = $value {
                separated.push(concat!($field_sql, " = "));
                separated.push_bind_unseparated(val);
                separated.push(concat!(" ", $field_sql, "_updated_at = "));
                separated.push_bind_unseparated(now.clone());
                separated.push(concat!(" ", $field_sql, "_updated_by = "));
                separated.push_bind_unseparated(user_id_str.clone());
                fields_updated = true;
            }
        };}

        macro_rules! add_lww_uuid {($field_name:ident, $field_sql:literal, $value:expr) => {
            if let Some(val) = $value {
                separated.push(concat!($field_sql, " = "));
                separated.push_bind_unseparated(val.to_string());
                separated.push(concat!(" ", $field_sql, "_updated_at = "));
                separated.push_bind_unseparated(now.clone());
                separated.push(concat!(" ", $field_sql, "_updated_by = "));
                separated.push_bind_unseparated(user_id_str.clone());
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
            return self.find_by_id_with_tx(id, tx).await;
        }

        separated.push("updated_at = ");
        separated.push_bind_unseparated(now);
        separated.push("updated_by_user_id = ");
        separated.push_bind_unseparated(user_id_str);

        builder.push(" WHERE id = ");
        builder.push_bind(id_str);
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound("Project Funding".to_string(), id));
        }

        self.find_by_id_with_tx(id, tx).await
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
        let project_id_str = project_id.to_string();
        
        // Get active funding count and total amount
        let result = query(
            r#"
            SELECT 
                COUNT(*) as funding_count,
                COALESCE(SUM(amount), 0) as total_amount
            FROM project_funding
            WHERE project_id = ? 
            AND deleted_at IS NULL
            "#
        )
        .bind(project_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let funding_count: i64 = result.try_get("funding_count").map_err(DbError::from)?;
        let total_amount: f64 = result.try_get("total_amount").map_err(DbError::from)?;
        
        Ok((funding_count, total_amount))
    }
    
    async fn get_donor_funding_stats(
        &self,
        donor_id: Uuid,
    ) -> DomainResult<(i64, f64)> {
        let donor_id_str = donor_id.to_string();
        
        // Get active funding count and total amount
        // For active count, we filter based on date range and status
        let result = query(
            r#"
            SELECT 
                COUNT(CASE WHEN 
                    (status IS NULL OR status NOT IN ('completed', 'cancelled')) AND
                    (start_date IS NULL OR DATE(start_date) <= DATE('now')) AND
                    (end_date IS NULL OR DATE(end_date) >= DATE('now'))
                THEN 1 ELSE NULL END) as active_count,
                COALESCE(SUM(amount), 0) as total_amount
            FROM project_funding
            WHERE donor_id = ? 
            AND deleted_at IS NULL
            "#
        )
        .bind(donor_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        let active_count: i64 = result.try_get("active_count").map_err(DbError::from)?;
        let total_amount: f64 = result.try_get("total_amount").map_err(DbError::from)?;
        
        Ok((active_count, total_amount))
    }

    // Add the document reference method implementation
    async fn set_document_reference(
        &self,
        funding_id: Uuid,
        field_name: &str,
        document_id: Uuid, // Currently unused as no doc-only fields
        auth: &AuthContext, // Currently unused, but needed for trait sig
    ) -> DomainResult<()> {
        // Validate the field name using the DocumentLinkable implementation
        let field_meta = ProjectFunding::field_metadata()
            .into_iter()
            .find(|meta| meta.field_name == field_name)
            .ok_or_else(|| {
                DomainError::Validation(ValidationError::Custom(
                    format!("Invalid field name for ProjectFunding: {}", field_name)
                ))
            })?;

        // Ensure the field actually supports documents (should already be true if found)
        if !field_meta.supports_documents {
             return Err(DomainError::Validation(ValidationError::Custom(
                format!("Field '{}' does not support document linking for ProjectFunding", field_name)
             )));
        }

        // Since ProjectFunding currently has no fields marked as `is_document_reference_only`,
        // this method doesn't need to update the funding record itself.
        // Documents are linked via the documents table's `parent_entity_id` and `parent_entity_type`.
        // If doc-only fields are added later, update logic would go here.
        
        // We should still check if the funding record exists to ensure valid linking
        let _ = self.find_by_id(funding_id).await?;

        Ok(())
    }
}