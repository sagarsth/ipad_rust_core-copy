use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;
use sqlx::SqlitePool;
use std::env;

use crate::domains::core::file_storage_service::FileStorageService;
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::types::{CompressionStatus, MediaDocument, SourceOfChange};
use crate::errors::{DbError, DomainError, ServiceError, ServiceResult};
use super::repository::CompressionRepository;
use super::types::{
    CompressionResult,
    CompressionQueueStatus, CompressionPriority, CompressionStats, CompressionConfig
};
use super::compressors::{
    Compressor,
    image_compressor::ImageCompressor,
    pdf_compressor::PdfCompressor, 
    office_compressor::OfficeCompressor,
    generic_compressor::GenericCompressor,
    get_extension
};
use crate::domains::core::repository::FindById;

// Maximum file size (bytes) we are willing to load fully into memory for compression.
// Default: 2GB, can be overridden by env var `MAX_IN_MEMORY_COMPRESSION_BYTES`.
fn max_in_memory_bytes() -> i64 {
    env::var("MAX_IN_MEMORY_COMPRESSION_BYTES")
        .ok()
        .and_then(|val| val.parse::<i64>().ok())
        .unwrap_or(2048 * 1024 * 1024) // 2GB
}

#[async_trait]
pub trait CompressionService: Send + Sync {
    /// Compress a document and update its status
    async fn compress_document(
        &self,
        document_id: Uuid,
        config: Option<CompressionConfig>,
    ) -> ServiceResult<CompressionResult>;
    
    /// Get current compression queue status
    async fn get_compression_queue_status(&self) -> ServiceResult<CompressionQueueStatus>;
    
    /// Queue a document for compression
    async fn queue_document_for_compression(
        &self,
        document_id: Uuid,
        priority: CompressionPriority,
    ) -> ServiceResult<()>;
    
    /// Cancel pending compression for a document
    async fn cancel_compression(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<bool>;
    
    /// Get compression statistics
    async fn get_compression_stats(&self) -> ServiceResult<CompressionStats>;
    
    /// Get compression status for a document
    async fn get_document_compression_status(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<CompressionStatus>;
    
    /// Update compression priority for a document
    async fn update_compression_priority(
        &self,
        document_id: Uuid,
        priority: CompressionPriority,
    ) -> ServiceResult<bool>;
    
    /// Bulk update compression priorities
    async fn bulk_update_compression_priority(
        &self,
        document_ids: &[Uuid],
        priority: CompressionPriority,
    ) -> ServiceResult<u64>;

    /// Check if document is currently in use
    async fn is_document_in_use(&self, document_id: Uuid) -> ServiceResult<bool>;
}

pub struct CompressionServiceImpl {
    pool: SqlitePool,
    compression_repo: Arc<dyn CompressionRepository>,
    file_storage_service: Arc<dyn FileStorageService>,
    media_doc_repo: Arc<dyn MediaDocumentRepository>,
    compressors: Vec<Box<dyn Compressor>>,
}

impl CompressionServiceImpl {
    pub fn new(
        pool: SqlitePool,
        compression_repo: Arc<dyn CompressionRepository>,
        file_storage_service: Arc<dyn FileStorageService>,
        media_doc_repo: Arc<dyn MediaDocumentRepository>,
        ghostscript_path: Option<String>,
    ) -> Self {
        // Initialize compressors
        let mut compressors: Vec<Box<dyn Compressor>> = Vec::new();
        
        // Add specialized compressors
        compressors.push(Box::new(ImageCompressor));
        compressors.push(Box::new(PdfCompressor::new(ghostscript_path)));
        compressors.push(Box::new(OfficeCompressor::new()));
        
        // Add generic compressor as fallback
        compressors.push(Box::new(GenericCompressor));
        
        Self {
            pool,
            compression_repo,
            file_storage_service,
            media_doc_repo,
            compressors,
        }
    }
    
    /// Find the appropriate compressor for a file
    async fn find_compressor(
        &self,
        mime_type: &str,
        extension: Option<&str>,
    ) -> &dyn Compressor {
        for compressor in &self.compressors {
            if compressor.can_handle(mime_type, extension).await {
                return compressor.as_ref();
            }
        }
        
        // We should never reach here as GenericCompressor handles all files,
        // but just in case, return the last compressor (which is GenericCompressor)
        self.compressors.last().unwrap().as_ref()
    }

    /// Central implementation of document-in-use check
    pub async fn is_document_in_use(&self, document_id: Uuid) -> Result<bool, ServiceError> {
        let doc_id_str = document_id.to_string();
        let result = sqlx::query!(
            r#"
            SELECT EXISTS(
                SELECT 1 
                FROM active_file_usage 
                WHERE 
                    document_id = ? AND 
                    last_active_at > datetime('now', '-5 minutes')
            ) as in_use
            "#,
            doc_id_str
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        Ok(result.in_use == 1)
    }
    
    /// Mark document with error when compression fails
    async fn mark_document_with_error(
        &self, 
        document_id: Uuid, 
        error_type: &str, 
        error_message: &str
    ) -> Result<(), ServiceError> {
        let doc_id_str = document_id.to_string();
        sqlx::query!(
            r#"
            UPDATE media_documents
            SET 
                has_error = 1,
                error_type = ?,
                error_message = ?,
                compression_status = 'error',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?
            "#,
            error_type,
            error_message,
            doc_id_str
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        Ok(())
    }
    
    /// Clear document error state
    async fn clear_document_error(
        &self, 
        document_id: Uuid
    ) -> Result<(), ServiceError> {
        let doc_id_str = document_id.to_string();
        sqlx::query!(
            r#"
            UPDATE media_documents
            SET 
                has_error = 0,
                error_type = NULL,
                error_message = NULL,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?
            "#,
            doc_id_str
        )
        .execute(&self.pool)
        .await
        .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        Ok(())
    }
}

#[async_trait]
impl CompressionService for CompressionServiceImpl {
    async fn compress_document(
        &self,
        document_id: Uuid,
        config: Option<CompressionConfig>,
    ) -> ServiceResult<CompressionResult> {
        // Start timing the operation
        let start_time = Instant::now();
        
        println!("🔄 [COMPRESSION_SERVICE] Starting compression for document {}", document_id);
        
        // Check if document is in use
        if self.is_document_in_use(document_id).await? {
            println!("⏸️ [COMPRESSION_SERVICE] Document {} is in use, cannot compress", document_id);
            return Err(ServiceError::Ui(
                "Cannot compress document that is currently in use".to_string()
            ));
        }
        
        // 1. Get document details
        println!("📄 [COMPRESSION_SERVICE] Fetching document details for {}", document_id);
        let document = FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| {
                println!("❌ [COMPRESSION_SERVICE] Failed to find document {}: {:?}", document_id, e);
                ServiceError::Domain(e)
            })?;
        
        // Skip compression for documents that originated from sync
        if document.source_of_change == SourceOfChange::Sync {
            println!("⏭️ [COMPRESSION_SERVICE] Skipping compression for synced document {}", document_id);
            return Err(ServiceError::Ui("Skipping compression for synced document".to_string()));
        }
        
        // 2. Check for error state or known issues
        // Skip documents with existing errors
        if document.has_error.unwrap_or(0) == 1 {
            println!("⚠️ [COMPRESSION_SERVICE] Document {} has existing error, skipping", document_id);
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom(&format!("Document has an error: {}", 
                    document.error_message.unwrap_or_else(|| "Unknown error".to_string())))
            )));
        }
        
        // Skip documents with ERROR file path
        if document.file_path == "ERROR" {
            println!("❌ [COMPRESSION_SERVICE] Document {} has invalid file path", document_id);
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Document file path is invalid")
            )));
        }
        
        // 3. Check if already compressed
        if document.compression_status == CompressionStatus::Completed.as_str() {
            println!("✅ [COMPRESSION_SERVICE] Document {} already compressed", document_id);
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Document is already compressed")
            )));
        }
        
        println!("🧹 [COMPRESSION_SERVICE] Clearing any previous errors for document {}", document_id);
        // 4. Clear any previous compression errors
        self.clear_document_error(document_id).await?;
        
        println!("📊 [COMPRESSION_SERVICE] Updating status to in_progress for document {}", document_id);
        // 5. Update document status to InProgress
        self.media_doc_repo.update_compression_status(
            document_id, 
            CompressionStatus::InProgress,
            None,
            None
        ).await.map_err(|e| {
            println!("❌ [COMPRESSION_SERVICE] Failed to update status for document {}: {:?}", document_id, e);
            ServiceError::Domain(e)
        })?;
        
        // 6. Get document type details to determine compression settings
        let config = config.unwrap_or_else(|| CompressionConfig::default());

        // BEFORE loading the entire file, check its size to avoid RAM spikes
        println!("📏 [COMPRESSION_SERVICE] Checking file size for document {}", document_id);
        let original_size_on_disk = match self.file_storage_service.get_file_size(&document.file_path).await {
            Ok(sz) => {
                println!("📐 [COMPRESSION_SERVICE] File size for document {}: {} bytes", document_id, sz);
                sz as i64
            },
            Err(e) => {
                // If we cannot stat file, fall back to reading (will likely error anyway)
                println!("⚠️ [COMPRESSION_SERVICE] Failed to stat file size for document {}, will attempt read: {:?}", document_id, e);
                0
            }
        };

        if original_size_on_disk > max_in_memory_bytes() {
            println!("📦 [COMPRESSION_SERVICE] File too large for in-memory compression: {} bytes > {} bytes", 
                     original_size_on_disk, max_in_memory_bytes());
            // Mark as skipped, update queue and stats, then return early.
            self.media_doc_repo.update_compression_status(
                document_id,
                CompressionStatus::Skipped,
                None,
                None
            ).await.map_err(|e| ServiceError::Domain(e))?;

            if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
                .map_err(|e| ServiceError::Domain(e))? {
                self.compression_repo.update_queue_entry_status(
                    queue_entry.id,
                    "skipped",
                    Some("File too large to compress in-memory")
                ).await.map_err(|e| ServiceError::Domain(e))?;
            }

            // Update global stats for skipped
            let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
            self.compression_repo.update_stats_for_skipped(&mut tx).await
                .map_err(|e| ServiceError::Domain(e))?;
            tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;

            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("File too large to compress in-memory")
            )));
        }

        // 7. Read the original file (safe size)
        println!("📖 [COMPRESSION_SERVICE] Reading file data for document {}", document_id);
        let file_data = match self.file_storage_service.get_file_data(&document.file_path).await {
            Ok(data) => {
                println!("✅ [COMPRESSION_SERVICE] Successfully read {} bytes for document {}", data.len(), document_id);
                data
            },
            Err(e) => {
                // Mark document with error and propagate failure
                let error_message = format!("Failed to read file: {:?}", e);
                println!("❌ [COMPRESSION_SERVICE] {}", error_message);
                self.mark_document_with_error(document_id, "storage_failure", &error_message).await?;
                
                return Err(ServiceError::Domain(DomainError::Internal(error_message)));
            }
        };
        
        let original_size = file_data.len() as i64;
        println!("📊 [COMPRESSION_SERVICE] Original file size: {} bytes for document {}", original_size, document_id);
        
        // 8. Determine MIME type and extension
        let mime_type = document.mime_type.as_str();
        let extension = get_extension(&document.original_filename);
        println!("🔍 [COMPRESSION_SERVICE] MIME type: {}, extension: {:?} for document {}", 
                 mime_type, extension, document_id);
        
        // 9. Select appropriate compressor
        let compressor = self.find_compressor(mime_type, extension).await;
        println!("🗜️ [COMPRESSION_SERVICE] Selected compressor for document {}", document_id);
        
        // 10. Compress the file
        println!("🔧 [COMPRESSION_SERVICE] Starting actual compression for document {}", document_id);
        let compressed_data = match compressor.compress(
            file_data, 
            config.method,
            config.quality_level
        ).await {
            Ok(data) => {
                println!("✅ [COMPRESSION_SERVICE] Compression successful: {} bytes -> {} bytes for document {}", 
                         original_size, data.len(), document_id);
                data
            },
            Err(e) => {
                // Mark document with error and propagate failure
                let error_message = format!("Compression failed: {:?}", e);
                println!("❌ [COMPRESSION_SERVICE] {}", error_message);
                self.mark_document_with_error(document_id, "compression_failure", &error_message).await?;
                
                // Update queue status
                if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
                    .map_err(|e| ServiceError::Domain(e))? 
                {
                    self.compression_repo.update_queue_entry_status(
                        queue_entry.id, 
                        "failed", 
                        Some(&error_message)
                    ).await.map_err(|e| ServiceError::Domain(e))?;
                }
                
                // Update stats for failed compression
                let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                if let Err(stats_err) = self.compression_repo.update_stats_for_failed(&mut tx).await {
                    eprintln!("Failed to update stats for failed compression: {:?}", stats_err);
                    let _ = tx.rollback().await;
                } else {
                    tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                }
                
                return Err(ServiceError::Domain(DomainError::Internal(error_message)));
            }
        };
        
        let compressed_size = compressed_data.len() as i64;
        
        // 11. Determine entity type and ID for file storage
        let entity_type = document.related_table.as_str();
        let entity_id = if let Some(related_id) = document.related_id {
            related_id.to_string()
        } else if let Some(temp_id) = document.temp_related_id {
            temp_id.to_string()
        } else {
            return Err(ServiceError::Domain(DomainError::Internal(
                "Document missing both related_id and temp_related_id".to_string()
            )));
        };
        
        // Create compressed filename with suffix
        let file_stem = Path::new(&document.original_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("compressed");
            
        let file_ext = Path::new(&document.original_filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
            
        let compressed_filename = if file_ext.is_empty() {
            format!("{}_compressed", file_stem)
        } else {
            format!("{}_compressed.{}", file_stem, file_ext)
        };
        
        // Save compressed file
        println!("💾 [COMPRESSION_SERVICE] Saving compressed file for document {}", document_id);
        let (compressed_path, _) = match self.file_storage_service
            .save_file(
                compressed_data, 
                entity_type, 
                &entity_id,
                &compressed_filename
            ).await {
                Ok(result) => {
                    println!("✅ [COMPRESSION_SERVICE] Saved compressed file: {} for document {}", result.0, document_id);
                    result
                },
                Err(e) => {
                    // Mark document with error on file save failure
                    let error_message = format!("Failed to save compressed file: {:?}", e);
                    println!("❌ [COMPRESSION_SERVICE] {}", error_message);
                    self.mark_document_with_error(document_id, "storage_failure", &error_message).await?;
                    
                    // Update document status
                    self.media_doc_repo.update_compression_status(
                        document_id, 
                        CompressionStatus::Failed, 
                        None,
                        None
                    ).await.map_err(|e| ServiceError::Domain(e))?;
                    
                    return Err(ServiceError::Domain(DomainError::Internal(error_message)));
                }
            };
        
        println!("🔄 [COMPRESSION_SERVICE] Starting database transaction for document {}", document_id);
        // 13. Start a transaction for updating document and stats
        let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 14. Update document status
        println!("📝 [COMPRESSION_SERVICE] Updating document status to completed for document {}", document_id);
        self.media_doc_repo.update_compression_status(
            document_id, 
            CompressionStatus::Completed, 
            Some(&compressed_path),
            Some(compressed_size)
        ).await.map_err(|e| {
            println!("❌ [COMPRESSION_SERVICE] Failed to update document status for {}: {:?}", document_id, e);
            ServiceError::Domain(e)
        })?;
        
        // 15. Update compression stats
        println!("📊 [COMPRESSION_SERVICE] Updating compression stats for document {}", document_id);
        self.compression_repo.update_stats_after_compression(
            original_size,
            compressed_size,
            &mut tx
        ).await.map_err(|e| ServiceError::Domain(e))?;
        
        // 16. Update queue entry if exists
        if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
            .map_err(|e| ServiceError::Domain(e))? 
        {
            println!("📋 [COMPRESSION_SERVICE] Updating queue entry to completed for document {}", document_id);
            self.compression_repo.update_queue_entry_status(
                queue_entry.id, 
                "completed", 
                None
            ).await.map_err(|e| ServiceError::Domain(e))?;
        }
        
        // 17. Commit transaction
        println!("💾 [COMPRESSION_SERVICE] Committing transaction for document {}", document_id);
        tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // Calculate metrics
        let space_saved_bytes = original_size - compressed_size;
        let space_saved_percentage = if original_size > 0 {
            (space_saved_bytes as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };
        
        let duration_ms = start_time.elapsed().as_millis() as i64;
        
        println!("🎉 [COMPRESSION_SERVICE] Compression completed for document {} in {}ms", document_id, duration_ms);
        
        // 18. Return result
        Ok(CompressionResult {
            document_id,
            original_size,
            compressed_size,
            compressed_file_path: compressed_path,
            space_saved_bytes,
            space_saved_percentage,
            method_used: config.method,
            quality_level: config.quality_level,
            duration_ms,
        })
    }
    
    async fn get_compression_queue_status(&self) -> ServiceResult<CompressionQueueStatus> {
        self.compression_repo.get_queue_status().await
            .map_err(|e| ServiceError::Domain(e))
    }
    
    async fn queue_document_for_compression(
        &self,
        document_id: Uuid,
        priority: CompressionPriority,
    ) -> ServiceResult<()> {
        println!("🗜️ [COMPRESSION] Starting queue_document_for_compression for {}", document_id);
        
        // Get document to make sure it exists
        let document = FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| {
                println!("❌ [COMPRESSION] Failed to find document {}: {:?}", document_id, e);
                ServiceError::Domain(e)
            })?;

        println!("✅ [COMPRESSION] Found document: {}", document.original_filename);
        println!("📊 [COMPRESSION] Document details:");
        println!("   - Source of change: {:?}", document.source_of_change);
        println!("   - Compression status: {}", document.compression_status);
        println!("   - Has error: {:?}", document.has_error);
        println!("   - Size: {} bytes", document.size_bytes);

        // Do not queue documents that came from sync
        if document.source_of_change == SourceOfChange::Sync {
            println!("⏭️ [COMPRESSION] Skipping compression for synced document: {}", document_id);
            // Update compression_status to SKIPPED to prevent reprocessing
            self.media_doc_repo.update_compression_status(
                document_id, 
                CompressionStatus::Skipped, 
                None, 
                None
            ).await.map_err(|e| ServiceError::Domain(e))?;
            return Ok(());
        }
            
        // Don't queue if already compressed or has error
        if document.has_error.unwrap_or(0) == 1 {
            println!("⚠️ [COMPRESSION] Document has error, not queuing: {}", document_id);
            return Ok(());  // Silently ignore error documents
        }
        
        if document.compression_status == CompressionStatus::Completed.as_str() || 
           document.compression_status == CompressionStatus::Skipped.as_str() {
            println!("⏭️ [COMPRESSION] Document already processed ({}), skipping: {}", document.compression_status, document_id);
            return Ok(());  // Already processed
        }
        
        println!("🔄 [COMPRESSION] Queuing document for compression with priority: {:?}", priority);
        
        // Queue the document
        self.compression_repo
            .queue_document(document_id, priority.into())
            .await
            .map_err(|e| {
                println!("❌ [COMPRESSION] Failed to queue document {}: {:?}", document_id, e);
                ServiceError::Domain(e)
            })?;
            
        println!("✅ [COMPRESSION] Successfully queued document {} for compression", document_id);
        Ok(())
    }
    
    async fn cancel_compression(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<bool> {
        // If document is in the 'processing' state, update its status to 'pending'
        if self.get_document_compression_status(document_id).await? == CompressionStatus::InProgress {
            self.media_doc_repo.update_compression_status(
                document_id,
                CompressionStatus::Pending,
                None,
                None
            ).await.map_err(|e| ServiceError::Domain(e))?;
        }
        
        // Remove from queue
        self.compression_repo
            .remove_from_queue(document_id)
            .await
            .map_err(|e| ServiceError::Domain(e))
    }
    
    async fn get_compression_stats(&self) -> ServiceResult<CompressionStats> {
        self.compression_repo
            .get_compression_stats()
            .await
            .map_err(|e| ServiceError::Domain(e))
    }
    
    async fn get_document_compression_status(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<CompressionStatus> {
        let document = FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| ServiceError::Domain(e))?;
        
        // Convert string status to enum
        match document.compression_status.as_str() {
            "pending" => Ok(CompressionStatus::Pending),
            "in_progress" => Ok(CompressionStatus::InProgress),
            "completed" => Ok(CompressionStatus::Completed),
            "skipped" => Ok(CompressionStatus::Skipped),
            "failed" => Ok(CompressionStatus::Failed),
            "error" => Ok(CompressionStatus::Failed), // Map "error" to Failed
            _ => Ok(CompressionStatus::Pending) // Default to Pending for unknown states
        }
    }
    
    async fn update_compression_priority(
        &self,
        document_id: Uuid,
        priority: CompressionPriority,
    ) -> ServiceResult<bool> {
        self.compression_repo
            .update_compression_priority(document_id, priority.into())
            .await
            .map_err(|e| ServiceError::Domain(e))
    }
    
    async fn bulk_update_compression_priority(
        &self,
        document_ids: &[Uuid],
        priority: CompressionPriority,
    ) -> ServiceResult<u64> {
        self.compression_repo
            .bulk_update_compression_priority(document_ids, priority.into())
            .await
            .map_err(|e| ServiceError::Domain(e))
    }

    /// Check if document is currently in use
    async fn is_document_in_use(&self, document_id: Uuid) -> ServiceResult<bool> {
        self.is_document_in_use(document_id).await
    }
}