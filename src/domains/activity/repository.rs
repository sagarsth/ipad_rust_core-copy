use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::activity::types::{NewActivity, Activity, ActivityRow, UpdateActivity};
use crate::errors::{DbError, DomainError, DomainResult};
use crate::types::{PaginatedResult, PaginationParams};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;

/// Trait defining activity repository operations
#[async_trait]
pub trait ActivityRepository: DeleteServiceRepository<Activity> + Send + Sync {
    async fn create(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity>;
    async fn create_with_tx<'t>(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity>;
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity>;

    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>>;
}

/// SQLite implementation for ActivityRepository
#[derive(Debug, Clone)]
pub struct SqliteActivityRepository {
    pool: SqlitePool,
}

impl SqliteActivityRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn map_row_to_entity(row: ActivityRow) -> DomainResult<Activity> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    // Helper to find by ID within a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity> {
        let row = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Activity".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<Activity> for SqliteActivityRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Activity> {
        let row = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Activity".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteActivityRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        
        let result = query(
            "UPDATE activities SET deleted_at = ?, deleted_by_user_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(deleted_by)
        .bind(id.to_string())
        .execute(&mut **tx) 
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Activity".to_string(), id))
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
impl HardDeletable for SqliteActivityRepository {
    fn entity_name(&self) -> &'static str {
        "activities"
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, 
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let result = query("DELETE FROM activities WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Activity".to_string(), id))
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
impl ActivityRepository for SqliteActivityRepository {
    async fn create(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.create_with_tx(new_activity, auth, &mut tx).await {
            Ok(activity) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(activity)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_activity: &NewActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let project_id_str = new_activity.project_id.map(|id| id.to_string());

        query(
            r#"
            INSERT INTO activities (
                id, project_id,
                description, description_updated_at, description_updated_by,
                kpi, kpi_updated_at, kpi_updated_by,
                target_value, target_value_updated_at, target_value_updated_by,
                actual_value, actual_value_updated_at, actual_value_updated_by,
                status_id, status_id_updated_at, status_id_updated_by,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(project_id_str)
        .bind(&new_activity.description).bind(new_activity.description.as_ref().map(|_| &now_str)).bind(new_activity.description.as_ref().map(|_| &user_id_str))
        .bind(&new_activity.kpi).bind(new_activity.kpi.as_ref().map(|_| &now_str)).bind(new_activity.kpi.as_ref().map(|_| &user_id_str))
        .bind(new_activity.target_value).bind(new_activity.target_value.map(|_| &now_str)).bind(new_activity.target_value.map(|_| &user_id_str))
        .bind(new_activity.actual_value.unwrap_or(0.0)).bind(new_activity.actual_value.map(|_| &now_str)).bind(new_activity.actual_value.map(|_| &user_id_str))
        .bind(new_activity.status_id).bind(new_activity.status_id.map(|_| &now_str)).bind(new_activity.status_id.map(|_| &user_id_str))
        .bind(&now_str).bind(&now_str)
        .bind(&user_id_str).bind(&user_id_str)
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateActivity,
        auth: &AuthContext,
    ) -> DomainResult<Activity> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.update_with_tx(id, update_data, auth, &mut tx).await {
            Ok(activity) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(activity)
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
        update_data: &UpdateActivity,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Activity> {
        let _current_activity = self.find_by_id_with_tx(id, tx).await?;

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
             ($field:ident, $value:expr, numeric) => {
                 if let Some(val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    params.push(Some(val.to_string()));
                    params.push(Some(now_str.clone()));
                    params.push(Some(user_id_str.clone()));
                 }
             };
             ($field:ident, $value:expr, optional_uuid) => {
                if let Some(opt_val) = $value {
                    set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                    params.push(opt_val.map(|id| id.to_string()));
                    params.push(Some(now_str.clone()));
                    params.push(Some(user_id_str.clone()));
                }
            };
        }
        
        add_lww_update!(project_id, &update_data.project_id, optional_uuid);
        add_lww_update!(description, &update_data.description, string);
        add_lww_update!(kpi, &update_data.kpi, string);
        add_lww_update!(target_value, &update_data.target_value, numeric);
        add_lww_update!(actual_value, &update_data.actual_value, numeric);
        add_lww_update!(status_id, &update_data.status_id, numeric);

        set_clauses.push("updated_at = ?".to_string());
        params.push(Some(now_str.clone()));
        set_clauses.push("updated_by_user_id = ?".to_string());
        params.push(Some(user_id_str.clone()));
        
        if set_clauses.len() <= 2 { 
             return Ok(_current_activity);
        }

        let query_str = format!(
            "UPDATE activities SET {} WHERE id = ? AND deleted_at IS NULL",
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
            return Err(DomainError::EntityNotFound("Activity".to_string(), id));
        }
        
        self.find_by_id_with_tx(id, tx).await
    }

    async fn find_by_project_id(
        &self,
        project_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Activity>> {
        let offset = (params.page - 1) * params.per_page;
        let project_id_str = project_id.to_string();

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM activities WHERE project_id = ? AND deleted_at IS NULL"
         )
         .bind(&project_id_str)
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ActivityRow>(
            "SELECT * FROM activities WHERE project_id = ? AND deleted_at IS NULL ORDER BY created_at ASC LIMIT ? OFFSET ?",
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
            .collect::<DomainResult<Vec<Activity>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
}
