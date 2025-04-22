 //! Background worker for processing the compression queue

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use uuid::Uuid;
use sqlx::SqlitePool;

use crate::errors::{DomainError, DomainResult};
use super::service::CompressionService;
use super::repository::CompressionRepository;
use super::types::CompressionConfig;

/// Worker for processing the compression queue in the background
pub struct CompressionWorker {
    compression_service: Arc<dyn CompressionService>,
    compression_repo: Arc<dyn CompressionRepository>,
    pool: SqlitePool,
    interval_ms: u64,
}

impl CompressionWorker {
    pub fn new(
        compression_service: Arc<dyn CompressionService>,
        compression_repo: Arc<dyn CompressionRepository>,
        pool: SqlitePool,
        interval_ms: Option<u64>,
    ) -> Self {
        Self {
            compression_service,
            compression_repo,
            pool,
            interval_ms: interval_ms.unwrap_or(5000), // Default to 5 seconds
        }
    }
    
    /// Start the worker process
    pub fn start(&self) -> (JoinHandle<()>, oneshot::Sender<()>) {
        let service = self.compression_service.clone();
        let repo = self.compression_repo.clone();
        let interval_ms = self.interval_ms;
        
        // Channel for signaling shutdown
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        
        // Start worker task
        let handle = tokio::spawn(async move {
            loop {
                // Check if shutdown signal received
                if shutdown_rx.try_recv().is_ok() {
                    // Shutdown received
                    break;
                }
                
                // Process next document
                match Self::process_next_document(&service, &repo).await {
                    Ok(true) => {
                        // Document processed, continue immediately to next one
                        continue;
                    },
                    Ok(false) => {
                        // No documents to process, sleep for interval
                        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
                    },
                    Err(e) => {
                        // Error processing document, log and continue after delay
                        eprintln!("Error processing compression queue: {:?}", e);
                        tokio::time::sleep(Duration::from_millis(interval_ms)).await;
                    }
                }
            }
            
            println!("Compression worker shutting down");
        });
        
        (handle, shutdown_tx)
    }
    
    /// Process the next document in the queue
    async fn process_next_document(
        service: &Arc<dyn CompressionService>,
        repo: &Arc<dyn CompressionRepository>,
    ) -> DomainResult<bool> {
        // Get next document from queue
        let queue_entry = match repo.get_next_document_for_compression().await? {
            Some(entry) => entry,
            None => return Ok(false), // No documents to process
        };
        
        // Default compression config
        let config = CompressionConfig::default();
        
        // Try to compress the document
        let result = service.compress_document(queue_entry.document_id, Some(config)).await;
        
        match result {
            Ok(_) => {
                // Success is handled in compress_document
                Ok(true)
            },
            Err(e) => {
                // Update queue entry to failed status
                let error_message = format!("{:?}", e);
                repo.update_queue_entry_status(
                    queue_entry.id,
                    "failed",
                    Some(&error_message)
                ).await?;
                
                // Log the error but continue processing
                eprintln!("Compression failed for document {}: {:?}", queue_entry.document_id, e);
                Ok(true)
            }
        }
    }
}