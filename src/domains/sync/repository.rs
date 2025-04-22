use crate::errors::{DomainError, DbError, DomainResult};
use crate::domains::sync::types::{Tombstone, ChangeLogEntry, ChangeOperationType};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{Pool, Sqlite, Transaction, Executor};
use uuid::Uuid;
use std::str::FromStr;

/// Repository for tombstone operations
#[async_trait]
pub trait TombstoneRepository: Send + Sync {
    /// Create a new tombstone (standalone)
    async fn create_tombstone(&self, tombstone: &Tombstone) -> DomainResult<()>;

    /// Create a new tombstone within a transaction
    async fn create_tombstone_with_tx(
        &self, 
        tombstone: &Tombstone, 
        tx: &mut Transaction<'_, Sqlite>
    ) -> DomainResult<()>;
    
    /// Create multiple tombstones in a batch
    async fn create_tombstones(&self, tombstones: &[Tombstone]) -> DomainResult<()>;
    
    /// Find tombstones by entity type
    async fn find_tombstones_by_entity_type(&self, entity_type: &str) -> DomainResult<Vec<Tombstone>>;
    
    /// Find tombstones by entity ID
    async fn find_tombstone_by_entity_id(&self, entity_id: Uuid) -> DomainResult<Option<Tombstone>>;
}

/// Repository for change log operations
#[async_trait]
pub trait ChangeLogRepository: Send + Sync {
    /// Create a new change log entry (standalone)
    async fn create_change_log(&self, change_log: &ChangeLogEntry) -> DomainResult<()>;

    /// Create a new change log entry within a transaction
    async fn create_change_log_with_tx(
        &self, 
        change_log: &ChangeLogEntry, 
        tx: &mut Transaction<'_, Sqlite>
    ) -> DomainResult<()>;
    
    /// Create multiple change log entries in a batch
    async fn create_change_logs(&self, change_logs: &[ChangeLogEntry]) -> DomainResult<()>;
    
    /// Find change logs by entity type and ID
    async fn find_change_logs_by_entity(&self, entity_table: &str, entity_id: &str) -> DomainResult<Vec<ChangeLogEntry>>;
}

/// SQLite implementation of the TombstoneRepository
pub struct SqliteTombstoneRepository {
    pool: Pool<Sqlite>,
}

#[async_trait]
impl TombstoneRepository for SqliteTombstoneRepository {
    async fn create_tombstone(&self, tombstone: &Tombstone) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(DbError::from)?;
        match self.create_tombstone_with_tx(tombstone, &mut tx).await {
            Ok(_) => {
                tx.commit().await.map_err(DbError::from)?;
                Ok(())
            },
            Err(e) => {
                let _ = tx.rollback().await;
                Err(e)
            }
        }
    }
    
    async fn create_tombstone_with_tx(
        &self, 
        tombstone: &Tombstone, 
        tx: &mut Transaction<'_, Sqlite>
    ) -> DomainResult<()> {
        sqlx::query(
            r#"
            INSERT INTO tombstones (id, entity_id, entity_type, deleted_by, deleted_at, operation_id)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(entity_id) DO UPDATE SET
                deleted_by = excluded.deleted_by,
                deleted_at = excluded.deleted_at,
                operation_id = excluded.operation_id
                WHERE excluded.deleted_at > tombstones.deleted_at
            "#,
        )
        .bind(tombstone.id.to_string())
        .bind(tombstone.entity_id.to_string())
        .bind(&tombstone.entity_type)
        .bind(tombstone.deleted_by.to_string())
        .bind(tombstone.deleted_at.to_rfc3339())
        .bind(tombstone.operation_id.to_string())
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        Ok(())
    }

    async fn create_tombstones(&self, tombstones: &[Tombstone]) -> DomainResult<()> {
        let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        for tombstone in tombstones {
             self.create_tombstone_with_tx(tombstone, &mut tx).await?;
        }
        tx.commit().await.map_err(DbError::from)?;
        Ok(())
    }
    
    async fn find_tombstones_by_entity_type(&self, entity_type: &str) -> DomainResult<Vec<Tombstone>> {
        let rows = sqlx::query!(
            r#"
            SELECT id, entity_id, entity_type, deleted_by, deleted_at, operation_id 
            FROM tombstones
            WHERE entity_type = ?
            "#,
            entity_type
        )
        .fetch_all(&self.pool)
        .await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        rows.into_iter()
            .map(|row| {
                 let id_str = row.id.as_ref()
                     .ok_or_else(|| DomainError::Internal("Tombstone ID is NULL in database".to_string()))?;
                 let id = Uuid::parse_str(id_str)
                    .map_err(|e| DomainError::Internal(format!("Tombstone ID parse error '{}': {}", id_str, e)))?;
                 let entity_id = Uuid::parse_str(&row.entity_id)
                    .map_err(|e| DomainError::Internal(format!("Tombstone entity_id parse error '{}': {}", row.entity_id, e)))?;
                 let deleted_by = Uuid::parse_str(&row.deleted_by)
                     .map_err(|e| DomainError::Internal(format!("Tombstone deleted_by parse error '{}': {}", row.deleted_by, e)))?;
                 let deleted_at = DateTime::parse_from_rfc3339(&row.deleted_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|e| DomainError::Internal(format!("Tombstone deleted_at parse error '{}': {}", row.deleted_at, e)))?;
                 let operation_id = Uuid::parse_str(&row.operation_id)
                    .map_err(|e| DomainError::Internal(format!("Tombstone operation_id parse error '{}': {}", row.operation_id, e)))?;
                 
                 Ok(Tombstone {
                    id,
                    entity_id,
                    entity_type: row.entity_type,
                    deleted_by,
                    deleted_at,
                    operation_id,
                })
            })
            .collect::<DomainResult<Vec<Tombstone>>>()
    }
    
    async fn find_tombstone_by_entity_id(&self, entity_id: Uuid) -> DomainResult<Option<Tombstone>> {
        let entity_id_str = entity_id.to_string();
        let row = sqlx::query!(
            r#"
            SELECT id, entity_id, entity_type, deleted_by, deleted_at, operation_id 
            FROM tombstones
            WHERE entity_id = ?
            "#,
            entity_id_str
        )
        .fetch_optional(&self.pool)
        .await.map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        match row {
            Some(r) => {
                 let result = (|| {
                     let id_str = r.id.as_ref()
                         .ok_or_else(|| DomainError::Internal("Tombstone ID is NULL in database".to_string()))?;
                     let id = Uuid::parse_str(id_str)
                        .map_err(|e| DomainError::Internal(format!("Tombstone ID parse error '{}': {}", id_str, e)))?;
                     let parsed_entity_id = Uuid::parse_str(&r.entity_id)?; 
                     let deleted_by = Uuid::parse_str(&r.deleted_by)?; 
                     let deleted_at = DateTime::parse_from_rfc3339(&r.deleted_at)?.with_timezone(&Utc);
                     let operation_id = Uuid::parse_str(&r.operation_id)?; 
                     Ok(Tombstone {
                         id,
                         entity_id: parsed_entity_id,
                         entity_type: r.entity_type,
                         deleted_by,
                         deleted_at,
                         operation_id,
                     })
                 })();
                 
                 result.map(Some).map_err(|e: Box<dyn std::error::Error + Send + Sync>| DomainError::Internal(format!("Tombstone parsing failed: {}", e)))
            },
            None => Ok(None),
        }
    }
}

impl SqliteTombstoneRepository {
    /// Create a new SQLite tombstone repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

/// SQLite implementation of the ChangeLogRepository
pub struct SqliteChangeLogRepository {
    pool: Pool<Sqlite>,
}

#[async_trait]
impl ChangeLogRepository for SqliteChangeLogRepository {
    async fn create_change_log(&self, change_log: &ChangeLogEntry) -> DomainResult<()> {
         let mut tx = self.pool.begin().await.map_err(DbError::from)?;
         match self.create_change_log_with_tx(change_log, &mut tx).await {
             Ok(_) => {
                 tx.commit().await.map_err(DbError::from)?;
                 Ok(())
             },
             Err(e) => {
                 let _ = tx.rollback().await;
                 Err(e)
             }
         }
    }

    async fn create_change_log_with_tx(
        &self, 
        change_log: &ChangeLogEntry, 
        tx: &mut Transaction<'_, Sqlite>
    ) -> DomainResult<()> {
        // Use the new as_str method
        let operation_type_str = change_log.operation_type.as_str();
        let user_id_str = change_log.user_id.to_string();
        let entity_id_str = change_log.entity_id.to_string();
        let timestamp_str = change_log.timestamp.to_rfc3339();
        let device_id_str = change_log.device_id.map(|id| id.to_string());
        let sync_batch_id_str = change_log.sync_batch_id.as_ref().map(|id| id.to_string());
        let processed_at_str = change_log.processed_at.map(|dt| dt.to_rfc3339());
        // Create binding for operation_id string
        let operation_id_str = change_log.operation_id.to_string();
        
        sqlx::query!(
            r#"
            INSERT INTO change_log (
                operation_id, entity_table, entity_id, operation_type, 
                field_name, old_value, new_value, timestamp, user_id, 
                device_id, sync_batch_id, processed_at, sync_error
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            // Use the bound variable
            operation_id_str,
            change_log.entity_table,
            entity_id_str,
            operation_type_str,
            change_log.field_name,
            change_log.old_value,
            change_log.new_value,
            timestamp_str,
            user_id_str,
            device_id_str,
            sync_batch_id_str,
            processed_at_str,
            change_log.sync_error
        )
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Database(DbError::from(e)))?;
        
        Ok(())
    }

    async fn create_change_logs(&self, change_logs: &[ChangeLogEntry]) -> DomainResult<()> {
         let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
         for change_log in change_logs {
             self.create_change_log_with_tx(change_log, &mut tx).await?;
         }
         tx.commit().await.map_err(DbError::from)?;
         Ok(())
    }

    async fn find_change_logs_by_entity(&self, entity_table: &str, entity_id: &str) -> DomainResult<Vec<ChangeLogEntry>> {
         let rows = sqlx::query!(
             r#"
             SELECT 
                 operation_id, entity_table, entity_id, operation_type, 
                 field_name, old_value, new_value, timestamp, user_id, 
                 device_id, sync_batch_id, processed_at, sync_error
             FROM change_log
             WHERE entity_table = ? AND entity_id = ?
             ORDER BY timestamp ASC
             "#,
             entity_table,
             entity_id
         )
         .fetch_all(&self.pool)
         .await
         .map_err(|e| DomainError::Database(DbError::from(e)))?;

         rows.into_iter().map(|row| {
             let result = (|| {
                // Handle Option<String> for operation_id
                let operation_id_str = row.operation_id.as_ref()
                    .ok_or_else(|| DomainError::Internal("ChangeLog operation_id is NULL in database".to_string()))?;
                let operation_id = Uuid::parse_str(operation_id_str)?; 
                let parsed_entity_id = Uuid::parse_str(&row.entity_id)?; 
                // Use the new from_str method and handle the Option
                let operation_type = ChangeOperationType::from_str(&row.operation_type)
                    .ok_or_else(|| DomainError::Internal(format!("Invalid operation_type in DB: {}", row.operation_type)))?; 
                let timestamp = DateTime::parse_from_rfc3339(&row.timestamp)?.with_timezone(&Utc);
                let user_id = Uuid::parse_str(&row.user_id)?; 
                let device_id = row.device_id.as_ref().map(|s| Uuid::parse_str(s.as_str())).transpose()
                    .map_err(|e| DomainError::InvalidUuid(format!("Invalid device_id UUID: {:?}", e)))?;
                let sync_batch_id = row.sync_batch_id.as_ref().map(|s| Uuid::parse_str(s.as_str())).transpose()
                    .map_err(|e| DomainError::InvalidUuid(format!("Invalid sync_batch_id UUID: {:?}", e)))?;
                let processed_at = row.processed_at.as_ref().map(|s| DateTime::parse_from_rfc3339(s.as_str())).transpose()
                    .map_err(|e| DomainError::Internal(format!("Invalid processed_at format: {:?}", e)))?
                    .map(|dt| dt.with_timezone(&Utc));

                 Ok(ChangeLogEntry {
                     operation_id,
                     entity_table: row.entity_table,
                     entity_id: parsed_entity_id,
                     operation_type,
                     field_name: row.field_name,
                     old_value: row.old_value,
                     new_value: row.new_value,
                     timestamp,
                     user_id,
                     device_id,
                     sync_batch_id: row.sync_batch_id.clone(),
                     processed_at,
                     sync_error: row.sync_error,
                 })
             })();
             result.map_err(|e: Box<dyn std::error::Error + Send + Sync>| DomainError::Internal(format!("ChangeLog parsing failed: {}", e)))
         }).collect::<DomainResult<Vec<ChangeLogEntry>>>()
    }
}

impl SqliteChangeLogRepository {
    /// Create a new SQLite change log repository
    pub fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}