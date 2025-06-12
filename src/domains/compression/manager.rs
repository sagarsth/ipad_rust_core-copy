use crate::domains::compression::service::CompressionService;
use crate::domains::compression::types::{CompressionConfig, CompressionPriority, CompressionQueueEntry};
use crate::domains::compression::repository::CompressionRepository;
use crate::errors::{DomainError, ServiceError, ServiceResult, DbError};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use uuid::Uuid;
use std::collections::HashMap;
use sqlx::SqlitePool;

/// Messages that can be sent to the compression manager
#[derive(Debug)]
pub enum CompressionMessage {
    /// Queue a document for compression
    Queue {
        document_id: Uuid,
        priority: CompressionPriority,
        response: oneshot::Sender<ServiceResult<()>>,
    },
    /// Cancel a pending compression
    Cancel {
        document_id: Uuid,
        response: oneshot::Sender<ServiceResult<bool>>,
    },
    /// Update compression priority
    UpdatePriority {
        document_id: Uuid,
        priority: CompressionPriority,
        response: oneshot::Sender<ServiceResult<bool>>,
    },
    /// Bulk update compression priorities
    BulkUpdatePriority {
        document_ids: Vec<Uuid>,
        priority: CompressionPriority,
        response: oneshot::Sender<ServiceResult<u64>>,
    },
    /// Get compression queue status
    GetQueueStatus {
        response: oneshot::Sender<ServiceResult<crate::domains::compression::types::CompressionQueueStatus>>,
    },
    /// Shutdown the manager
    Shutdown {
        response: oneshot::Sender<()>,
    },
}

/// Trait for the compression manager
#[async_trait]
pub trait CompressionManager: Send + Sync {
    /// Get the sender to communicate with the manager
    fn get_sender(&self) -> mpsc::Sender<CompressionMessage>;
    
    /// Start the manager
    fn start(&self) -> JoinHandle<()>;
    
    /// Stop the manager
    async fn stop(&self) -> ServiceResult<()>;
    
    /// Queue a document for compression
    async fn queue_document(&self, document_id: Uuid, priority: CompressionPriority) -> ServiceResult<()>;
    
    /// Cancel a pending compression
    async fn cancel_compression(&self, document_id: Uuid) -> ServiceResult<bool>;
    
    /// Update compression priority
    async fn update_priority(&self, document_id: Uuid, priority: CompressionPriority) -> ServiceResult<bool>;
    
    /// Bulk update compression priorities
    async fn bulk_update_priority(&self, document_ids: &[Uuid], priority: CompressionPriority) -> ServiceResult<u64>;
    
    /// Get compression queue status
    async fn get_queue_status(&self) -> ServiceResult<crate::domains::compression::types::CompressionQueueStatus>;
}

/// Implementation of the compression manager
pub struct CompressionManagerImpl {
    sender: mpsc::Sender<CompressionMessage>,
    worker_handle: tokio::sync::Mutex<Option<JoinHandle<()>>>,
}

impl CompressionManagerImpl {
    pub fn new(
        compression_service: Arc<dyn CompressionService>,
        compression_repo: Arc<dyn CompressionRepository>,
        max_concurrent_jobs: usize,
        queue_poll_interval_ms: u64,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(100);

        // Create the background worker first
        let worker = CompressionWorker::new(
            receiver,
            compression_service,
            compression_repo,
            max_concurrent_jobs,
            queue_poll_interval_ms,
        );

        // Start the worker and get its handle
        let handle = worker.start();

        // Now create the manager instance
        Self {
            sender: sender.clone(),
            worker_handle: tokio::sync::Mutex::new(Some(handle)), // Store the handle directly
        }
    }
}

#[async_trait]
impl CompressionManager for CompressionManagerImpl {
    fn get_sender(&self) -> mpsc::Sender<CompressionMessage> {
        self.sender.clone()
    }
    
    fn start(&self) -> JoinHandle<()> {
        // Placeholder - actual start happens in new()
        tokio::spawn(async {})
    }
    
    async fn stop(&self) -> ServiceResult<()> {
        let (tx, rx) = oneshot::channel();
        
        // Send shutdown message
        self.sender.send(CompressionMessage::Shutdown { response: tx })
            .await
            .map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to send shutdown message".to_string())))?;
            
        // Wait for shutdown confirmation
        rx.await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to receive shutdown confirmation".to_string())))?;
        
        // Join the worker task
        let mut lock = self.worker_handle.lock().await;
        if let Some(handle) = lock.take() {
            handle.await.map_err(|e| ServiceError::Domain(DomainError::Internal(format!("Failed to join worker task: {}", e))))?;
        }
        
        Ok(())
    }
    
    async fn queue_document(&self, document_id: Uuid, priority: CompressionPriority) -> ServiceResult<()> {
        let (tx, rx) = oneshot::channel();
        
        self.sender.send(CompressionMessage::Queue {
            document_id,
            priority,
            response: tx,
        }).await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to send queue message".to_string())))?;
        
        rx.await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to receive queue response".to_string())))?
    }
    
    async fn cancel_compression(&self, document_id: Uuid) -> ServiceResult<bool> {
        let (tx, rx) = oneshot::channel();
        
        self.sender.send(CompressionMessage::Cancel {
            document_id,
            response: tx,
        }).await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to send cancel message".to_string())))?;
        
        rx.await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to receive cancel response".to_string())))?
    }
    
    async fn update_priority(&self, document_id: Uuid, priority: CompressionPriority) -> ServiceResult<bool> {
        let (tx, rx) = oneshot::channel();
        
        self.sender.send(CompressionMessage::UpdatePriority {
            document_id,
            priority,
            response: tx,
        }).await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to send update priority message".to_string())))?;
        
        rx.await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to receive update priority response".to_string())))?
    }
    
    async fn bulk_update_priority(&self, document_ids: &[Uuid], priority: CompressionPriority) -> ServiceResult<u64> {
        let (tx, rx) = oneshot::channel();
        
        self.sender.send(CompressionMessage::BulkUpdatePriority {
            document_ids: document_ids.to_vec(),
            priority,
            response: tx,
        }).await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to send bulk update priority message".to_string())))?;
        
        rx.await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to receive bulk update priority response".to_string())))?
    }
    
    async fn get_queue_status(&self) -> ServiceResult<crate::domains::compression::types::CompressionQueueStatus> {
        let (tx, rx) = oneshot::channel();
        
        self.sender.send(CompressionMessage::GetQueueStatus {
            response: tx,
        }).await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to send get queue status message".to_string())))?;
        
        rx.await.map_err(|_| ServiceError::Domain(DomainError::Internal("Failed to receive queue status response".to_string())))?
    }
}

/// Worker for processing the compression queue
struct CompressionWorker {
    receiver: mpsc::Receiver<CompressionMessage>,
    compression_service: Arc<dyn CompressionService>,
    compression_repo: Arc<dyn CompressionRepository>,
    max_concurrent_jobs: usize,
    poll_interval_ms: u64,
    active_jobs: tokio::sync::Mutex<HashMap<Uuid, JoinHandle<()>>>,
}

impl CompressionWorker {
    fn new(
        receiver: mpsc::Receiver<CompressionMessage>,
        compression_service: Arc<dyn CompressionService>,
        compression_repo: Arc<dyn CompressionRepository>,
        max_concurrent_jobs: usize,
        poll_interval_ms: u64,
    ) -> Self {
        Self {
            receiver,
            compression_service,
            compression_repo,
            max_concurrent_jobs,
            poll_interval_ms,
            active_jobs: tokio::sync::Mutex::new(HashMap::new()),
        }
    }
    
    fn start(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut shutdown_receiver = None;
            
            loop {
                // Process any incoming messages
                match self.receiver.try_recv() {
                    Ok(message) => {
                        match message {
                            CompressionMessage::Queue { document_id, priority, response } => {
                                let result = self.compression_repo.queue_document(document_id, priority.into()).await;
                                let _ = response.send(result.map(|_| ()).map_err(|e| ServiceError::Domain(e)));
                            },
                            CompressionMessage::Cancel { document_id, response } => {
                                let result = self.compression_repo.remove_from_queue(document_id).await;
                                let _ = response.send(result.map_err(|e| ServiceError::Domain(e)));
                            },
                            CompressionMessage::UpdatePriority { document_id, priority, response } => {
                                let result = self.compression_repo.update_compression_priority(document_id, priority.into()).await;
                                let _ = response.send(result.map_err(|e| ServiceError::Domain(e)));
                            },
                            CompressionMessage::BulkUpdatePriority { document_ids, priority, response } => {
                                let result = self.compression_repo.bulk_update_compression_priority(&document_ids, priority.into()).await;
                                let _ = response.send(result.map_err(|e| ServiceError::Domain(e)));
                            },
                            CompressionMessage::GetQueueStatus { response } => {
                                let result = self.compression_repo.get_queue_status().await;
                                let _ = response.send(result.map_err(|e| ServiceError::Domain(e)));
                            },
                            CompressionMessage::Shutdown { response } => {
                                shutdown_receiver = Some(response);
                                break;
                            }
                        }
                    },
                    Err(mpsc::error::TryRecvError::Empty) => {
                        // No messages, continue
                    },
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        // Channel closed, shut down
                        break;
                    }
                }
                
                // Check active jobs and clean up completed ones
                {
                    let mut jobs = self.active_jobs.lock().await;
                    jobs.retain(|_, handle| !handle.is_finished());
                    
                    // If we have capacity, process more jobs
                    if jobs.len() < self.max_concurrent_jobs {
                        match self.compression_repo.get_next_document_for_compression().await {
                            Ok(Some(entry)) => {
                                // --- Extract document_id before moving entry --- 
                                let document_id_for_map = entry.document_id;

                                // Start a new compression job (entry is moved here)
                                let job_handle = self.start_compression_job(entry).await;
                                jobs.insert(document_id_for_map, job_handle); // Use extracted ID
                            },
                            Ok(None) => {
                                // No jobs in queue, sleep
                                drop(jobs); // Release lock before sleeping
                                sleep(Duration::from_millis(self.poll_interval_ms)).await;
                            },
                            Err(e) => {
                                eprintln!("Error fetching next compression job: {:?}", e);
                                drop(jobs); // Release lock before sleeping
                                sleep(Duration::from_millis(self.poll_interval_ms)).await;
                            }
                        }
                    } else {
                        // At capacity, wait for jobs to complete
                        drop(jobs); // Release lock before sleeping
                        sleep(Duration::from_millis(100)).await; // Short sleep before checking again
                    }
                }
            }
            
            // On shutdown, cancel all active jobs
            let mut jobs = self.active_jobs.lock().await;
            for (_, handle) in jobs.drain() {
                handle.abort();
            }
            
            // Send shutdown confirmation
            if let Some(response) = shutdown_receiver {
                let _ = response.send(());
            }
        })
    }
    
    async fn start_compression_job(&self, entry: CompressionQueueEntry) -> JoinHandle<()> {
        let document_id = entry.document_id;
        let compression_service = self.compression_service.clone();
        let compression_repo = self.compression_repo.clone();
        
        tokio::spawn(async move {
            // Update entry status to "processing"
            if let Err(e) = compression_repo.update_queue_entry_status(
                entry.id,
                "processing",
                None,
            ).await {
                eprintln!("Failed to update queue entry status: {:?}", e);
                return;
            }
            
            // Check if document is in use before compressing
            match compression_service.is_document_in_use(document_id).await {
                Ok(true) => {
                    // Document is in use, requeue for later
                    if let Err(e) = compression_repo.update_queue_entry_status(
                        entry.id,
                        "pending", // Back to pending
                        Some("Document is in use"),
                    ).await {
                        eprintln!("Failed to requeue document: {:?}", e);
                    }
                    return;
                },
                Err(e) => {
                    eprintln!("Error checking if document is in use: {:?}", e);
                    // Continue anyway
                },
                Ok(false) => {
                    // Not in use, proceed with compression
                }
            }
            
            // Perform compression
            match compression_service.compress_document(
                document_id,
                Some(CompressionConfig::default()),
            ).await {
                Ok(result) => {
                    // Compression successful, update entry status
                    if let Err(e) = compression_repo.update_queue_entry_status(
                        entry.id,
                        "completed",
                        None,
                    ).await {
                        eprintln!("Failed to update queue entry status after completion: {:?}", e);
                    }
                    
                    // Handle original file deletion if appropriate
                    // Queue the original file for deletion after grace period
                    if let Err(e) = queue_original_for_deletion(document_id).await {
                        eprintln!("Failed to queue original file for deletion: {:?}", e);
                    }
                    
                    println!("Compression completed for document {}: {:?}", document_id, result);
                },
                Err(e) => {
                    // Compression failed, update entry status with error
                    let error_message = format!("Compression failed: {:?}", e);
                    if let Err(update_err) = compression_repo.update_queue_entry_status(
                        entry.id,
                        "failed",
                        Some(&error_message),
                    ).await {
                        eprintln!("Failed to update queue entry status after failure: {:?}", update_err);
                    }
                    
                    eprintln!("Compression failed for document {}: {:?}", document_id, e);
                }
            }
        })
    }
}

/// Queue original file for deletion after successful compression
/// This function would be implemented using file_deletion_queue
async fn queue_original_for_deletion(document_id: Uuid) -> Result<(), ServiceError> {
    // For now, just log that we would queue for deletion to avoid crashes
    // TODO: Implement proper file deletion queue when database pool is properly available
    eprintln!("INFO: Would queue document {} for deletion after compression", document_id);
    
    // Return early to avoid the unimplemented pool access
    Ok(())
}

// REMOVED UNSAFE GLOBAL POOL ACCESS - THIS WAS CAUSING THE CRASH
// The unsafe global static with raw pointer access was causing memory protection faults
// 
// TODO: Implement proper dependency injection or pass the pool through function parameters
// For now, this entire global pool system is disabled to prevent crashes

/// Stub implementation of CompressionManager that delegates to service/repository
/// This is used for FFI compatibility while the actual compression work is done by CompressionWorker
pub struct StubCompressionManager {
    compression_service: Arc<dyn CompressionService>,
    compression_repo: Arc<dyn CompressionRepository>,
}

impl StubCompressionManager {
    pub fn new(
        compression_service: Arc<dyn CompressionService>,
        compression_repo: Arc<dyn CompressionRepository>,
    ) -> Self {
        Self {
            compression_service,
            compression_repo,
        }
    }
}

#[async_trait]
impl CompressionManager for StubCompressionManager {
    fn get_sender(&self) -> mpsc::Sender<CompressionMessage> {
        // Return a dummy channel that will be dropped immediately
        // The actual work is done by CompressionWorker, not through messages
        let (sender, _receiver) = mpsc::channel(1);
        sender
    }
    
    fn start(&self) -> JoinHandle<()> {
        // No-op start since CompressionWorker handles the actual work
        tokio::spawn(async {})
    }
    
    async fn stop(&self) -> ServiceResult<()> {
        // No-op stop since CompressionWorker handles shutdown separately
        Ok(())
    }
    
    async fn queue_document(&self, document_id: Uuid, priority: CompressionPriority) -> ServiceResult<()> {
        // Delegate to service
        self.compression_service.queue_document_for_compression(document_id, priority).await
    }
    
    async fn cancel_compression(&self, document_id: Uuid) -> ServiceResult<bool> {
        // Delegate to service
        self.compression_service.cancel_compression(document_id).await
    }
    
    async fn update_priority(&self, document_id: Uuid, priority: CompressionPriority) -> ServiceResult<bool> {
        // Delegate to service
        self.compression_service.update_compression_priority(document_id, priority).await
    }
    
    async fn bulk_update_priority(&self, document_ids: &[Uuid], priority: CompressionPriority) -> ServiceResult<u64> {
        // Delegate to service
        self.compression_service.bulk_update_compression_priority(document_ids, priority).await
    }
    
    async fn get_queue_status(&self) -> ServiceResult<crate::domains::compression::types::CompressionQueueStatus> {
        // Delegate to service
        self.compression_service.get_compression_queue_status().await
    }
}