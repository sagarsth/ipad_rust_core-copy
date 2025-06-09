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
        
        println!("🚀 [COMPRESSION_WORKER] Starting compression worker with {} max concurrent jobs, polling every {}ms", 
                 self.max_concurrent_jobs, self.interval_ms);
        
        loop {
            // First check if shutdown requested
            if let Some(ref mut signal) = self.shutdown_signal {
                if signal.try_recv().is_ok() {
                    println!("🛑 [COMPRESSION_WORKER] Shutdown signal received, stopping compression worker");
                    break;
                }
            }
            
            tokio::select! {
                _ = interval.tick() => {
                    // Check and clean up completed jobs
                    let initial_job_count = running_jobs.len();
                    running_jobs.retain(|handle| !handle.is_finished());
                    let completed_jobs = initial_job_count - running_jobs.len();
                    
                    if completed_jobs > 0 {
                        println!("✅ [COMPRESSION_WORKER] {} jobs completed, {} still running", completed_jobs, running_jobs.len());
                    }
                    
                    // Only start new jobs if we're below max concurrent limit
                    if running_jobs.len() < self.max_concurrent_jobs {
                        // Get next document to process
                        match self.compression_repo.get_next_document_for_compression().await {
                            Ok(Some(queue_entry)) => {
                                println!("📄 [COMPRESSION_WORKER] Got document to process: {} (attempts: {}, status: {})", 
                                         queue_entry.document_id, queue_entry.attempts, queue_entry.status);
                                
                                // Check if document is in use
                                let document_id = queue_entry.document_id;
                                let is_in_use = match self.compression_service.is_document_in_use(document_id).await {
                                    Ok(true) => {
                                        println!("⏸️ [COMPRESSION_WORKER] Document {} is in use, skipping for now", document_id);
                                        true
                                    },
                                    Ok(false) => {
                                        println!("✅ [COMPRESSION_WORKER] Document {} not in use, proceeding with compression", document_id);
                                        false
                                    },
                                    Err(e) => {
                                        println!("⚠️ [COMPRESSION_WORKER] Error checking if document {} is in use: {:?}", document_id, e);
                                        false
                                    }
                                };
                                
                                if is_in_use {
                                    // Document is in use, requeue for later
                                    if let Err(e) = self.compression_repo.update_queue_entry_status(
                                        queue_entry.id,
                                        "pending", // Back to pending
                                        Some("Document is in use"),
                                    ).await {
                                        println!("❌ [COMPRESSION_WORKER] Failed to requeue document {}: {:?}", document_id, e);
                                    }
                                    continue;
                                }
                                
                                // Start a new compression job
                                let service = self.compression_service.clone();
                                let repo = self.compression_repo.clone();
                                let pool = self.pool.clone();
                                
                                println!("🚀 [COMPRESSION_WORKER] Starting compression job for document {}", document_id);
                                
                                let job_handle = tokio::spawn(async move {
                                    println!("🔄 [COMPRESSION_JOB] Processing document {}", document_id);
                                    
                                    // Process the document
                                    let start_time = std::time::Instant::now();
                                    let result = service.compress_document(document_id, None).await;
                                    let duration = start_time.elapsed();
                                    
                                    match result {
                                        Ok(compression_result) => {
                                            // Validate compression effectiveness (95% threshold)
                                            let size_threshold = (compression_result.original_size as f32 * 0.95) as i64;
                                            
                                            if compression_result.compressed_size > size_threshold {
                                                println!("⚠️ [COMPRESSION_JOB] Compression ineffective for document {}: {} -> {} bytes (>{:.1}% of original)",
                                                         document_id,
                                                         compression_result.original_size,
                                                         compression_result.compressed_size,
                                                         (compression_result.compressed_size as f32 / compression_result.original_size as f32) * 100.0);
                                                
                                                // Mark as skipped instead of completed
                                                if let Err(e) = repo.update_queue_entry_status(
                                                    queue_entry.id,
                                                    "skipped", 
                                                    Some("Compression would increase file size")
                                                ).await {
                                                    println!("❌ [COMPRESSION_JOB] Failed to update status to skipped: {:?}", e);
                                                }
                                                return;
                                            }
                                            
                                            // Successfully compressed with meaningful size reduction
                                            println!("✅ [COMPRESSION_JOB] Document {} compressed successfully in {:?}", 
                                                     document_id, duration);
                                            println!("   📊 Original: {} bytes, Compressed: {} bytes, Saved: {:.1}%",
                                                     compression_result.original_size,
                                                     compression_result.compressed_size,
                                                     compression_result.space_saved_percentage);
                                            
                                            // Queue original file for deletion after grace period
                                            if let Err(e) = queue_original_for_deletion(pool, document_id).await {
                                                println!("⚠️ [COMPRESSION_JOB] Failed to queue original file for deletion: {:?}", e);
                                            }
                                        },
                                        Err(e) => {
                                            // Compression failed - error already logged in service
                                            println!("❌ [COMPRESSION_JOB] Document {} compression failed after {:?}: {:?}", 
                                                     document_id, duration, e);
                                        }
                                    }
                                    
                                    println!("🏁 [COMPRESSION_JOB] Job finished for document {}", document_id);
                                });
                                
                                running_jobs.push(job_handle);
                                println!("📈 [COMPRESSION_WORKER] Now running {} jobs (max: {})", running_jobs.len(), self.max_concurrent_jobs);
                            },
                            Ok(None) => {
                                // No documents in queue - only log this occasionally to avoid spam
                                static mut LAST_NO_WORK_LOG: Option<std::time::Instant> = None;
                                let now = std::time::Instant::now();
                                unsafe {
                                    if let Some(last) = LAST_NO_WORK_LOG {
                                        if last.elapsed().as_secs() >= 30 {
                                            println!("😴 [COMPRESSION_WORKER] No documents in queue, {} jobs running", running_jobs.len());
                                            LAST_NO_WORK_LOG = Some(now);
                                        }
                                    } else {
                                        println!("😴 [COMPRESSION_WORKER] No documents in queue, {} jobs running", running_jobs.len());
                                        LAST_NO_WORK_LOG = Some(now);
                                    }
                                }
                            },
                            Err(e) => {
                                println!("❌ [COMPRESSION_WORKER] Error fetching next compression job: {:?}", e);
                            }
                        }
                    } else {
                        // At capacity - only log this occasionally
                        static mut LAST_CAPACITY_LOG: Option<std::time::Instant> = None;
                        let now = std::time::Instant::now();
                        unsafe {
                            if let Some(last) = LAST_CAPACITY_LOG {
                                if last.elapsed().as_secs() >= 10 {
                                    println!("🚦 [COMPRESSION_WORKER] At capacity: {} jobs running (max: {})", running_jobs.len(), self.max_concurrent_jobs);
                                    LAST_CAPACITY_LOG = Some(now);
                                }
                            } else {
                                println!("🚦 [COMPRESSION_WORKER] At capacity: {} jobs running (max: {})", running_jobs.len(), self.max_concurrent_jobs);
                                LAST_CAPACITY_LOG = Some(now);
                            }
                        }
                    }
                }
            }
        }
        
        // Wait for all jobs to complete on shutdown
        println!("🛑 [COMPRESSION_WORKER] Waiting for {} remaining jobs to complete...", running_jobs.len());
        for job in running_jobs {
            if let Err(e) = job.await {
                println!("⚠️ [COMPRESSION_WORKER] Job failed to complete cleanly: {:?}", e);
            }
        }
        
        println!("✅ [COMPRESSION_WORKER] All compression jobs completed, worker shut down");
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