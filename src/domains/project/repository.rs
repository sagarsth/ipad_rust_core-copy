use crate::auth::AuthContext;
use sqlx::{Executor, Row, Sqlite, Transaction, SqlitePool, Arguments, sqlite::SqliteArguments};
use sqlx::QueryBuilder;
use crate::domains::core::delete_service::DeleteServiceRepository;
use crate::domains::core::repository::{FindById, HardDeletable, SoftDeletable};
use crate::domains::core::document_linking::DocumentLinkable;
use crate::domains::project::types::{NewProject, Project, ProjectRow, UpdateProject, ProjectStatistics, ProjectStatusBreakdown, ProjectMetadataCounts, ProjectDocumentReference};
use crate::domains::sync::repository::ChangeLogRepository;
use crate::domains::sync::types::{ChangeLogEntry, ChangeOperationType, MergeOutcome};
use crate::errors::{DbError, DomainError, DomainResult, ValidationError};
use crate::types::{PaginatedResult, PaginationParams};
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;
use std::str::FromStr;
use async_trait::async_trait;
use chrono::{Utc, DateTime};
use sqlx::{query, query_as, query_scalar};
use uuid::Uuid;
use std::collections::HashMap;
use std::sync::Arc;
use serde_json;
use crate::domains::user::repository::MergeableEntityRepository;

/// Trait defining project repository operations
#[async_trait]
pub trait ProjectRepository: DeleteServiceRepository<Project> + MergeableEntityRepository<Project> + Send + Sync {
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

    /// Find projects by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>>;

    async fn update_sync_priority(
        &self,
        ids: &[Uuid],
        priority: SyncPriorityFromSyncDomain,
        auth: &AuthContext,
    ) -> DomainResult<u64>;

    /// Count projects by status
    async fn count_by_status(&self) -> DomainResult<Vec<(Option<i64>, i64)>>;
    
    /// Count projects by strategic goal
    async fn count_by_strategic_goal(&self) -> DomainResult<Vec<(Option<Uuid>, i64)>>;
    
    /// Count projects by responsible team
    async fn count_by_responsible_team(&self) -> DomainResult<Vec<(Option<String>, i64)>>;
    
    /// Get comprehensive project statistics
    async fn get_project_statistics(&self) -> DomainResult<ProjectStatistics>;
    
    /// Find projects by status
    async fn find_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>>;
    
    /// Find projects by responsible team
    async fn find_by_responsible_team(
        &self,
        team: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>>;
    
    /// Get project document references
    async fn get_project_document_references(
        &self,
        project_id: Uuid,
    ) -> DomainResult<Vec<ProjectDocumentReference>>;
    
    /// Search projects by name or objective
    async fn search_projects(
        &self,
        query: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>>;
    
    /// Get project status breakdown
    async fn get_project_status_breakdown(&self) -> DomainResult<Vec<ProjectStatusBreakdown>>;
    
    /// Get project metadata counts
    async fn get_project_metadata_counts(&self) -> DomainResult<ProjectMetadataCounts>;

    /// Find projects within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>>;

    /// ADDED: Find project IDs that match complex filter criteria.
    /// This is the key to enabling efficient UI-driven bulk selections and exports.
    /// Follows the same pattern as StrategicGoalRepository::find_ids_by_filter.
    async fn find_ids_by_filter(
        &self,
        filter: crate::domains::project::types::ProjectFilter,
    ) -> DomainResult<Vec<Uuid>>;
}

/// SQLite implementation for ProjectRepository
#[derive(Clone)]
pub struct SqliteProjectRepository {
    pool: SqlitePool,
    change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>,
}

impl SqliteProjectRepository {
    pub fn new(pool: SqlitePool, change_log_repo: Arc<dyn ChangeLogRepository + Send + Sync>) -> Self {
        Self { pool, change_log_repo }
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

    /// Static table name reference to avoid trait method ambiguity
    pub const ENTITY_TABLE: &'static str = "projects";
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
        let deleted_by_device_id = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        
        let result = query(
            "UPDATE projects SET deleted_at = ?, deleted_by_user_id = ?, deleted_by_device_id = ? WHERE id = ? AND deleted_at IS NULL"
        )
        .bind(now)
        .bind(deleted_by)
        .bind(deleted_by_device_id)
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
            Ok(project) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(project)
            }
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }

    async fn create_with_tx<'t>(
        &self,
        new_project: &NewProject,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Project> {
        let id = Uuid::new_v4();
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = auth.user_id;
        let user_id_str = user_id.to_string();
        let device_uuid: Option<Uuid> = auth.device_id.parse::<Uuid>().ok();
        let device_id_str = device_uuid.map(|u| u.to_string());
        let strategic_goal_id_str = new_project.strategic_goal_id.map(|id| id.to_string());
        let created_by_id_str = new_project.created_by_user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| user_id_str.clone());

        let mut builder = QueryBuilder::new(
            r#"INSERT INTO projects (
                id, strategic_goal_id, 
                name, name_updated_at, name_updated_by, name_updated_by_device_id,
                objective, objective_updated_at, objective_updated_by, objective_updated_by_device_id,
                outcome, outcome_updated_at, outcome_updated_by, outcome_updated_by_device_id,
                status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id,
                timeline, timeline_updated_at, timeline_updated_by, timeline_updated_by_device_id,
                responsible_team, responsible_team_updated_at, responsible_team_updated_by, responsible_team_updated_by_device_id,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) "#
        );

        builder.push_values([ (
            id.to_string(), strategic_goal_id_str,
            new_project.name.clone(), now_str.clone(), user_id_str.clone(), device_id_str.clone(),
            new_project.objective.clone(), new_project.objective.as_ref().map(|_| &now_str), new_project.objective.as_ref().map(|_| &user_id_str), new_project.objective.as_ref().map(|_| &device_id_str),
            new_project.outcome.clone(), new_project.outcome.as_ref().map(|_| &now_str), new_project.outcome.as_ref().map(|_| &user_id_str), new_project.outcome.as_ref().map(|_| &device_id_str),
            new_project.status_id.clone(), new_project.status_id.as_ref().map(|_| &now_str), new_project.status_id.as_ref().map(|_| &user_id_str), new_project.status_id.as_ref().map(|_| &device_id_str),
            new_project.timeline.clone(), new_project.timeline.as_ref().map(|_| &now_str), new_project.timeline.as_ref().map(|_| &user_id_str), new_project.timeline.as_ref().map(|_| &device_id_str),
            new_project.responsible_team.clone(), new_project.responsible_team.as_ref().map(|_| &now_str), new_project.responsible_team.as_ref().map(|_| &user_id_str), new_project.responsible_team.as_ref().map(|_| &device_id_str),
            new_project.sync_priority.as_str(),
            now_str.clone(), now_str.clone(), created_by_id_str, user_id_str.clone(),
            device_id_str.clone(), device_id_str.clone(), // created_by_device_id, updated_by_device_id
            Option::<String>::None, Option::<String>::None, Option::<String>::None // deleted_at, deleted_by_user_id, deleted_by_device_id
        )], |mut b, values| {
             b.push_bind(values.0); b.push_bind(values.1);
             b.push_bind(values.2); b.push_bind(values.3); b.push_bind(values.4); b.push_bind(values.5.clone());
             b.push_bind(values.6); b.push_bind(values.7); b.push_bind(values.8); b.push_bind(values.9.clone());
             b.push_bind(values.10); b.push_bind(values.11); b.push_bind(values.12); b.push_bind(values.13.clone());
             b.push_bind(values.14); b.push_bind(values.15); b.push_bind(values.16); b.push_bind(values.17.clone());
             b.push_bind(values.18); b.push_bind(values.19); b.push_bind(values.20); b.push_bind(values.21.clone());
             b.push_bind(values.22); b.push_bind(values.23); b.push_bind(values.24); b.push_bind(values.25.clone());
             b.push_bind(values.26);
             b.push_bind(values.27); b.push_bind(values.28); b.push_bind(values.29); b.push_bind(values.30);
             b.push_bind(values.31); b.push_bind(values.32); // created_by_device_id, updated_by_device_id
             b.push_bind(values.33); b.push_bind(values.34); b.push_bind(values.35); // deleted_at, deleted_by_user_id, deleted_by_device_id
        });

        let query = builder.build();
        query.execute(&mut **tx).await.map_err(DbError::from)?;

        // Log Create Operation
        let entry = ChangeLogEntry {
            operation_id: Uuid::new_v4(),
            entity_table: Self::ENTITY_TABLE.to_string(),
            entity_id: id,
            operation_type: ChangeOperationType::Create,
            field_name: None,
            old_value: None,
            new_value: None, 
            timestamp: now,
            user_id: user_id,
            device_id: device_uuid,
            document_metadata: None,
            sync_batch_id: None,
            processed_at: None,
            sync_error: None,
        };
        self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;

        self.find_by_id_with_tx(id, tx).await
    }

    async fn update(
        &self,
        id: Uuid,
        update_data: &UpdateProject,
        auth: &AuthContext,
    ) -> DomainResult<Project> {
        // **OPTIMIZATION: Pre-validate foreign key constraint BEFORE starting transaction**
        // This completely eliminates strategic_goals table locks during the main transaction
        if let Some(opt_sg_id) = &update_data.strategic_goal_id {
            if let Some(sg_id) = opt_sg_id {
                // Quick validation outside of main transaction
                let exists: bool = query_scalar("SELECT EXISTS(SELECT 1 FROM strategic_goals WHERE id = ? AND deleted_at IS NULL)")
                    .bind(sg_id.to_string())
                    .fetch_one(&self.pool)
                    .await
                    .map_err(DbError::from)?;
                    
                if !exists {
                    return Err(DomainError::Validation(crate::errors::ValidationError::custom(
                        &format!("Strategic goal with id {} not found", sg_id)
                    )));
                }
            }
        }

        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        let result = self.update_with_tx(id, update_data, auth, &mut tx).await;
        match result {
            Ok(project) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(project)
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
        update_data: &UpdateProject,
        auth: &AuthContext,
        tx: &mut Transaction<'t, Sqlite>,
    ) -> DomainResult<Project> {
        let old_entity = self.find_by_id_with_tx(id, tx).await?;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id = update_data.updated_by_user_id;
        let user_id_str = user_id.to_string();
        let id_str = id.to_string();
        let device_id_str = auth.device_id.parse::<Uuid>().ok().map(|u| u.to_string());
        let device_uuid = auth.device_id.parse::<Uuid>().ok();

        let mut builder = QueryBuilder::new("UPDATE projects SET ");
        let mut separated = builder.separated(", ");
        let mut fields_updated = false;

        // Macro for Last Write Wins fields
        macro_rules! add_lww {
            ($field:ident, $field_sql:literal, $value:expr) => {
                if let Some(val) = $value {
                    separated.push(concat!($field_sql, " = "));
                    separated.push_bind_unseparated(val.clone());
                    separated.push(concat!($field_sql, "_updated_at = "));
                    separated.push_bind_unseparated(now_str.clone());
                    separated.push(concat!($field_sql, "_updated_by = "));
                    separated.push_bind_unseparated(user_id_str.clone());
                    separated.push(concat!($field_sql, "_updated_by_device_id = "));
                    separated.push_bind_unseparated(device_id_str.clone());
                    fields_updated = true;
                }
            };
        }

        // Special handling for strategic_goal_id (Option<Option<Uuid>> in DTO)
        if let Some(opt_sg_id) = update_data.strategic_goal_id {
            let sg_id_str = opt_sg_id.map(|id| id.to_string());
            println!("🔧 [REPOSITORY_UPDATE] Strategic goal update detected:");
            println!("   • Raw value from DTO: {:?}", update_data.strategic_goal_id);
            println!("   • Extracted opt_sg_id: {:?}", opt_sg_id);
            println!("   • String representation: {:?}", sg_id_str);
            separated.push("strategic_goal_id = ");
            separated.push_bind_unseparated(sg_id_str); // Bind Option<String>
            // Note: We don't track LWW for foreign keys directly here, assumed handled if FK constraint changes
            fields_updated = true;
        }

        add_lww!(name, "name", &update_data.name.as_ref());
        add_lww!(objective, "objective", &update_data.objective.as_ref());
        add_lww!(outcome, "outcome", &update_data.outcome.as_ref());
        add_lww!(status_id, "status_id", &update_data.status_id.as_ref());
        add_lww!(timeline, "timeline", &update_data.timeline.as_ref());
        add_lww!(responsible_team, "responsible_team", &update_data.responsible_team.as_ref());

        if let Some(priority) = update_data.sync_priority {
            separated.push("sync_priority = ");
            separated.push_bind_unseparated(priority.as_str());
            fields_updated = true;
        }

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
        
        // Debug: Print the SQL query (Note: This won't show bound parameters, but shows structure)
        println!("🔧 [REPOSITORY_UPDATE] Executing SQL query for project {}", id);
        
        let result = query.execute(&mut **tx).await.map_err(DbError::from)?;
        println!("🔧 [REPOSITORY_UPDATE] Query executed. Rows affected: {}", result.rows_affected());
        
        if result.rows_affected() == 0 {
            return Err(DomainError::EntityNotFound(Self::ENTITY_TABLE.to_string(), id));
        }

        let new_entity = self.find_by_id_with_tx(id, tx).await?;
        println!("🔧 [REPOSITORY_UPDATE] Entity comparison:");
        println!("   • BEFORE: strategic_goal_id = {:?}", old_entity.strategic_goal_id);
        println!("   • AFTER:  strategic_goal_id = {:?}", new_entity.strategic_goal_id);
        println!("   • BEFORE: updated_at = {}", old_entity.updated_at);
        println!("   • AFTER:  updated_at = {}", new_entity.updated_at);

        // **OPTIMIZATION: Batch change log entries to reduce transaction overhead**
        let mut change_entries = Vec::new();

        // Helper macro to collect change log entries
        macro_rules! collect_change {
            ($old_field:expr, $new_field:expr, $field_name:literal) => {
                if $old_field != $new_field {
                    change_entries.push(ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: Self::ENTITY_TABLE.to_string(),
                        entity_id: id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some($field_name.to_string()),
                        old_value: serde_json::to_string(&$old_field).ok(),
                        new_value: serde_json::to_string(&$new_field).ok(),
                        timestamp: now,
                        user_id: user_id,
                        device_id: device_uuid.clone(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    });
                }
            };
        }

        // Collect all changes
        collect_change!(old_entity.name, new_entity.name, "name");
        collect_change!(old_entity.objective, new_entity.objective, "objective");
        collect_change!(old_entity.outcome, new_entity.outcome, "outcome");
        collect_change!(old_entity.status_id, new_entity.status_id, "status_id");
        collect_change!(old_entity.timeline, new_entity.timeline, "timeline");
        collect_change!(old_entity.responsible_team, new_entity.responsible_team, "responsible_team");
        collect_change!(old_entity.strategic_goal_id, new_entity.strategic_goal_id, "strategic_goal_id");
        
        // Special handling for sync_priority
        if old_entity.sync_priority != new_entity.sync_priority {
            change_entries.push(ChangeLogEntry {
                operation_id: Uuid::new_v4(),
                entity_table: Self::ENTITY_TABLE.to_string(),
                entity_id: id,
                operation_type: ChangeOperationType::Update,
                field_name: Some("sync_priority".to_string()),
                old_value: serde_json::to_string(old_entity.sync_priority.as_str()).ok(),
                new_value: serde_json::to_string(new_entity.sync_priority.as_str()).ok(),
                timestamp: now,
                user_id: user_id,
                device_id: device_uuid.clone(),
                document_metadata: None,
                sync_batch_id: None,
                processed_at: None,
                sync_error: None,
            });
        }

        // **OPTIMIZATION: Batch insert all change log entries**
        // This reduces the number of individual database operations within the transaction
        if !change_entries.is_empty() {
            for entry in change_entries {
                self.change_log_repo.create_change_log_with_tx(&entry, tx).await?;
            }
        }
        
        Ok(new_entity)
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
        priority: SyncPriorityFromSyncDomain,
        auth: &AuthContext,
    ) -> DomainResult<u64> {
        if ids.is_empty() { return Ok(0); }
        
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;

        // Fetch old priorities
        let id_strings: Vec<String> = ids.iter().map(Uuid::to_string).collect();
        let select_query = format!(
            "SELECT id, sync_priority FROM projects WHERE id IN ({})",
            vec!["?"; ids.len()].join(", ")
        );
        // Fetch as String
        let mut select_builder = query_as::<_, (String, String)>(&select_query);
        for id_str in &id_strings {
            select_builder = select_builder.bind(id_str);
        }
        let old_priorities: HashMap<Uuid, SyncPriorityFromSyncDomain> = select_builder
            .fetch_all(&mut *tx)
            .await.map_err(DbError::from)?
            .into_iter()
            .filter_map(|(id_str, prio_text)| {
                match Uuid::parse_str(&id_str) {
                    Ok(id) => Some((id, SyncPriorityFromSyncDomain::from_str(&prio_text).unwrap_or_default())),
                    Err(_) => None,
                }
            }).collect();

        // Perform Update
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let user_id_str = auth.user_id.to_string();
        let priority_str = priority.as_str(); // Now correctly uses the method from SyncPriorityFromSyncDomain

        let mut update_builder = QueryBuilder::new("UPDATE projects SET ");
        update_builder.push("sync_priority = "); update_builder.push_bind(priority_str); // Bind TEXT
        update_builder.push(", updated_at = "); update_builder.push_bind(now_str.clone());
        update_builder.push(", updated_by_user_id = "); update_builder.push_bind(user_id_str.clone());
        update_builder.push(" WHERE id IN (");
        let mut id_separated = update_builder.separated(",");
        for id in ids { id_separated.push_bind(id.to_string()); }
        update_builder.push(") AND deleted_at IS NULL");

        let query = update_builder.build();
        let result = query.execute(&mut *tx).await.map_err(DbError::from)?;
        let rows_affected = result.rows_affected();

        // Log changes
        for id in ids {
            if let Some(old_priority) = old_priorities.get(id) {
                if *old_priority != priority {
                    let entry = ChangeLogEntry {
                        operation_id: Uuid::new_v4(),
                        entity_table: Self::ENTITY_TABLE.to_string(),
                        entity_id: *id,
                        operation_type: ChangeOperationType::Update,
                        field_name: Some("sync_priority".to_string()),
                        old_value: serde_json::to_string(old_priority.as_str()).ok(), // Log as TEXT
                        new_value: serde_json::to_string(priority_str).ok(), // Log as TEXT
                        timestamp: now,
                        user_id: auth.user_id,
                        device_id: auth.device_id.parse::<Uuid>().ok(),
                        document_metadata: None,
                        sync_batch_id: None,
                        processed_at: None,
                        sync_error: None,
                    };
                    self.change_log_repo.create_change_log_with_tx(&entry, &mut tx).await?;
                }
            }
        }

        tx.commit().await.map_err(DbError::from)?;
        Ok(rows_affected)
    }

    async fn count_by_status(&self) -> DomainResult<Vec<(Option<i64>, i64)>> {
        let counts = query_as::<_, (Option<i64>, i64)>(
            "SELECT status_id, COUNT(*) 
             FROM projects 
             WHERE deleted_at IS NULL 
             GROUP BY status_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn count_by_strategic_goal(&self) -> DomainResult<Vec<(Option<Uuid>, i64)>> {
        let rows = query(
            "SELECT strategic_goal_id, COUNT(*) as count
             FROM projects 
             WHERE deleted_at IS NULL 
             GROUP BY strategic_goal_id"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Manual mapping to handle Option<Uuid>
        let mut results = Vec::new();
        for row in rows {
            let sg_id_str: Option<String> = row.get("strategic_goal_id");
            let count: i64 = row.get("count");
            
            let sg_id = match sg_id_str {
                Some(id_str) => Some(Uuid::parse_str(&id_str).map_err(|_| 
                    DomainError::Internal(format!("Invalid UUID in strategic_goal_id: {}", id_str))
                )?),
                None => None,
            };
            
            results.push((sg_id, count));
        }

        Ok(results)
    }
    
    async fn count_by_responsible_team(&self) -> DomainResult<Vec<(Option<String>, i64)>> {
        let counts = query_as::<_, (Option<String>, i64)>(
            "SELECT responsible_team, COUNT(*) 
             FROM projects 
             WHERE deleted_at IS NULL 
             GROUP BY responsible_team"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(DbError::from)?;

        Ok(counts)
    }
    
    async fn get_project_statistics(&self) -> DomainResult<ProjectStatistics> {
        // Get total project count
        let total_projects: i64 = query_scalar(
            "SELECT COUNT(*) FROM projects WHERE deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Get document count
        let document_count: i64 = query_scalar(
            "SELECT COUNT(*) 
             FROM media_documents 
             WHERE related_table = 'projects' -- Corrected entity_type to related_table
             AND deleted_at IS NULL"
        )
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;
        
        // Get status distribution
        let status_counts = self.count_by_status().await?;
        let mut by_status = HashMap::new();
        for (status_id_opt, count) in status_counts {
            let status_name = match status_id_opt {
                Some(1) => "On Track".to_string(),
                Some(2) => "At Risk".to_string(),
                Some(3) => "Delayed".to_string(),
                Some(4) => "Completed".to_string(),
                Some(id) => format!("Status {}", id),
                None => "Unspecified".to_string(),
            };
            by_status.insert(status_name, count);
        }
        
        // Get strategic goal distribution
        let sg_counts = self.count_by_strategic_goal().await?;
        let mut by_strategic_goal = HashMap::new();
        for (sg_id_opt, count) in sg_counts {
            let goal_name = match sg_id_opt {
                Some(id) => {
                    match query_scalar::<_, String>(
                        "SELECT objective_code FROM strategic_goals WHERE id = ? AND deleted_at IS NULL"
                    )
                    .bind(id.to_string())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(DbError::from)? {
                        Some(code) => code,
                        None => format!("Goal {}", id), // Fallback if goal not found
                    }
                },
                None => "No Goal".to_string(),
            };
            by_strategic_goal.insert(goal_name, count);
        }
        
        // Get team distribution
        let team_counts = self.count_by_responsible_team().await?;
        let mut by_responsible_team = HashMap::new();
        for (team_opt, count) in team_counts {
            let team_name = team_opt.unwrap_or_else(|| "Unassigned".to_string());
            by_responsible_team.insert(team_name, count);
        }
        
        Ok(ProjectStatistics {
            total_projects,
            by_status,
            by_strategic_goal,
            by_responsible_team,
            document_count,
        })
    }
    
    async fn find_by_status(
        &self,
        status_id: i64,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM projects 
             WHERE status_id = ? AND deleted_at IS NULL"
        )
        .bind(status_id)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectRow>(
            "SELECT * FROM projects 
             WHERE status_id = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(status_id)
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
    
    async fn find_by_responsible_team(
        &self,
        team: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM projects 
             WHERE responsible_team = ? AND deleted_at IS NULL"
        )
        .bind(team)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectRow>(
            "SELECT * FROM projects 
             WHERE responsible_team = ? AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(team)
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
    
    async fn get_project_document_references(
        &self,
        project_id: Uuid,
    ) -> DomainResult<Vec<ProjectDocumentReference>> {
        let project_id_str = project_id.to_string();
        
        let mut references = Vec::new();
        let doc_ref_fields: Vec<_> = Project::field_metadata()
            .into_iter()
            .filter(|field| field.is_document_reference_only)
            .collect();
            
        for field in doc_ref_fields {
            let column_name = format!("{}_ref", field.field_name);
            
            // Query updated to fetch document details directly
            let query_str = format!(
                "SELECT p.{} as doc_id, m.original_filename, m.created_at, m.size_bytes as file_size 
                 FROM projects p 
                 LEFT JOIN media_documents m ON p.{} = m.id AND m.deleted_at IS NULL
                 WHERE p.id = ? AND p.deleted_at IS NULL", 
                column_name, column_name
            );
            
            let row = query(&query_str)
                .bind(&project_id_str)
                .fetch_optional(&self.pool)
                .await
                .map_err(DbError::from)?;
                
            if let Some(row) = row {
                let doc_id_str: Option<String> = row.get("doc_id");
                let doc_id = doc_id_str.map(|id_str| 
                    Uuid::parse_str(&id_str)
                        .map_err(|_| DomainError::Internal(format!("Invalid UUID: {}", id_str)))
                ).transpose()?;
                
                let (filename, upload_date, file_size) = if doc_id.is_some() {
                    (
                        row.get("original_filename"),
                        row.get::<Option<String>, _>("created_at").map(|dt_str| 
                            DateTime::parse_from_rfc3339(&dt_str)
                                .map_err(|_| DomainError::Internal(format!("Invalid datetime: {}", dt_str)))
                                .map(|dt| dt.with_timezone(&Utc))
                        ).transpose()?, // Parse and convert Option<String> to Option<DateTime<Utc>>
                        row.get::<Option<i64>, _>("file_size").map(|fs| fs as u64), // Corrected type: file_size from i64 to u64
                    )
                } else {
                    (None, None, None)
                };
                
                references.push(ProjectDocumentReference {
                    field_name: field.field_name.to_string(),
                    display_name: field.display_name.to_string(),
                    document_id: doc_id,
                    filename,
                    upload_date,
                    file_size,
                });
            }
        }
        
        Ok(references)
    }
    
    async fn search_projects(
        &self,
        query: &str,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>> {
        let offset = (params.page - 1) * params.per_page;
        let search_term = format!("%{}%", query);

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM projects 
             WHERE (name LIKE ? OR objective LIKE ? OR outcome LIKE ? OR responsible_team LIKE ?) 
             AND deleted_at IS NULL"
        )
        .bind(&search_term)
        .bind(&search_term)
        .bind(&search_term)
        .bind(&search_term)
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectRow>(
            "SELECT * FROM projects 
             WHERE (name LIKE ? OR objective LIKE ? OR outcome LIKE ? OR responsible_team LIKE ?) 
             AND deleted_at IS NULL 
             ORDER BY name ASC LIMIT ? OFFSET ?"
        )
        .bind(&search_term)
        .bind(&search_term)
        .bind(&search_term)
        .bind(&search_term)
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
    
    async fn get_project_status_breakdown(&self) -> DomainResult<Vec<ProjectStatusBreakdown>> {
        // Get status counts
        let status_counts = self.count_by_status().await?;
        
        // Get total count for percentage calculation
        let total: i64 = status_counts.iter().map(|(_, count)| count).sum();
        
        // Create breakdown objects
        let mut breakdown = Vec::new();
        for (status_id_opt, count) in status_counts {
            let status_id = status_id_opt.unwrap_or(0); // Treat None as 0 or another ID
            let status_name = match status_id {
                1 => "On Track".to_string(),
                2 => "At Risk".to_string(),
                3 => "Delayed".to_string(),
                4 => "Completed".to_string(),
                _ => "Unknown".to_string(), // Handle 0 or unexpected IDs
            };
            
            let percentage = if total > 0 {
                (count as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            
            breakdown.push(ProjectStatusBreakdown {
                status_id,
                status_name,
                count,
                percentage,
            });
        }
        
        // Sort by status ID (consistent order)
        breakdown.sort_by_key(|b| b.status_id);
        
        Ok(breakdown)
    }
    
    async fn get_project_metadata_counts(&self) -> DomainResult<ProjectMetadataCounts> {
        // Get team counts
        let team_counts = self.count_by_responsible_team().await?;
        let mut projects_by_team = HashMap::new();
        for (team_opt, count) in team_counts {
            let team_name = team_opt.unwrap_or_else(|| "Unassigned".to_string());
            projects_by_team.insert(team_name, count);
        }
        
        // Get status counts
        let status_counts = self.count_by_status().await?;
        let mut projects_by_status = HashMap::new();
        for (status_id_opt, count) in status_counts {
            let status_name = match status_id_opt {
                Some(1) => "On Track".to_string(),
                Some(2) => "At Risk".to_string(),
                Some(3) => "Delayed".to_string(),
                Some(4) => "Completed".to_string(),
                Some(id) => format!("Status {}", id),
                None => "Unspecified".to_string(),
            };
            projects_by_status.insert(status_name, count);
        }
        
        // Get goal counts
        let goal_counts = self.count_by_strategic_goal().await?;
        let mut projects_by_goal = HashMap::new();
        for (goal_id_opt, count) in goal_counts {
            let goal_name = match goal_id_opt {
                Some(id) => {
                    match query_scalar::<_, String>(
                        "SELECT objective_code FROM strategic_goals WHERE id = ? AND deleted_at IS NULL"
                    )
                    .bind(id.to_string())
                    .fetch_optional(&self.pool)
                    .await
                    .map_err(DbError::from)? {
                        Some(code) => code,
                        None => format!("Goal {}", id),
                    }
                },
                None => "No Goal".to_string(),
            };
            projects_by_goal.insert(goal_name, count);
        }
        
        Ok(ProjectMetadataCounts {
            projects_by_team,
            projects_by_status,
            projects_by_goal,
        })
    }

    /// Find projects within a date range (created_at or updated_at)
    async fn find_by_date_range(
        &self,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>> {
        let offset = (params.page - 1) * params.per_page;

        // Get total count
        let total: i64 = query_scalar(
            "SELECT COUNT(*) FROM projects 
             WHERE updated_at >= ? AND updated_at <= ? 
             AND deleted_at IS NULL"
        )
        .bind(start_date.to_rfc3339())
        .bind(end_date.to_rfc3339())
        .fetch_one(&self.pool)
        .await
        .map_err(DbError::from)?;

        // Fetch paginated rows
        let rows = query_as::<_, ProjectRow>(
            "SELECT * FROM projects 
             WHERE updated_at >= ? AND updated_at <= ? 
             AND deleted_at IS NULL 
             ORDER BY updated_at ASC LIMIT ? OFFSET ?"
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
            .collect::<DomainResult<Vec<Project>>>()?;

        Ok(PaginatedResult::new(
            entities,
            total as u64,
            params,
        ))
    }

    /// ADDED: Find project IDs matching a complex set of filter criteria.
    /// Follows the same pattern as StrategicGoalRepository::find_ids_by_filter.
    async fn find_ids_by_filter(
        &self,
        filter: crate::domains::project::types::ProjectFilter,
    ) -> DomainResult<Vec<Uuid>> {
        use sqlx::QueryBuilder;
        
        let mut query_builder = QueryBuilder::new("SELECT p.id FROM projects p WHERE 1=1");
        
        if filter.exclude_deleted.unwrap_or(true) {
            query_builder.push(" AND p.deleted_at IS NULL");
        }
        
        if let Some(status_ids) = &filter.status_ids {
            if !status_ids.is_empty() {
                query_builder.push(" AND p.status_id IN (");
                let mut separated = query_builder.separated(", ");
                for status_id in status_ids {
                    separated.push_bind(status_id);
                }
                separated.push_unseparated(")");
            }
        }
        
        if let Some(strategic_goal_ids) = &filter.strategic_goal_ids {
            if !strategic_goal_ids.is_empty() {
                query_builder.push(" AND p.strategic_goal_id IN (");
                let mut separated = query_builder.separated(", ");
                for sg_id in strategic_goal_ids {
                    separated.push_bind(sg_id.to_string());
                }
                separated.push_unseparated(")");
            }
        }
        
        if let Some(teams) = &filter.responsible_teams {
            if !teams.is_empty() {
                query_builder.push(" AND p.responsible_team IN (");
                let mut separated = query_builder.separated(", ");
                for team in teams {
                    separated.push_bind(team);
                }
                separated.push_unseparated(")");
            }
        }
        
        if let Some(search_text) = &filter.search_text {
            if !search_text.trim().is_empty() {
                let search_pattern = format!("%{}%", search_text.trim());
                query_builder.push(" AND (p.name LIKE ")
                    .push_bind(search_pattern.clone())
                    .push(" OR p.objective LIKE ")
                    .push_bind(search_pattern.clone())
                    .push(" OR p.outcome LIKE ")
                    .push_bind(search_pattern)
                    .push(")");
            }
        }
        
        if let Some((start_date, end_date)) = &filter.date_range {
            if let (Ok(start), Ok(end)) = (
                DateTime::parse_from_rfc3339(start_date),
                DateTime::parse_from_rfc3339(end_date)
            ) {
                let start_utc = start.with_timezone(&Utc);
                let end_utc = end.with_timezone(&Utc);
                
                query_builder.push(" AND p.updated_at BETWEEN ")
                    .push_bind(start_utc.to_rfc3339())
                    .push(" AND ")
                    .push_bind(end_utc.to_rfc3339());
            }
        }
        
        let query = query_builder.build_query_as::<(String,)>();
        let rows = query.fetch_all(&self.pool).await.map_err(DbError::from)?;
        
        rows.into_iter()
            .map(|(id_str,)| Uuid::parse_str(&id_str).map_err(|e| DomainError::InvalidUuid(e.to_string())))
            .collect()
    }

    /// Find projects by specific IDs
    async fn find_by_ids(
        &self,
        ids: &[Uuid],
        params: PaginationParams,
    ) -> DomainResult<PaginatedResult<Project>> {
        if ids.is_empty() {
            return Ok(PaginatedResult::new(Vec::new(), 0, params));
        }

        let offset = (params.page - 1) * params.per_page;

        // Build COUNT query with dynamic placeholders
        let count_placeholders = vec!["?"; ids.len()].join(", ");
        let count_query = format!(
            "SELECT COUNT(*) FROM projects WHERE id IN ({}) AND deleted_at IS NULL",
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
            "SELECT * FROM projects WHERE id IN ({}) AND deleted_at IS NULL ORDER BY name ASC LIMIT ? OFFSET ?",
            select_placeholders
        );

        let mut select_builder = QueryBuilder::new(&select_query);
        for id in ids {
            select_builder.push_bind(id.to_string());
        }
        select_builder.push_bind(params.per_page as i64);
        select_builder.push_bind(offset as i64);

        let rows = select_builder
            .build_query_as::<ProjectRow>()
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
}

// === Sync Merge Implementation for Project ===
#[async_trait]
impl MergeableEntityRepository<Project> for SqliteProjectRepository {
    fn entity_name(&self) -> &'static str { "projects" }
    async fn merge_remote_change<'t>(
        &self,
        tx: &mut Transaction<'t, Sqlite>,
        remote_change: &ChangeLogEntry,
    ) -> DomainResult<MergeOutcome> {
        // Ensure the change is for the correct table
        if remote_change.entity_table != Self::ENTITY_TABLE {
            return Err(DomainError::Internal(format!(
                "ProjectRepository received change for incorrect table: {}",
                remote_change.entity_table
            )));
        }

        let remote_device_id_str = remote_change.device_id.map(|id| id.to_string());

        match remote_change.operation_type {
            ChangeOperationType::Create | ChangeOperationType::Update => {
                let state_json = remote_change.new_value.as_ref().ok_or_else(|| DomainError::Validation(ValidationError::custom("Missing new_value for project change")))?;
                let remote_state: Project = serde_json::from_str(state_json)
                    .map_err(|e| DomainError::Validation(ValidationError::format("new_value_project", &format!("Invalid JSON: {}", e))))?;
                let local_opt = match self.find_by_id_with_tx(remote_state.id, tx).await {
                    Ok(ent) => Some(ent),
                    Err(DomainError::EntityNotFound(_, _)) => None,
                    Err(e) => return Err(e),
                };
                if let Some(local) = local_opt.clone() {
                    if remote_state.updated_at <= local.updated_at {
                        return Ok(MergeOutcome::NoOp("Local copy newer or equal".into()));
                    }
                    self.upsert_remote_state_with_tx(tx, &remote_state, remote_device_id_str.clone()).await?;
                    Ok(MergeOutcome::Updated(remote_state.id))
                } else {
                    self.upsert_remote_state_with_tx(tx, &remote_state, remote_device_id_str.clone()).await?;
                    Ok(MergeOutcome::Created(remote_state.id))
                }
            }
            ChangeOperationType::Delete => Ok(MergeOutcome::NoOp("Remote soft delete ignored".into())),
            ChangeOperationType::HardDelete => Ok(MergeOutcome::HardDeleted(remote_change.entity_id)),
        }
    }
}

impl SqliteProjectRepository {
    /// Upsert remote Project state within a transaction
    async fn upsert_remote_state_with_tx<'t>(&self, tx: &mut Transaction<'t, Sqlite>, remote: &Project, remote_device_id_str: Option<String>) -> DomainResult<()> {
        sqlx::query(
            r#"INSERT OR REPLACE INTO projects (
                id, strategic_goal_id,
                name, name_updated_at, name_updated_by, name_updated_by_device_id,
                objective, objective_updated_at, objective_updated_by, objective_updated_by_device_id,
                outcome, outcome_updated_at, outcome_updated_by, outcome_updated_by_device_id,
                status_id, status_id_updated_at, status_id_updated_by, status_id_updated_by_device_id,
                timeline, timeline_updated_at, timeline_updated_by, timeline_updated_by_device_id,
                responsible_team, responsible_team_updated_at, responsible_team_updated_by, responsible_team_updated_by_device_id,
                sync_priority,
                created_at, updated_at, created_by_user_id, updated_by_user_id,
                created_by_device_id, updated_by_device_id,
                deleted_at, deleted_by_user_id, deleted_by_device_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(remote.id.to_string())
        .bind(remote.strategic_goal_id.map(|u| u.to_string()))
        .bind(remote.name.clone())
        .bind(remote.name_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.name_updated_by.map(|u| u.to_string()))
        .bind(remote.name_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.objective.clone())
        .bind(remote.objective_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.objective_updated_by.map(|u| u.to_string()))
        .bind(remote.objective_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.outcome.clone())
        .bind(remote.outcome_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.outcome_updated_by.map(|u| u.to_string()))
        .bind(remote.outcome_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.status_id.clone())
        .bind(remote.status_id_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.status_id_updated_by.map(|u| u.to_string()))
        .bind(remote.status_id_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.timeline.clone())
        .bind(remote.timeline_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.timeline_updated_by.map(|u| u.to_string()))
        .bind(remote.timeline_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.responsible_team.clone())
        .bind(remote.responsible_team_updated_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.responsible_team_updated_by.map(|u| u.to_string()))
        .bind(remote.responsible_team_updated_by_device_id.map(|u| u.to_string()))
        .bind(remote.sync_priority.as_str())
        .bind(remote.created_at.to_rfc3339())
        .bind(remote.updated_at.to_rfc3339())
        .bind(remote.created_by_user_id.map(|u| u.to_string()))
        .bind(remote.updated_by_user_id.map(|u| u.to_string()))
        .bind(remote.created_by_device_id.map(|u| u.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.updated_by_device_id.map(|u| u.to_string()).or(remote_device_id_str.clone()))
        .bind(remote.deleted_at.map(|dt| dt.to_rfc3339()))
        .bind(remote.deleted_by_user_id.map(|u| u.to_string()))
        .bind(remote.deleted_by_device_id.map(|u| u.to_string()).or(remote_device_id_str))
        .execute(&mut **tx)
        .await
        .map_err(DbError::from)?;
        Ok(())
    }
}
