use async_trait::async_trait;
use sqlx::{SqlitePool, Transaction, Sqlite};
use uuid::Uuid;

use crate::errors::DomainResult;

use super::types::{ExportJob, ExportStatus};

/// Trait for transactional job repository operations
#[async_trait]
pub trait TransactionalJobRepository: Send + Sync {
    async fn begin_transaction(&self) -> Result<Transaction<'_, Sqlite>, crate::errors::ServiceError>;
    async fn create_job_tx(&self, tx: &mut Transaction<'_, Sqlite>, job: &ExportJob) -> Result<(), crate::errors::ServiceError>;
    async fn update_status_tx(
        &self, 
        tx: &mut Transaction<'_, Sqlite>,
        job_id: Uuid,
        status: ExportStatus,
        error_message: Option<String>,
        local_path: Option<String>,
        total_entities: Option<i64>,
        total_bytes: Option<i64>
    ) -> Result<(), crate::errors::ServiceError>;
}



#[async_trait]
pub trait ExportJobRepository: Send + Sync {
    async fn create_job(&self, job: &ExportJob) -> DomainResult<()>;
    async fn update_status(
        &self,
        id: Uuid,
        status: ExportStatus,
        error: Option<String>,
        local_path: Option<String>,
        total_entities: Option<i64>,
        total_bytes: Option<i64>,
    ) -> DomainResult<()>;
    async fn find_by_id(&self, id: Uuid) -> DomainResult<ExportJob>;
    
    /// Get transactional capabilities if supported
    fn as_transactional(&self) -> Option<&dyn TransactionalJobRepository> {
        None // Default implementation returns None
    }
}

pub struct SqliteExportJobRepository {
    pool: SqlitePool,
}

impl SqliteExportJobRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
    
    /// Begin a database transaction
    pub async fn begin_transaction(&self) -> Result<Transaction<'_, Sqlite>, sqlx::Error> {
        self.pool.begin().await
    }
}

fn status_to_str(status: &ExportStatus) -> &'static str {
    match status {
        ExportStatus::Pending => "pending",
        ExportStatus::Queued => "queued",
        ExportStatus::Running => "running",
        ExportStatus::Completed => "completed",
        ExportStatus::Failed => "failed",
    }
}

fn str_to_status(s: &str) -> Option<ExportStatus> {
    match s {
        "pending" => Some(ExportStatus::Pending),
        "queued" => Some(ExportStatus::Queued),
        "running" => Some(ExportStatus::Running),
        "completed" => Some(ExportStatus::Completed),
        "failed" => Some(ExportStatus::Failed),
        _ => None,
    }
}



/// Implementation of transactional operations
#[async_trait]
impl TransactionalJobRepository for SqliteExportJobRepository {
    async fn begin_transaction(&self) -> Result<Transaction<'_, Sqlite>, crate::errors::ServiceError> {
        self.pool.begin().await
            .map_err(|e| crate::errors::ServiceError::DatabaseError(e.to_string()))
    }

    async fn create_job_tx(&self, tx: &mut Transaction<'_, Sqlite>, job: &ExportJob) -> Result<(), crate::errors::ServiceError> {
        use sqlx::query;
        query("INSERT INTO export_jobs (id, requested_by_user_id, requested_at, include_blobs, status, local_path, total_entities, total_bytes, error_message) VALUES (?,?,?,?,?,?,?,?,?)")
            .bind(job.id.to_string())
            .bind(job.requested_by_user_id.map(|u| u.to_string()))
            .bind(job.requested_at.to_rfc3339())
            .bind(if job.include_blobs { 1 } else { 0 })
            .bind(status_to_str(&job.status))
            .bind(&job.local_path)
            .bind(job.total_entities)
            .bind(job.total_bytes)
            .bind(&job.error_message)
            .execute(&mut **tx)
            .await
            .map_err(|e| crate::errors::ServiceError::DatabaseError(e.to_string()))?;
        Ok(())
    }

    async fn update_status_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        job_id: Uuid,
        status: ExportStatus,
        error_message: Option<String>,
        local_path: Option<String>,
        total_entities: Option<i64>,
        total_bytes: Option<i64>
    ) -> Result<(), crate::errors::ServiceError> {
        use sqlx::query;
        query("UPDATE export_jobs SET status = ?, error_message = ?, local_path = COALESCE(?, local_path), total_entities = COALESCE(?, total_entities), total_bytes = COALESCE(?, total_bytes) WHERE id = ?")
            .bind(status_to_str(&status))
            .bind(error_message)
            .bind(local_path)
            .bind(total_entities)
            .bind(total_bytes)
            .bind(job_id.to_string())
            .execute(&mut **tx)
            .await
            .map_err(|e| crate::errors::ServiceError::DatabaseError(e.to_string()))?;
        Ok(())
    }
}

/// Override the default implementation to return transactional capabilities
impl SqliteExportJobRepository {
    /// Get transactional capabilities for this repository
    pub fn as_transactional_impl(&self) -> &dyn TransactionalJobRepository {
        self
    }
}

#[async_trait]
impl ExportJobRepository for SqliteExportJobRepository {
    async fn create_job(&self, job: &ExportJob) -> DomainResult<()> {
        use sqlx::query;
        query("INSERT INTO export_jobs (id, requested_by_user_id, requested_at, include_blobs, status, local_path, total_entities, total_bytes, error_message) VALUES (?,?,?,?,?,?,?,?,?)")
            .bind(job.id.to_string())
            .bind(job.requested_by_user_id.map(|u| u.to_string()))
            .bind(job.requested_at.to_rfc3339())
            .bind(if job.include_blobs { 1 } else { 0 })
            .bind(status_to_str(&job.status))
            .bind(&job.local_path)
            .bind(job.total_entities)
            .bind(job.total_bytes)
            .bind(&job.error_message)
            .execute(&self.pool)
            .await
            .map_err(|e| crate::errors::DomainError::Database(e.into()))?;
        Ok(())
    }

    async fn update_status(
        &self,
        id: Uuid,
        status: ExportStatus,
        error: Option<String>,
        local_path: Option<String>,
        total_entities: Option<i64>,
        total_bytes: Option<i64>,
    ) -> DomainResult<()> {
        use sqlx::query;
        query("UPDATE export_jobs SET status = ?, error_message = ?, local_path = COALESCE(?, local_path), total_entities = COALESCE(?, total_entities), total_bytes = COALESCE(?, total_bytes) WHERE id = ?")
            .bind(status_to_str(&status))
            .bind(error)
            .bind(local_path)
            .bind(total_entities)
            .bind(total_bytes)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| crate::errors::DomainError::Database(e.into()))?;
        Ok(())
    }

    async fn find_by_id(&self, id: Uuid) -> DomainResult<ExportJob> {
        use sqlx::query_as;
        #[derive(sqlx::FromRow)]
        struct Row {
            id: String,
            requested_by_user_id: Option<String>,
            requested_at: String,
            include_blobs: i64,
            status: String,
            local_path: Option<String>,
            total_entities: Option<i64>,
            total_bytes: Option<i64>,
            error_message: Option<String>,
        }

        let row: Row = query_as("SELECT * FROM export_jobs WHERE id = ?")
            .bind(id.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| crate::errors::DomainError::Database(e.into()))?;

        let status = str_to_status(&row.status)
            .ok_or_else(|| crate::errors::DomainError::Internal(format!("Invalid status {} in export_jobs", row.status)))?;

        let job = ExportJob {
            id: Uuid::parse_str(&row.id).map_err(|e| crate::errors::DomainError::InvalidUuid(e.to_string()))?,
            requested_by_user_id: row.requested_by_user_id.map(|s| Uuid::parse_str(&s).unwrap_or(Uuid::nil())),
            requested_at: chrono::DateTime::parse_from_rfc3339(&row.requested_at).map_err(|e| crate::errors::DomainError::Internal(format!("Bad timestamp: {}", e)))?.with_timezone(&chrono::Utc),
            include_blobs: row.include_blobs != 0,
            status,
            local_path: row.local_path,
            total_entities: row.total_entities,
            total_bytes: row.total_bytes,
            error_message: row.error_message,
        };

        Ok(job)
    }
    
    /// Override to provide transactional capabilities
    fn as_transactional(&self) -> Option<&dyn TransactionalJobRepository> {
        Some(self)
    }
} 