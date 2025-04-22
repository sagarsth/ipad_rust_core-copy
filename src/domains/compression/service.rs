 //! Compression service implementation

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::time::Duration;
use uuid::Uuid;
use sqlx::{SqlitePool, Sqlite, Transaction};

use crate::domains::core::file_storage_service::{FileStorageService, FileStorageResult};
use crate::domains::document::repository::MediaDocumentRepository;
use crate::domains::document::types::{CompressionStatus, MediaDocument};
use crate::errors::{DbError, DomainError, DomainResult, ServiceError, ServiceResult};
use super::repository::CompressionRepository;
use super::types::{
    CompressionMethod, CompressionResult, CompressionQueueEntry,
    CompressionQueueStatus, CompressionPriority, CompressionStats, CompressionConfig
};
use super::compressors::{
    Compressor,
    image_compressor::ImageCompressor,
    pdf_compressor::PdfCompressor, 
    office_compressor::OfficeCompressor,
    generic_compressor::GenericCompressor,
    guess_mime_type,
    get_extension
};
use crate::domains::core::repository::FindById;

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
        
        // 1. Get document details
        let document = FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| ServiceError::Domain(e))?;
        
        // 2. Check if already compressed
        if let Some(CompressionStatus::Compressed) = document.compression_status {
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Document is already compressed")
            )));
        }
        
        // 3. Get document type details to determine compression settings
        let config = config.unwrap_or_else(|| CompressionConfig::default());
        
        // 4. Read the original file
        let file_data = self.file_storage_service.get_file_data(&document.file_path).await
            .map_err(|e| ServiceError::Domain(DomainError::Internal(format!("Failed to read file: {:?}", e))))?;
        
        let original_size = file_data.len() as i64;
        
        // 5. Determine MIME type and extension
        let mime_type = document.mime_type.as_deref().unwrap_or("application/octet-stream");
        let extension = get_extension(&document.file_name);
        
        // 6. Select appropriate compressor
        let compressor = self.find_compressor(mime_type, extension).await;
        
        // 7. Compress the file
        let compressed_data = compressor.compress(
            file_data, 
            config.method,
            config.quality_level
        ).await.map_err(|e| ServiceError::Domain(e))?;
        
        let compressed_size = compressed_data.len() as i64;
        
        // 8. Check if compression was effective
        if compressed_size >= original_size {
            // If compressed size is not smaller, update status to skipped and return original
            let mut tx = self.pool.begin().await.map_err(|e| DomainError::Database(DbError::from(e)))?;
            
            // Update document status
            self.media_doc_repo.update_compression_status(
                document_id, 
                CompressionStatus::Skipped, 
                None
            ).await.map_err(|e| ServiceError::Domain(e))?;
            
            // Update queue status
            if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
                .map_err(|e| ServiceError::Domain(e))? 
            {
                self.compression_repo.update_queue_entry_status(
                    queue_entry.id, 
                    "skipped", 
                    Some("Compression not effective")
                ).await.map_err(|e| ServiceError::Domain(e))?;
            }
            
            // Update stats
            self.compression_repo.update_stats_for_skipped(&mut tx).await
                .map_err(|e| ServiceError::Domain(e))?;
                
            tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
            
            return Err(ServiceError::Domain(DomainError::Validation(
                crate::errors::ValidationError::custom("Compression not effective - file size not reduced")
            )));
        }
        
        // 9. Save the compressed file
        let entity_type = document.related_table.as_str();
        let entity_id = document.related_id.unwrap_or_else(|| 
            document.temp_related_id.expect("Document must have either related_id or temp_related_id")
        ).to_string();
        
        // Create compressed filename with suffix
        let file_stem = Path::new(&document.file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("compressed");
            
        let file_ext = Path::new(&document.file_name)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
            
        let compressed_filename = if file_ext.is_empty() {
            format!("{}_compressed", file_stem)
        } else {
            format!("{}_compressed.{}", file_stem, file_ext)
        };
        
        // Save compressed file
        let (compressed_path, _) = self.file_storage_service
            .save_file(
                compressed_data, 
                entity_type, 
                &entity_id,
                &compressed_filename
            ).await
            .map_err(|e| ServiceError::Domain(DomainError::Internal(format!("Failed to save compressed file: {:?}", e))))?;
        
        // 10. Start a transaction for updating document and stats
        let mut tx = self.pool.begin().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // 11. Update document status
        self.media_doc_repo.update_compression_status(
            document_id, 
            CompressionStatus::Compressed, 
            Some(&compressed_path)
        ).await.map_err(|e| ServiceError::Domain(e))?;
        
        // 12. Update compression stats
        self.compression_repo.update_stats_after_compression(
            original_size,
            compressed_size,
            &mut tx
        ).await.map_err(|e| ServiceError::Domain(e))?;
        
        // 13. Update queue entry if exists
        if let Some(queue_entry) = self.compression_repo.get_queue_entry_by_document_id(document_id).await
            .map_err(|e| ServiceError::Domain(e))? 
        {
            self.compression_repo.update_queue_entry_status(
                queue_entry.id, 
                "completed", 
                None
            ).await.map_err(|e| ServiceError::Domain(e))?;
        }
        
        // 14. Commit transaction
        tx.commit().await.map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
        
        // Calculate metrics
        let space_saved_bytes = original_size - compressed_size;
        let space_saved_percentage = if original_size > 0 {
            (space_saved_bytes as f64 / original_size as f64) * 100.0
        } else {0.0
        };
        
        let duration_ms = start_time.elapsed().as_millis() as i64;
        
        // 15. Return result
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
        // Get document to make sure it exists
        let document = FindById::<MediaDocument>::find_by_id(&*self.media_doc_repo, document_id).await
            .map_err(|e| ServiceError::Domain(e))?;
            
        // Don't queue if already compressed
        if let Some(status) = document.compression_status {
            match status {
                CompressionStatus::Compressed | CompressionStatus::Skipped => {
                    return Ok(());
                },
                _ => {}
            }
        }
        
        // Queue the document
        self.compression_repo
            .queue_document(document_id, priority.into())
            .await
            .map_err(|e| ServiceError::Domain(e))?;
            
        Ok(())
    }
    
    async fn cancel_compression(
        &self,
        document_id: Uuid,
    ) -> ServiceResult<bool> {
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
            
        Ok(document.compression_status.unwrap_or(CompressionStatus::Pending))
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
}