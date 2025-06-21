use crate::domains::export::types::*;
use async_trait::async_trait;
use futures::stream::Stream;
use serde::Serialize;
use std::path::Path;
use tokio::io::AsyncWrite;
use std::sync::Arc;

/// Modern streaming export writer trait - using enum for trait objects
#[async_trait]
pub trait StreamingExportWriter: Send + Sync {
    /// Write a stream of JSON values with backpressure support
    async fn write_json_stream(&mut self, stream: Box<dyn Stream<Item = Result<serde_json::Value, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError>;
    
    /// Write a stream of record batches for parquet
    async fn write_batch_stream(&mut self, stream: Box<dyn Stream<Item = Result<arrow::record_batch::RecordBatch, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError>;
    
    /// Flush any buffered data
    async fn flush(&mut self) -> Result<(), ExportError>;
    
    /// Finalize the export and return metadata
    async fn finalize(self: Box<Self>) -> Result<ExportMetadata, ExportError>;
    
    /// Get current format
    fn format(&self) -> ExportFormat;
    
    /// Check if can handle memory pressure
    fn can_handle_pressure(&self, level: MemoryPressureLevel) -> bool;
    
    /// Get optimal batch size for current conditions
    fn optimal_batch_size(&self) -> usize;
}

/// Unified export writer for backwards compatibility
#[async_trait]
pub trait UnifiedExportWriter: Send + Sync {
    async fn write_json_entity(&mut self, entity: &serde_json::Value) -> Result<(), ExportError>;
    async fn finalize(self: Box<Self>) -> Result<ExportMetadata, ExportError>;
    async fn flush(&mut self) -> Result<(), ExportError>;
    fn format(&self) -> ExportFormat;
    
    // iOS-specific interface
    fn check_memory_pressure(&self) -> Result<(), ExportError> {
        let pressure = DeviceCapabilities::current_memory_pressure();
        match pressure {
            MemoryPressureLevel::Critical => Err(ExportError::MemoryPressure),
            _ => Ok(())
        }
    }
    
    fn optimal_batch_size(&self) -> usize {
        DeviceCapabilities::optimal_batch_size(self.format())
    }
}

/// Device capabilities helper for iOS optimizations
pub struct DeviceCapabilities;

impl DeviceCapabilities {
    pub fn current_memory_pressure() -> MemoryPressureLevel {
        // This would call into iOS APIs via FFI
        // For now, return Normal as placeholder
        MemoryPressureLevel::Normal
    }
    
    pub fn optimal_batch_size(format: ExportFormat) -> usize {
        let base = match format {
            ExportFormat::JsonLines => 500,
            ExportFormat::Csv { .. } => 1000,
            ExportFormat::Parquet { .. } => 200,
        };
        
        let multiplier = match Self::current_memory_pressure() {
            MemoryPressureLevel::Normal => 1.0,
            MemoryPressureLevel::Warning => 0.5,
            MemoryPressureLevel::Critical => 0.2,
        };
        
        (base as f32 * multiplier) as usize
    }
    
    pub fn device_tier() -> DeviceTier {
        // Detect based on device model via FFI
        // Placeholder implementation
        DeviceTier::Pro
    }
    
    pub fn current_thermal_state() -> ThermalState {
        // Get thermal state from iOS
        ThermalState::Nominal
    }
}

/// Writer factory for creating format-specific writers
pub struct WriterFactory;

impl WriterFactory {
    pub fn create_writer(
        format: ExportFormat,
        output_path: &Path,
    ) -> Result<impl UnifiedExportWriter, ExportError> {
        match format {
            ExportFormat::JsonLines => {
                use crate::domains::export::service::OptimizedJsonLWriter;
                let writer = OptimizedJsonLWriter::new(output_path)
                    .map_err(|e| ExportError::Io(e.to_string()))?;
                Ok(writer)
            }
            ExportFormat::Csv { .. } => {
                // For now, CSV writers implement StreamingExportWriter, not UnifiedExportWriter
                // This is a placeholder until we create a bridge implementation
                Err(ExportError::InvalidConfig("CSV writer requires streaming interface".to_string()))
            }
            ExportFormat::Parquet { .. } => {
                // For now, Parquet writers implement StreamingExportWriter, not UnifiedExportWriter
                // This is a placeholder until we create a bridge implementation
                Err(ExportError::InvalidConfig("Parquet writer requires streaming interface".to_string()))
            }
        }
    }
}

/// Helper trait for estimating memory usage of serializable types
pub trait MemoryEstimate {
    fn estimated_size(&self) -> usize;
}

impl MemoryEstimate for serde_json::Value {
    fn estimated_size(&self) -> usize {
        match self {
            serde_json::Value::Null => 4,
            serde_json::Value::Bool(_) => 1,
            serde_json::Value::Number(_) => 8,
            serde_json::Value::String(s) => s.len() + 24,
            serde_json::Value::Array(arr) => {
                arr.iter().map(|v| v.estimated_size()).sum::<usize>() + 24
            }
            serde_json::Value::Object(obj) => {
                obj.iter()
                    .map(|(k, v)| k.len() + v.estimated_size())
                    .sum::<usize>() + 24
            }
        }
    }
}

/// Adaptive buffer manager for streaming operations
pub struct AdaptiveBufferManager {
    current_size: usize,
    max_size: usize,
    min_size: usize,
}

impl AdaptiveBufferManager {
    pub fn new() -> Self {
        Self {
            current_size: 64 * 1024, // 64KB
            max_size: 4 * 1024 * 1024, // 4MB
            min_size: 16 * 1024, // 16KB
        }
    }
    
    pub fn adjust_for_pressure(&mut self, pressure: MemoryPressureLevel) {
        self.current_size = match pressure {
            MemoryPressureLevel::Normal => self.max_size,
            MemoryPressureLevel::Warning => self.max_size / 2,
            MemoryPressureLevel::Critical => self.min_size,
        };
    }
    
    pub fn adjust_for_thermal(&mut self, thermal: ThermalState) {
        let thermal_multiplier = match thermal {
            ThermalState::Nominal => 1.0,
            ThermalState::Fair => 0.8,
            ThermalState::Serious => 0.6,
            ThermalState::Critical => 0.4,
        };
        
        self.current_size = ((self.current_size as f32) * thermal_multiplier) as usize;
        self.current_size = self.current_size.max(self.min_size);
    }
    
    pub fn buffer_size(&self) -> usize {
        self.current_size
    }
}

/// iOS-optimized memory pool for efficient buffer reuse
pub struct IOSMemoryPool {
    buffers: Arc<tokio::sync::Mutex<Vec<Vec<u8>>>>,
    max_buffers: usize,
    buffer_size: usize,
    memory_observer: crate::domains::export::ios::memory::MemoryPressureObserver,
    thermal_monitor: crate::domains::export::ios::memory::ThermalMonitor,
}

impl IOSMemoryPool {
    pub fn new(max_buffers: usize, buffer_size: usize) -> Self {
        Self {
            buffers: Arc::new(tokio::sync::Mutex::new(Vec::with_capacity(max_buffers))),
            max_buffers,
            buffer_size,
            memory_observer: crate::domains::export::ios::memory::MemoryPressureObserver::new(),
            thermal_monitor: crate::domains::export::ios::memory::ThermalMonitor::new(),
        }
    }
    
    /// Get buffer with iOS-specific optimizations
    pub async fn get_buffer(&self) -> Vec<u8> {
        // Check memory pressure before allocation
        match self.memory_observer.current_level() {
            MemoryPressureLevel::Critical => {
                // Force cleanup of existing buffers
                self.clear_all_buffers().await;
                // Return minimal buffer
                return Vec::with_capacity(self.buffer_size / 4);
            }
            MemoryPressureLevel::Warning => {
                // Return smaller buffer
                return self.get_or_create_buffer(self.buffer_size / 2).await;
            }
            MemoryPressureLevel::Normal => {
                // Normal operation
            }
        }
        
        // Check thermal state
        let buffer_size = match self.thermal_monitor.current_state() {
            ThermalState::Critical => self.buffer_size / 4,
            ThermalState::Serious => self.buffer_size / 2,
            ThermalState::Fair => (self.buffer_size as f32 * 0.8) as usize,
            ThermalState::Nominal => self.buffer_size,
        };
        
        self.get_or_create_buffer(buffer_size).await
    }
    
    async fn get_or_create_buffer(&self, size: usize) -> Vec<u8> {
        let mut buffers = self.buffers.lock().await;
        
        // Try to reuse existing buffer
        if let Some(mut buffer) = buffers.pop() {
            buffer.clear();
            buffer.reserve(size);
            buffer
        } else {
            // Create new buffer with capacity
            Vec::with_capacity(size)
        }
    }
    
    /// Return buffer to pool for reuse
    pub async fn return_buffer(&self, mut buffer: Vec<u8>) {
        // Only keep buffer if not under memory pressure
        if matches!(self.memory_observer.current_level(), MemoryPressureLevel::Critical) {
            return; // Drop buffer immediately
        }
        
        let mut buffers = self.buffers.lock().await;
        
        // Only store if we have space and buffer is reasonable size
        if buffers.len() < self.max_buffers && buffer.capacity() <= self.buffer_size * 2 {
            buffer.clear();
            buffer.shrink_to(self.buffer_size); // Shrink if oversized
            buffers.push(buffer);
        }
        // Otherwise let buffer drop
    }
    
    /// Clear all buffers under memory pressure
    async fn clear_all_buffers(&self) {
        let mut buffers = self.buffers.lock().await;
        buffers.clear();
    }
    
    /// Get pool statistics for monitoring
    pub async fn stats(&self) -> IOSMemoryPoolStats {
        let buffers = self.buffers.lock().await;
        let total_memory = buffers.iter().map(|b| b.capacity()).sum::<usize>();
        
        IOSMemoryPoolStats {
            buffer_count: buffers.len(),
            total_memory_bytes: total_memory,
            memory_pressure: self.memory_observer.current_level(),
            thermal_state: self.thermal_monitor.current_state(),
        }
    }
}

/// Statistics for iOS memory pool monitoring
#[derive(Debug, Clone)]
pub struct IOSMemoryPoolStats {
    pub buffer_count: usize,
    pub total_memory_bytes: usize,
    pub memory_pressure: MemoryPressureLevel,
    pub thermal_state: ThermalState,
}

/// Global iOS memory pool instance
static IOS_MEMORY_POOL: std::sync::OnceLock<IOSMemoryPool> = std::sync::OnceLock::new();

/// Get or initialize the global iOS memory pool
pub fn get_ios_memory_pool() -> &'static IOSMemoryPool {
    IOS_MEMORY_POOL.get_or_init(|| {
        let max_buffers = match crate::domains::export::ios::memory::ios_device_tier() {
            DeviceTier::Max => 16,
            DeviceTier::Pro => 8,
            DeviceTier::Standard => 4,
            DeviceTier::Basic => 2,
        };
        
        IOSMemoryPool::new(max_buffers, 1024 * 1024) // 1MB buffers
    })
}