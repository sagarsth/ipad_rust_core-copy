use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use uuid::Uuid;
use sqlx::{SqlitePool, Transaction, Sqlite};
use std::env;
use chrono::Utc;

use crate::domains::core::file_storage_service::FileStorageService;
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::types::{CompressionStatus, MediaDocument, SourceOfChange};
use crate::errors::{DbError, DomainError, ServiceError, ServiceResult};
use super::repository::CompressionRepository;
use super::types::{
    CompressionResult,
    CompressionQueueStatus, CompressionPriority, CompressionStats, CompressionConfig, CompressionMethod
};
use super::compressors::{
    Compressor,
    image_compressor::ImageCompressor,
    pdf_compressor::PdfCompressor, 
    office_compressor::OfficeCompressor,
    generic_compressor::GenericCompressor,
    get_extension,
    video_compressor::VideoCompressor,
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
    
    /// Queue a document for compression within an existing transaction
    async fn queue_document_for_compression_with_tx(
        &self,
        document_id: Uuid,
        priority: CompressionPriority,
        tx: &mut Transaction<'_, Sqlite>,
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
    
    /// Get document details for size-based timeout calculation
    async fn get_document_details(&self, document_id: Uuid) -> ServiceResult<Option<MediaDocument>>;

    /// Queue original file for safe deletion after successful compression
    async fn queue_original_for_safe_deletion(
        &self,
        document_id: Uuid,
        compressed_file_path: &str,
    ) -> ServiceResult<()>;

    /// Clean up stale documents and queue entries (automated maintenance)
    async fn cleanup_stale_documents(&self) -> ServiceResult<u64>;

    /// Reset stuck compression jobs (automated recovery)
    async fn reset_stuck_jobs(&self) -> ServiceResult<u64>;
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
        compressors.push(Box::new(VideoCompressor::new()));
        
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
                compression_status = 'failed',
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
        
        log::info!("Starting compression for document {}", document_id);
        
        // Check if document is in use
        if self.is_document_in_use(document_id).await? {
            log::debug!("Document {} is in use, cannot compress", document_id);
            return Err(ServiceError::Ui(
                "Cannot compress document that is currently in use".to_string()
            ));
        }
        
        // 1. Get document details
        log::debug!("Fetching document details for {}", document_id);
        let document = FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| {
                log::error!("Failed to find document {}: {:?}", document_id, e);
                ServiceError::Domain(e)
            })?;
        
        // Skip compression for documents that originated from sync
        if document.source_of_change == SourceOfChange::Sync {
            log::debug!("Skipping compression for synced document {}", document_id);
            return Err(ServiceError::Ui("Skipping compression for synced document".to_string()));
        }
        
        // 2. Check for error state or known issues
        // Skip documents with existing errors
        if document.has_error.unwrap_or(0) == 1 {
            log::warn!("Document {} has existing error, skipping compression", document_id);
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Document has an existing error")
            )));
        }
        
        // Skip documents with ERROR file path
        if document.file_path == "ERROR" {
            log::error!("Document {} has invalid file path", document_id);
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Document file path is invalid")
            )));
        }
        
        // 3. Check if already compressed
        if document.compression_status == CompressionStatus::Completed.as_str() {
            log::debug!("Document {} already compressed", document_id);
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Document is already compressed")
            )));
        }
        
        log::debug!("Clearing any previous errors for document {}", document_id);
        // 4. Clear any previous compression errors
        self.clear_document_error(document_id).await?;
        
        log::debug!("Updating status to processing for document {}", document_id);
        // 5. Update document status to Processing
        self.media_doc_repo.update_compression_status(
            document_id, 
            CompressionStatus::Processing,
            None,
            None
        ).await.map_err(|e| {
            log::error!("Failed to update status for document {}: {:?}", document_id, e);
            ServiceError::Domain(e)
        })?;
        
        // 6. Get document type details to determine compression settings
        let config = config.unwrap_or_else(|| {
            // Create format-specific default config based on MIME type
            match document.mime_type.as_str() {
                "image/jpeg" | "image/jpg" => CompressionConfig {
                    method: CompressionMethod::Lossy,
                    quality_level: 80, // Good balance for JPEG
                    min_size_bytes: 5120, // 5KB minimum for images
                },
                "image/png" => CompressionConfig {
                    method: CompressionMethod::Lossless,
                    quality_level: 9, // Best compression for PNG
                    min_size_bytes: 10240, // 10KB minimum for PNG
                },
                "application/pdf" => CompressionConfig {
                    method: CompressionMethod::PdfOptimize,
                    quality_level: 5, // Balanced PDF compression
                    min_size_bytes: 51200, // 50KB minimum for PDFs
                },
                _ => CompressionConfig::default()
            }
        });

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
            log::warn!("File too large for in-memory compression: {} bytes > {} bytes (document {})", 
                     original_size_on_disk, max_in_memory_bytes(), document_id);
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
                    Some("File too large to compress safely")
                ).await.map_err(|e| ServiceError::Domain(e))?;
            }

            // Update global stats for skipped
            let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
            self.compression_repo.update_stats_for_skipped(&mut tx).await
                .map_err(|e| ServiceError::Domain(e))?;
            tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;

            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("File is too large for compression")
            )));
        }

        // 7. Read the original file (safe size)
        log::debug!("Reading file data for document {}", document_id);
        let file_data = match self.file_storage_service.get_file_data(&document.file_path).await {
            Ok(data) => {
                log::debug!("Successfully read {} bytes for document {}", data.len(), document_id);
                data
            },
            Err(e) => {
                // Mark document with error and propagate failure
                log::error!("Failed to read file for document {}: {:?}", document_id, e);
                self.mark_document_with_error(document_id, "storage_failure", "Failed to read document file").await?;
                
                return Err(ServiceError::Domain(DomainError::Internal("Failed to access document file".to_string())));
            }
        };
        
        let original_size = file_data.len() as i64;
        log::debug!("Original file size: {} bytes for document {}", original_size, document_id);
        
        // Check minimum size threshold for compression
        if original_size < config.min_size_bytes {
            log::debug!("File too small to compress: {} bytes < {} bytes minimum (document {})", 
                     original_size, config.min_size_bytes, document_id);
            
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
                    Some("File below minimum size threshold")
                ).await.map_err(|e| ServiceError::Domain(e))?;
            }
            
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("File is below minimum size for compression")
            )));
        }
        
        // 8. Determine MIME type and extension
        let mime_type = document.mime_type.as_str();
        let extension = get_extension(&document.original_filename);
        log::debug!("MIME type: {}, extension: {:?} for document {}", 
                 mime_type, extension, document_id);
        
        // 9. Select appropriate compressor
        let compressor = self.find_compressor(mime_type, extension).await;
        log::debug!("Selected compressor for document {}", document_id);
        
        // 10. Compress the file
        log::info!("Starting compression for document {}", document_id);
        let compressed_data = match compressor.compress(
            file_data, 
            config.method,
            config.quality_level
        ).await {
            Ok(data) => {
                // Validate compression output to prevent zero-byte files
                let compressed_size = data.len() as i64;
                
                if compressed_size == 0 {
                    log::error!("Compression resulted in zero-byte file for document {}", document_id);
                    self.mark_document_with_error(document_id, "compression_failure", "Compression produced invalid output").await?;
                    return Err(ServiceError::Domain(DomainError::Internal("Compression failed - invalid output".to_string())));
                }
                
                // Add minimum size check (should be at least 1% of original or 100 bytes minimum)
                let min_expected_size = std::cmp::max(100, original_size / 100);
                if compressed_size < min_expected_size {
                    log::warn!("Compressed size ({} bytes) suspiciously small vs original ({} bytes) for document {}", 
                             compressed_size, original_size, document_id);
                    self.mark_document_with_error(document_id, "compression_failure", "Compression produced unexpectedly small output").await?;
                    return Err(ServiceError::Domain(DomainError::Internal("Compression failed - invalid result size".to_string())));
                }
                
                log::info!("Compression successful: {} bytes -> {} bytes for document {}", 
                         original_size, compressed_size, document_id);
                data
            },
            Err(e) => {
                // Check if this is a PDF skip request
                if let DomainError::Internal(ref msg) = e {
                    if msg == "PDF_SKIP_COMPRESSION" {
                        log::info!("PDF compression skipped for document {} - PDFs are already compressed", document_id);
                        
                        // Mark as skipped instead of failed
                        self.media_doc_repo.update_compression_status(
                            document_id,
                            CompressionStatus::Skipped,
                            None,
                            None
                        ).await.map_err(|e| ServiceError::Domain(e))?;
                        
                        // Update queue status to skipped
                        if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
                            .map_err(|e| ServiceError::Domain(e))? 
                        {
                            self.compression_repo.update_queue_entry_status(
                                queue_entry.id, 
                                "skipped", 
                                Some("PDF files are already compressed - skipped to save CPU cycles")
                            ).await.map_err(|e| ServiceError::Domain(e))?;
                        }
                        
                        // Update stats for skipped compression
                        let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                        if let Err(stats_err) = self.compression_repo.update_stats_for_skipped(&mut tx).await {
                            log::error!("Failed to update stats for skipped compression: {:?}", stats_err);
                            let _ = tx.rollback().await;
                        } else {
                            tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                        }
                        
                        return Err(ServiceError::Domain(DomainError::Validation(
                            crate::errors::ValidationError::custom("PDF compression skipped - already compressed format")
                        )));
                    }
                }
                
                // Handle regular compression failures
                log::error!("Compression failed for document {}: {:?}", document_id, e);
                self.mark_document_with_error(document_id, "compression_failure", "Compression process failed").await?;
                
                // Update queue status
                if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
                    .map_err(|e| ServiceError::Domain(e))? 
                {
                    self.compression_repo.update_queue_entry_status(
                        queue_entry.id, 
                        "failed", 
                        Some("Compression process failed")
                    ).await.map_err(|e| ServiceError::Domain(e))?;
                }
                
                // Update stats for failed compression
                let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                if let Err(stats_err) = self.compression_repo.update_stats_for_failed(&mut tx).await {
                    log::error!("Failed to update stats for failed compression: {:?}", stats_err);
                    let _ = tx.rollback().await;
                } else {
                    tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
                }
                
                return Err(ServiceError::Domain(DomainError::Internal("Compression operation failed".to_string())));
            }
        };
        
        let compressed_size = compressed_data.len() as i64;
        
        // Validate compression effectiveness - adaptive thresholds by MIME type
        let effectiveness_threshold = match mime_type {
            "image/jpeg" | "image/jpg" => 0.98, // Images should compress well (98% threshold)
            "image/png" => 0.95, // PNG less compressible (95% threshold)
            "application/pdf" => 0.90, // PDFs may not compress much (90% threshold)
            _ => 0.95 // Default threshold
        };
        
        let size_threshold = (original_size as f32 * effectiveness_threshold) as i64;
        if compressed_size > size_threshold {
            println!("⚠️ [COMPRESSION_SERVICE] Compression ineffective for document {}: {} -> {} bytes (>{:.1}% of original, threshold: {:.1}%)",
                     document_id,
                     original_size,
                     compressed_size,
                     (compressed_size as f32 / original_size as f32) * 100.0,
                     effectiveness_threshold * 100.0);
            
            // Mark as skipped instead of completed
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
                    Some("Compression would not reduce file size significantly")
                ).await.map_err(|e| ServiceError::Domain(e))?;
            }
            
            // Update stats for skipped
            let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
            self.compression_repo.update_stats_for_skipped(&mut tx).await
                .map_err(|e| ServiceError::Domain(e))?;
            tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
            
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Compression would not reduce file size significantly")
            )));
        }
        
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
            .save_compressed_file(
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
        
        // 13. Update document status (separate operation to avoid transaction conflicts)
        println!("📝 [COMPRESSION_SERVICE] Updating document status to completed for document {}", document_id);
        
        // Retry logic for database locks
        let mut retry_count = 0;
        let max_retries = 3;
        
        while retry_count < max_retries {
            match self.media_doc_repo.update_compression_status(
                document_id, 
                CompressionStatus::Completed, 
                Some(&compressed_path),
                Some(compressed_size)
            ).await {
                Ok(_) => {
                    println!("✅ [COMPRESSION_SERVICE] Document status updated successfully for {}", document_id);
                    break;
                },
                Err(e) => {
                    retry_count += 1;
                    if e.to_string().contains("database is locked") && retry_count < max_retries {
                        println!("🔄 [COMPRESSION_SERVICE] Database locked, retrying in {}ms (attempt {}/{})", 
                                100 * retry_count, retry_count, max_retries);
                        tokio::time::sleep(Duration::from_millis(100 * retry_count as u64)).await;
                        continue;
                    } else {
                        println!("❌ [COMPRESSION_SERVICE] Failed to update document status for {} after {} attempts: {:?}", 
                                document_id, retry_count, e);
                        return Err(ServiceError::Domain(e));
                    }
                }
            }
        }
        
        // 14. Update queue entry if exists (separate operation)
        if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
            .map_err(|e| ServiceError::Domain(e))? 
        {
            println!("📋 [COMPRESSION_SERVICE] Updating queue entry to completed for document {}", document_id);
            retry_count = 0;
            while retry_count < max_retries {
                match self.compression_repo.update_queue_entry_status(
                    queue_entry.id, 
                    "completed", 
                    None
                ).await {
                    Ok(_) => {
                        println!("✅ [COMPRESSION_SERVICE] Queue entry updated successfully for {}", document_id);
                        break;
                    },
                    Err(e) => {
                        retry_count += 1;
                        if e.to_string().contains("database is locked") && retry_count < max_retries {
                            println!("🔄 [COMPRESSION_SERVICE] Database locked updating queue, retrying in {}ms (attempt {}/{})", 
                                    100 * retry_count, retry_count, max_retries);
                            tokio::time::sleep(Duration::from_millis(100 * retry_count as u64)).await;
                            continue;
                        } else {
                            println!("❌ [COMPRESSION_SERVICE] Failed to update queue entry for {} after {} attempts: {:?}", 
                                    document_id, retry_count, e);
                            // Don't fail the entire operation for queue update failures
                            break;
                        }
                    }
                }
            }
        }
        
        // 15. Update compression stats (in separate transaction)
        println!("📊 [COMPRESSION_SERVICE] Updating compression stats for document {}", document_id);
        retry_count = 0;
        while retry_count < max_retries {
            match self.pool.begin().await {
                Ok(mut tx) => {
                    match self.compression_repo.update_stats_after_compression(
                        original_size,
                        compressed_size,
                        &mut tx
                    ).await {
                        Ok(_) => {
                            match tx.commit().await {
                                Ok(_) => {
                                    println!("✅ [COMPRESSION_SERVICE] Stats updated successfully for {}", document_id);
                                    break;
                                },
                                Err(e) => {
                                    retry_count += 1;
                                    if e.to_string().contains("database is locked") && retry_count < max_retries {
                                        println!("🔄 [COMPRESSION_SERVICE] Database locked committing stats, retrying in {}ms (attempt {}/{})", 
                                                100 * retry_count, retry_count, max_retries);
                                        tokio::time::sleep(Duration::from_millis(100 * retry_count as u64)).await;
                                        continue;
                                    } else {
                                        println!("❌ [COMPRESSION_SERVICE] Failed to commit stats for {} after {} attempts: {:?}", 
                                                document_id, retry_count, e);
                                        // Don't fail the entire operation for stats update failures
                                        break;
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            retry_count += 1;
                            if e.to_string().contains("database is locked") && retry_count < max_retries {
                                println!("🔄 [COMPRESSION_SERVICE] Database locked updating stats, retrying in {}ms (attempt {}/{})", 
                                        100 * retry_count, retry_count, max_retries);
                                tokio::time::sleep(Duration::from_millis(100 * retry_count as u64)).await;
                                continue;
                            } else {
                                println!("❌ [COMPRESSION_SERVICE] Failed to update stats for {} after {} attempts: {:?}", 
                                        document_id, retry_count, e);
                                // Don't fail the entire operation for stats update failures
                                break;
                            }
                        }
                    }
                },
                Err(e) => {
                    retry_count += 1;
                    if e.to_string().contains("database is locked") && retry_count < max_retries {
                        println!("🔄 [COMPRESSION_SERVICE] Database locked beginning transaction, retrying in {}ms (attempt {}/{})", 
                                100 * retry_count, retry_count, max_retries);
                        tokio::time::sleep(Duration::from_millis(100 * retry_count as u64)).await;
                        continue;
                    } else {
                        println!("❌ [COMPRESSION_SERVICE] Failed to begin transaction for stats update for {} after {} attempts: {:?}", 
                                document_id, retry_count, e);
                        // Don't fail the entire operation for stats update failures
                        break;
                    }
                }
            }
        }
        
        // Calculate metrics
        let space_saved_bytes = original_size - compressed_size;
        let space_saved_percentage = if original_size > 0 {
            (space_saved_bytes as f64 / original_size as f64) * 100.0
        } else {
            0.0
        };
        
        let duration_ms = start_time.elapsed().as_millis() as i64;
        
        println!("🎉 [COMPRESSION_SERVICE] Compression completed for document {} in {}ms", document_id, duration_ms);
        
        // 18. Queue original file for safe deletion after grace period
        if let Err(e) = self.queue_original_for_safe_deletion(document_id, &compressed_path).await {
            // Log error but don't fail the compression operation
            log::warn!("Failed to queue original file for deletion for document {}: {:?}", document_id, e);
            println!("⚠️ [COMPRESSION_SERVICE] Warning: Could not queue original file for deletion: {:?}", e);
        }
        
        // 19. Return result
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
    
    async fn queue_document_for_compression_with_tx(
        &self,
        document_id: Uuid,
        priority: CompressionPriority,
        tx: &mut Transaction<'_, Sqlite>,
    ) -> ServiceResult<()> {
        println!("🗜️ [COMPRESSION] Starting queue_document_for_compression_with_tx for {}", document_id);
        
        // Get document to make sure it exists (within transaction)
        let document = self.media_doc_repo.find_by_id_with_tx(document_id, tx).await
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
            self.media_doc_repo.update_compression_status_with_tx(
                document_id, 
                CompressionStatus::Skipped, 
                None, 
                None,
                tx
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
        
        // Queue the document within the transaction
        self.compression_repo
            .queue_document_with_tx(document_id, priority.into(), tx)
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
        if self.get_document_compression_status(document_id).await? == CompressionStatus::Processing {
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
            "processing" => Ok(CompressionStatus::Processing),  // Updated to match DB constraint
            "in_progress" => Ok(CompressionStatus::Processing), // Backwards compatibility
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
    
    async fn get_document_details(&self, document_id: Uuid) -> ServiceResult<Option<MediaDocument>> {
        match FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await {
            Ok(doc) => Ok(Some(doc)),
            Err(DomainError::EntityNotFound { .. }) => Ok(None),
            Err(e) => Err(ServiceError::Domain(e)),
        }
    }

    /// Queue original file for safe deletion after successful compression
    async fn queue_original_for_safe_deletion(
        &self,
        document_id: Uuid,
        compressed_file_path: &str,
    ) -> ServiceResult<()> {
        println!("🗑️ [COMPRESSION_SERVICE] Queuing original file for deletion after grace period for document {}", document_id);
        
        // Get document details to fetch the original file path
        let document = FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| {
                log::error!("Failed to find document {} for deletion queuing: {:?}", document_id, e);
                ServiceError::Domain(e)
            })?;

        // Skip if document has error path
        if document.file_path == "ERROR" {
            println!("⚠️ [COMPRESSION_SERVICE] Skipping deletion queue for error document {}", document_id);
            return Ok(());
        }

        // Verify compressed file exists before queuing original for deletion
        match self.file_storage_service.get_file_size(compressed_file_path).await {
            Ok(size) if size > 0 => {
                println!("✅ [COMPRESSION_SERVICE] Verified compressed file exists ({} bytes) for document {}", size, document_id);
            },
            Ok(_) => {
                println!("❌ [COMPRESSION_SERVICE] Compressed file is empty, keeping original for document {}", document_id);
                return Ok(());
            },
            Err(e) => {
                println!("❌ [COMPRESSION_SERVICE] Compressed file verification failed, keeping original for document {}: {:?}", document_id, e);
                return Ok(());
            }
        }

        // Queue the original file for deletion with 24-hour grace period
        let queue_id_str = Uuid::new_v4().to_string();
        let doc_id_str = document_id.to_string();
        
        // Try to get any admin user for the deletion request to satisfy foreign key constraint
        let (user_id_str, device_id_str) = {
            match sqlx::query!(
                "SELECT id FROM users WHERE role = 'admin' LIMIT 1"
            )
            .fetch_optional(&self.pool)
            .await {
                Ok(Some(row)) => (row.id, "compression_service".to_string()),
                _ => {
                    // If no admin user found, skip deletion queue to avoid constraint issues
                    println!("⚠️ [COMPRESSION_SERVICE] No admin user found for deletion queue, skipping original file deletion queue");
                    return Ok(());
                }
            }
        };
        
        let now_str = Utc::now().to_rfc3339();
        
        sqlx::query!(
            r#"
            INSERT INTO file_deletion_queue (
                id,
                document_id,
                file_path,
                compressed_file_path,
                requested_at,
                requested_by,
                requested_by_device_id,
                grace_period_seconds
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
            queue_id_str,
            doc_id_str,
            document.file_path, // Original file path to delete
            None::<String>, // Don't delete compressed file
            now_str,
            user_id_str,
            device_id_str,
            86400 // 24 hour grace period
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            log::error!("Failed to queue original file for deletion for document {}: {:?}", document_id, e);
            ServiceError::Domain(DomainError::Database(DbError::from(e)))
        })?;

        println!("✅ [COMPRESSION_SERVICE] Queued original file for deletion: {} (document {})", document.file_path, document_id);
        println!("   📅 Grace period: 24 hours from now");
        println!("   🔒 Compressed file will be preserved: {}", compressed_file_path);
        
        Ok(())
    }

    /// Clean up stale documents and queue entries (automated maintenance)
    async fn cleanup_stale_documents(&self) -> ServiceResult<u64> {
        println!("🧹 [COMPRESSION_SERVICE] Starting automated stale document cleanup");
        let mut total_cleaned = 0u64;

        // 1. Clean up orphaned queue entries (documents that no longer exist)
        match sqlx::query!(
            r#"
            DELETE FROM compression_queue 
            WHERE document_id NOT IN (
                SELECT id FROM media_documents WHERE deleted_at IS NULL
            )
            "#
        )
        .execute(&self.pool)
        .await {
            Ok(result) => {
                let orphaned_count = result.rows_affected();
                if orphaned_count > 0 {
                    total_cleaned += orphaned_count;
                    println!("✅ [CLEANUP] Removed {} orphaned queue entries", orphaned_count);
                }
            },
            Err(e) => {
                println!("❌ [CLEANUP] Failed to clean orphaned queue entries: {:?}", e);
            }
        }

        // 2. Clean up queue entries for deleted documents
        match sqlx::query!(
            r#"
            DELETE FROM compression_queue 
            WHERE document_id IN (
                SELECT id FROM media_documents WHERE deleted_at IS NOT NULL
            )
            "#
        )
        .execute(&self.pool)
        .await {
            Ok(result) => {
                let deleted_count = result.rows_affected();
                if deleted_count > 0 {
                    total_cleaned += deleted_count;
                    println!("✅ [CLEANUP] Removed {} queue entries for deleted documents", deleted_count);
                }
            },
            Err(e) => {
                println!("❌ [CLEANUP] Failed to clean deleted document queues: {:?}", e);
            }
        }

        // 3. Reset documents stuck in 'processing' state for more than 1 hour
        match sqlx::query!(
            r#"
            UPDATE media_documents 
            SET 
                compression_status = 'pending',
                updated_at = datetime('now'),
                has_error = 0,
                error_message = NULL
            WHERE compression_status = 'processing'
            AND (julianday('now') - julianday(updated_at)) * 24 * 60 > 60
            AND deleted_at IS NULL
            "#
        )
        .execute(&self.pool)
        .await {
            Ok(result) => {
                let stuck_count = result.rows_affected();
                if stuck_count > 0 {
                    total_cleaned += stuck_count;
                    println!("✅ [CLEANUP] Reset {} stuck documents from processing to pending", stuck_count);
                }
            },
            Err(e) => {
                println!("❌ [CLEANUP] Failed to reset stuck documents: {:?}", e);
            }
        }

        // 4. Clean up very old failed documents (older than 7 days) - mark as skipped to stop retries
        match sqlx::query!(
            r#"
            UPDATE media_documents 
            SET 
                compression_status = 'skipped',
                updated_at = datetime('now'),
                error_message = 'Skipped after 7 days of failures'
            WHERE compression_status = 'failed'
            AND (julianday('now') - julianday(updated_at)) > 7
            AND deleted_at IS NULL
            "#
        )
        .execute(&self.pool)
        .await {
            Ok(result) => {
                let old_failed_count = result.rows_affected();
                if old_failed_count > 0 {
                    total_cleaned += old_failed_count;
                    println!("✅ [CLEANUP] Marked {} old failed documents as skipped", old_failed_count);
                }
            },
            Err(e) => {
                println!("❌ [CLEANUP] Failed to clean old failed documents: {:?}", e);
            }
        }

        if total_cleaned > 0 {
            println!("🎉 [CLEANUP] Completed stale document cleanup: {} items processed", total_cleaned);
        } else {
            println!("✨ [CLEANUP] No stale documents found - system is clean");
        }

        Ok(total_cleaned)
    }

    /// Reset stuck compression jobs (automated recovery)
    async fn reset_stuck_jobs(&self) -> ServiceResult<u64> {
        println!("🔄 [COMPRESSION_SERVICE] Starting stuck job recovery");
        let mut total_reset = 0u64;

        // 1. Reset queue entries stuck in 'processing' state for more than 30 minutes
        match sqlx::query!(
            r#"
            UPDATE compression_queue 
            SET 
                status = 'pending',
                attempts = CASE 
                    WHEN attempts >= 3 THEN 0  -- Reset attempts if too many failures
                    ELSE attempts 
                END,
                error_message = NULL,
                updated_at = datetime('now')
            WHERE status = 'processing'
            AND (julianday('now') - julianday(updated_at)) * 24 * 60 > 30
            "#
        )
        .execute(&self.pool)
        .await {
            Ok(result) => {
                let stuck_queue_count = result.rows_affected();
                if stuck_queue_count > 0 {
                    total_reset += stuck_queue_count;
                    println!("✅ [RECOVERY] Reset {} stuck queue entries", stuck_queue_count);
                }
            },
            Err(e) => {
                println!("❌ [RECOVERY] Failed to reset stuck queue entries: {:?}", e);
            }
        }

        // 2. Reset failed queue entries that have been failing for less than 24 hours (give them another chance)
        match sqlx::query!(
            r#"
            UPDATE compression_queue 
            SET 
                status = 'pending',
                attempts = 0,
                error_message = NULL,
                updated_at = datetime('now')
            WHERE status = 'failed'
            AND attempts < 5  -- Only retry if not too many attempts
            AND (julianday('now') - julianday(updated_at)) * 24 < 24  -- Less than 24 hours old
            "#
        )
        .execute(&self.pool)
        .await {
            Ok(result) => {
                let retry_count = result.rows_affected();
                if retry_count > 0 {
                    total_reset += retry_count;
                    println!("✅ [RECOVERY] Gave {} failed jobs another chance", retry_count);
                }
            },
            Err(e) => {
                println!("❌ [RECOVERY] Failed to retry failed jobs: {:?}", e);
            }
        }

        // 3. Remove very old failed queue entries (older than 7 days) to prevent queue bloat
        match sqlx::query!(
            r#"
            DELETE FROM compression_queue 
            WHERE status = 'failed'
            AND (julianday('now') - julianday(updated_at)) > 7
            "#
        )
        .execute(&self.pool)
        .await {
            Ok(result) => {
                let old_failed_count = result.rows_affected();
                if old_failed_count > 0 {
                    total_reset += old_failed_count;
                    println!("✅ [RECOVERY] Removed {} very old failed queue entries", old_failed_count);
                }
            },
            Err(e) => {
                println!("❌ [RECOVERY] Failed to remove old failed entries: {:?}", e);
            }
        }

        if total_reset > 0 {
            println!("🎉 [RECOVERY] Completed stuck job recovery: {} items processed", total_reset);
        } else {
            println!("✨ [RECOVERY] No stuck jobs found - queue is healthy");
        }

        Ok(total_reset)
    }
}