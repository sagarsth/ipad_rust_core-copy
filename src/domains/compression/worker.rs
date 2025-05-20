//! Background worker for processing the compression queue

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use sqlx::SqlitePool;
use chrono::Utc;
use uuid::Uuid;

use crate::errors::{DomainError, ServiceError, ServiceResult, DomainResult};
use crate::errors::DbError;
use crate::domains::compression::service::CompressionService;
use crate::domains::compression::repository::CompressionRepository;
use crate::domains::compression::types::{CompressionConfig, CompressionPriority};

/// Worker for processing the compression queue in the background
pub struct CompressionWorker {
    compression_service: Arc<dyn CompressionService>,
    compression_repo: Arc<dyn CompressionRepository>,
    pool: SqlitePool,
    interval_ms: u64,
    shutdown_signal: Option<tokio::sync::oneshot::Receiver<()>>,
    max_concurrent_jobs: usize,
}

impl CompressionWorker {
    pub fn new(
        compression_service: Arc<dyn CompressionService>,
        compression_repo: Arc<dyn CompressionRepository>,
        pool: SqlitePool,
        interval_ms: Option<u64>,
        max_concurrent_jobs: Option<usize>,
    ) -> Self {
        Self {
            compression_service,
            compression_repo,
            pool,
            interval_ms: interval_ms.unwrap_or(5000), // Default to 5 seconds
            shutdown_signal: None,
            max_concurrent_jobs: max_concurrent_jobs.unwrap_or(3), // Default to 3 concurrent jobs
        }
    }
    
    /// Set shutdown signal receiver
    pub fn with_shutdown_signal(mut self, receiver: tokio::sync::oneshot::Receiver<()>) -> Self {
        self.shutdown_signal = Some(receiver);
        self
    }

    /// Start the worker process
    pub fn start(mut self) -> (JoinHandle<()>, oneshot::Sender<()>) {
        // Channel for signaling shutdown
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_signal = Some(shutdown_rx);
        
        // Start worker task
        let handle = tokio::spawn(async move {
            self.run().await;
            println!("Compression worker shut down");
        });
        
        (handle, shutdown_tx)
    }
    
    /// Run the worker process until shutdown
    async fn run(&mut self) {
        // Keep track of running job handles
        let mut running_jobs: Vec<JoinHandle<()>> = Vec::new();
        
        // Create interval timer for polling
        let mut interval = tokio::time::interval(Duration::from_millis(self.interval_ms));
        
        loop {
            // First check if shutdown requested
            if let Some(ref mut signal) = self.shutdown_signal {
                if signal.try_recv().is_ok() {
                    println!("Shutdown signal received, stopping compression worker");
                    break;
                }
            }
            
            tokio::select! {
                _ = interval.tick() => {
                    // Check and clean up completed jobs
                    running_jobs.retain(|handle| !handle.is_finished());
                    
                    // Only start new jobs if we're below max concurrent limit
                    if running_jobs.len() < self.max_concurrent_jobs {
                        // Get next document to process
                        match self.compression_repo.get_next_document_for_compression().await {
                            Ok(Some(queue_entry)) => {
                                // Check if document is in use
                                let document_id = queue_entry.document_id;
                                let is_in_use = match self.compression_service.is_document_in_use(document_id).await {
                                    Ok(true) => true,
                                    Ok(false) => false,
                                    Err(e) => {
                                        eprintln!("Error checking if document is in use: {:?}", e);
                                        false
                                    }
                                };
                                
                                if is_in_use {
                                    // Document is in use, requeue for later
                                    let _ = self.compression_repo.update_queue_entry_status(
                                        queue_entry.id,
                                        "pending", // Back to pending
                                        Some("Document is in use"),
                                    ).await;
                                    continue;
                                }
                                
                                // Start a new compression job
                                let service = self.compression_service.clone();
                                let repo = self.compression_repo.clone();
                                let pool = self.pool.clone();
                                
                                let job_handle = tokio::spawn(async move {
                                    // Process the document
                                    let result = service.compress_document(document_id, None).await;
                                    
                                    match result {
                                        Ok(compression_result) => {
                                            // Successfully compressed
                                            
                                            // If compression successful and we have a compressed file,
                                            // queue the original file for eventual deletion
                                            let compressed_path = compression_result.compressed_file_path;

                                            // Queue original file for deletion after grace period
                                            queue_original_for_deletion(pool, document_id).await
                                                .unwrap_or_else(|e| {
                                                    eprintln!("Failed to queue original file for deletion: {:?}", e)
                                                });
                                            
                                        },
                                        Err(e) => {
                                            // Compression failed - error already logged in service
                                            eprintln!("Compression job failed for document {}: {:?}", document_id, e);
                                        }
                                    }
                                });
                                
                                running_jobs.push(job_handle);
                            },
                            Ok(None) => {
                                // No documents in queue, nothing to do
                            },
                            Err(e) => {
                                eprintln!("Error getting next document for compression: {:?}", e);
                            }
                        }
                    }
                }
                else => {
                    // No other select arms, continue with the loop
                }
            }
        }
        
        // On shutdown, wait for running jobs to complete with a timeout
        for handle in running_jobs {
            match tokio::time::timeout(Duration::from_secs(30), handle).await {
                Ok(_) => {
                    // Job completed normally
                },
                Err(_) => {
                    // Job did not complete within timeout, could abort if needed
                    // handle.abort();
                    println!("Compression job timed out during shutdown");
                }
            }
        }
    }
}

/// Queue original file for deletion after successful compression
async fn queue_original_for_deletion(pool: SqlitePool, document_id: Uuid) -> Result<(), ServiceError> {
    // Fetch the document details to get file paths
    let doc_id_str_fetch = document_id.to_string();
    let doc = sqlx::query!(
        r#"
        SELECT 
            file_path, 
            compressed_file_path, 
            compression_status
        FROM media_documents
        WHERE id = ?
        "#,
        doc_id_str_fetch
    )
    .fetch_one(&pool)
    .await
    .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
    
    // Only queue for deletion if compression is completed and we have a compressed path
    if doc.compression_status == "completed" && doc.compressed_file_path.is_some() {
        // Add to file_deletion_queue with grace period
        let queue_id_str = Uuid::new_v4().to_string();
        let doc_id_str_insert = document_id.to_string();
        let now_str = Utc::now().to_rfc3339();
        let grace_period_val: i64 = 7 * 24 * 60 * 60;
        let requested_by_str = "system";
        
        sqlx::query!(
            r#"
            INSERT INTO file_deletion_queue (
                id, 
                document_id,
                file_path, 
                compressed_file_path, 
                requested_at, 
                requested_by, 
                grace_period_seconds
            )
            VALUES (?, ?, ?, NULL, ?, ?, ?)
            "#,
            queue_id_str,
            doc_id_str_insert,
            doc.file_path,
            now_str,
            requested_by_str,
            grace_period_val
        )
        .execute(&pool)
        .await
        .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
    }
    
    Ok(())
}