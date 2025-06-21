use crate::domains::export::types::*;
use crate::domains::export::service_v2::ExportProgress;
use crate::domains::export::ios::memory::*;
use std::sync::Arc;
use tokio::sync::{watch, Mutex};
use tokio::time::{Duration, interval};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::ffi::{CString, c_void, c_char};
use async_trait::async_trait;

/// iOS Background Processing Task wrapper with proper lifetime management
pub struct BGProcessingTask {
    pub identifier: String,
    pub requires_network_connectivity: bool,
    pub requires_external_power: bool,
    pub expiration_handler: Option<Box<dyn Fn() + Send + Sync>>,
}

/// Safe background task handle with automatic cleanup
pub struct SafeBackgroundTaskHandle {
    task_id: i32,
    identifier: String,
    is_active: Arc<AtomicBool>,
}

impl SafeBackgroundTaskHandle {
    pub fn new(identifier: String) -> Result<Self, ExportError> {
        let is_active = Arc::new(AtomicBool::new(false));
        
        // Create a stable callback context using Arc instead of raw pointers
        let callback_context = Arc::new(BackgroundTaskCallbackContext {
            is_active: is_active.clone(),
        });
        
        // Convert Arc to raw pointer safely
        let context_ptr = Arc::into_raw(callback_context) as *mut c_void;
        
        unsafe {
            let c_identifier = CString::new(identifier.clone())
                .map_err(|_| ExportError::InvalidConfig("Invalid task identifier".to_string()))?;
            
            let task_id = ios_begin_background_task_safe(
                c_identifier.as_ptr(),
                safe_background_time_callback,
                context_ptr,
            );
            
            if task_id == -1 {
                // Clean up the Arc we created
                let _ = Arc::from_raw(context_ptr as *const BackgroundTaskCallbackContext);
                return Err(ExportError::BackgroundTaskExpired);
            }
            
            is_active.store(true, Ordering::Relaxed);
            
            Ok(Self {
                task_id,
                identifier,
                is_active,
            })
        }
    }
    
    pub fn is_active(&self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }
    
    pub fn remaining_time(&self) -> f64 {
        if self.is_active() {
            unsafe { ios_background_time_remaining() }
        } else {
            0.0
        }
    }
}

impl Drop for SafeBackgroundTaskHandle {
    fn drop(&mut self) {
        if self.is_active.load(Ordering::Relaxed) {
            self.is_active.store(false, Ordering::Relaxed);
            unsafe {
                ios_end_background_task_safe(self.task_id);
            }
        }
    }
}

/// Safe callback context with proper lifetime management
struct BackgroundTaskCallbackContext {
    is_active: Arc<AtomicBool>,
}

/// Safe callback function that doesn't rely on raw self pointers
extern "C" fn safe_background_time_callback(context: *mut c_void) {
    if context.is_null() {
        return;
    }
    
    unsafe {
        // Safely reconstruct Arc from pointer
        let callback_context = Arc::from_raw(context as *const BackgroundTaskCallbackContext);
        
        // Mark as inactive
        callback_context.is_active.store(false, Ordering::Relaxed);
        
        // Immediately recreate Arc to prevent dropping
        let _ = Arc::into_raw(callback_context);
    }
}

/// Modern background exporter with iOS 17 features and safe task management
pub struct ModernBackgroundExporter {
    processing_task: Arc<Mutex<Option<BGProcessingTask>>>,
    progress: Arc<watch::Sender<ExportProgress>>,
    checkpoint_manager: CheckpointManager,
    background_task_handle: Arc<Mutex<Option<SafeBackgroundTaskHandle>>>,
    is_background: Arc<AtomicBool>,
}

impl ModernBackgroundExporter {
    pub fn new() -> Self {
        let (progress_tx, _) = watch::channel(ExportProgress {
            job_id: Uuid::new_v4(),
            completed_bytes: 0,
            total_bytes: 0,
            entities_processed: 0,
            current_domain: String::new(),
            estimated_time_remaining: 0.0,
            status: ExportStatus::Queued,
        });
        
        Self {
            processing_task: Arc::new(Mutex::new(None)),
            progress: Arc::new(progress_tx),
            checkpoint_manager: CheckpointManager::new(),
            background_task_handle: Arc::new(Mutex::new(None)),
            is_background: Arc::new(AtomicBool::new(false)),
        }
    }
    
    /// Export with automatic resume capability
    pub async fn export_with_resume(&self, request: ExportRequest) -> Result<ExportSummary, ExportError> {
        let request_id = Uuid::new_v4();
        
        // Check for existing checkpoint
        let checkpoint = self.checkpoint_manager.load_checkpoint(&request_id).await?;
        
        // Configure background task
        let mut task = BGProcessingTask {
            identifier: format!("com.actionaid.export.{}", request_id),
            requires_network_connectivity: false,
            requires_external_power: self.should_require_power(&request),
            expiration_handler: None,
        };
        
        // Set up expiration handler
        let checkpoint_manager = self.checkpoint_manager.clone();
        let progress = self.progress.clone();
        task.expiration_handler = Some(Box::new(move || {
            let checkpoint_manager = checkpoint_manager.clone();
            let progress = progress.clone();
            
            tokio::spawn(async move {
                // Create checkpoint from current progress
                let current_progress = progress.borrow().clone();
                let checkpoint = ExportCheckpoint::from_progress(current_progress, request_id);
                let _ = checkpoint_manager.save(request_id, checkpoint).await;
            });
        }));
        
        // Store task
        *self.processing_task.lock().await = Some(task);
        
        // Begin iOS background task
        self.begin_background_task().await?;
        
        // Run export with checkpoints
        let result = self.run_export_with_checkpoints(request, checkpoint, request_id).await;
        
        // End background task
        self.end_background_task().await;
        
        result
    }
    
    /// Determine if external power should be required
    fn should_require_power(&self, request: &ExportRequest) -> bool {
        // Require power for large exports or Parquet format
        matches!(request.format, Some(ExportFormat::Parquet { .. })) || 
        self.estimate_export_size_gb(request) > 1.0
    }
    
    /// Estimate export size in GB
    fn estimate_export_size_gb(&self, request: &ExportRequest) -> f64 {
        // Simple heuristic based on filters and format
        let entity_count = request.filters.len() * 1000; // Rough estimate
        let bytes_per_entity = match &request.format {
            Some(ExportFormat::Parquet { .. }) => 200, // Compressed
            Some(ExportFormat::Csv { compress: true, .. }) => 150,
            Some(ExportFormat::Csv { .. }) => 500,
            _ => 300, // JSONL
        };
        
        (entity_count * bytes_per_entity) as f64 / (1024.0 * 1024.0 * 1024.0)
    }
    
    /// Begin iOS background task with safe handle management
    async fn begin_background_task(&self) -> Result<(), ExportError> {
        self.is_background.store(true, Ordering::Relaxed);
        
        // Create safe background task handle
        let task_identifier = format!("export_task_{}", Uuid::new_v4());
        let handle = SafeBackgroundTaskHandle::new(task_identifier)?;
        
        // Store the handle
        *self.background_task_handle.lock().await = Some(handle);
        
        Ok(())
    }
    
    /// End iOS background task with safe cleanup
    async fn end_background_task(&self) {
        self.is_background.store(false, Ordering::Relaxed);
        
        // Drop the handle, which will automatically clean up the background task
        *self.background_task_handle.lock().await = None;
    }
    
    /// Run export with checkpoint support
    async fn run_export_with_checkpoints(
        &self,
        request: ExportRequest,
        start_checkpoint: Option<ExportCheckpoint>,
        request_id: Uuid,
    ) -> Result<ExportSummary, ExportError> {
        let mut current_state = start_checkpoint.unwrap_or_else(|| {
            ExportCheckpoint::new(request_id, &request)
        });
        
        let mut checkpoint_interval = interval(Duration::from_secs(30));
        
        loop {
            // Check background time using safe handle
            let time_remaining = {
                let handle_guard = self.background_task_handle.lock().await;
                match handle_guard.as_ref() {
                    Some(handle) => handle.remaining_time(),
                    None => 0.0,
                }
            };
            
            if time_remaining < 5.0 {
                // Save checkpoint before expiration
                self.checkpoint_manager.save(request_id, current_state.clone()).await?;
                return Err(ExportError::BackgroundTimeExpired);
            }
            
            // Process batch
            tokio::select! {
                result = self.process_batch(&request, &mut current_state) => {
                    match result {
                        Ok(completed) => {
                            if completed {
                                // Export completed
                                self.checkpoint_manager.delete(request_id).await?;
                                return Ok(self.create_summary(current_state));
                            }
                        }
                        Err(e) => {
                            // Save checkpoint on error
                            self.checkpoint_manager.save(request_id, current_state).await?;
                            return Err(e);
                        }
                    }
                }
                _ = checkpoint_interval.tick() => {
                    // Periodic checkpoint
                    self.checkpoint_manager.save(request_id, current_state.clone()).await?;
                }
            }
            
            // Update progress
            self.update_progress(&current_state).await;
            
            // Yield for iOS
            if self.is_background.load(Ordering::Relaxed) {
                tokio::task::yield_now().await;
            }
        }
    }
    
    /// Process a batch of export data
    async fn process_batch(
        &self,
        request: &ExportRequest,
        state: &mut ExportCheckpoint,
    ) -> Result<bool, ExportError> {
        // Simulate processing based on format
        let batch_size = match &request.format {
            Some(ExportFormat::Parquet { .. }) => 50,  // Parquet processes larger batches
            Some(ExportFormat::Csv { .. }) => 100,
            _ => 200, // JSONL is fastest
        };
        
        // Simulate processing time
        let processing_delay = if self.is_background.load(Ordering::Relaxed) {
            Duration::from_millis(200) // Slower in background
        } else {
            Duration::from_millis(50)
        };
        
        tokio::time::sleep(processing_delay).await;
        
        // Update state
        state.entities_processed += batch_size;
        state.bytes_written += batch_size * 256; // Average entity size
        state.last_processed_id = Some(Uuid::new_v4());
        state.last_checkpoint = Utc::now();
        
        // Check if we've processed enough for completion
        let completion_threshold = match &request.format {
            Some(ExportFormat::Parquet { .. }) => 1000,
            _ => 2000,
        };
        
        Ok(state.entities_processed >= completion_threshold)
    }
    
    /// Update progress for observers
    async fn update_progress(&self, state: &ExportCheckpoint) {
        let progress = ExportProgress {
            job_id: state.export_id,
            completed_bytes: state.bytes_written,
            total_bytes: state.estimated_total_bytes,
            entities_processed: state.entities_processed as u64,
            current_domain: state.current_domain.clone(),
            estimated_time_remaining: self.estimate_time_remaining(state),
            status: ExportStatus::Running,
        };
        
        let _ = self.progress.send(progress);
    }
    
    /// Estimate remaining time based on current progress
    fn estimate_time_remaining(&self, state: &ExportCheckpoint) -> f64 {
        if state.entities_processed == 0 {
            return 0.0;
        }
        
        let elapsed = Utc::now().signed_duration_since(state.started_at);
        let rate = state.entities_processed as f64 / elapsed.num_seconds() as f64;
        let remaining = state.estimated_total_entities.saturating_sub(state.entities_processed);
        
        if rate > 0.0 {
            remaining as f64 / rate
        } else {
            0.0
        }
    }
    
    /// Create final export summary
    fn create_summary(&self, state: ExportCheckpoint) -> ExportSummary {
        let job = crate::domains::export::types::ExportJob {
            id: state.export_id,
            requested_by_user_id: None,
            requested_at: state.started_at,
            include_blobs: false,
            status: ExportStatus::Completed,
            local_path: None,
            total_entities: Some(state.entities_processed as i64),
            total_bytes: Some(state.bytes_written as i64),
            error_message: None,
        };
        
        ExportSummary { job }
    }
    
    /// Get current background time remaining safely
    pub async fn background_time_remaining(&self) -> f64 {
        let handle_guard = self.background_task_handle.lock().await;
        match handle_guard.as_ref() {
            Some(handle) => handle.remaining_time(),
            None => 0.0,
        }
    }
    
    /// Get progress receiver for monitoring
    pub fn progress_receiver(&self) -> watch::Receiver<ExportProgress> {
        self.progress.subscribe()
    }
}

/// Export checkpoint for resume capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportCheckpoint {
    pub export_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub last_checkpoint: DateTime<Utc>,
    pub entities_processed: usize,
    pub bytes_written: usize,
    pub current_domain: String,
    pub last_processed_id: Option<Uuid>,
    pub estimated_total_entities: usize,
    pub estimated_total_bytes: usize,
    pub schema_version: u32,
    pub format: ExportFormat,
}

impl ExportCheckpoint {
    pub fn new(export_id: Uuid, request: &ExportRequest) -> Self {
        let estimated_entities = request.filters.len() * 1000; // Rough estimate
        let estimated_bytes = estimated_entities * 300; // Average size
        
        Self {
            export_id,
            started_at: Utc::now(),
            last_checkpoint: Utc::now(),
            entities_processed: 0,
            bytes_written: 0,
            current_domain: "strategic_goals".to_string(),
            last_processed_id: None,
            estimated_total_entities: estimated_entities,
            estimated_total_bytes: estimated_bytes,
            schema_version: 1,
            format: request.format.clone().unwrap_or(ExportFormat::JsonLines),
        }
    }
    
    pub fn from_progress(progress: ExportProgress, export_id: Uuid) -> Self {
        Self {
            export_id,
            started_at: Utc::now(), // Would be persisted from original
            last_checkpoint: Utc::now(),
            entities_processed: progress.entities_processed as usize,
            bytes_written: progress.completed_bytes,
            current_domain: progress.current_domain,
            last_processed_id: None,
            estimated_total_entities: progress.total_bytes / 300, // Estimate
            estimated_total_bytes: progress.total_bytes,
            schema_version: 1,
            format: ExportFormat::JsonLines, // Would be persisted
        }
    }
}

/// Checkpoint manager for saving/loading export state
#[derive(Clone)]
pub struct CheckpointManager {
    storage: Arc<dyn CheckpointStorage>,
}

impl CheckpointManager {
    pub fn new() -> Self {
        Self {
            storage: Arc::new(KeychainCheckpointStorage::new()),
        }
    }
    
    pub async fn save(&self, export_id: Uuid, checkpoint: ExportCheckpoint) -> Result<(), ExportError> {
        self.storage.save(&export_id.to_string(), checkpoint).await
    }
    
    pub async fn load_checkpoint(&self, export_id: &Uuid) -> Result<Option<ExportCheckpoint>, ExportError> {
        self.storage.load(&export_id.to_string()).await
    }
    
    pub async fn delete(&self, export_id: Uuid) -> Result<(), ExportError> {
        self.storage.delete(&export_id.to_string()).await
    }
}

/// Checkpoint storage trait
#[async_trait]
trait CheckpointStorage: Send + Sync {
    async fn save(&self, key: &str, checkpoint: ExportCheckpoint) -> Result<(), ExportError>;
    async fn load(&self, key: &str) -> Result<Option<ExportCheckpoint>, ExportError>;
    async fn delete(&self, key: &str) -> Result<(), ExportError>;
}

/// iOS Keychain-based checkpoint storage
struct KeychainCheckpointStorage;

impl KeychainCheckpointStorage {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl CheckpointStorage for KeychainCheckpointStorage {
    async fn save(&self, key: &str, checkpoint: ExportCheckpoint) -> Result<(), ExportError> {
        let data = serde_json::to_vec(&checkpoint)
            .map_err(|e| ExportError::SerializationError(e.to_string()))?;
        
        unsafe {
            let key_cstr = CString::new(key).unwrap();
            let result = ios_keychain_save(
                key_cstr.as_ptr(),
                data.as_ptr(),
                data.len(),
            );
            
            if result != 0 {
                return Err(ExportError::CheckpointSaveFailed);
            }
        }
        
        Ok(())
    }
    
    async fn load(&self, key: &str) -> Result<Option<ExportCheckpoint>, ExportError> {
        unsafe {
            let key_cstr = CString::new(key).unwrap();
            let mut data_ptr: *mut u8 = std::ptr::null_mut();
            let mut data_len: usize = 0;
            
            let result = ios_keychain_load(
                key_cstr.as_ptr(),
                &mut data_ptr,
                &mut data_len,
            );
            
            if result != 0 || data_ptr.is_null() {
                return Ok(None);
            }
            
            let data = std::slice::from_raw_parts(data_ptr, data_len);
            let checkpoint: ExportCheckpoint = serde_json::from_slice(data)
                .map_err(|e| ExportError::SerializationError(e.to_string()))?;
            
            // Free the allocated memory
            ios_keychain_free(data_ptr);
            
            Ok(Some(checkpoint))
        }
    }
    
    async fn delete(&self, key: &str) -> Result<(), ExportError> {
        unsafe {
            let key_cstr = CString::new(key).unwrap();
            ios_keychain_delete(key_cstr.as_ptr());
        }
        Ok(())
    }
}

// Safe FFI declarations for iOS integration
extern "C" {
    fn ios_begin_background_task_safe(
        identifier: *const c_char,
        expiration_handler: extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) -> i32;
    
    fn ios_end_background_task_safe(task_id: i32);
    fn ios_background_time_remaining() -> f64;
    
    fn ios_keychain_save(key: *const c_char, data: *const u8, len: usize) -> i32;
    fn ios_keychain_load(key: *const c_char, data: *mut *mut u8, len: *mut usize) -> i32;
    fn ios_keychain_delete(key: *const c_char);
    fn ios_keychain_free(data: *mut u8);
}

extern "C" fn background_time_callback(context: *mut c_void) {
    // Handle background time expiration - safer implementation
    unsafe {
        // Context is now a pointer to background_time_remaining
        let time_remaining_ptr = context as *const tokio::sync::Mutex<f64>;
        
        // In a production environment, you'd want to use a proper 
        // synchronization mechanism instead of raw pointers
        eprintln!("iOS background task expiration callback triggered");
        
        // Signal that background time is expired
        // In a real implementation, you'd need a thread-safe way to 
        // communicate this back to the Rust async runtime
    }
} 