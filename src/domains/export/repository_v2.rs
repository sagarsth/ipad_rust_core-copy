use crate::domains::export::types::*;
use crate::errors::{ServiceError, ServiceResult};
use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use sqlx::{SqlitePool, Row, QueryBuilder, Execute};
use std::pin::Pin;
use uuid::Uuid;
use tokio_stream::wrappers::ReceiverStream;
use tokio::sync::mpsc;
use serde_json;

/// Modern repository with streaming support
#[async_trait]
pub trait StreamingExportRepository: Send + Sync {
    /// Stream entities using cursor-based pagination
    async fn stream_by_cursor<T>(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
    ) -> ServiceResult<Vec<T>>
    where
        T: ExportEntity;
    
    /// Create an async stream of entities
    fn create_stream<T>(
        &self,
        filter: EntityFilter,
        batch_size: usize,
    ) -> Pin<Box<dyn Stream<Item = ServiceResult<T>> + Send>>
    where
        T: ExportEntity + 'static;
    
    /// Get count for progress estimation
    async fn count_entities(&self, filter: &EntityFilter) -> ServiceResult<usize>;
    
    /// Stream entities as JSON values for flexible export
    fn create_json_stream(
        &self,
        filter: EntityFilter,
        batch_size: usize,
    ) -> Pin<Box<dyn Stream<Item = ServiceResult<serde_json::Value>> + Send>>;
}

/// Trait for exportable entities
pub trait ExportEntity: Send + Sync + Sized {
    fn table_name() -> &'static str;
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> ServiceResult<Self>;
    fn id(&self) -> Uuid;
    fn to_json(&self) -> ServiceResult<serde_json::Value>;
}

/// SQLite implementation with streaming
pub struct SqliteStreamingRepository {
    pool: SqlitePool,
}

impl SqliteStreamingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Stream strategic goals with cursor pagination - fetches all necessary fields for export
    async fn stream_strategic_goals(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
        status_filter: Option<i64>,
    ) -> ServiceResult<Vec<StrategicGoalExport>> {
        let mut query = QueryBuilder::new(
            "SELECT id, objective_code, outcome, kpi, target_value, actual_value, status_id, responsible_team, 
                    sync_priority, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at
             FROM strategic_goals"
        );
        
        let mut has_where = false;
        
        if let Some(cursor_id) = cursor {
            query.push(" WHERE id > ");
            query.push_bind(cursor_id.to_string());
            has_where = true;
        }
        
        if let Some(status) = status_filter {
            if has_where {
                query.push(" AND status_id = ");
            } else {
                query.push(" WHERE status_id = ");
            }
            query.push_bind(status);
        }
        
        // Limit to 1000 items for performance
        let actual_limit = std::cmp::min(limit, 1000);
        query.push(" ORDER BY id ASC LIMIT ");
        query.push_bind(actual_limit as i64);
        
        let rows = query
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?; 
        
        rows.into_iter()
            .map(|row| StrategicGoalExport::from_row(&row))
            .collect()
    }

    /// Stream strategic goals by specific IDs - fetches all necessary fields for export
    async fn stream_strategic_goals_by_ids(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
        ids: Vec<Uuid>,
    ) -> ServiceResult<Vec<StrategicGoalExport>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }

        // Limit to first 1000 IDs for performance
        let limited_ids = if ids.len() > 1000 {
            &ids[..1000]
        } else {
            &ids
        };

        let mut query_builder = QueryBuilder::new(
            "SELECT id, objective_code, outcome, kpi, target_value, actual_value, status_id, responsible_team, 
                    sync_priority, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at 
             FROM strategic_goals WHERE id IN ("
        );
        
        let mut separated = query_builder.separated(", ");
        for id in limited_ids {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(")");

        if let Some(cursor_id) = cursor {
            query_builder.push(" AND id > ");
            query_builder.push_bind(cursor_id.to_string());
        }

        // Limit to 1000 items for performance
        let actual_limit = std::cmp::min(limit, 1000);
        query_builder.push(" ORDER BY id LIMIT ");
        query_builder.push_bind(actual_limit as i64);

        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;

        rows.into_iter()
            .map(|row| StrategicGoalExport::from_row(&row))
            .collect()
    }
    
    /// Stream projects with efficient queries
    async fn stream_projects(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
    ) -> ServiceResult<Vec<ProjectExport>> {
        log::debug!("[PROJECT_STREAM] Starting stream_projects with cursor: {:?}, limit: {}", cursor, limit);
        
        let mut query = QueryBuilder::new(
            "SELECT id, name, objective, outcome, status_id, timeline, responsible_team, strategic_goal_id, 
                    sync_priority, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at 
             FROM projects"
        );
        
        if let Some(cursor_id) = cursor {
            log::debug!("[PROJECT_STREAM] Adding cursor filter: {}", cursor_id);
            query.push(" WHERE id > ");
            query.push_bind(cursor_id.to_string());
        }
        
        query.push(" ORDER BY id ASC LIMIT ");
        query.push_bind(limit as i64);
        
        let built_query = query.build();
        log::debug!("[PROJECT_STREAM] Executing query for projects");
        
        let rows = built_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| {
                log::error!("[PROJECT_STREAM] Database query failed: {}", e);
                ServiceError::DatabaseError(e.to_string())
            })?;
        
        log::debug!("[PROJECT_STREAM] Retrieved {} rows from database", rows.len());
        
        let results: ServiceResult<Vec<ProjectExport>> = rows.into_iter()
            .enumerate()
            .map(|(idx, row)| {
                log::debug!("[PROJECT_STREAM] Processing row {}", idx);
                ProjectExport::from_row(&row)
            })
            .collect();
            
        match &results {
            Ok(projects) => log::debug!("[PROJECT_STREAM] Successfully converted {} projects", projects.len()),
            Err(e) => log::error!("[PROJECT_STREAM] Row conversion failed: {}", e),
        }
        
        results
    }

    /// Stream projects by specific IDs - fetches all necessary fields for export
    async fn stream_projects_by_ids(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
        ids: Vec<Uuid>,
    ) -> ServiceResult<Vec<ProjectExport>> {
        log::debug!("[PROJECT_STREAM_BY_IDS] Starting with {} IDs, cursor: {:?}, limit: {}", ids.len(), cursor, limit);
        
        if ids.is_empty() {
            log::debug!("[PROJECT_STREAM_BY_IDS] No IDs provided, returning empty result");
            return Ok(vec![]);
        }

        // Limit to first 1000 IDs for performance
        let limited_ids = if ids.len() > 1000 {
            log::warn!("[PROJECT_STREAM_BY_IDS] Limiting {} IDs to first 1000", ids.len());
            &ids[..1000]
        } else {
            &ids
        };

        log::debug!("[PROJECT_STREAM_BY_IDS] Using {} limited IDs", limited_ids.len());

        let mut query_builder = QueryBuilder::new(
            "SELECT id, name, objective, outcome, status_id, timeline, responsible_team, strategic_goal_id,
                    sync_priority, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at 
             FROM projects WHERE id IN ("
        );
        
        let mut separated = query_builder.separated(", ");
        for (idx, id) in limited_ids.iter().enumerate() {
            log::debug!("[PROJECT_STREAM_BY_IDS] Adding ID {} to query: {}", idx, id);
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(")");

        if let Some(cursor_id) = cursor {
            log::debug!("[PROJECT_STREAM_BY_IDS] Adding cursor filter: {}", cursor_id);
            query_builder.push(" AND id > ");
            query_builder.push_bind(cursor_id.to_string());
        }

        // Limit to 1000 items for performance
        let actual_limit = std::cmp::min(limit, 1000);
        query_builder.push(" ORDER BY id LIMIT ");
        query_builder.push_bind(actual_limit as i64);

        log::debug!("[PROJECT_STREAM_BY_IDS] Executing query with limit: {}", actual_limit);
        
        let query = query_builder.build();
        let rows = query.fetch_all(&self.pool).await
            .map_err(|e| {
                log::error!("[PROJECT_STREAM_BY_IDS] Database query failed: {}", e);
                ServiceError::DatabaseError(e.to_string())
            })?;

        log::debug!("[PROJECT_STREAM_BY_IDS] Retrieved {} rows from database", rows.len());

        let results: ServiceResult<Vec<ProjectExport>> = rows.into_iter()
            .enumerate()
            .map(|(idx, row)| {
                log::debug!("[PROJECT_STREAM_BY_IDS] Processing row {}", idx);
                match ProjectExport::from_row(&row) {
                    Ok(project) => {
                        log::debug!("[PROJECT_STREAM_BY_IDS] Successfully converted project: {}", project.id);
                        Ok(project)
                    }
                    Err(e) => {
                        log::error!("[PROJECT_STREAM_BY_IDS] Failed to convert row {}: {}", idx, e);
                        Err(e)
                    }
                }
            })
            .collect();
            
        match &results {
            Ok(projects) => log::debug!("[PROJECT_STREAM_BY_IDS] Successfully converted {} projects", projects.len()),
            Err(e) => log::error!("[PROJECT_STREAM_BY_IDS] Row conversion failed: {}", e),
        }
        
        results
    }
    
    /// Stream workshops with participants count
    async fn stream_workshops(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
        include_participants: bool,
    ) -> ServiceResult<Vec<WorkshopExport>> {
        let query = if include_participants {
            r#"
                SELECT 
                    w.id, w.title, w.description, w.conducted_at, w.created_at, w.updated_at,
                    COUNT(wp.user_id) as participant_count
                FROM workshops w
                LEFT JOIN workshop_participants wp ON w.id = wp.workshop_id
                WHERE ($1::text IS NULL OR w.id::text > $1)
                GROUP BY w.id, w.title, w.description, w.conducted_at, w.created_at, w.updated_at
                ORDER BY w.id ASC
                LIMIT $2
            "#
        } else {
            r#"
                SELECT id, title, description, conducted_at, created_at, updated_at, 0 as participant_count
                FROM workshops
                WHERE ($1::text IS NULL OR id::text > $1)
                ORDER BY id ASC
                LIMIT $2
            "#
        };
        
        let rows = sqlx::query(query)
            .bind(cursor.map(|id| id.to_string()))
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        rows.into_iter()
            .map(|row| WorkshopExport::from_row(&row))
            .collect()
    }
    
    /// Create unified stream for any domain
    fn create_domain_stream<T>(
        &self,
        filter: EntityFilter,
        batch_size: usize,
    ) -> Pin<Box<dyn Stream<Item = ServiceResult<T>> + Send>>
    where
        T: ExportEntity + 'static,
    {
        // iOS-optimized channel buffer size based on memory pressure
        let buffer_size = match crate::domains::export::ios::memory::MemoryPressureObserver::new().current_level() {
            crate::domains::export::types::MemoryPressureLevel::Normal => 10,
            crate::domains::export::types::MemoryPressureLevel::Warning => 5,
            crate::domains::export::types::MemoryPressureLevel::Critical => 2,
        };
        
        let (tx, rx) = mpsc::channel(buffer_size);
        let pool = self.pool.clone();
        
        tokio::spawn(async move {
            let mut cursor: Option<Uuid> = None;
            
            loop {
                // Get batch based on entity type and filter using safe queries
                let batch_result = match &filter {
                    EntityFilter::StrategicGoals { status_id } => {
                        // Use safe parameterized query with all necessary fields
                        let mut query = QueryBuilder::new(
                            "SELECT id, objective_code, outcome, kpi, target_value, actual_value, status_id, responsible_team, 
                                    sync_priority, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at 
                             FROM strategic_goals"
                        );
                        
                        let mut has_where = false;
                                                 if let Some(cursor_id) = cursor {
                             query.push(" WHERE id > ");
                             query.push_bind(cursor_id.to_string());
                             has_where = true;
                         }
                        
                        if let Some(status) = status_id {
                            if has_where {
                                query.push(" AND status_id = ");
                            } else {
                                query.push(" WHERE status_id = ");
                            }
                            query.push_bind(*status);
                        }
                        
                        query.push(" ORDER BY id ASC LIMIT ");
                        query.push_bind(batch_size as i64);
                        
                        query.build().fetch_all(&pool).await
                    }
                    _ => {
                        // Use safe generic query for other types
                        let table = T::table_name();
                        let mut query = QueryBuilder::new(format!("SELECT * FROM {}", table));
                        
                                                 if let Some(cursor_id) = cursor {
                             query.push(" WHERE id > ");
                             query.push_bind(cursor_id.to_string());
                         }
                        
                        query.push(" ORDER BY id ASC LIMIT ");
                        query.push_bind(batch_size as i64);
                        
                        query.build().fetch_all(&pool).await
                    }
                };
                
                match batch_result {
                    Ok(rows) => {
                        if rows.is_empty() {
                            break;
                        }
                        
                        for row in rows {
                            match T::from_row(&row) {
                                Ok(entity) => {
                                    cursor = Some(entity.id());
                                    if tx.send(Ok(entity)).await.is_err() {
                                        return; // Receiver dropped
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e)).await;
                                    return;
                                }
                            }
                        }
                        
                        // Yield to prevent blocking
                        tokio::task::yield_now().await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(ServiceError::DatabaseError(e.to_string()))).await;
                        return;
                    }
                }
            }
        });
        
        Box::pin(ReceiverStream::new(rx))
    }

    async fn stream_participants(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
    ) -> ServiceResult<Vec<ParticipantExport>> {
        log::debug!("[PARTICIPANT_STREAM] Starting participant stream with cursor: {:?}, limit: {}", cursor, limit);
        
        let mut query = sqlx::QueryBuilder::new(
            "SELECT id, name, gender, disability, disability_type, age_group, location, sync_priority, \
             created_at, updated_at, created_by_user_id, created_by_device_id, \
             updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id \
             FROM participants"
        );
        
        if let Some(cursor_id) = cursor {
            query.push(" WHERE id > ");
            query.push_bind(cursor_id.to_string());
        }
        
        query.push(" ORDER BY id LIMIT ");
        query.push_bind(limit as i64);
        
        let built_query = query.build();
        log::debug!("[PARTICIPANT_STREAM] Executing query: {}", built_query.sql());
        
        let rows = built_query.fetch_all(&self.pool).await
            .map_err(|e| {
                log::error!("[PARTICIPANT_STREAM] Database query failed: {}", e);
                ServiceError::DatabaseError(e.to_string())
            })?;
        
        let mut participants = Vec::new();
        for row in rows {
            match ParticipantExport::from_row(&row) {
                Ok(participant) => participants.push(participant),
                Err(e) => {
                    log::error!("[PARTICIPANT_STREAM] Failed to convert row to participant: {}", e);
                    return Err(e);
                }
            }
        }
        
        log::debug!("[PARTICIPANT_STREAM] Successfully loaded {} participants", participants.len());
        Ok(participants)
    }
    
    async fn stream_participants_by_ids(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
        ids: Vec<Uuid>,
    ) -> ServiceResult<Vec<ParticipantExport>> {
        if ids.is_empty() {
            log::debug!("[PARTICIPANT_STREAM_BY_IDS] No IDs provided, returning empty");
            return Ok(Vec::new());
        }
        
        log::debug!("[PARTICIPANT_STREAM_BY_IDS] Starting stream with {} IDs, cursor: {:?}, limit: {}", ids.len(), cursor, limit);
        
        let mut query = sqlx::QueryBuilder::new(
            "SELECT id, name, gender, disability, disability_type, age_group, location, sync_priority, \
             created_at, updated_at, created_by_user_id, created_by_device_id, \
             updated_by_user_id, updated_by_device_id, deleted_at, deleted_by_user_id, deleted_by_device_id \
             FROM participants WHERE id IN ("
        );
        
        let mut separated = query.separated(", ");
        for id in &ids {
            separated.push_bind(id.to_string());
        }
        separated.push_unseparated(")");
        
        if let Some(cursor_id) = cursor {
            query.push(" AND id > ");
            query.push_bind(cursor_id.to_string());
        }
        
        query.push(" ORDER BY id LIMIT ");
        query.push_bind(limit as i64);
        
        let built_query = query.build();
        log::debug!("[PARTICIPANT_STREAM_BY_IDS] Executing query: {}", built_query.sql());
        
        let rows = built_query.fetch_all(&self.pool).await
            .map_err(|e| {
                log::error!("[PARTICIPANT_STREAM_BY_IDS] Database query failed: {}", e);
                ServiceError::DatabaseError(e.to_string())
            })?;
        
        let mut participants = Vec::new();
        for row in rows {
            match ParticipantExport::from_row(&row) {
                Ok(participant) => participants.push(participant),
                Err(e) => {
                    log::error!("[PARTICIPANT_STREAM_BY_IDS] Failed to convert row to participant: {}", e);
                    return Err(e);
                }
            }
        }
        
        log::debug!("[PARTICIPANT_STREAM_BY_IDS] Successfully loaded {} participants", participants.len());
        Ok(participants)
    }
}

#[async_trait]
impl StreamingExportRepository for SqliteStreamingRepository {
    async fn stream_by_cursor<T>(
        &self,
        cursor: Option<Uuid>,
        limit: usize,
    ) -> ServiceResult<Vec<T>>
    where
        T: ExportEntity,
    {
        let table = T::table_name();
        let mut query = QueryBuilder::new(format!("SELECT * FROM {}", table));
        
        if let Some(cursor_id) = cursor {
            query.push(" WHERE id > ");
            query.push_bind(cursor_id.to_string());
        }
        
        query.push(" ORDER BY id ASC LIMIT ");
        query.push_bind(limit as i64);
        
        let rows = query
            .build()
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        rows.into_iter()
            .map(|row| T::from_row(&row))
            .collect()
    }
    
    fn create_stream<T>(
        &self,
        filter: EntityFilter,
        batch_size: usize,
    ) -> Pin<Box<dyn Stream<Item = ServiceResult<T>> + Send>>
    where
        T: ExportEntity + 'static,
    {
        self.create_domain_stream(filter, batch_size)
    }
    
    async fn count_entities(&self, filter: &EntityFilter) -> ServiceResult<usize> {
        match filter {
            EntityFilter::StrategicGoals { status_id } => {
                let where_clause = status_id
                    .map(|s| format!("WHERE status_id = {}", s))
                    .unwrap_or_default();
                let query = format!("SELECT COUNT(*) as count FROM strategic_goals {}", where_clause);
                let row = sqlx::query(&query)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
                Ok(row.get::<i64, _>("count") as usize)
            }
            EntityFilter::StrategicGoalsByIds { ids } => {
                // For ID-based filters, return the count of IDs (limited to actual available IDs)
                if ids.is_empty() {
                    return Ok(0);
                }
                let limited_count = std::cmp::min(ids.len(), 1000);
                Ok(limited_count)
            }
            EntityFilter::ProjectsAll => {
                let query = "SELECT COUNT(*) as count FROM projects";
                let row = sqlx::query(query)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
                Ok(row.get::<i64, _>("count") as usize)
            }
            EntityFilter::ProjectsByIds { ids } => {
                // For ID-based filters, return the count of IDs (limited to actual available IDs)
                if ids.is_empty() {
                    return Ok(0);
                }
                let limited_count = std::cmp::min(ids.len(), 1000);
                Ok(limited_count)
            }
            EntityFilter::WorkshopsAll { .. } => {
                let query = "SELECT COUNT(*) as count FROM workshops";
                let row = sqlx::query(query)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
                Ok(row.get::<i64, _>("count") as usize)
            }
            EntityFilter::ParticipantsAll => {
                let query = "SELECT COUNT(*) as count FROM participants";
                let row = sqlx::query(query)
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
                Ok(row.get::<i64, _>("count") as usize)
            }
            EntityFilter::ParticipantsByIds { ids } => {
                // For ID-based filters, return the count of IDs (limited to actual available IDs)
                if ids.is_empty() {
                    return Ok(0);
                }
                let limited_count = std::cmp::min(ids.len(), 1000);
                Ok(limited_count)
            }
            _ => {
                // For unsupported filters, return 0
                Ok(0)
            }
        }
    }
    
    fn create_json_stream(
        &self,
        filter: EntityFilter,
        batch_size: usize,
    ) -> Pin<Box<dyn Stream<Item = ServiceResult<serde_json::Value>> + Send>> {
        let (tx, rx) = mpsc::channel(50); // Increased buffer for better throughput
        let pool = self.pool.clone();
        
        tokio::spawn(async move {
            let mut cursor: Option<Uuid> = None;
            let mut total_processed = 0;
            
            log::debug!("Starting JSON stream for filter: {:?}, batch_size: {}", filter, batch_size);
            
            loop {
                let entities = match &filter {
                    EntityFilter::StrategicGoals { status_id } => {
                        let repo = SqliteStreamingRepository { pool: pool.clone() };
                        match repo.stream_strategic_goals(cursor, batch_size, *status_id).await {
                            Ok(goals) => {
                                let json_results: Result<Vec<_>, _> = goals.into_iter()
                                    .map(|g| g.to_json())
                                    .collect();
                                match json_results {
                                    Ok(json_vec) => json_vec,
                                    Err(e) => {
                                        log::error!("JSON conversion error: {}", e);
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Database query error: {}", e);
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    EntityFilter::StrategicGoalsByIds { ids } => {
                        let repo = SqliteStreamingRepository { pool: pool.clone() };
                        match repo.stream_strategic_goals_by_ids(cursor, batch_size, ids.clone()).await {
                            Ok(goals) => {
                                let json_results: Result<Vec<_>, _> = goals.into_iter()
                                    .map(|g| g.to_json())
                                    .collect();
                                match json_results {
                                    Ok(json_vec) => json_vec,
                                    Err(e) => {
                                        log::error!("JSON conversion error: {}", e);
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Database query error: {}", e);
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    EntityFilter::ProjectsAll => {
                        let repo = SqliteStreamingRepository { pool: pool.clone() };
                        match repo.stream_projects(cursor, batch_size).await {
                            Ok(projects) => {
                                let json_results: Result<Vec<_>, _> = projects.into_iter()
                                    .map(|p| p.to_json())
                                    .collect();
                                match json_results {
                                    Ok(json_vec) => json_vec,
                                    Err(e) => {
                                        log::error!("JSON conversion error: {}", e);
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Database query error: {}", e);
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    EntityFilter::ProjectsByIds { ids } => {
                        log::debug!("[JSON_STREAM] Processing ProjectsByIds filter with {} IDs, cursor: {:?}, batch_size: {}", ids.len(), cursor, batch_size);
                        let repo = SqliteStreamingRepository { pool: pool.clone() };
                        match repo.stream_projects_by_ids(cursor, batch_size, ids.clone()).await {
                            Ok(projects) => {
                                log::debug!("[JSON_STREAM] Retrieved {} projects from repository", projects.len());
                                let json_results: Result<Vec<_>, _> = projects.into_iter()
                                    .enumerate()
                                    .map(|(idx, p)| {
                                        log::debug!("[JSON_STREAM] Converting project {} to JSON: {}", idx, p.id);
                                        p.to_json()
                                    })
                                    .collect();
                                match json_results {
                                    Ok(json_vec) => {
                                        log::debug!("[JSON_STREAM] Successfully converted {} projects to JSON", json_vec.len());
                                        json_vec
                                    },
                                    Err(e) => {
                                        log::error!("[JSON_STREAM] JSON conversion error: {}", e);
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("[JSON_STREAM] Database query error: {}", e);
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    EntityFilter::WorkshopsAll { include_participants } => {
                        let repo = SqliteStreamingRepository { pool: pool.clone() };
                        match repo.stream_workshops(cursor, batch_size, *include_participants).await {
                            Ok(workshops) => {
                                let json_results: Result<Vec<_>, _> = workshops.into_iter()
                                    .map(|w| w.to_json())
                                    .collect();
                                match json_results {
                                    Ok(json_vec) => json_vec,
                                    Err(e) => {
                                        log::error!("JSON conversion error: {}", e);
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Database query error: {}", e);
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    EntityFilter::ParticipantsAll => {
                        let repo = SqliteStreamingRepository { pool: pool.clone() };
                        match repo.stream_participants(cursor, batch_size).await {
                            Ok(participants) => {
                                let json_results: Result<Vec<_>, _> = participants.into_iter()
                                    .map(|p| p.to_json())
                                    .collect();
                                match json_results {
                                    Ok(json_vec) => json_vec,
                                    Err(e) => {
                                        log::error!("JSON conversion error: {}", e);
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Database query error: {}", e);
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    EntityFilter::ParticipantsByIds { ids } => {
                        let repo = SqliteStreamingRepository { pool: pool.clone() };
                        match repo.stream_participants_by_ids(cursor, batch_size, ids.clone()).await {
                            Ok(participants) => {
                                let json_results: Result<Vec<_>, _> = participants.into_iter()
                                    .map(|p| p.to_json())
                                    .collect();
                                match json_results {
                                    Ok(json_vec) => json_vec,
                                    Err(e) => {
                                        log::error!("JSON conversion error: {}", e);
                                        let _ = tx.send(Err(e)).await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("Database query error: {}", e);
                                let _ = tx.send(Err(e)).await;
                                return;
                            }
                        }
                    }
                    _ => {
                        log::error!("Unsupported filter type: {:?}", filter);
                        let _ = tx.send(Err(ServiceError::NotImplemented("Filter not supported".into()))).await;
                        return;
                    }
                };
                
                if entities.is_empty() {
                    log::debug!("No more entities found, ending stream. Total processed: {}", total_processed);
                    break;
                }
                
                log::debug!("Processing batch of {} entities (total: {})", entities.len(), total_processed + entities.len());
                
                // Find the last ID for cursor progression (fixed logic)
                let mut last_id: Option<Uuid> = None;
                
                for (entity_idx, json) in entities.into_iter().enumerate() {
                    // Update cursor with current ID to ensure progression
                    if let Some(id_value) = json.get("id") {
                        if let Some(id_str) = id_value.as_str() {
                            log::debug!("[JSON_STREAM] Processing entity {} with ID: {}", entity_idx, id_str);
                            if let Ok(id) = Uuid::parse_str(id_str) {
                                last_id = Some(id);
                            }
                        }
                    }
                    
                    total_processed += 1;
                    
                    log::debug!("[JSON_STREAM] Sending entity {} to stream (total processed: {})", entity_idx, total_processed);
                    if tx.send(Ok(json)).await.is_err() {
                        log::debug!("[JSON_STREAM] Receiver dropped, stopping stream");
                        return; // Receiver dropped
                    }
                }
                
                // Update cursor to the last processed ID to ensure proper pagination
                cursor = last_id;
                
                // For ID-based filters, we've processed all requested items
                if matches!(&filter, EntityFilter::StrategicGoalsByIds { .. } | EntityFilter::ProjectsByIds { .. }) && total_processed >= batch_size * 10 {
                    log::debug!("Reached reasonable limit for ID-based query, stopping");
                    break;
                }
                
                // Yield control to prevent blocking
                tokio::task::yield_now().await;
            }
            
            log::debug!("JSON stream completed. Total entities processed: {}", total_processed);
        });
        
        Box::pin(ReceiverStream::new(rx))
    }
}

// Helper functions

fn build_strategic_goals_query(cursor: Option<Uuid>, limit: usize, status_id: Option<i64>) -> String {
    // This function is deprecated and should not be used due to SQL injection risks
    // Use QueryBuilder with push_bind instead
    unimplemented!("Use QueryBuilder with parameter binding to prevent SQL injection")
}

fn build_generic_query<T: ExportEntity>(
    filter: &EntityFilter,
    cursor: Option<Uuid>,
    limit: usize,
) -> String {
    // This function is deprecated and should not be used due to SQL injection risks
    // Use QueryBuilder with push_bind instead
    unimplemented!("Use QueryBuilder with parameter binding to prevent SQL injection")
}

// Export-specific entity types that implement ExportEntity

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StrategicGoalExport {
    pub id: Uuid,
    pub objective_code: String,
    pub outcome: Option<String>,
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub status_id: Option<i64>,
    pub responsible_team: Option<String>,
    pub sync_priority: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ExportEntity for StrategicGoalExport {
    fn table_name() -> &'static str {
        "strategic_goals"
    }
    
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> ServiceResult<Self> {
        use sqlx::Row;
        Ok(Self {
            id: Uuid::parse_str(&row.get::<String, _>("id"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?,
            objective_code: row.get("objective_code"),
            outcome: row.get("outcome"),
            kpi: row.get("kpi"),
            target_value: row.get("target_value"),
            actual_value: row.get("actual_value"),
            status_id: row.get("status_id"),
            responsible_team: row.get("responsible_team"),
            sync_priority: row.get("sync_priority"),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?
                .with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?
                .with_timezone(&chrono::Utc),
            created_by_user_id: row.get::<Option<String>, _>("created_by_user_id")
                .and_then(|s| Uuid::parse_str(&s).ok()),
            updated_by_user_id: row.get::<Option<String>, _>("updated_by_user_id")
                .and_then(|s| Uuid::parse_str(&s).ok()),
            deleted_at: row.get::<Option<String>, _>("deleted_at")
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
        })
    }
    
    fn id(&self) -> Uuid {
        self.id
    }
    
    fn to_json(&self) -> ServiceResult<serde_json::Value> {
        serde_json::to_value(self)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectExport {
    pub id: Uuid,
    pub name: String,
    pub objective: Option<String>,
    pub outcome: Option<String>,
    pub status_id: Option<i64>,
    pub timeline: Option<String>,
    pub responsible_team: Option<String>,
    pub strategic_goal_id: Option<Uuid>,
    pub sync_priority: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ExportEntity for ProjectExport {
    fn table_name() -> &'static str {
        "projects"
    }
    
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> ServiceResult<Self> {
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Starting row conversion");
        
        let id_str = row.get::<String, _>("id");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got id: {}", id_str);
        let id = Uuid::parse_str(&id_str)
            .map_err(|e| {
                log::error!("[PROJECT_EXPORT_FROM_ROW] Failed to parse UUID: {}", e);
                ServiceError::ValidationError(e.to_string())
            })?;
        
        let name: String = row.get("name");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got name: {}", name);
        
        let objective: Option<String> = row.get("objective");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got objective: {:?}", objective);
        
        let outcome: Option<String> = row.get("outcome");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got outcome: {:?}", outcome);
        
        let status_id: Option<i64> = row.get("status_id");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got status_id: {:?}", status_id);
        
        let timeline: Option<String> = row.get("timeline");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got timeline: {:?}", timeline);
        
        let responsible_team: Option<String> = row.get("responsible_team");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got responsible_team: {:?}", responsible_team);
        
        let strategic_goal_id_str: Option<String> = row.get("strategic_goal_id");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got strategic_goal_id_str: {:?}", strategic_goal_id_str);
        let strategic_goal_id = strategic_goal_id_str.and_then(|s| {
            match Uuid::parse_str(&s) {
                Ok(uuid) => {
                    log::debug!("[PROJECT_EXPORT_FROM_ROW] Parsed strategic_goal_id: {}", uuid);
                    Some(uuid)
                }
                Err(e) => {
                    log::warn!("[PROJECT_EXPORT_FROM_ROW] Failed to parse strategic_goal_id '{}': {}", s, e);
                    None
                }
            }
        });
        
        let sync_priority: Option<String> = row.get("sync_priority");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got sync_priority: {:?}", sync_priority);
        
        let created_at_str: String = row.get("created_at");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got created_at: {}", created_at_str);
        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                log::error!("[PROJECT_EXPORT_FROM_ROW] Failed to parse created_at: {}", e);
                ServiceError::ValidationError(e.to_string())
            })?
            .with_timezone(&chrono::Utc);
        
        let updated_at_str: String = row.get("updated_at");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got updated_at: {}", updated_at_str);
        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| {
                log::error!("[PROJECT_EXPORT_FROM_ROW] Failed to parse updated_at: {}", e);
                ServiceError::ValidationError(e.to_string())
            })?
            .with_timezone(&chrono::Utc);
        
        let created_by_user_id_str: Option<String> = row.get("created_by_user_id");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got created_by_user_id_str: {:?}", created_by_user_id_str);
        let created_by_user_id = created_by_user_id_str.and_then(|s| Uuid::parse_str(&s).ok());
        
        let updated_by_user_id_str: Option<String> = row.get("updated_by_user_id");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got updated_by_user_id_str: {:?}", updated_by_user_id_str);
        let updated_by_user_id = updated_by_user_id_str.and_then(|s| Uuid::parse_str(&s).ok());
        
        let deleted_at_str: Option<String> = row.get("deleted_at");
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Got deleted_at_str: {:?}", deleted_at_str);
        let deleted_at = deleted_at_str.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .ok()
        });
        
        let project = Self {
            id,
            name,
            objective,
            outcome,
            status_id,
            timeline,
            responsible_team,
            strategic_goal_id,
            sync_priority,
            created_at,
            updated_at,
            created_by_user_id,
            updated_by_user_id,
            deleted_at,
        };
        
        log::debug!("[PROJECT_EXPORT_FROM_ROW] Successfully created ProjectExport: {}", project.id);
        Ok(project)
    }
    
    fn id(&self) -> Uuid {
        self.id
    }
    
    fn to_json(&self) -> ServiceResult<serde_json::Value> {
        log::debug!("[PROJECT_EXPORT_TO_JSON] Converting project {} to JSON", self.id);
        match serde_json::to_value(self) {
            Ok(json) => {
                log::debug!("[PROJECT_EXPORT_TO_JSON] Successfully converted project {} to JSON", self.id);
                Ok(json)
            }
            Err(e) => {
                log::error!("[PROJECT_EXPORT_TO_JSON] Failed to convert project {} to JSON: {}", self.id, e);
                Err(ServiceError::SerializationError(e.to_string()))
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParticipantExport {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub disability: bool,
    pub disability_type: Option<String>,
    pub age_group: Option<String>,
    pub location: Option<String>,
    pub sync_priority: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,
}

impl ExportEntity for ParticipantExport {
    fn table_name() -> &'static str {
        "participants"
    }
    
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> ServiceResult<Self> {
        log::debug!("[PARTICIPANT_EXPORT] Converting database row to ParticipantExport");
        
        let result = Self {
            id: Uuid::parse_str(&row.get::<String, _>("id"))
                .map_err(|e| ServiceError::ValidationError(format!("Invalid participant UUID: {}", e)))?,
            name: row.get("name"),
            gender: row.get("gender"),
            disability: row.get::<i64, _>("disability") != 0, // SQLite stores boolean as integer
            disability_type: row.get("disability_type"),
            age_group: row.get("age_group"),
            location: row.get("location"),
            sync_priority: row.get::<Option<String>, _>("sync_priority"),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                .map_err(|e| ServiceError::ValidationError(format!("Invalid created_at timestamp: {}", e)))?
                .with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                .map_err(|e| ServiceError::ValidationError(format!("Invalid updated_at timestamp: {}", e)))?
                .with_timezone(&chrono::Utc),
            created_by_user_id: row.get::<Option<String>, _>("created_by_user_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| ServiceError::ValidationError(format!("Invalid created_by_user_id UUID: {}", e)))?,
            created_by_device_id: row.get::<Option<String>, _>("created_by_device_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| ServiceError::ValidationError(format!("Invalid created_by_device_id UUID: {}", e)))?,
            updated_by_user_id: row.get::<Option<String>, _>("updated_by_user_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| ServiceError::ValidationError(format!("Invalid updated_by_user_id UUID: {}", e)))?,
            updated_by_device_id: row.get::<Option<String>, _>("updated_by_device_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| ServiceError::ValidationError(format!("Invalid updated_by_device_id UUID: {}", e)))?,
            deleted_at: row.get::<Option<String>, _>("deleted_at")
                .map(|s| chrono::DateTime::parse_from_rfc3339(&s))
                .transpose()
                .map_err(|e| ServiceError::ValidationError(format!("Invalid deleted_at timestamp: {}", e)))?
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            deleted_by_user_id: row.get::<Option<String>, _>("deleted_by_user_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| ServiceError::ValidationError(format!("Invalid deleted_by_user_id UUID: {}", e)))?,
            deleted_by_device_id: row.get::<Option<String>, _>("deleted_by_device_id")
                .map(|s| Uuid::parse_str(&s))
                .transpose()
                .map_err(|e| ServiceError::ValidationError(format!("Invalid deleted_by_device_id UUID: {}", e)))?,
        };
        
        log::debug!("[PARTICIPANT_EXPORT] Successfully converted participant: {} ({})", result.name, result.id);
        Ok(result)
    }
    
    fn id(&self) -> Uuid {
        self.id
    }
    
    fn to_json(&self) -> ServiceResult<serde_json::Value> {
        serde_json::to_value(self)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkshopExport {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub conducted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub participant_count: i64,
}

impl ExportEntity for WorkshopExport {
    fn table_name() -> &'static str {
        "workshops"
    }
    
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> ServiceResult<Self> {
        Ok(Self {
            id: Uuid::parse_str(&row.get::<String, _>("id"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?,
            title: row.get("title"),
            description: row.get("description"),
            conducted_at: row.get::<Option<String>, _>("conducted_at")
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?
                .with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?
                .with_timezone(&chrono::Utc),
            participant_count: row.get("participant_count"),
        })
    }
    
    fn id(&self) -> Uuid {
        self.id
    }
    
    fn to_json(&self) -> ServiceResult<serde_json::Value> {
        serde_json::to_value(self)
            .map_err(|e| ServiceError::SerializationError(e.to_string()))
    }
}

// Helper trait to get entity type from filter
trait EntityFilterExt {
    fn entity_type(&self) -> &'static str;
}

impl EntityFilterExt for EntityFilter {
    fn entity_type(&self) -> &'static str {
        match self {
            EntityFilter::StrategicGoals { .. } => "strategic_goals",
            EntityFilter::ProjectsAll => "projects", 
            EntityFilter::WorkshopsAll { .. } => "workshops",
            _ => "unknown",
        }
    }
} 