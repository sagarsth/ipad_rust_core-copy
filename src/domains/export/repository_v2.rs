use crate::domains::export::types::*;
use crate::errors::{ServiceError, ServiceResult};
use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use sqlx::{SqlitePool, Row, QueryBuilder};
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
        let mut query = QueryBuilder::new(
            "SELECT id, name, description, strategic_goal_id, created_at, updated_at 
             FROM projects"
        );
        
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
            .map(|row| ProjectExport::from_row(&row))
            .collect()
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
        let (table, where_clause) = match filter {
            EntityFilter::StrategicGoals { status_id } => {
                let where_clause = status_id
                    .map(|s| format!("WHERE status_id = {}", s))
                    .unwrap_or_default();
                ("strategic_goals", where_clause)
            }
            EntityFilter::ProjectsAll => {
                ("projects", String::new())
            }
            EntityFilter::WorkshopsAll { .. } => {
                ("workshops", String::new())
            }
            _ => ("strategic_goals", String::new()),
        };
        
        let query = format!("SELECT COUNT(*) as count FROM {} {}", table, where_clause);
        let row = sqlx::query(&query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ServiceError::DatabaseError(e.to_string()))?;
        
        Ok(row.get::<i64, _>("count") as usize)
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
                
                for json in entities {
                    // Update cursor with current ID to ensure progression
                    if let Some(id_value) = json.get("id") {
                        if let Some(id_str) = id_value.as_str() {
                            if let Ok(id) = Uuid::parse_str(id_str) {
                                last_id = Some(id);
                            }
                        }
                    }
                    
                    total_processed += 1;
                    
                    if tx.send(Ok(json)).await.is_err() {
                        log::debug!("Receiver dropped, stopping stream");
                        return; // Receiver dropped
                    }
                }
                
                // Update cursor to the last processed ID to ensure proper pagination
                cursor = last_id;
                
                // For ID-based filters, we've processed all requested items
                if matches!(&filter, EntityFilter::StrategicGoalsByIds { .. }) && total_processed >= batch_size * 10 {
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
    pub description: Option<String>,
    pub strategic_goal_id: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl ExportEntity for ProjectExport {
    fn table_name() -> &'static str {
        "projects"
    }
    
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> ServiceResult<Self> {
        Ok(Self {
            id: Uuid::parse_str(&row.get::<String, _>("id"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?,
            name: row.get("name"),
            description: row.get("description"),
            strategic_goal_id: row.get::<Option<String>, _>("strategic_goal_id")
                .and_then(|s| Uuid::parse_str(&s).ok()),
            created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("created_at"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?
                .with_timezone(&chrono::Utc),
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.get::<String, _>("updated_at"))
                .map_err(|e| ServiceError::ValidationError(e.to_string()))?
                .with_timezone(&chrono::Utc),
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