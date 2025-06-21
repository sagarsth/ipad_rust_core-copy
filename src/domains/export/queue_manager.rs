use crate::domains::export::types::*;
use crate::domains::export::ios::memory::*;
use std::collections::{VecDeque, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::{Duration, interval};
use uuid::Uuid;
use chrono::{DateTime, Utc};

/// Export job for queue management
#[derive(Debug, Clone)]
pub struct ExportJob {
    pub id: Uuid,
    pub request: ExportRequest,
    pub priority: JobPriority,
    pub created_at: DateTime<Utc>,
    pub status: JobStatus,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed(String),
}

/// Handle for tracking export jobs
pub struct JobHandle {
    pub id: Uuid,
    status_receiver: tokio::sync::watch::Receiver<JobStatus>,
}

impl JobHandle {
    pub async fn wait_for_completion(&mut self) -> Result<(), ExportError> {
        loop {
            self.status_receiver.changed().await
                .map_err(|_| ExportError::ChannelClosed)?;
            
            match &*self.status_receiver.borrow() {
                JobStatus::Completed => return Ok(()),
                JobStatus::Failed(err) => return Err(ExportError::JobFailed(err.clone())),
                _ => continue,
            }
        }
    }
    
    pub fn status(&self) -> JobStatus {
        self.status_receiver.borrow().clone()
    }
}

/// Export queue manager with intelligent scheduling
pub struct ExportQueueManager {
    queue: Arc<Mutex<VecDeque<ExportJob>>>,
    active_exports: Arc<AtomicUsize>,
    max_concurrent: usize,
    semaphore: Arc<Semaphore>,
    memory_observer: MemoryPressureObserver,
    thermal_monitor: ThermalMonitor,
    job_statuses: Arc<Mutex<HashMap<Uuid, tokio::sync::watch::Sender<JobStatus>>>>,
    processor_running: Arc<Mutex<bool>>,
    job_processor: Weak<dyn crate::domains::export::service_v2::JobProcessor>,
}

impl ExportQueueManager {
    pub fn new(job_processor: Weak<dyn crate::domains::export::service_v2::JobProcessor>) -> Self {
        let max_concurrent = Self::calculate_max_concurrent();
        
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            active_exports: Arc::new(AtomicUsize::new(0)),
            max_concurrent,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            memory_observer: MemoryPressureObserver::new(),
            thermal_monitor: ThermalMonitor::new(),
            job_statuses: Arc::new(Mutex::new(HashMap::new())),
            processor_running: Arc::new(Mutex::new(false)),
            job_processor,
        }
    }
    
    fn calculate_max_concurrent() -> usize {
        match get_device_tier() {
            DeviceTier::Max => 4,
            DeviceTier::Pro => 2,
            DeviceTier::Standard => 1,
            DeviceTier::Basic => 1,
        }
    }
    
    /// Enqueue a new export job with intelligent scheduling
    pub async fn enqueue(&self, mut job: ExportJob) -> Result<JobHandle, ExportError> {
        // Check if we can accept new jobs
        self.check_queue_health().await?;
        
        // Adjust priority based on current conditions
        job.priority = self.adjust_priority(job.priority).await;
        
        // Create status channel
        let (tx, rx) = tokio::sync::watch::channel(JobStatus::Queued);
        let job_id = job.id;
        
        // Add to queue
        {
            let mut queue = self.queue.lock().await;
            let mut statuses = self.job_statuses.lock().await;
            
            // Insert based on priority
            let position = queue.iter()
                .position(|j| j.priority < job.priority)
                .unwrap_or(queue.len());
            
            queue.insert(position, job);
            statuses.insert(job_id, tx);
        }
        
        // Start processing if not already running
        self.ensure_processor_running().await;
        
        Ok(JobHandle {
            id: job_id,
            status_receiver: rx,
        })
    }
    
    /// Check system health before accepting new jobs
    async fn check_queue_health(&self) -> Result<(), ExportError> {
        // Check memory pressure
        match self.memory_observer.current_level() {
            MemoryPressureLevel::Critical => {
                return Err(ExportError::SystemOverloaded("Critical memory pressure".into()));
            }
            MemoryPressureLevel::Warning => {
                // Allow but with reduced capacity
                if self.active_exports.load(Ordering::Relaxed) >= self.max_concurrent / 2 {
                    return Err(ExportError::SystemOverloaded("High memory pressure".into()));
                }
            }
            _ => {}
        }
        
        // Check thermal state
        match self.thermal_monitor.current_state() {
            ThermalState::Critical => {
                return Err(ExportError::SystemOverloaded("Device too hot".into()));
            }
            ThermalState::Serious => {
                if self.active_exports.load(Ordering::Relaxed) > 0 {
                    return Err(ExportError::SystemOverloaded("Thermal throttling".into()));
                }
            }
            _ => {}
        }
        
        // Check queue size
        let queue_size = self.queue.lock().await.len();
        if queue_size > 100 {
            return Err(ExportError::QueueFull);
        }
        
        Ok(())
    }
    
    /// Adjust priority based on system conditions
    async fn adjust_priority(&self, base_priority: JobPriority) -> JobPriority {
        // Lower priority during thermal/memory pressure
        match (self.memory_observer.current_level(), self.thermal_monitor.current_state()) {
            (MemoryPressureLevel::Warning, _) | (_, ThermalState::Serious) => {
                match base_priority {
                    JobPriority::Critical => JobPriority::High,
                    JobPriority::High => JobPriority::Normal,
                    JobPriority::Normal => JobPriority::Low,
                    JobPriority::Low => JobPriority::Low,
                }
            }
            _ => base_priority,
        }
    }
    
    /// Ensure the processor task is running
    async fn ensure_processor_running(&self) {
        let mut running = self.processor_running.lock().await;
        if *running {
            return;
        }
        
        *running = true;
        
        let queue = self.queue.clone();
        let active = self.active_exports.clone();
        let semaphore = self.semaphore.clone();
        let statuses = self.job_statuses.clone();
        let job_processor = self.job_processor.clone();
        let processor_running = self.processor_running.clone();
        
        tokio::spawn(async move {
            loop {
                // Wait for available slot
                let permit = match semaphore.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => break,
                };
                
                // Get next job
                let job = {
                    let mut queue = queue.lock().await;
                    queue.pop_front()
                };
                
                let Some(job) = job else {
                    drop(permit);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                };
                
                // Check thermal state before processing
                let thermal_monitor = ThermalMonitor::new();
                match thermal_monitor.current_state() {
                    ThermalState::Critical => {
                        // Re-queue the job and wait
                        queue.lock().await.push_front(job);
                        drop(permit);
                        tokio::time::sleep(Duration::from_secs(10)).await;
                        continue;
                    }
                    ThermalState::Serious => {
                        // Only process if no other jobs are running
                        if active.load(Ordering::Relaxed) > 0 {
                            queue.lock().await.push_front(job);
                            drop(permit);
                            tokio::time::sleep(Duration::from_secs(5)).await;
                            continue;
                        }
                    }
                    _ => {} // Normal processing
                }
                
                // Process job using the actual job processor
                if let Some(processor) = job_processor.upgrade() {
                    // Update status
                    if let Some(tx) = statuses.lock().await.get(&job.id) {
                        let _ = tx.send(JobStatus::Running);
                    }
                    
                    active.fetch_add(1, Ordering::Relaxed);
                    
                    let job_id = job.id;
                    let statuses_clone = statuses.clone();
                    let active_clone = active.clone();
                    
                    tokio::spawn(async move {
                        let result = processor.process(job).await;
                        
                        // Update final status with better error handling
                        if let Some(tx) = statuses_clone.lock().await.get(&job_id) {
                            let status = match result {
                                Ok(_) => JobStatus::Completed,
                                Err(e) => {
                                    eprintln!("Export job {} failed: {}", job_id, e);
                                    JobStatus::Failed(e.to_string())
                                },
                            };
                            if let Err(e) = tx.send(status) {
                                eprintln!("Failed to send job status update for {}: {:?}", job_id, e);
                            }
                        }
                        
                        active_clone.fetch_sub(1, Ordering::Relaxed);
                        drop(permit);
                    });
                } else {
                    // Processor was dropped, stop processing
                    eprintln!("Job processor was dropped, stopping queue processing");
                    break;
                }
            }
            
            // Mark processor as stopped
            *processor_running.lock().await = false;
        });
    }
    
    /// Get current queue statistics
    pub async fn get_stats(&self) -> QueueStats {
        let queue = self.queue.lock().await;
        let active = self.active_exports.load(Ordering::Relaxed);
        
        QueueStats {
            queued: queue.len(),
            active,
            total_capacity: self.max_concurrent,
            memory_pressure: self.memory_observer.current_level(),
            thermal_state: self.thermal_monitor.current_state(),
        }
    }
    
    /// Pause all exports
    pub async fn pause_all(&self) -> Result<(), ExportError> {
        let queue = self.queue.lock().await;
        let statuses = self.job_statuses.lock().await;
        
        for job in queue.iter() {
            if let Some(tx) = statuses.get(&job.id) {
                let _ = tx.send(JobStatus::Paused);
            }
        }
        
        Ok(())
    }
    
    /// Resume paused exports
    pub async fn resume_all(&self) -> Result<(), ExportError> {
        let mut queue = self.queue.lock().await;
        let statuses = self.job_statuses.lock().await;
        
        for job in queue.iter_mut() {
            if job.status == JobStatus::Paused {
                job.status = JobStatus::Queued;
                if let Some(tx) = statuses.get(&job.id) {
                    let _ = tx.send(JobStatus::Queued);
                }
            }
        }
        
        Ok(())
    }
    
    /// Cancel a specific job
    pub async fn cancel_job(&self, job_id: Uuid) -> Result<(), ExportError> {
        let mut queue = self.queue.lock().await;
        let statuses = self.job_statuses.lock().await;
        
        // Remove from queue if not yet running
        queue.retain(|job| job.id != job_id);
        
        // Update status
        if let Some(tx) = statuses.get(&job_id) {
            let _ = tx.send(JobStatus::Failed("Cancelled by user".to_string()));
        }
        
        Ok(())
    }
}

// Deprecated: process_export_job function removed
// Job processing is now handled by the JobProcessor trait implementation

/// Queue statistics
#[derive(Debug, Clone)]
pub struct QueueStats {
    pub queued: usize,
    pub active: usize,
    pub total_capacity: usize,
    pub memory_pressure: MemoryPressureLevel,
    pub thermal_state: ThermalState,
}

// Helper functions that would be implemented elsewhere
fn get_device_tier() -> DeviceTier {
    // This would detect actual device capabilities
    DeviceTier::Standard
}

fn get_thermal_state() -> ThermalState {
    // This would read actual thermal state
    ThermalState::Nominal
} 