use crate::auth::AuthContext;
use sqlx::SqlitePool;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::strategic_goal::types::{
    NewStrategicGoal, StrategicGoal, StrategicGoalRow, UpdateStrategicGoal,
};
use crate::errors::{DbError, DomainError, DomainResult};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar, Executor, Row, Sqlite, Transaction, Arguments, sqlite::SqliteArguments};
use uuid::Uuid;

/// Trait defining strategic goal repository operations
#[async_trait]
pub trait StrategicGoalRepository:
    DeleteServiceRepository<StrategicGoal> + Send + Sync
{
    async fn create(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal>;

    /// Create strategic goal within an existing transaction
    /// Used for operations that need to manage document linking
    async fn create_with_tx<'t>(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal>;

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal>;

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>>;
    
    async fn find_by_objective_code(&self, code: &str) -> DomainResult<Option<StrategicGoal>>;

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;
}

/// SQLite implementation for StrategicGoalRepository
#[derive(Debug, Clone)]
pub struct SqliteStrategicGoalRepository {
    pool: SqlitePool,
}

impl SqliteStrategicGoalRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn map_row_to_entity(row: StrategicGoalRow) -> DomainResult<StrategicGoal> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    fn entity_name(&self) -> &'static str {
        "strategic_goals"
    }

    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal> {
        let row = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx) // Use &mut **tx for borrowing
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("StrategicGoal".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<StrategicGoal> for SqliteStrategicGoalRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<StrategicGoal> {
        let row = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("StrategicGoal".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteStrategicGoalRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        
        let result = query(
            "UPDATE strategic_goals SET 
             deleted_at = ?, 
             deleted_by_user_id = ?
             WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(deleted_by)
        .bind(id.to_string())
        .execute(&mut **tx) // Use &mut **tx here
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("StrategicGoal".to_string(), id))
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
impl HardDeletable for SqliteStrategicGoalRepository {
    fn entity_name(&self) -> &'static str {
        SqliteStrategicGoalRepository::entity_name(self)
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM strategic_goals WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx) // Use &mut **tx here
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("StrategicGoal".to_string(), id))
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
impl StrategicGoalRepository for SqliteStrategicGoalRepository {
    async fn create(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.create_with_tx(new_goal, auth, &mut tx).await;
        match result {
            Ok(goal) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(goal)
            }
            Err(e) => {
                let _ = tx.rollback().await; // Ignore rollback error
                Err(e)
            }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_goal: &NewStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal> {
        let id = new_goal.id.unwrap_or_else(Uuid::new_v4);
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        // Insert the new strategic goal
        query(
            r#"
            INSERT INTO strategic_goals (
                id, objective_code, objective_code_updated_at, objective_code_updated_by,
                outcome, outcome_updated_at, outcome_updated_by,
                kpi, kpi_updated_at, kpi_updated_by,
                target_value, target_value_updated_at, target_value_updated_by,
                actual_value, actual_value_updated_at, actual_value_updated_by,
                status_id, status_id_updated_at, status_id_updated_by,
                responsible_team, responsible_team_updated_at, responsible_team_updated_by,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 
                ?, -- for sync_priority
                ?, ?, ?, ?, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(&new_goal.objective_code)
        .bind(&now_str).bind(&user_id_str) // objective_code LWW
        .bind(&new_goal.outcome)
        .bind(new_goal.outcome.as_ref().map(|_| &now_str)).bind(new_goal.outcome.as_ref().map(|_| &user_id_str)) // outcome LWW
        .bind(&new_goal.kpi)
        .bind(new_goal.kpi.as_ref().map(|_| &now_str)).bind(new_goal.kpi.as_ref().map(|_| &user_id_str)) // kpi LWW
        .bind(new_goal.target_value)
        .bind(new_goal.target_value.map(|_| &now_str)).bind(new_goal.target_value.map(|_| &user_id_str)) // target_value LWW
        .bind(new_goal.actual_value.unwrap_or(0.0)) // Use default if None
        .bind(new_goal.actual_value.map(|_| &now_str)).bind(new_goal.actual_value.map(|_| &user_id_str)) // actual_value LWW
        .bind(new_goal.status_id)
        .bind(new_goal.status_id.map(|_| &now_str)).bind(new_goal.status_id.map(|_| &user_id_str)) // status_id LWW
        .bind(&new_goal.responsible_team)
        .bind(new_goal.responsible_team.as_ref().map(|_| &now_str)).bind(new_goal.responsible_team.as_ref().map(|_| &user_id_str)) // responsible_team LWW
        .bind(new_goal.sync_priority as i64)
        .bind(&now_str).bind(&now_str) // created_at, updated_at
        .bind(&user_id_str).bind(&user_id_str) // created_by, updated_by
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        // Fetch the created goal to return it
        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
    ) -> DomainResult<StrategicGoal> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(goal) => { tx.commit().await.map_err(DbError::from)?; Ok(goal) }
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateStrategicGoal,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<StrategicGoal> {
        let _current_goal = self.find_by_id_with_tx(id, tx).await?;
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        let mut set_clauses = Vec::new();
        let mut args = SqliteArguments::default();
        
        macro_rules! add_lww_update {($field:ident, $value:expr) => {
            if let Some(val) = $value {
                set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                args.add(val);
                args.add(&now);
                args.add(&user_id_str);
            }
        };}

        add_lww_update!(objective_code, &update_data.objective_code);
        add_lww_update!(outcome, &update_data.outcome);
        add_lww_update!(kpi, &update_data.kpi);
        add_lww_update!(target_value, &update_data.target_value);
        add_lww_update!(actual_value, &update_data.actual_value);
        add_lww_update!(status_id, &update_data.status_id);
        add_lww_update!(responsible_team, &update_data.responsible_team);

        if let Some(priority) = update_data.sync_priority {
            set_clauses.push("sync_priority = ?".to_string());
            args.add(priority as i64);
        }

        if set_clauses.is_empty() {
             return Ok(_current_goal);
        }
        set_clauses.push("updated_at = ?".to_string());
        args.add(&now);
        set_clauses.push("updated_by_user_id = ?".to_string());
        args.add(&user_id_str);

        let query_str = format!(
            "UPDATE strategic_goals SET {} WHERE id = ? AND deleted_at IS NULL",
            set_clauses.join(", ")
        );
        args.add(id.to_string());
        
        let result = sqlx::query_with(&query_str, args)
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
             return Err(DomainError::EntityNotFound(Self::entity_name(self).to_string(), id));
        }
        
        self.find_by_id_with_tx(id, tx).await
    }

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<StrategicGoal>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar("SELECT COUNT(*) FROM strategic_goals WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, StrategicGoalRow>(
            "SELECT * FROM strategic_goals WHERE deleted_at IS NULL ORDER BY objective_code ASC LIMIT ? OFFSET ?",
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<StrategicGoal>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
     async fn find_by_objective_code(&self, code: &str) -> DomainResult<Option<StrategicGoal>> {
         let row = query_as::<_, StrategicGoalRow>(
             "SELECT * FROM strategic_goals WHERE objective_code = ? AND deleted_at IS NULL",
         )
         .bind(code)
         .fetch_optional(&self.pool)
         .await
         .map_err(DbError::from)?;
         
         match row {
             Some(r) => Self::map_row_to_entity(r).map(Some),
             None => Ok(None),
         }
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
        
        let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "UPDATE {} SET sync_priority = ?, updated_at = ?, updated_by_user_id = ? WHERE id IN ({}) AND deleted_at IS NULL",
            Self::entity_name(self),
            placeholders
        );
        
        let mut query_builder = sqlx::query(&query_str)
            .bind(priority_val)
            .bind(now)
            .bind(user_id_str);
            
        for id in ids {
            query_builder = query_builder.bind(id.to_string());
        }
        
        let result = query_builder.execute(&self.pool).await.map_err(DbError::from)?;
        Ok(result.rows_affected())
    }
}