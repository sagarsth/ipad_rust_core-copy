//! Background worker for processing the compression queue with superior message-based architecture

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::{oneshot, mpsc};
use tokio::task::JoinHandle;
use sqlx::SqlitePool;
use chrono::Utc;
use uuid::Uuid;
use std::sync::{OnceLock, Mutex};
use std::time::Instant;

use crate::errors::{DomainError, ServiceError, ServiceResult, DomainResult};
use crate::errors::DbError;
use crate::domains::compression::service::CompressionService;
use crate::domains::compression::repository::CompressionRepository;
use crate::domains::compression::types::{CompressionConfig, CompressionPriority, CompressionQueueEntry};

/// Messages that can be sent to the compression worker for real-time control
#[derive(Debug)]
pub enum CompressionWorkerMessage {
    /// Force immediate queue processing
    ProcessNow {
        response: oneshot::Sender<ServiceResult<usize>>, // Returns number of jobs started
    },
    /// Cancel a specific document compression
    CancelDocument {
        document_id: Uuid,
        response: oneshot::Sender<ServiceResult<bool>>,
    },
    /// Update compression priority for a document
    UpdatePriority {
        document_id: Uuid,
        priority: CompressionPriority,
        response: oneshot::Sender<ServiceResult<bool>>,
    },
    /// Get current worker status
    GetStatus {
        response: oneshot::Sender<WorkerStatus>,
    },
    /// Adjust concurrency settings
    SetMaxConcurrency {
        max_jobs: usize,
        response: oneshot::Sender<()>,
    },
    /// Shutdown the worker
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

/// Worker status information
#[derive(Debug, Clone)]
pub struct WorkerStatus {
    pub active_jobs: usize,
    pub max_concurrent_jobs: usize,
    pub queue_poll_interval_ms: u64,
    pub running_document_ids: Vec<Uuid>,
}

/// Enhanced worker for processing the compression queue with superior message-based architecture
pub struct CompressionWorker {
    compression_service: Arc<dyn CompressionService>,
    compression_repo: Arc<dyn CompressionRepository>,
    pool: SqlitePool,
    interval_ms: u64,
    max_concurrent_jobs: usize,
    message_receiver: Option<mpsc::Receiver<CompressionWorkerMessage>>,
    message_sender: mpsc::Sender<CompressionWorkerMessage>,
    active_jobs: tokio::sync::Mutex<HashMap<Uuid, JoinHandle<()>>>,
}

impl CompressionWorker {
    pub fn new(
        compression_service: Arc<dyn CompressionService>,
        compression_repo: Arc<dyn CompressionRepository>,
        pool: SqlitePool,
        interval_ms: Option<u64>,
        max_concurrent_jobs: Option<usize>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(100);
        
        Self {
            compression_service,
            compression_repo,
            pool,
            interval_ms: interval_ms.unwrap_or(2000), // Reduced from 5000ms for faster response
            max_concurrent_jobs: max_concurrent_jobs.unwrap_or(3),
            message_receiver: Some(receiver),
            message_sender: sender,
            active_jobs: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
    
    /// Get message sender for external control
    pub fn get_message_sender(&self) -> mpsc::Sender<CompressionWorkerMessage> {
        self.message_sender.clone()
    }
    
    /// Set shutdown signal receiver (for backward compatibility)
    pub fn with_shutdown_signal(self, _receiver: tokio::sync::oneshot::Receiver<()>) -> Self {
        // Message-based architecture handles shutdown better
        self
    }

    /// Start the worker process with superior message-based architecture
    pub fn start(mut self) -> (JoinHandle<()>, mpsc::Sender<CompressionWorkerMessage>) {
        let sender = self.message_sender.clone();
        let mut receiver = self.message_receiver.take().expect("Receiver should be available");
        
        // Start enhanced worker task
        let handle = tokio::spawn(async move {
            self.run_enhanced_worker(&mut receiver).await;
            println!("✅ [COMPRESSION_WORKER] Enhanced compression worker shut down gracefully");
        });
        
        (handle, sender)
    }
    
    /// Enhanced worker loop with message-based architecture and superior performance
    async fn run_enhanced_worker(&mut self, receiver: &mut mpsc::Receiver<CompressionWorkerMessage>) {
        let mut shutdown_response: Option<oneshot::Sender<()>> = None;
        
        println!("🚀 [COMPRESSION_WORKER] Starting enhanced compression worker:");
        println!("   📊 Max concurrent jobs: {}", self.max_concurrent_jobs);
        println!("   ⏱️ Poll interval: {}ms", self.interval_ms);
        println!("   📨 Message-based control: ENABLED");
        println!("   🎯 Active job tracking: ENABLED");
        
        // Create interval timer for polling, but prioritize messages
        let mut interval = tokio::time::interval(Duration::from_millis(self.interval_ms));
        
        loop {
            // Check if shutdown requested
            if shutdown_response.is_some() {
                break;
            }
            
            tokio::select! {
                // Priority 1: Handle real-time messages (blocking receive)
                message = receiver.recv() => {
                    match message {
                        Some(msg) => {
                            match msg {
                                CompressionWorkerMessage::ProcessNow { response } => {
                                    let jobs_started = self.process_queue_immediately().await;
                                    let _ = response.send(Ok(jobs_started));
                                },
                                CompressionWorkerMessage::CancelDocument { document_id, response } => {
                                    let cancelled = self.cancel_document_job(document_id).await;
                                    let _ = response.send(Ok(cancelled));
                                },
                                CompressionWorkerMessage::UpdatePriority { document_id, priority, response } => {
                                    let updated = self.update_document_priority(document_id, priority).await;
                                    let _ = response.send(updated);
                                },
                                CompressionWorkerMessage::GetStatus { response } => {
                                    let status = self.get_worker_status().await;
                                    let _ = response.send(status);
                                },
                                CompressionWorkerMessage::SetMaxConcurrency { max_jobs, response } => {
                                    self.max_concurrent_jobs = max_jobs;
                                    println!("⚙️ [COMPRESSION_WORKER] Updated max concurrency to {}", max_jobs);
                                    let _ = response.send(());
                                },
                                CompressionWorkerMessage::Shutdown { response } => {
                                    shutdown_response = Some(response);
                                    println!("🛑 [COMPRESSION_WORKER] Shutdown requested via message");
                                    break;
                                }
                            }
                        },
                        None => {
                            println!("🔌 [COMPRESSION_WORKER] Message channel disconnected, shutting down");
                            break;
                        }
                    }
                },
                
                // Priority 2: Process queue on interval
                _ = interval.tick() => {
                    self.process_queue_tick().await;
                }
            }
        }
        
        // Shutdown: Cancel all active jobs
        println!("🔄 [COMPRESSION_WORKER] Cancelling all active jobs...");
        let mut jobs = self.active_jobs.lock().await;
        let job_count = jobs.len();
        for (document_id, handle) in jobs.drain() {
            println!("❌ [COMPRESSION_WORKER] Aborting job for document {}", document_id);
            handle.abort();
        }
        drop(jobs);
        
        if job_count > 0 {
            println!("🧹 [COMPRESSION_WORKER] Cancelled {} active jobs", job_count);
        }
        
        // Send shutdown confirmation
        if let Some(response) = shutdown_response {
            let _ = response.send(());
        }
    }
    
    /// Process queue immediately (for manual triggers)
    async fn process_queue_immediately(&self) -> usize {
        println!("⚡ [COMPRESSION_WORKER] Manual queue processing triggered");
        
        let mut jobs_started = 0;
        let max_new_jobs = 5; // Limit burst processing
        
        for _ in 0..max_new_jobs {
            if !self.has_capacity().await {
                break;
            }
            
            if self.start_next_job_if_available().await {
                jobs_started += 1;
            } else {
                break; // No more jobs available
            }
        }
        
        if jobs_started > 0 {
            println!("🚀 [COMPRESSION_WORKER] Started {} jobs from manual trigger", jobs_started);
        }
        
        jobs_started
    }
    
    /// Regular queue processing tick
    async fn process_queue_tick(&self) {
        // Clean up completed jobs
        self.cleanup_completed_jobs().await;
        
        // Start new jobs if we have capacity
        if self.has_capacity().await {
            if self.start_next_job_if_available().await {
                // Successfully started a job, try to start more immediately
                for _ in 0..2 { // Try up to 2 more jobs
                    if !self.has_capacity().await || !self.start_next_job_if_available().await {
                        break;
                    }
                }
            } else {
                // No jobs in queue, less frequent logging
                let active_count = self.get_active_job_count().await;
                log_idle_if_needed(active_count);
            }
        }
    }
    
    /// Check if worker has capacity for more jobs
    async fn has_capacity(&self) -> bool {
        let jobs = self.active_jobs.lock().await;
        jobs.len() < self.max_concurrent_jobs
    }
    
    /// Get count of active jobs
    async fn get_active_job_count(&self) -> usize {
        let jobs = self.active_jobs.lock().await;
        jobs.len()
    }
    
    /// Clean up completed jobs
    async fn cleanup_completed_jobs(&self) {
        let mut jobs = self.active_jobs.lock().await;
        let initial_count = jobs.len();
        jobs.retain(|_, handle| !handle.is_finished());
        let completed_count = initial_count - jobs.len();
        
        if completed_count > 0 {
            println!("✅ [COMPRESSION_WORKER] {} jobs completed, {} still running", completed_count, jobs.len());
        }
    }
    
    /// Try to start the next available job
    async fn start_next_job_if_available(&self) -> bool {
        match self.compression_repo.get_next_document_for_compression().await {
            Ok(Some(queue_entry)) => {
                println!("📄 [COMPRESSION_WORKER] Got document to process: {} (attempts: {}, status: {})", 
                         queue_entry.document_id, queue_entry.attempts, queue_entry.status);
                
                // Check if document is in use (non-blocking)
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
                        false // Proceed anyway if check fails
                    }
                };
                
                if is_in_use {
                    // Requeue for later processing
                    if let Err(e) = self.compression_repo.update_queue_entry_status(
                        queue_entry.id,
                        "pending",
                        Some("Document is in use"),
                    ).await {
                        println!("❌ [COMPRESSION_WORKER] Failed to requeue document {}: {:?}", document_id, e);
                    }
                    return false;
                }
                
                // Start the compression job
                let job_handle = self.start_enhanced_compression_job(queue_entry).await;
                
                // Add to active jobs tracking
                let mut jobs = self.active_jobs.lock().await;
                jobs.insert(document_id, job_handle);
                
                println!("🚀 [COMPRESSION_WORKER] Started compression job for document {}", document_id);
                println!("📈 [COMPRESSION_WORKER] Now running {} jobs (max: {})", jobs.len(), self.max_concurrent_jobs);
                
                true
            },
            Ok(None) => false, // No jobs available
            Err(e) => {
                println!("❌ [COMPRESSION_WORKER] Error fetching next document: {:?}", e);
                false
            }
        }
    }
    
    /// Start an enhanced compression job with better error handling
    async fn start_enhanced_compression_job(&self, queue_entry: CompressionQueueEntry) -> JoinHandle<()> {
        let document_id = queue_entry.document_id;
        let compression_service = self.compression_service.clone();
        let compression_repo = self.compression_repo.clone();
        let pool = self.pool.clone();
        
        tokio::spawn(async move {
            println!("🔄 [COMPRESSION_JOB] Processing document {}", document_id);
            
            // Process the document with timeout to prevent infinite hangs
            let start_time = std::time::Instant::now();
            
            // Enhanced timeout calculation based on file size
            let timeout_secs = if let Ok(Some(doc)) = compression_service.get_document_details(document_id).await {
                if doc.size_bytes > 100_000_000 { 600 } // 10 minutes for very large files
                else if doc.size_bytes > 50_000_000 { 300 } // 5 minutes for large files
                else if doc.size_bytes > 10_000_000 { 180 } // 3 minutes for medium files
                else { 120 } // 2 minutes for small files
            } else { 120 }; // Default 2 minutes
            
            println!("⏱️ [COMPRESSION_JOB] Using {}s timeout for document {}", timeout_secs, document_id);
            
            // *** TRANSACTION-BASED STATUS UPDATE ***
            // Update status to "processing" in a short-lived transaction
            let mut tx = match pool.begin().await {
                Ok(tx) => tx,
                Err(e) => {
                    println!("❌ [COMPRESSION_JOB] Failed to start transaction for status update: {:?}", e);
                    return;
                }
            };
            
            if let Err(e) = compression_repo.update_queue_entry_status_with_tx(
                queue_entry.id,
                "processing",
                None,
                &mut tx
            ).await {
                println!("❌ [COMPRESSION_JOB] Failed to update status to processing: {:?}", e);
                let _ = tx.rollback().await;
                return;
            }
            
            if let Err(e) = tx.commit().await {
                println!("❌ [COMPRESSION_JOB] Failed to commit processing status: {:?}", e);
                return;
            }
            
            println!("✅ [COMPRESSION_JOB] Status updated to processing for document {}", document_id);
            
            // Perform compression (outside transaction)
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(timeout_secs), 
                compression_service.compress_document(document_id, None)
            ).await;
            
            let duration = start_time.elapsed();
            
            let final_result = match result {
                Ok(compression_result) => compression_result,
                Err(_timeout_error) => {
                    println!("⏰ [COMPRESSION_JOB] TIMEOUT after {}s for document {}", timeout_secs, document_id);
                    Err(ServiceError::Domain(DomainError::Internal(format!("Compression timed out after {} seconds", timeout_secs))))
                }
            };
            
            // *** TRANSACTION-BASED COMPLETION UPDATE ***
            // Update final status in another short-lived transaction
            let mut tx = match pool.begin().await {
                Ok(tx) => tx,
                Err(e) => {
                    println!("❌ [COMPRESSION_JOB] Failed to start transaction for completion update: {:?}", e);
                    return;
                }
            };
            
            match final_result {
                Ok(compression_result) => {
                    // Enhanced compression validation (95% threshold)
                    let size_threshold = (compression_result.original_size as f32 * 0.95) as i64;
                    
                    if compression_result.compressed_size > size_threshold {
                        println!("⚠️ [COMPRESSION_JOB] Compression ineffective for document {}: {} -> {} bytes (>{:.1}% of original)",
                                 document_id,
                                 compression_result.original_size,
                                 compression_result.compressed_size,
                                 (compression_result.compressed_size as f32 / compression_result.original_size as f32) * 100.0);
                        
                        // Mark as skipped instead of completed
                        if let Err(e) = compression_repo.update_queue_entry_status_with_tx(
                            queue_entry.id,
                            "skipped", 
                            Some("Compression would increase file size"),
                            &mut tx
                        ).await {
                            println!("❌ [COMPRESSION_JOB] Failed to update status to skipped: {:?}", e);
                            let _ = tx.rollback().await;
                            return;
                        }
                    } else {
                        // Successfully compressed with meaningful size reduction
                        println!("✅ [COMPRESSION_JOB] Document {} compressed successfully in {:?}", 
                                 document_id, duration);
                        println!("   📊 Original: {} bytes, Compressed: {} bytes, Saved: {:.1}%",
                                 compression_result.original_size,
                                 compression_result.compressed_size,
                                 100.0 - (compression_result.compressed_size as f32 / compression_result.original_size as f32) * 100.0);
                        
                        // Update queue entry to completed
                        if let Err(e) = compression_repo.update_queue_entry_status_with_tx(
                            queue_entry.id,
                            "completed",
                            None,
                            &mut tx
                        ).await {
                            println!("❌ [COMPRESSION_JOB] Failed to update status to completed: {:?}", e);
                            let _ = tx.rollback().await;
                            return;
                        }
                    }
                },
                Err(e) => {
                    println!("❌ [COMPRESSION_JOB] Document {} compression failed after {:?}: {:?}", document_id, duration, e);
                    
                    // Update queue entry to failed with error message
                    let error_message = format!("Compression failed: {}", e);
                    if let Err(update_err) = compression_repo.update_queue_entry_status_with_tx(
                        queue_entry.id,
                        "failed",
                        Some(&error_message),
                        &mut tx
                    ).await {
                        println!("❌ [COMPRESSION_JOB] Failed to update status to failed: {:?}", update_err);
                        let _ = tx.rollback().await;
                        return;
                    }
                }
            }
            
            // Commit the final status update
            if let Err(e) = tx.commit().await {
                println!("❌ [COMPRESSION_JOB] Failed to commit final status: {:?}", e);
                return;
            }
            
            println!("✅ [COMPRESSION_JOB] Final status committed for document {}", document_id);
        })
    }
    
    /// Cancel a specific document compression job
    async fn cancel_document_job(&self, document_id: Uuid) -> bool {
        let mut jobs = self.active_jobs.lock().await;
        if let Some(handle) = jobs.remove(&document_id) {
            handle.abort();
            println!("❌ [COMPRESSION_WORKER] Cancelled compression job for document {}", document_id);
            
            // Update database status
            if let Err(e) = self.compression_repo.remove_from_queue(document_id).await {
                println!("⚠️ [COMPRESSION_WORKER] Failed to remove cancelled job from queue: {:?}", e);
            }
            
            true
        } else {
            println!("⚠️ [COMPRESSION_WORKER] No active job found for document {}", document_id);
            false
        }
    }
    
    /// Update compression priority for a document
    async fn update_document_priority(&self, document_id: Uuid, priority: CompressionPriority) -> ServiceResult<bool> {
        match self.compression_repo.update_compression_priority(document_id, priority.into()).await {
            Ok(updated) => {
                if updated {
                    println!("⚡ [COMPRESSION_WORKER] Updated priority for document {} to {:?}", document_id, priority);
                }
                Ok(updated)
            },
            Err(e) => {
                println!("❌ [COMPRESSION_WORKER] Failed to update priority for document {}: {:?}", document_id, e);
                Err(ServiceError::Domain(e))
            }
        }
    }
    
    /// Get current worker status
    async fn get_worker_status(&self) -> WorkerStatus {
        let jobs = self.active_jobs.lock().await;
        let running_document_ids: Vec<Uuid> = jobs.keys().copied().collect();
        
        WorkerStatus {
            active_jobs: jobs.len(),
            max_concurrent_jobs: self.max_concurrent_jobs,
            queue_poll_interval_ms: self.interval_ms,
            running_document_ids,
        }
    }
}

/// Queue original file for deletion after successful compression
async fn queue_original_for_deletion(pool: SqlitePool, document_id: Uuid) -> Result<(), ServiceError> {
    let document_id_str = document_id.to_string();
    let now = Utc::now().to_rfc3339();
    
    sqlx::query!(
        "INSERT INTO file_deletion_queue (document_id, requested_at) VALUES (?, ?)",
        document_id_str,
        now
    )
    .execute(&pool)
    .await
    .map_err(|e| ServiceError::Domain(DomainError::Database(DbError::from(e))))?;
    
    Ok(())
}

/// Synchronous helper for less-frequent idle logging
fn log_idle_if_needed(active_count: usize) {
    static LAST_IDLE_LOG: OnceLock<Mutex<Instant>> = OnceLock::new();
    let last_log = LAST_IDLE_LOG.get_or_init(|| Mutex::new(Instant::now()));
    if let Ok(mut guard) = last_log.lock() {
        if guard.elapsed().as_secs() > 30 {
            println!("😴 [COMPRESSION_WORKER] No documents in queue, {} jobs running", active_count);
            *guard = Instant::now();
        }
    }
}