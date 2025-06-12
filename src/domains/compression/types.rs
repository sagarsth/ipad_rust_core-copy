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