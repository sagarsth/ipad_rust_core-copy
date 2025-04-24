use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, Arguments, sqlite::SqliteArguments};
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::project::types::{NewProject, Project, ProjectRow, UpdateProject};
use crate::errors::{DbError, DomainError, DomainResult};
use crate::types::{PaginatedResult, PaginationParams, SyncPriority};
use async_trait::async_trait;
use chrono::Utc;
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;

/// Trait defining project repository operations
#[async_trait]
pub trait ProjectRepository: DeleteServiceRepository<Project> + Send + Sync {
    async fn create(
        &self,
        new_project: &NewProject,
        auth: &AuthContext,
    ) -> DomainResult<Project>;
    async fn create_with_tx<'t>(
        &self,
        new_project: &NewProject,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Project>;

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateProject,
        auth: &AuthContext,
    ) -> DomainResult<Project>;
    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateProject,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Project>;

    async fn find_all(
        &self,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>>;
    
    async fn find_by_strategic_goal(
        &self,
        strategic_goal_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>>;

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriority,
        auth: &AuthContext,
    ) -> DomainResult<u64>;
}

/// SQLite implementation for ProjectRepository
#[derive(Debug, Clone)]
pub struct SqliteProjectRepository {
    pool: SqlitePool,
}

impl SqliteProjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    fn map_row_to_entity(row: ProjectRow) -> DomainResult<Project> {
        row.into_entity()
            .map_err(|e| DomainError::Internal(format!("Failed to map row to entity: {}", e)))
    }

    // Helper to find by ID within a transaction
    async fn find_by_id_with_tx<'t>(
        &self,
        id: Uuid,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Project> {
        let row = query_as::<_, ProjectRow>(
            "SELECT * FROM projects WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&mut **tx)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Project".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl FindById<Project> for SqliteProjectRepository {
    async fn find_by_id(&self, id: Uuid) -> DomainResult<Project> {
        let row = query_as::<_, ProjectRow>(
            "SELECT * FROM projects WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(DbError::from)?
        .ok_or_else(|| DomainError::EntityNotFound("Project".to_string(), id))?;

        Self::map_row_to_entity(row)
    }
}

#[async_trait]
impl SoftDeletable for SqliteProjectRepository {
    async fn soft_delete_with_tx(
        &self,
        id: Uuid,
        auth: &AuthContext,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        let now = Utc::now().to_rfc3339();
        let deleted_by = auth.user_id.to_string();
        
        let result = query(
            "UPDATE projects SET deleted_at = ?, deleted_by_user_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(deleted_by)
        .bind(id.to_string())
        .execute(&mut **tx) 
        .await
        .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Project".to_string(), id))
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
impl HardDeletable for SqliteProjectRepository {
    fn entity_name(&self) -> &'static str {
        "projects"
    }
    
    async fn hard_delete_with_tx(
        &self,
        id: Uuid,
        _auth: &AuthContext, // Auth context might be used for logging/checks later
        tx: &mut Transaction<'_, Sqlite>,
    ) -> DomainResult<()> {
        // Note: Cascade delete for related activities is handled by DB schema
        let result = query("DELETE FROM projects WHERE id = ?")
            .bind(id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Project".to_string(), id))
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
impl ProjectRepository for SqliteProjectRepository {
    async fn create(
        &self,
        new_project: &NewProject,
        auth: &AuthContext,
    ) -> DomainResult<Project> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.create_with_tx(new_project, auth, &mut tx).await;
        match result {
            Ok(project) => { tx.commit().await.map_err(DbError::from)?; Ok(project) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_project: &NewProject,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Project> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let sg_id_str = new_project.strategic_goal_id.map(|id| id.to_string());

        query(
            r#"
            INSERT INTO projects (
                id, strategic_goal_id, 
                name, name_updated_at, name_updated_by,
                objective, objective_updated_at, objective_updated_by,
                outcome, outcome_updated_at, outcome_updated_by,
                status_id, status_id_updated_at, status_id_updated_by,
                timeline, timeline_updated_at, timeline_updated_by,
                responsible_team, responsible_team_updated_at, responsible_team_updated_by,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                deleted_at, deleted_by_user_id
            ) VALUES (
                ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 
                ?,
                ?, ?, ?, ?, NULL, NULL
            )
            "#,
        )
        .bind(id.to_string())
        .bind(sg_id_str)
        .bind(&new_project.name).bind(&now).bind(&user_id_str) // name LWW
        .bind(&new_project.objective)
        .bind(new_project.objective.as_ref().map(|_| &now)).bind(new_project.objective.as_ref().map(|_| &user_id_str)) // objective LWW
        .bind(&new_project.outcome)
        .bind(new_project.outcome.as_ref().map(|_| &now)).bind(new_project.outcome.as_ref().map(|_| &user_id_str)) // outcome LWW
        .bind(new_project.status_id)
        .bind(new_project.status_id.map(|_| &now)).bind(new_project.status_id.map(|_| &user_id_str)) // status_id LWW
        .bind(&new_project.timeline)
        .bind(new_project.timeline.as_ref().map(|_| &now)).bind(new_project.timeline.as_ref().map(|_| &user_id_str)) // timeline LWW
        .bind(&new_project.responsible_team)
        .bind(new_project.responsible_team.as_ref().map(|_| &now)).bind(new_project.responsible_team.as_ref().map(|_| &user_id_str)) // responsible_team LWW
        .bind(new_project.sync_priority as i64)
        .bind(&now).bind(&now) // created_at, updated_at
        .bind(&user_id_str).bind(&user_id_str) // created_by, updated_by
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;

        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateProject,
        auth: &AuthContext,
    ) -> DomainResult<Project> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(project) => { tx.commit().await.map_err(DbError::from)?; Ok(project) },
            Err(e) => { let _ = tx.rollback().await; Err(e) }
        }
    }

    async fn update_with_tx<'t>(
        &self,
        id: Uuid,
        update_data: &UpdateProject,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Project> {
        let _ = self.find_by_id_with_tx(id, tx).await?; // Ensure exists

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();

        let mut set_clauses = Vec::new();
        let mut args = SqliteArguments::default();

        macro_rules! add_lww_update_option {($field:ident, $value:expr) => {
            if let Some(val) = $value {
                set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                args.add(val);
                args.add(&now);
                args.add(&user_id_str);
            }
        };}
        macro_rules! add_lww_update {($field:ident, $value:expr) => {
            if let Some(val) = $value {
                set_clauses.push(format!("{0} = ?, {0}_updated_at = ?, {0}_updated_by = ?", stringify!($field)));
                args.add(val);
                args.add(&now);
                args.add(&user_id_str);
            }
        };}

        if let Some(opt_sg_id) = update_data.strategic_goal_id {
             set_clauses.push("strategic_goal_id = ?".to_string());
             args.add(opt_sg_id.map(|id| id.to_string())); 
        }
        
        add_lww_update!(name, &update_data.name);
        add_lww_update_option!(objective, &update_data.objective);
        add_lww_update_option!(outcome, &update_data.outcome);
        add_lww_update!(status_id, &update_data.status_id);
        add_lww_update_option!(timeline, &update_data.timeline);
        add_lww_update_option!(responsible_team, &update_data.responsible_team);

        if let Some(priority) = update_data.sync_priority {
            set_clauses.push("sync_priority = ?".to_string());
            args.add(priority as i64);
        }
        
        if set_clauses.is_empty() {
             return self.find_by_id_with_tx(id, tx).await;
        }

        set_clauses.push("updated_at = ?".to_string());
        args.add(&now);
        set_clauses.push("updated_by_user_id = ?".to_string());
        args.add(&user_id_str);

        let query_str = format!(
            "UPDATE projects SET {} WHERE id = ? AND deleted_at IS NULL",
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
    ) -> DomainResult<PaginatedResult<Project>> {
        let offset = (params.page - 1) * params.per_page;

        let total: i64 = query_scalar("SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL")
            .fetch_one(&self.pool)
            .await
            .map_err(DbError::from)?;

        let rows = query_as::<_, ProjectRow>(
            "SELECT * FROM projects WHERE deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?",
        )
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Project>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }
    
    async fn find_by_strategic_goal(
        &self,
        strategic_goal_id: Uuid,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>> {
        let offset = (params.page - 1) * params.per_page;
        let sg_id_str = strategic_goal_id.to_string();

        let total: i64 = query_scalar(
             "SELECT COUNT(*) FROM projects WHERE strategic_goal_id = ? AND deleted_at IS NULL"
         )
         .bind(&sg_id_str)
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;

        let rows = query_as::<_, ProjectRow>(
            "SELECT * FROM projects WHERE strategic_goal_id = ? AND deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?",
        )
        .bind(sg_id_str)
        .bind(params.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        let entities = rows
            .into_iter()
            .map(Self::map_row_to_entity)
            .collect::<DomainResult<Vec<Project>>>()?;

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
