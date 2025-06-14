//! Type definitions for the compression domain.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;
use crate::errors::{DomainError, ValidationError};

/// Compression methods available in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionMethod {
    /// General-purpose lossless compression (deflate, zlib)
    Lossless,
    
    /// Lossy compression for images and media
    Lossy,
    
    /// Optimized PDF compression
    PdfOptimize,
    
    /// Office document optimization
    OfficeOptimize,
    
    /// Video container optimization and metadata removal
    VideoOptimize,
    
    /// No compression
    None,
}

impl CompressionMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionMethod::Lossless => "lossless",
            CompressionMethod::Lossy => "lossy",
            CompressionMethod::PdfOptimize => "pdf_optimize",
            CompressionMethod::OfficeOptimize => "office_optimize",
            CompressionMethod::VideoOptimize => "video_optimize",
            CompressionMethod::None => "none",
        }
    }
}

impl FromStr for CompressionMethod {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "lossless" => Ok(CompressionMethod::Lossless),
            "lossy" => Ok(CompressionMethod::Lossy),
            "pdf_optimize" => Ok(CompressionMethod::PdfOptimize),
            "office_optimize" => Ok(CompressionMethod::OfficeOptimize),
            "video_optimize" => Ok(CompressionMethod::VideoOptimize),
            "none" => Ok(CompressionMethod::None),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid compression method: {}", s)
            )))
        }
    }
}

impl From<CompressionMethod> for String {
    fn from(method: CompressionMethod) -> Self {
        method.as_str().to_string()
    }
}

/// Priority for compression operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum CompressionPriority {
    High = 10,
    Normal = 5,
    Low = 1,
    Background = 0,
}

impl CompressionPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            CompressionPriority::High => "HIGH",
            CompressionPriority::Normal => "NORMAL",
            CompressionPriority::Low => "LOW",
            CompressionPriority::Background => "BACKGROUND",
        }
    }
    
    pub fn from_i64(value: i64) -> Option<Self> {
        match value {
            10 => Some(CompressionPriority::High),
            5 => Some(CompressionPriority::Normal),
            1 => Some(CompressionPriority::Low),
            0 => Some(CompressionPriority::Background),
            _ => None,
        }
    }
}

impl From<i32> for CompressionPriority {
    fn from(value: i32) -> Self {
        match value {
            v if v >= 8 => CompressionPriority::High,
            v if v >= 3 => CompressionPriority::Normal,
            v if v >= 1 => CompressionPriority::Low,
            _ => CompressionPriority::Background,
        }
    }
}

impl From<CompressionPriority> for i32 {
    fn from(priority: CompressionPriority) -> Self {
        match priority {
            CompressionPriority::High => 10,
            CompressionPriority::Normal => 5,
            CompressionPriority::Low => 1,
            CompressionPriority::Background => 0,
        }
    }
}

impl From<CompressionPriority> for i64 {
    fn from(priority: CompressionPriority) -> Self {
        match priority {
            CompressionPriority::High => 10,
            CompressionPriority::Normal => 5,
            CompressionPriority::Low => 1,
            CompressionPriority::Background => 0,
        }
    }
}

impl FromStr for CompressionPriority {
    type Err = DomainError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "HIGH" => Ok(CompressionPriority::High),
            "NORMAL" => Ok(CompressionPriority::Normal),
            "LOW" => Ok(CompressionPriority::Low),
            "BACKGROUND" | "BG" => Ok(CompressionPriority::Background),
            _ => Err(DomainError::Validation(ValidationError::custom(
                &format!("Invalid compression priority string: {}", s)
            )))
        }
    }
}

/// Configuration for compression
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    pub method: CompressionMethod,
    pub quality_level: i32, // 0-100 for lossy, 0-9 for lossless
    pub min_size_bytes: i64, // Minimum file size to compress
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            method: CompressionMethod::Lossless,
            quality_level: 75, // Default quality level
            min_size_bytes: 10240, // 10KB minimum
        }
    }
}

/// Result from a compression operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionResult {
    pub document_id: Uuid,
    pub original_size: i64,
    pub compressed_size: i64,
    pub compressed_file_path: String,
    pub space_saved_bytes: i64,
    pub space_saved_percentage: f64,
    pub method_used: CompressionMethod,
    pub quality_level: i32,
    pub duration_ms: i64,
}

/// Entry in the compression queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionQueueEntry {
    pub id: Uuid,
    pub document_id: Uuid,
    pub priority: i32,
    pub attempts: i32,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub error_message: Option<String>,
}

/// Status of the compression queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionQueueStatus {
    pub pending_count: i64,
    pub processing_count: i64,
    pub completed_count: i64,
    pub failed_count: i64,
    pub skipped_count: i64,
}

/// iOS device capabilities and state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOSDeviceState {
    pub battery_level: f32, // 0.0 to 1.0
    pub is_charging: bool,
    pub thermal_state: IOSThermalState,
    pub app_state: IOSAppState,
    pub available_memory_mb: Option<u64>,
    pub last_updated: DateTime<Utc>,
}

/// iOS thermal states (matching iOS ProcessInfo.ThermalState)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IOSThermalState {
    Nominal = 0,    // Normal
    Fair = 1,       // Slight throttling
    Serious = 2,    // Moderate throttling
    Critical = 3,   // Heavy throttling
}

/// iOS app states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IOSAppState {
    Active,         // App is active and visible
    Background,     // App is in background
    Inactive,       // App is inactive (transitioning)
}

/// Enhanced compression configuration with iOS optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOSCompressionConfig {
    pub base_config: CompressionConfig,
    pub ios_optimizations: IOSOptimizations,
}

/// iOS-specific compression optimizations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOSOptimizations {
    pub respect_low_power_mode: bool,
    pub pause_on_critical_thermal: bool,
    pub reduce_quality_on_thermal: bool,
    pub background_processing_limit: usize, // Max jobs when backgrounded
    pub min_battery_level: f32, // Don't compress below this battery level
    pub max_memory_usage_mb: u64,
}

impl Default for IOSOptimizations {
    fn default() -> Self {
        Self {
            respect_low_power_mode: true,
            pause_on_critical_thermal: true,
            reduce_quality_on_thermal: true,
            background_processing_limit: 1, // Only 1 job in background
            min_battery_level: 0.20, // Don't compress below 20% battery
            max_memory_usage_mb: 100, // Limit memory usage to 100MB
        }
    }
}

/// Enhanced worker status with iOS information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOSWorkerStatus {
    pub active_jobs: usize,
    pub max_concurrent_jobs: usize,
    pub effective_max_jobs: usize, // Adjusted for iOS state
    pub queue_poll_interval_ms: u64,
    pub running_document_ids: Vec<Uuid>,
    pub ios_state: IOSDeviceState,
    pub is_throttled: bool,
    pub throttle_reason: Option<String>,
}

/// Compression statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionStats {
    pub total_original_size: i64,
    pub total_compressed_size: i64,
    pub space_saved: i64,
    pub compression_ratio: f64,
    pub total_files_compressed: i64,
    pub total_files_pending: i64,
    pub total_files_failed: i64,
    pub total_files_skipped: i64,
    pub last_compression_date: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// Mobile-optimized compression job for iOS
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MobileCompressionJob {
    pub document_id: Uuid,
    pub priority: u8,
    pub file_size: i64,
    pub queued_at: u64,
    pub attempts: u32,
    pub estimated_duration: u64, // seconds
}

impl MobileCompressionJob {
    pub fn new(document_id: Uuid, priority: u8, file_size: i64) -> Self {
        // Estimate compression duration based on file size
        let estimated_duration = match file_size {
            0..=1_000_000 => 5,        // < 1MB = 5 seconds
            1_000_001..=10_000_000 => 15,   // 1-10MB = 15 seconds  
            10_000_001..=50_000_000 => 45,  // 10-50MB = 45 seconds
            _ => 120,                       // > 50MB = 2 minutes
        };
        
        Self {
            document_id,
            priority,
            file_size,
            queued_at: chrono::Utc::now().timestamp() as u64,
            attempts: 0,
            estimated_duration,
        }
    }
}

/// Mobile queue statistics for iOS
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MobileQueueStats {
    pub total_queued: u64,
    pub completed: u64,
    pub failed: u64,
    pub skipped_low_battery: u64,
    pub skipped_thermal: u64,
}

/// Device type detection for iOS optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IOSDeviceType {
    IPhone,     // Most conservative limits
    IPad,       // Can handle slightly more
    IPadPro,    // Most capable
}

/// iOS device capabilities based on device type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IOSDeviceCapabilities {
    pub device_type: IOSDeviceType,
    pub max_concurrent_jobs: usize,
    pub memory_limit_mb: usize,
    pub thermal_throttle_threshold: f32,
    pub battery_level_threshold: f32,
    pub is_charging: bool,
}

impl IOSDeviceCapabilities {
    pub fn detect_ios_device() -> Self {
        // In a real iOS app, you'd get this from system APIs
        // For now, we'll use conservative defaults
        Self {
            device_type: IOSDeviceType::IPhone, // Most conservative
            max_concurrent_jobs: 1,  // Start with 1 for safety
            memory_limit_mb: 100,    // Conservative memory limit
            thermal_throttle_threshold: 0.8,
            battery_level_threshold: 0.2, // Don't compress if battery < 20%
            is_charging: false,
        }
    }
    
    pub fn should_allow_compression(&self) -> bool {
        // Don't compress if battery is too low and not charging
        if !self.is_charging && self.battery_level_threshold < 0.2 {
            return false;
        }
        true
    }
    
    pub fn get_safe_concurrency(&self) -> usize {
        match self.device_type {
            IOSDeviceType::IPhone => 1,      // Always single-threaded on iPhone
            IOSDeviceType::IPad => 2,        // iPad can handle 2 at most
            IOSDeviceType::IPadPro => 3,     // iPad Pro can handle 3
        }
    }
}