//! Background worker for processing the compression queue with superior message-based architecture

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use tokio::sync::{oneshot, mpsc};
use tokio::task::JoinHandle;
use sqlx::SqlitePool;
use chrono::{Timelike, Utc};
use uuid::Uuid;
use std::sync::{OnceLock, Mutex};
use std::time::Instant;

use crate::errors::{DomainError, ServiceError, ServiceResult, DomainResult};
use crate::errors::DbError;
use crate::domains::compression::service::CompressionService;
use crate::domains::compression::repository::CompressionRepository;
use crate::domains::compression::types::{
    CompressionConfig, CompressionPriority, CompressionQueueEntry,
    IOSDeviceState, IOSThermalState, IOSAppState, IOSOptimizations, IOSWorkerStatus, IOSDeviceCapabilities
};

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
    /// Get iOS-enhanced worker status
    GetIOSStatus {
        response: oneshot::Sender<IOSWorkerStatus>,
    },
    /// Adjust concurrency settings
    SetMaxConcurrency {
        max_jobs: usize,
        response: oneshot::Sender<()>,
    },
    /// Update iOS device state (called from Swift)
    UpdateIOSState {
        battery_level: f32,
        is_charging: bool,
        thermal_state: IOSThermalState,
        app_state: IOSAppState,
        available_memory_mb: Option<u64>,
        response: oneshot::Sender<()>,
    },
    /// Handle iOS memory pressure (0=normal, 1=warning, 2=critical)
    HandleMemoryPressure {
        level: u8,
        response: oneshot::Sender<()>,
    },
    /// Pause/resume compression based on iOS state
    SetPaused {
        paused: bool,
        reason: Option<String>,
        response: oneshot::Sender<()>,
    },
    /// Update iOS-specific optimizations
    UpdateIOSOptimizations {
        optimizations: IOSOptimizations,
        response: oneshot::Sender<()>,
    },
    /// Handle iOS background task extension (new)
    HandleBackgroundTaskExtension {
        granted_seconds: u32,
        response: oneshot::Sender<()>,
    },
    /// Handle content visibility change (new)
    HandleContentVisibility {
        is_visible: bool,
        response: oneshot::Sender<()>,
    },
    /// Handle iOS app lifecycle event (new)
    HandleAppLifecycleEvent {
        event: String, // "entering_background", "becoming_active", "resigned_active"
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
    
    // iOS-specific state
    ios_state: Arc<tokio::sync::RwLock<IOSDeviceState>>,
    ios_optimizations: Arc<tokio::sync::RwLock<IOSOptimizations>>,
    is_paused: Arc<tokio::sync::RwLock<bool>>,
    pause_reason: Arc<tokio::sync::RwLock<Option<String>>>,
    
    // Enhanced iOS state management
    background_task_remaining_seconds: Arc<tokio::sync::RwLock<u32>>,
    is_content_visible: Arc<tokio::sync::RwLock<bool>>,
    last_memory_warning: Arc<tokio::sync::RwLock<Option<Instant>>>,
    device_capabilities: Arc<tokio::sync::RwLock<IOSDeviceCapabilities>>,
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
        
        // Initialize iOS state with default values
        let default_ios_state = IOSDeviceState {
            battery_level: 1.0, // Assume full battery initially
            is_charging: false,
            thermal_state: IOSThermalState::Nominal,
            app_state: IOSAppState::Active,
            available_memory_mb: Some(512), // Default assumption
            last_updated: Utc::now(),
        };
        
        // Detect device capabilities
        let device_capabilities = IOSDeviceCapabilities::detect_ios_device();
        
        Self {
            compression_service,
            compression_repo,
            pool,
            interval_ms: interval_ms.unwrap_or(2000), // Reduced from 5000ms for faster response
            max_concurrent_jobs: max_concurrent_jobs.unwrap_or(device_capabilities.get_safe_concurrency()),
            message_receiver: Some(receiver),
            message_sender: sender,
            active_jobs: tokio::sync::Mutex::new(HashMap::new()),
            
            // iOS-specific state initialization
            ios_state: Arc::new(tokio::sync::RwLock::new(default_ios_state)),
            ios_optimizations: Arc::new(tokio::sync::RwLock::new(IOSOptimizations::default())),
            is_paused: Arc::new(tokio::sync::RwLock::new(false)),
            pause_reason: Arc::new(tokio::sync::RwLock::new(None)),
            
            // Enhanced iOS state management
            background_task_remaining_seconds: Arc::new(tokio::sync::RwLock::new(0)),
            is_content_visible: Arc::new(tokio::sync::RwLock::new(true)),
            last_memory_warning: Arc::new(tokio::sync::RwLock::new(None)),
            device_capabilities: Arc::new(tokio::sync::RwLock::new(device_capabilities)),
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
        
        log::info!("Starting enhanced compression worker");
        log::info!("Max concurrent jobs: {}", self.max_concurrent_jobs);
        log::info!("Poll interval: {}ms", self.interval_ms);
        log::debug!("Message-based control enabled");
        log::debug!("Active job tracking enabled");
        log::debug!("iOS integration enabled");
        
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
                                CompressionWorkerMessage::GetIOSStatus { response } => {
                                    let ios_status = self.get_ios_worker_status().await;
                                    let _ = response.send(ios_status);
                                },
                                CompressionWorkerMessage::SetMaxConcurrency { max_jobs, response } => {
                                    self.max_concurrent_jobs = max_jobs;
                                    log::info!("Updated max concurrency to {}", max_jobs);
                                    let _ = response.send(());
                                },
                                CompressionWorkerMessage::UpdateIOSState { 
                                    battery_level, is_charging, thermal_state, app_state, available_memory_mb, response 
                                } => {
                                    self.update_ios_state(battery_level, is_charging, thermal_state, app_state, available_memory_mb).await;
                                    let _ = response.send(());
                                },
                                CompressionWorkerMessage::HandleMemoryPressure { level, response } => {
                                    self.handle_memory_pressure(level).await;
                                    let _ = response.send(());
                                },
                                CompressionWorkerMessage::SetPaused { paused, reason, response } => {
                                    self.set_paused(paused, reason).await;
                                    let _ = response.send(());
                                },
                                CompressionWorkerMessage::UpdateIOSOptimizations { optimizations, response } => {
                                    *self.ios_optimizations.write().await = optimizations;
                                    println!("🍎 [COMPRESSION_WORKER] Updated iOS optimizations");
                                    let _ = response.send(());
                                },
                                // NEW: Handle background task extension
                                CompressionWorkerMessage::HandleBackgroundTaskExtension { granted_seconds, response } => {
                                    self.handle_background_task_extension(granted_seconds).await;
                                    let _ = response.send(());
                                },
                                // NEW: Handle content visibility
                                CompressionWorkerMessage::HandleContentVisibility { is_visible, response } => {
                                    self.handle_content_visibility(is_visible).await;
                                    let _ = response.send(());
                                },
                                // NEW: Handle app lifecycle events
                                CompressionWorkerMessage::HandleAppLifecycleEvent { event, response } => {
                                    self.handle_app_lifecycle_event(&event).await;
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
        
        // Automated maintenance: Run cleanup every 10 minutes (600 seconds)
        static mut LAST_CLEANUP: Option<Instant> = None;
        let should_cleanup = unsafe {
            match LAST_CLEANUP {
                None => true,
                Some(last) => last.elapsed().as_secs() >= 600, // 10 minutes
            }
        };
        
        if should_cleanup {
            println!("🧹 [COMPRESSION_WORKER] Running automated maintenance");
            
            // Run stale document cleanup
            match self.compression_service.cleanup_stale_documents().await {
                Ok(cleaned_count) => {
                    if cleaned_count > 0 {
                        println!("✅ [MAINTENANCE] Cleaned up {} stale documents", cleaned_count);
                    }
                },
                Err(e) => {
                    println!("❌ [MAINTENANCE] Stale document cleanup failed: {:?}", e);
                }
            }
            
            // Run stuck job recovery
            match self.compression_service.reset_stuck_jobs().await {
                Ok(reset_count) => {
                    if reset_count > 0 {
                        println!("✅ [MAINTENANCE] Reset {} stuck jobs", reset_count);
                    }
                },
                Err(e) => {
                    println!("❌ [MAINTENANCE] Stuck job recovery failed: {:?}", e);
                }
            }
            
            unsafe {
                LAST_CLEANUP = Some(Instant::now());
            }
            
            println!("🎉 [MAINTENANCE] Automated maintenance completed");
        }
        
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
    
    /// Check if worker has capacity for more jobs (iOS-aware)
    async fn has_capacity(&self) -> bool {
        // Check if paused
        if *self.is_paused.read().await {
            return false;
        }
        
        // Check content visibility
        if !*self.is_content_visible.read().await {
            return false;
        }
        
        // Check memory pressure
        if let Some(last_warning) = *self.last_memory_warning.read().await {
            if last_warning.elapsed().as_secs() < 30 {
                return false; // Still within memory pressure window
            }
        }
        
        // Check background task time remaining
        let background_time = *self.background_task_remaining_seconds.read().await;
        let ios_state = self.ios_state.read().await;
        if ios_state.app_state == IOSAppState::Background && background_time < 10 {
            return false; // Less than 10 seconds remaining
        }
        
        let jobs = self.active_jobs.lock().await;
        let effective_max_jobs = self.calculate_effective_max_jobs().await;
        
        jobs.len() < effective_max_jobs
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
        let device_capabilities = self.device_capabilities.clone();
        
        tokio::spawn(async move {
            println!("🔄 [COMPRESSION_JOB] Processing document {}", document_id);
            
            // Process the document with timeout to prevent infinite hangs
            let start_time = std::time::Instant::now();
            
            // Enhanced timeout calculation based on device type and file size
            let timeout_secs = if let Ok(Some(doc)) = compression_service.get_document_details(document_id).await {
                let device_caps = device_capabilities.read().await;
                let base_timeout = if doc.size_bytes > 100_000_000 { 600 } // 10 minutes for very large files
                else if doc.size_bytes > 50_000_000 { 300 } // 5 minutes for large files
                else if doc.size_bytes > 10_000_000 { 180 } // 3 minutes for medium files
                else { 120 }; // 2 minutes for small files
                
                // Apply device-specific multiplier
                let timeout_multiplier = match device_caps.device_type {
                    crate::domains::compression::types::IOSDeviceType::IPhone => 3.0,
                    crate::domains::compression::types::IOSDeviceType::IPad => 2.0,
                    crate::domains::compression::types::IOSDeviceType::IPadPro => 1.5,
                };
                
                (base_timeout as f32 * timeout_multiplier) as u64
            } else { 
                120 // Default 2 minutes
            };
            
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
                    
                    // Check if the document actually completed during the timeout window
                    if let Ok(Some(doc)) = compression_service.get_document_details(document_id).await {
                        if doc.compression_status == "completed" {
                            println!("✅ [COMPRESSION_JOB] Document {} actually completed during timeout window, treating as success", document_id);
                            // Skip error handling - document was actually successful
                            return;
                        }
                    }
                    
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
                        
                        // Note: Original file deletion is now handled by the compression service itself
                        // via queue_original_for_safe_deletion() method with 24-hour grace period
                    }
                },
                Err(e) => {
                    println!("❌ [COMPRESSION_JOB] Document {} compression failed after {:?}: {:?}", document_id, duration, e);
                    
                    // Check if this is a PDF skip or validation skip that should be marked as skipped
                    let should_skip = if let ServiceError::Domain(DomainError::Validation(ref validation_err)) = e {
                        let msg = validation_err.to_string();
                        msg.contains("PDF compression skipped") ||
                        msg.contains("already compressed format") ||
                        msg.contains("would not reduce file size significantly") ||
                        msg.contains("below minimum size") ||
                        msg.contains("too large for compression")
                    } else {
                        false
                    };
                    
                    if should_skip {
                        // Mark as skipped instead of failed
                        if let Err(e) = compression_repo.update_queue_entry_status_with_tx(
                            queue_entry.id,
                            "skipped",
                            Some("Compression skipped - file already optimized or not suitable"),
                            &mut tx
                        ).await {
                            println!("❌ [COMPRESSION_JOB] Failed to update status to skipped: {:?}", e);
                            let _ = tx.rollback().await;
                            return;
                        }
                    } else {
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
    
    /// Get iOS-enhanced worker status
    async fn get_ios_worker_status(&self) -> IOSWorkerStatus {
        let jobs = self.active_jobs.lock().await;
        let running_document_ids: Vec<Uuid> = jobs.keys().copied().collect();
        let ios_state = self.ios_state.read().await.clone();
        let is_paused = *self.is_paused.read().await;
        let pause_reason = self.pause_reason.read().await.clone();
        let effective_max_jobs = self.calculate_effective_max_jobs().await;
        
        IOSWorkerStatus {
            active_jobs: jobs.len(),
            max_concurrent_jobs: self.max_concurrent_jobs,
            effective_max_jobs,
            queue_poll_interval_ms: self.interval_ms,
            running_document_ids,
            ios_state,
            is_throttled: is_paused || effective_max_jobs < self.max_concurrent_jobs,
            throttle_reason: if is_paused { pause_reason } else { None },
        }
    }
    
    /// Update iOS device state from Swift
    async fn update_ios_state(
        &self,
        battery_level: f32,
        is_charging: bool,
        thermal_state: IOSThermalState,
        app_state: IOSAppState,
        available_memory_mb: Option<u64>,
    ) {
        let mut state = self.ios_state.write().await;
        state.battery_level = battery_level;
        state.is_charging = is_charging;
        state.thermal_state = thermal_state;
        state.app_state = app_state;
        state.available_memory_mb = available_memory_mb;
        state.last_updated = Utc::now();
        
        println!("🍎 [COMPRESSION_WORKER] iOS state updated: battery={:.0}%, charging={}, thermal={:?}, app={:?}", 
                 battery_level * 100.0, is_charging, thermal_state, app_state);
        
        // Auto-adjust worker behavior based on new state
        self.auto_adjust_for_ios_state().await;
    }
    
    /// Handle iOS memory pressure
    async fn handle_memory_pressure(&self, level: u8) {
        *self.last_memory_warning.write().await = Some(Instant::now());
        
        match level {
            0 => {
                println!("🟢 [iOS MEMORY] Normal pressure - resuming operations");
                // Only resume if paused for memory reasons
                let pause_reason = self.pause_reason.read().await.clone();
                if let Some(reason) = &pause_reason {
                    if reason.contains("memory") || reason.contains("Memory") {
                        self.set_paused(false, None).await;
                    }
                }
            },
            1 => {
                println!("🟡 [iOS MEMORY] Warning pressure - reducing activity");
                // Update iOS optimizations to reduce concurrent jobs
                let mut optimizations = self.ios_optimizations.write().await;
                optimizations.background_processing_limit = 1;
                optimizations.max_memory_usage_mb = 50; // Reduce memory limit
                println!("🟡 [iOS MEMORY] Reduced memory limits due to pressure");
            },
            2 => {
                println!("🔴 [iOS MEMORY] Critical pressure - pausing compression");
                self.set_paused(true, Some("Critical memory pressure".to_string())).await;
                
                // Cancel non-essential active jobs
                let jobs = self.active_jobs.lock().await;
                let job_count = jobs.len();
                if job_count > 1 {
                    println!("🔴 [iOS MEMORY] Cancelling {} non-essential jobs due to critical memory pressure", job_count - 1);
                    // Keep only the first job, cancel the rest
                    let mut jobs_to_cancel = Vec::new();
                    for (i, (document_id, _)) in jobs.iter().enumerate() {
                        if i > 0 {
                            jobs_to_cancel.push(*document_id);
                        }
                    }
                    drop(jobs);
                    
                    for document_id in jobs_to_cancel {
                        self.cancel_document_job(document_id).await;
                    }
                }
            },
            _ => {
                println!("⚠️ [iOS MEMORY] Unknown pressure level: {}", level);
            }
        }
    }
    
    /// Set paused state
    async fn set_paused(&self, paused: bool, reason: Option<String>) {
        *self.is_paused.write().await = paused;
        *self.pause_reason.write().await = reason.clone();
        
        if paused {
            if let Some(reason) = &reason {
                println!("⏸️ [COMPRESSION_WORKER] Paused: {}", reason);
            } else {
                println!("⏸️ [COMPRESSION_WORKER] Paused");
            }
        } else {
            println!("▶️ [COMPRESSION_WORKER] Resumed");
        }
    }
    
    /// Calculate effective max jobs based on iOS state
    async fn calculate_effective_max_jobs(&self) -> usize {
        let ios_state = self.ios_state.read().await;
        let optimizations = self.ios_optimizations.read().await;
        let device_caps = self.device_capabilities.read().await;
        
        let mut effective_max = self.max_concurrent_jobs.min(device_caps.get_safe_concurrency());
        
        // Reduce jobs when in background
        if ios_state.app_state == IOSAppState::Background {
            effective_max = effective_max.min(optimizations.background_processing_limit);
        }
        
        // Reduce jobs on low battery (if not charging)
        if !ios_state.is_charging && ios_state.battery_level < optimizations.min_battery_level {
            effective_max = effective_max.min(1);
        }
        
        // Reduce jobs on thermal pressure
        match ios_state.thermal_state {
            IOSThermalState::Nominal => {}, // No reduction
            IOSThermalState::Fair => effective_max = effective_max.min(2),
            IOSThermalState::Serious => effective_max = effective_max.min(1),
            IOSThermalState::Critical => effective_max = 0, // Stop all jobs
        }
        
        // Additional time-based battery optimization
        if !ios_state.is_charging {
            let current_hour = chrono::Utc::now().hour();
            if current_hour > 1 && current_hour < 6 {
                // Nighttime - more aggressive battery saving
                effective_max = effective_max.min(1);
            }
        }
        
        effective_max
    }
    
    /// Auto-adjust worker behavior based on iOS state
    async fn auto_adjust_for_ios_state(&self) {
        let ios_state = self.ios_state.read().await;
        let optimizations = self.ios_optimizations.read().await;
        
        // Check if we should pause due to critical thermal state
        if ios_state.thermal_state == IOSThermalState::Critical && optimizations.pause_on_critical_thermal {
            self.set_paused(true, Some("Critical thermal state".to_string())).await;
            return;
        }
        
        // Check if we should pause due to low battery
        if !ios_state.is_charging && 
           ios_state.battery_level < optimizations.min_battery_level && 
           optimizations.respect_low_power_mode {
            self.set_paused(true, Some(format!("Low battery: {:.0}%", ios_state.battery_level * 100.0))).await;
            return;
        }
        
        // Resume if conditions are good and we're currently paused for iOS reasons
        let is_paused = *self.is_paused.read().await;
        let pause_reason = self.pause_reason.read().await.clone();
        
        if is_paused {
            if let Some(reason) = &pause_reason {
                if reason.contains("thermal") || reason.contains("battery") || reason.contains("Low battery") {
                    // iOS-related pause - check if we can resume
                    if ios_state.thermal_state != IOSThermalState::Critical &&
                       (ios_state.is_charging || ios_state.battery_level >= optimizations.min_battery_level) {
                        self.set_paused(false, None).await;
                    }
                }
            }
        }
    }
    
    /// Handle background task extension from iOS
    async fn handle_background_task_extension(&self, granted_seconds: u32) {
        *self.background_task_remaining_seconds.write().await = granted_seconds;
        println!("🍎 [COMPRESSION_WORKER] Background task extended: {} seconds remaining", granted_seconds);
        
        // If we have very little time left, pause new jobs
        if granted_seconds < 10 {
            self.set_paused(true, Some("Background task time running out".to_string())).await;
        } else if granted_seconds > 20 {
            // If we have sufficient time, resume if paused for background reasons
            let pause_reason = self.pause_reason.read().await.clone();
            if let Some(reason) = &pause_reason {
                if reason.contains("Background task") {
                    self.set_paused(false, None).await;
                }
            }
        }
    }
    
    /// Handle content visibility change
    async fn handle_content_visibility(&self, is_visible: bool) {
        *self.is_content_visible.write().await = is_visible;
        
        if is_visible {
            println!("👀 [COMPRESSION_WORKER] Content visible - resuming operations");
            // Resume if paused for visibility reasons
            let pause_reason = self.pause_reason.read().await.clone();
            if let Some(reason) = &pause_reason {
                if reason.contains("content visible") {
                    self.set_paused(false, None).await;
                }
            }
        } else {
            println!("🙈 [COMPRESSION_WORKER] Content not visible - pausing operations");
            self.set_paused(true, Some("Content not visible".to_string())).await;
        }
    }
    
    /// Handle iOS app lifecycle events
    async fn handle_app_lifecycle_event(&self, event: &str) {
        match event {
            "entering_background" => {
                println!("📱 [COMPRESSION_WORKER] App entering background");
                // Request background task extension via FFI callback
                self.request_background_task_extension().await;
            },
            "becoming_active" => {
                println!("📱 [COMPRESSION_WORKER] App becoming active");
                // Resume if paused for background reasons
                let pause_reason = self.pause_reason.read().await.clone();
                if let Some(reason) = &pause_reason {
                    if reason.contains("background") || reason.contains("Background") {
                        self.set_paused(false, None).await;
                    }
                }
            },
            "resigned_active" => {
                println!("📱 [COMPRESSION_WORKER] App resigned active");
                // Prepare for potential backgrounding
                // Don't pause immediately, wait for entering_background
            },
            _ => {
                println!("📱 [COMPRESSION_WORKER] Unknown app lifecycle event: {}", event);
            }
        }
    }
    
    /// Request background task extension (to be called from Swift)
    async fn request_background_task_extension(&self) {
        // This would typically trigger a callback to Swift code
        println!("🍎 [COMPRESSION_WORKER] Requesting background task extension from iOS");
        // In a real implementation, you'd call a Swift callback here
        // For now, we'll assume a default 30-second extension
        *self.background_task_remaining_seconds.write().await = 30;
    }
}

// Note: Original file deletion is now handled by the CompressionService itself
// via the queue_original_for_safe_deletion() method with proper verification and grace period

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