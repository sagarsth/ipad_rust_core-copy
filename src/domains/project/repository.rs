use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, Arguments, sqlite::SqliteArguments};
use sqlx::QueryBuilder;
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::project::types::{NewProject, Project, ProjectRow, UpdateProject};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
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

    async fn set_document_reference(
        &self,
        project_id: Uuid,
        field_name: &str, // e.g., "proposal_document"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()>;
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
        let created_by_id_str = new_project.created_by_user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| user_id_str.clone());

        let mut builder = QueryBuilder::new(
            r#"INSERT INTO projects (
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
            ) "#
        );

        builder.push_values([ (
            id.to_string(), sg_id_str,
            new_project.name.clone(), now.clone(), user_id_str.clone(),
            new_project.objective.clone(), new_project.objective.as_ref().map(|_| &now), new_project.objective.as_ref().map(|_| &user_id_str),
            new_project.outcome.clone(), new_project.outcome.as_ref().map(|_| &now), new_project.outcome.as_ref().map(|_| &user_id_str),
            new_project.status_id.clone(), new_project.status_id.as_ref().map(|_| &now), new_project.status_id.as_ref().map(|_| &user_id_str),
            new_project.timeline.clone(), new_project.timeline.as_ref().map(|_| &now), new_project.timeline.as_ref().map(|_| &user_id_str),
            new_project.responsible_team.clone(), new_project.responsible_team.as_ref().map(|_| &now), new_project.responsible_team.as_ref().map(|_| &user_id_str),
            new_project.sync_priority as i64,
            now.clone(), now.clone(), created_by_id_str, user_id_str.clone(),
            Option::<String>::None, Option::<String>::None
        )], |mut b, values| {
             b.push_bind(values.0); b.push_bind(values.1);
             b.push_bind(values.2); b.push_bind(values.3); b.push_bind(values.4);
             b.push_bind(values.5); b.push_bind(values.6); b.push_bind(values.7);
             b.push_bind(values.8); b.push_bind(values.9); b.push_bind(values.10);
             b.push_bind(values.11); b.push_bind(values.12); b.push_bind(values.13);
             b.push_bind(values.14); b.push_bind(values.15); b.push_bind(values.16);
             b.push_bind(values.17); b.push_bind(values.18); b.push_bind(values.19);
             b.push_bind(values.20);
             b.push_bind(values.21); b.push_bind(values.22); b.push_bind(values.23); b.push_bind(values.24);
             b.push_bind(values.25); b.push_bind(values.26);
        });

        let query = builder.build();
        query.execute(&mut **tx).await.map_err(DbError::from)?;

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
        let _ = self.find_by_id_with_tx(id, tx).await?; // Ensure the project exists
        
        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let id_str = id.to_string();

        // Define LWW macros locally
        macro_rules! add_lww_option {($builder:expr, $separated:expr, $field_sql:literal, $value:expr, $now_ref:expr, $user_id_ref:expr, $fields_updated_flag:expr) => {
            if let Some(ref val) = $value {
                $separated.push(concat!($field_sql, " = "));
                $separated.push_bind_unseparated(val.clone());
                $separated.push(concat!(" ", $field_sql, "_updated_at = "));
                $separated.push_bind_unseparated($now_ref.clone());
                $separated.push(concat!(" ", $field_sql, "_updated_by = "));
                $separated.push_bind_unseparated($user_id_ref.clone());
                $fields_updated_flag = true;
            }
        };}
        
        macro_rules! add_lww_uuid_option {($builder:expr, $separated:expr, $field_sql:literal, $value:expr, $now_ref:expr, $user_id_ref:expr, $fields_updated_flag:expr) => {
            // Handle Option<Option<Uuid>> by checking both Some layers
            if let Some(inner_option) = $value { 
                if let Some(uuid_val) = inner_option { // Check inner Option
                    let uuid_str = uuid_val.to_string(); // Convert unwrapped Uuid
                    $separated.push(concat!($field_sql, " = "));
                    $separated.push_bind_unseparated(uuid_str);
                } else { // Handle case where outer is Some but inner is None (explicit NULL)
                    $separated.push(concat!($field_sql, " = NULL")); 
                }
                // Regardless of inner value, update LWW timestamps if outer Option was Some
                $separated.push(concat!(" ", $field_sql, "_updated_at = "));
                $separated.push_bind_unseparated($now_ref.clone());
                $separated.push(concat!(" ", $field_sql, "_updated_by = "));
                $separated.push_bind_unseparated($user_id_ref.clone());
                $fields_updated_flag = true;
            }
        };}

        let mut builder = QueryBuilder::new("UPDATE projects SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        // Apply LWW updates using the macros
        add_lww_uuid_option!(builder, separated, "strategic_goal_id", update_data.strategic_goal_id, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "name", update_data.name, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "objective", update_data.objective, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "outcome", update_data.outcome, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "status_id", update_data.status_id, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "timeline", update_data.timeline, &now, &user_id_str, fields_updated);
        add_lww_option!(builder, separated, "responsible_team", update_data.responsible_team, &now, &user_id_str, fields_updated);
        
        // Sync priority is not an LWW field, update directly if present
        if let Some(priority) = update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority as i64);
            fields_updated = true;
        }

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
            return Err(DomainError::EntityNotFound("Project".to_string(), id));
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
        
        let mut builder = QueryBuilder::new("UPDATE projects SET ");
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
        builder.push(") AND deleted_at IS NULL");
        
        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;
        
        Ok(result.rows_affected())
    }

    async fn set_document_reference(
        &self,
        project_id: Uuid,
        field_name: &str, // e.g., "proposal_document"
        document_id: Uuid,
        auth: &AuthContext,
    ) -> DomainResult<()> {
        let column_name = format!("{}_ref", field_name); 
        
        // Validate the field name
        if !Project::field_metadata().iter().any(|m| m.field_name == field_name && m.is_document_reference_only) {
             return Err(DomainError::Validation(ValidationError::custom(&format!("Invalid document reference field for Project: {}", field_name))));
        }

        let now = Utc::now().to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let document_id_str = document_id.to_string();
        
        let mut builder = sqlx::QueryBuilder::new("UPDATE projects SET ");
        builder.push(&column_name);
        builder.push(" = ");
        builder.push_bind(document_id_str);
        builder.push(", updated_at = ");
        builder.push_bind(now);
        builder.push(", updated_by_user_id = ");
        builder.push_bind(user_id_str);
        builder.push(" WHERE id = ");
        builder.push_bind(project_id.to_string());
        builder.push(" AND deleted_at IS NULL");

        let query = builder.build();
        let result = query.execute(&self.pool).await.map_err(DbError::from)?;

        if result.rows_affected() == 0 {
            Err(DomainError::EntityNotFound("Project".to_string(), project_id))
        } else {
            Ok(())
        }
    }
}
