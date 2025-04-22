use crate::auth::AuthContext;
use sqlx::SqlitePool;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::workshop::types::{
    NewWorkshop, Workshop, WorkshopRow, UpdateWorkshop,
};
use crate::errors::{DbError, DomainError, DomainResult};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar, Executor, Row, Sqlite, Transaction, sqlite::SqliteArguments};
use sqlx::Arguments;
use uuid::Uuid;
use rust_decimal::Decimal;
use std::str::FromStr;

/// Trait defining workshop repository operations
#[async_trait]
pub trait WorkshopRepository:
    DeleteServiceRepository<Workshop> + Send + Sync
{
    async fn create(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop>;
    async fn create_with_tx<'t>(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop>;
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop>;

    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>, // Optional filter by project
    ) -> DomainResult<PaginatedResult<Workshop>>;
    
    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Workshop>>;
}

/// SQLite implementation for WorkshopRepository
#[derive(Debug, Clone)]
pub struct SqliteWorkshopRepository {
    pool: SqlitePool,
}

impl SqliteWorkshopRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn map_row_to_entity(row: WorkshopRow) -> DomainResult<Workshop> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "workshops"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop> {
        let row = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Workshop".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<Workshop> for SqliteWorkshopRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Workshop> {
        let row = query_as::<_, WorkshopRow>(
            "SELECT * FROM workshops WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Workshop".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteWorkshopRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        
        let result = query(
            "UPDATE workshops SET 
             deleted_at = ?, 
             deleted_by_user_id = ?
             WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(deleted_by)
        .bind(id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // Either not found or already deleted
            Err(DomainError::EntityNotFound("Workshop".to_string(), id)) 
        } else {
            Ok(())
        }
    }

    async fn soft_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.soft_delete_with_tx(id, auth, &mut tx).await;
        match result {
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
impl HardDeletable for SqliteWorkshopRepository {
    fn entity_name(&self) -> &'static str {
        SqliteWorkshopRepository::entity_name(self)
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, // Usually only admin role check is needed, done in service
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM workshops WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
             // This might happen if the record was deleted between check and execution, or never existed.
            Err(DomainError::EntityNotFound("Workshop".to_string(), id))
        } else {
            Ok(())
        }
    }

    async fn hard_delete(&self, id: Uuid, auth: &AuthContext) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.hard_delete_with_tx(id, auth, &mut tx).await;
        match result {
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
impl WorkshopRepository for SqliteWorkshopRepository {
    async fn create(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.create_with_tx(new_workshop, auth, &mut tx).await;
        match result {
            Ok(workshop) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(workshop)
            }
            Err(e) => {
                let _ = tx.rollback().await; // Ignore rollback error
                Err(e)
            }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_workshop: &NewWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let project_id_str = new_workshop.project_id.map(|id| id.to_string());

        // Convert Decimals to String for DB storage
        let budget_str = new_workshop.budget.map(|d| d.to_string());
        let actuals_str = new_workshop.actuals.map(|d| d.to_string());

        // Insert the new workshop
        query(
            r#"
            INSERT INTO workshops (
                id, project_id, 
                purpose, purpose_updated_at, purpose_updated_by,
                event_date, event_date_updated_at, event_date_updated_by,
                location, location_updated_at, location_updated_by,
                budget, budget_updated_at, budget_updated_by,
                actuals, actuals_updated_at, actuals_updated_by,
                participant_count, participant_count_updated_at, participant_count_updated_by,
                local_partner, local_partner_updated_at, local_partner_updated_by,
                partner_responsibility, partner_responsibility_updated_at, partner_responsibility_updated_by,
                -- Skip post-event fields on create
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, 
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                ?, ?, ?, ?, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(project_id_str) // Bind Option<String>
        .bind(&new_workshop.purpose)
        .bind(new_workshop.purpose.as_ref().map(|_| &now_str)).bind(new_workshop.purpose.as_ref().map(|_| &user_id_str)) // purpose LWW
        .bind(&new_workshop.event_date)
        .bind(new_workshop.event_date.as_ref().map(|_| &now_str)).bind(new_workshop.event_date.as_ref().map(|_| &user_id_str)) // event_date LWW
        .bind(&new_workshop.location)
        .bind(new_workshop.location.as_ref().map(|_| &now_str)).bind(new_workshop.location.as_ref().map(|_| &user_id_str)) // location LWW
        .bind(&budget_str) // Bind Option<String>
        .bind(new_workshop.budget.map(|_| &now_str)).bind(new_workshop.budget.map(|_| &user_id_str)) // budget LWW
        .bind(&actuals_str) // Bind Option<String>
        .bind(new_workshop.actuals.map(|_| &now_str)).bind(new_workshop.actuals.map(|_| &user_id_str)) // actuals LWW
        .bind(new_workshop.participant_count.unwrap_or(0)) // Default to 0 if None
        .bind(new_workshop.participant_count.map(|_| &now_str)).bind(new_workshop.participant_count.map(|_| &user_id_str)) // participant_count LWW
        .bind(&new_workshop.local_partner)
        .bind(new_workshop.local_partner.as_ref().map(|_| &now_str)).bind(new_workshop.local_partner.as_ref().map(|_| &user_id_str)) // local_partner LWW
        .bind(&new_workshop.partner_responsibility)
        .bind(new_workshop.partner_responsibility.as_ref().map(|_| &now_str)).bind(new_workshop.partner_responsibility.as_ref().map(|_| &user_id_str)) // partner_responsibility LWW
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&user_id_str).bind(&user_id_str) // created_by, updated_by
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        // Fetch the created workshop to return it
        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
    ) -> DomainResult<Workshop> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(workshop) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(workshop)
            }
            Err(e) => {
                let _ = tx.rollback().await; // Ignore rollback error
                Err(e)
            }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateWorkshop,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Workshop> {
        // Fetch current to ensure it exists before update
        let _current_workshop = self.find_by_id_with_tx(id, tx).await?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        let mut set_clauses = Vec::new();
        let mut args = SqliteArguments::default(); // Use SqliteArguments

        macro_rules! add_lww_update {
            ($field:ident, $value:expr) => {
                if let Some(val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    let _ = args.add(val);
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                }
            };
            ($field:ident, $value:expr, string_convert) => {
                if let Some(val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    let _ = args.add(val.to_string()); // Convert to string before adding
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                }
            };
            // Handle Option<String> fields
            ($field:ident, $value:expr, option_string) => {
                if let Some(opt_val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    let _ = args.add(opt_val);
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                }
            };
            // Handle Option<Decimal> fields
            ($field:ident, $value:expr, option_decimal) => {
                if let Some(opt_val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    let _ = args.add(opt_val.to_string()); // Convert Decimal to String
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                }
            };
            // Handle Option<Option<Uuid>> for project_id
            ($field:ident, $value:expr, option_option_uuid) => {
                if let Some(opt_opt_val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    let _ = args.add(opt_opt_val.map(|u| u.to_string()));
                    let _ = args.add(&now_str);
                    let _ = args.add(&user_id_str);
                }
            };
        }

        // Apply the macros for each updatable field
        add_lww_update!(project_id, &update_data.project_id, option_option_uuid);
        add_lww_update!(purpose, &update_data.purpose, option_string);
        add_lww_update!(event_date, &update_data.event_date, option_string);
        add_lww_update!(location, &update_data.location, option_string);
        add_lww_update!(budget, &update_data.budget, option_decimal);
        add_lww_update!(actuals, &update_data.actuals, option_decimal);
        add_lww_update!(participant_count, &update_data.participant_count, string_convert);
        add_lww_update!(local_partner, &update_data.local_partner, option_string);
        add_lww_update!(partner_responsibility, &update_data.partner_responsibility, option_string);
        // Post-event fields
        add_lww_update!(partnership_success, &update_data.partnership_success, option_string);
        add_lww_update!(capacity_challenges, &update_data.capacity_challenges, option_string);
        add_lww_update!(strengths, &update_data.strengths, option_string);
        add_lww_update!(outcomes, &update_data.outcomes, option_string);
        add_lww_update!(recommendations, &update_data.recommendations, option_string);
        add_lww_update!(challenge_resolution, &update_data.challenge_resolution, option_string);

        if set_clauses.is_empty() {
            // No fields to update other than the main timestamp/user
            return self.find_by_id_with_tx(id, tx).await;
        }

        // Always update the main timestamp and user ID
        set_clauses.push("updated_at = ?".to_string());
        let _ = args.add(&now_str);
        set_clauses.push("updated_by_user_id = ?".to_string());
        let _ = args.add(&user_id_str);

        let query_str = format!(
            "UPDATE workshops SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );

        // Add the ID parameter last
        let _ = args.add(id.to_string());

        // Build and execute the query with arguments
        let result = sqlx::query_with(&query_str, args)
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            // Could be already deleted or ID not found during update
            return Err(DomainError::EntityNotFound("Workshop".to_string(), id));
        }

        // Fetch and return the updated entity
        self.find_by_id_with_tx(id, tx).await
    }

    async fn find_all(
        &self,
        params: PaginationParams,
        project_id: Option<Uuid>,
    ) -> DomainResult<PaginatedResult<Workshop>> {
        let offset = (params.page - 1) * params.per_page;
        
        let mut conditions = vec!["deleted_at IS NULL"];
        let mut bind_values: Vec<String> = Vec::new();

        if let Some(p_id) = project_id {
            conditions.push("project_id = ?");
            bind_values.push(p_id.to_string());
        }
        
        let where_clause = if conditions.is_empty() { "".to_string() } else { format!("WHERE {}", conditions.join(" AND ")) };

        // Get total count with filter
        let count_query_str = format!("SELECT COUNT(*) FROM workshops {}", where_clause);
        let mut count_query = query_scalar(&count_query_str);
        for val in &bind_values {
            count_query = count_query.bind(val);
        }
        let total: i64 = count_query
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Fetch paginated rows with filter
        let select_query_str = format!(
            "SELECT * FROM workshops {} ORDER BY event_date DESC, created_at DESC LIMIT ? OFFSET ?", 
            where_clause
        );
        let mut select_query = query_as::<_, WorkshopRow>(&select_query_str);
        for val in &bind_values {
            select_query = select_query.bind(val);
        }
        // Bind limit and offset
        select_query = select_query.bind(params.per_page as i64);
        select_query = select_query.bind(offset as i64);
        
        let rows = select_query
            .fetch_all(&self.pool)
            .await
            .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Workshop>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
     async fn find_by_project_id(
         &self,
         project_id: Uuid,
         params: PaginationParams,
     ) -> DomainResult<PaginatedResult<Workshop>> {
         self.find_all(params, Some(project_id)).await
     }
}
