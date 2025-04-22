 //! Repository for compression queue and stats

 use async_trait::async_trait;
 use chrono::Utc;
 use sqlx::{Pool, Sqlite, Transaction};
 use uuid::Uuid;
 use std::str::FromStr;
 
 use crate::errors::{DbError, DomainError, DomainResult};
 use super::types::{
     CompressionQueueEntry, CompressionQueueStatus, CompressionStats, 
     CompressionMethod
 };
 
 #[async_trait]
 pub trait CompressionRepository: Send + Sync {
     /// Queue a document for compression
     async fn queue_document(
         &self,
         document_id: Uuid,
         priority: i32,
     ) -> DomainResult<CompressionQueueEntry>;
     
     /// Get the next document for compression
     async fn get_next_document_for_compression(&self) -> DomainResult<Option<CompressionQueueEntry>>;
     
     /// Update the status of a queued document
     async fn update_queue_entry_status(
         &self,
         queue_id: Uuid,
         status: &str,
         error_message: Option<&str>,
     ) -> DomainResult<()>;
     
     /// Update the compression priority for a document
     async fn update_compression_priority(
         &self,
         document_id: Uuid,
         priority: i32,
     ) -> DomainResult<bool>;
     
     /// Get the status of the compression queue
     async fn get_queue_status(&self) -> DomainResult<CompressionQueueStatus>;
     
     /// Get compression statistics
     async fn get_compression_stats(&self) -> DomainResult<CompressionStats>;
     
     /// Update compression statistics after successful compression
     async fn update_stats_after_compression(
         &self,
         original_size: i64,
         compressed_size: i64,
         tx: &mut Transaction<'_, Sqlite>,
     ) -> DomainResult<()>;
     
     /// Update stats when a compression job is skipped
     async fn update_stats_for_skipped(
         &self,
         tx: &mut Transaction<'_, Sqlite>,
     ) -> DomainResult<()>;
     
     /// Update stats when a compression job fails
     async fn update_stats_for_failed(
         &self,
         tx: &mut Transaction<'_, Sqlite>,
     ) -> DomainResult<()>;
     
     /// Get a queue entry by document ID
     async fn get_queue_entry_by_document_id(
         &self,
         document_id: Uuid,
     ) -> DomainResult<Option<CompressionQueueEntry>>;
     
     /// Remove a document from the compression queue
     async fn remove_from_queue(
         &self,
         document_id: Uuid,
     ) -> DomainResult<bool>;
     
     /// Bulk update compression priorities
     async fn bulk_update_compression_priority(
         &self,
         document_ids: &[Uuid],
         priority: i32,
     ) -> DomainResult<u64>;
 }
 
 pub struct SqliteCompressionRepository {
     pool: Pool<Sqlite>,
 }
 
 impl SqliteCompressionRepository {
     pub fn new(pool: Pool<Sqlite>) -> Self {
         Self { pool }
     }
 }
 
 #[async_trait]
 impl CompressionRepository for SqliteCompressionRepository {
     async fn queue_document(
         &self,
         document_id: Uuid,
         priority: i32,
     ) -> DomainResult<CompressionQueueEntry> {
         let mut tx = self.pool.begin().await.map_err(DbError::from)?;
         
         // Check if document is already in queue
         let document_id_str_check = document_id.to_string();
         let existing = sqlx::query!(
             "SELECT id FROM compression_queue WHERE document_id = ?",
             document_id_str_check
         )
         .fetch_optional(&mut *tx)
         .await
         .map_err(DbError::from)?;
         
         if let Some(row) = existing {
             let updated_at_str = Utc::now().to_rfc3339();
             let row_id_str = row.id.as_deref()
                 .ok_or_else(|| DomainError::Internal("Queue entry ID missing in existing row".to_string()))?;
             // Update priority if already queued
             sqlx::query!(
                 "UPDATE compression_queue SET 
                  priority = ?, 
                  updated_at = ? 
                  WHERE id = ?",
                 priority,
                 updated_at_str,
                 row_id_str
             )
             .execute(&mut *tx)
             .await
             .map_err(DbError::from)?;
             
             let queue_id = Uuid::parse_str(row_id_str)
                 .map_err(|_| DomainError::InvalidUuid(row_id_str.to_string()))?;
                 
             let entry = self.get_queue_entry_internal(queue_id, &mut tx).await?;
             tx.commit().await.map_err(DbError::from)?;
             return Ok(entry);
         }
         
         // Add new queue entry
         let queue_id = Uuid::new_v4();
         let now_str = Utc::now().to_rfc3339();
         let queue_id_str = queue_id.to_string();
         let document_id_str = document_id.to_string();
         
         // Use sqlx::query (unchecked) to bypass persistent prepare analysis error
         sqlx::query(
             "INSERT INTO compression_queue
              (id, document_id, priority, attempts, status, created_at, updated_at)
              VALUES (?, ?, ?, 0, 'pending', ?, ?)"
         )
         .bind(queue_id_str)
         .bind(document_id_str)
         .bind(priority)
         .bind(&now_str)
         .bind(&now_str)
         .execute(&mut *tx)
         .await
         .map_err(DbError::from)?;
         
         // Update stats
         sqlx::query!(
             "UPDATE compression_stats 
              SET total_files_pending = total_files_pending + 1,
                  updated_at = ?
              WHERE id = 'global'",
             now_str
         )
         .execute(&mut *tx)
         .await
         .map_err(DbError::from)?;
         
         let entry = self.get_queue_entry_internal(queue_id, &mut tx).await?;
         tx.commit().await.map_err(DbError::from)?;
         
         Ok(entry)
     }
     
     async fn get_next_document_for_compression(&self) -> DomainResult<Option<CompressionQueueEntry>> {
         let mut tx = self.pool.begin().await.map_err(DbError::from)?;
         
         let row = sqlx::query!(
             "SELECT id, document_id, priority, attempts, status, created_at, updated_at, error_message
              FROM compression_queue
              WHERE status = 'pending'
              ORDER BY priority DESC, created_at ASC
              LIMIT 1"
         )
         .fetch_optional(&mut *tx)
         .await
         .map_err(DbError::from)?;
         
         let entry = match row {
             Some(row) => {
                  let row_id_str = row.id.as_deref()
                      .ok_or_else(|| DomainError::Internal("Queue entry ID missing when fetching next".to_string()))?;
                  let queue_id = Uuid::parse_str(row_id_str)
                     .map_err(|_| DomainError::InvalidUuid(row_id_str.to_string()))?;
                 
                 let updated_at_str = Utc::now().to_rfc3339();
                 // Mark as processing
                 sqlx::query!(
                     "UPDATE compression_queue 
                      SET status = 'processing', 
                          attempts = attempts + 1,
                          updated_at = ?
                      WHERE id = ?",
                     updated_at_str,
                     row_id_str
                 )
                 .execute(&mut *tx)
                 .await
                 .map_err(DbError::from)?;
                 
                 self.get_queue_entry_internal(queue_id, &mut tx).await?
             }
             None => {
                 tx.commit().await.map_err(DbError::from)?;
                 return Ok(None);
             }
         };
         
         tx.commit().await.map_err(DbError::from)?;
         Ok(Some(entry))
     }
     
     async fn update_queue_entry_status(
         &self,
         queue_id: Uuid,
         status: &str,
         error_message: Option<&str>,
     ) -> DomainResult<()> {
         let updated_at_str = Utc::now().to_rfc3339();
         let queue_id_str = queue_id.to_string();
         sqlx::query!(
             "UPDATE compression_queue 
              SET status = ?, 
                  error_message = ?,
                  updated_at = ?
              WHERE id = ?",
             status,
             error_message,
             updated_at_str,
             queue_id_str
         )
         .execute(&self.pool)
         .await
         .map_err(DbError::from)?;
         
         Ok(())
     }
     
     async fn update_compression_priority(
         &self,
         document_id: Uuid,
         priority: i32,
     ) -> DomainResult<bool> {
         let updated_at_str = Utc::now().to_rfc3339();
         let document_id_str = document_id.to_string();
         let result = sqlx::query!(
             "UPDATE compression_queue 
              SET priority = ?, 
                  updated_at = ?
              WHERE document_id = ? AND status = 'pending'",
             priority,
             updated_at_str,
             document_id_str
         )
         .execute(&self.pool)
         .await
         .map_err(DbError::from)?;
         
         Ok(result.rows_affected() > 0)
     }
     
     async fn get_queue_status(&self) -> DomainResult<CompressionQueueStatus> {
         let row = sqlx::query!(
             "SELECT 
                 SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END) as pending_count,
                 SUM(CASE WHEN status = 'processing' THEN 1 ELSE 0 END) as processing_count,
                 SUM(CASE WHEN status = 'completed' THEN 1 ELSE 0 END) as completed_count,
                 SUM(CASE WHEN status = 'failed' THEN 1 ELSE 0 END) as failed_count,
                 SUM(CASE WHEN status = 'skipped' THEN 1 ELSE 0 END) as skipped_count
              FROM compression_queue"
         )
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;
         
         Ok(CompressionQueueStatus {
             pending_count: row.pending_count.unwrap_or(0),
             processing_count: row.processing_count.unwrap_or(0),
             completed_count: row.completed_count.unwrap_or(0),
             failed_count: row.failed_count.unwrap_or(0),
             skipped_count: row.skipped_count.unwrap_or(0),
         })
     }
     
     async fn get_compression_stats(&self) -> DomainResult<CompressionStats> {
         let row = sqlx::query!(
             "SELECT 
                 total_original_size, total_compressed_size, space_saved,
                 compression_ratio, total_files_compressed, total_files_pending,
                 total_files_failed, total_files_skipped, last_compression_date, updated_at
              FROM compression_stats
              WHERE id = 'global'"
         )
         .fetch_one(&self.pool)
         .await
         .map_err(DbError::from)?;
         
         let last_compression_date = match &row.last_compression_date {
             Some(date_str) => Some(
                 chrono::DateTime::parse_from_rfc3339(date_str)
                     .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", date_str)))?
                     .with_timezone(&Utc)
             ),
             None => None,
         };
         
         let updated_at = chrono::DateTime::parse_from_rfc3339(&row.updated_at)
             .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", row.updated_at)))?
             .with_timezone(&Utc);
         
         Ok(CompressionStats {
             total_original_size: row.total_original_size.unwrap_or(0),
             total_compressed_size: row.total_compressed_size.unwrap_or(0),
             space_saved: row.space_saved.unwrap_or(0),
             compression_ratio: row.compression_ratio.unwrap_or(0.0),
             total_files_compressed: row.total_files_compressed.unwrap_or(0),
             total_files_pending: row.total_files_pending.unwrap_or(0),
             total_files_failed: row.total_files_failed.unwrap_or(0),
             total_files_skipped: row.total_files_skipped,
             last_compression_date,
             updated_at,
         })
     }
     
     async fn update_stats_after_compression(
         &self,
         original_size: i64,
         compressed_size: i64,
         tx: &mut Transaction<'_, Sqlite>,
     ) -> DomainResult<()> {
         let space_saved = original_size - compressed_size;
         let now_str = Utc::now().to_rfc3339();
         
         sqlx::query!(
             "UPDATE compression_stats SET
                 total_original_size = total_original_size + ?,
                 total_compressed_size = total_compressed_size + ?,
                 space_saved = space_saved + ?,
                 compression_ratio = CASE 
                     WHEN total_original_size + ? > 0 THEN 
                         ((space_saved + ?) * 100.0) / (total_original_size + ?)
                     ELSE 0 END,
                 total_files_compressed = total_files_compressed + 1,
                 total_files_pending = total_files_pending - 1,
                 last_compression_date = ?,
                 updated_at = ?
             WHERE id = 'global'",
             original_size,
             compressed_size,
             space_saved,
             original_size,
             space_saved,
             original_size,
             now_str,
             now_str
         )
         .execute(&mut **tx)
         .await
         .map_err(DbError::from)?;
         
         Ok(())
     }
     
     async fn update_stats_for_skipped(
         &self,
         tx: &mut Transaction<'_, Sqlite>,
     ) -> DomainResult<()> {
         let now_str = Utc::now().to_rfc3339();
         sqlx::query!(
             "UPDATE compression_stats SET
                 total_files_skipped = total_files_skipped + 1,
                 total_files_pending = total_files_pending - 1,
                 updated_at = ?
             WHERE id = 'global'",
             now_str
         )
         .execute(&mut **tx)
         .await
         .map_err(DbError::from)?;
         
         Ok(())
     }
     
     async fn update_stats_for_failed(
         &self,
         tx: &mut Transaction<'_, Sqlite>,
     ) -> DomainResult<()> {
         let now_str = Utc::now().to_rfc3339();
         sqlx::query!(
             "UPDATE compression_stats SET
                 total_files_failed = total_files_failed + 1,
                 total_files_pending = total_files_pending - 1,
                 updated_at = ?
             WHERE id = 'global'",
             now_str
         )
         .execute(&mut **tx)
         .await
         .map_err(DbError::from)?;
         
         Ok(())
     }
     
     async fn get_queue_entry_by_document_id(
         &self,
         document_id: Uuid,
     ) -> DomainResult<Option<CompressionQueueEntry>> {
         let document_id_str = document_id.to_string();
         let row = sqlx::query!(
             "SELECT id, document_id, priority, attempts, status, created_at, updated_at, error_message
              FROM compression_queue
              WHERE document_id = ?",
             document_id_str
         )
         .fetch_optional(&self.pool)
         .await
         .map_err(DbError::from)?;
         
         match row {
             Some(row) => {
                 let id_str = row.id.as_deref()
                     .ok_or_else(|| DomainError::Internal(format!("Queue entry ID missing for doc {}", document_id)))?;
                 let queue_id = Uuid::parse_str(id_str)
                     .map_err(|_| DomainError::InvalidUuid(id_str.to_string()))?;
                     
                 let document_id_uuid = Uuid::parse_str(&row.document_id)
                     .map_err(|_| DomainError::InvalidUuid(row.document_id.clone()))?;
                 
                 let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
                     .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", row.created_at)))?
                     .with_timezone(&Utc);
                 
                 let updated_at = chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                     .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", row.updated_at)))?
                     .with_timezone(&Utc);
                 
                 Ok(Some(CompressionQueueEntry {
                     id: queue_id,
                     document_id: document_id_uuid,
                     priority: row.priority.unwrap_or(0) as i32,
                     attempts: row.attempts.unwrap_or(0) as i32,
                     status: row.status.clone(),
                     created_at,
                     updated_at,
                     error_message: row.error_message,
                 }))
             },
             None => Ok(None),
         }
     }
     
     async fn remove_from_queue(
         &self,
         document_id: Uuid,
     ) -> DomainResult<bool> {
         let document_id_str = document_id.to_string();
         // Use sqlx::query (unchecked) to bypass persistent prepare analysis error
         let result = sqlx::query(
             "DELETE FROM compression_queue WHERE document_id = ?"
         )
         .bind(document_id_str)
         .execute(&self.pool)
         .await
         .map_err(DbError::from)?;
         
         Ok(result.rows_affected() > 0)
     }
     
     async fn bulk_update_compression_priority(
         &self,
         document_ids: &[Uuid],
         priority: i32,
     ) -> DomainResult<u64> {
         if document_ids.is_empty() {
             return Ok(0);
         }
         
         let mut affected = 0;
         let now = Utc::now().to_rfc3339();
         
         // Process in batches of 100
         for chunk in document_ids.chunks(100) {
             let placeholders = chunk.iter().map(|_| "?").collect::<Vec<_>>().join(",");
             let query_str = format!(
                 "UPDATE compression_queue 
                  SET priority = ?, updated_at = ? 
                  WHERE document_id IN ({}) AND status = 'pending'",
                 placeholders
             );
             
             let mut query = sqlx::query(&query_str);
             query = query.bind(priority);
             query = query.bind(&now);
             
             for id in chunk {
                 query = query.bind(id.to_string());
             }
             
             let result = query.execute(&self.pool).await.map_err(DbError::from)?;
             affected += result.rows_affected();
         }
         
         Ok(affected)
     }
 }
 
 // Internal helper methods
 impl SqliteCompressionRepository {
     async fn get_queue_entry_internal(
         &self,
         queue_id: Uuid,
         tx: &mut Transaction<'_, Sqlite>,
     ) -> DomainResult<CompressionQueueEntry> {
         let queue_id_str = queue_id.to_string();
         let row = sqlx::query!(
             "SELECT id, document_id, priority, attempts, status, created_at, updated_at, error_message
              FROM compression_queue
              WHERE id = ?",
             queue_id_str
         )
         .fetch_one(&mut **tx)
         .await
         .map_err(DbError::from)?;
         
         let document_id = Uuid::parse_str(&row.document_id)
             .map_err(|_| DomainError::InvalidUuid(row.document_id.clone()))?;
         
         let created_at = chrono::DateTime::parse_from_rfc3339(&row.created_at)
             .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", row.created_at)))?
             .with_timezone(&Utc);
         
         let updated_at = chrono::DateTime::parse_from_rfc3339(&row.updated_at)
             .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", row.updated_at)))?
             .with_timezone(&Utc);
         
         Ok(CompressionQueueEntry {
             id: queue_id,
             document_id,
             priority: row.priority.unwrap_or(0) as i32,
             attempts: row.attempts.unwrap_or(0) as i32,
             status: row.status.clone(),
             created_at,
             updated_at,
             error_message: row.error_message,
         })
     }
 }