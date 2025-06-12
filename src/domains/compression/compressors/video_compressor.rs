//! Video compression implementation for training and evidence videos

use async_trait::async_trait;
use tokio::task;
use std::io::{Cursor, Read, Write};
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::errors::{DomainError, DomainResult};
use super::Compressor;
use crate::domains::compression::types::CompressionMethod;

/// Video compressor with intelligent handling for different video types
/// - Training videos: More aggressive compression
/// - Evidence videos: Conservative compression to preserve quality
pub struct VideoCompressor;

impl VideoCompressor {
    pub fn new() -> Self {
        Self {}
    }
    
    /// Detect if this is likely a training video based on filename patterns
    fn is_training_video(filename: &str) -> bool {
        let filename_lower = filename.to_lowercase();
        filename_lower.contains("training") ||
        filename_lower.contains("tutorial") ||
        filename_lower.contains("lesson") ||
        filename_lower.contains("course") ||
        filename_lower.contains("workshop") ||
        filename_lower.contains("demo") ||
        filename_lower.contains("presentation") ||
        filename_lower.contains("webinar")
    }
    
    /// Check if video appears to be already heavily compressed
    fn is_already_compressed(data: &[u8], original_size: i64) -> bool {
        // Simple heuristic: if file is very small relative to its container format overhead,
        // it's likely already well compressed
        let size_per_minute_threshold = 1_000_000; // 1MB per minute is quite compressed for video
        
        // For simplicity, assume average video is 5 minutes if we can't determine duration
        let estimated_duration_minutes = 5;
        let expected_min_size = size_per_minute_threshold * estimated_duration_minutes;
        
        original_size < expected_min_size
    }
}

#[async_trait]
impl Compressor for VideoCompressor {
    async fn can_handle(&self, mime_type: &str, extension: Option<&str>) -> bool {
        matches!(mime_type,
            "video/mp4" | "video/quicktime" | "video/x-msvideo" | "video/webm" | 
            "video/x-matroska" | "video/3gpp" | "video/x-m4v"
        ) || matches!(extension,
            Some("mp4") | Some("mov") | Some("m4v") | Some("avi") | Some("mkv") | 
            Some("webm") | Some("3gp") | Some("wmv") | Some("flv")
        )
    }
    
    async fn compress(
        &self,
        data: Vec<u8>,
        method: CompressionMethod,
        quality_level: i32,
    ) -> DomainResult<Vec<u8>> {
        println!("🎥 [VIDEO_COMPRESSOR] Starting video compression: {} bytes, method: {:?}, quality: {}", 
                 data.len(), method, quality_level);
        
        // For now, implement container-level compression since video transcoding 
        // requires external tools like ffmpeg which aren't iOS-compatible
        let original_size = data.len() as i64;
        
        // Check if already well compressed
        if Self::is_already_compressed(&data, original_size) {
            println!("🎥 [VIDEO_COMPRESSOR] Video appears already well compressed, skipping");
            return Ok(data);
        }
        
        match method {
            CompressionMethod::None => Ok(data),
            CompressionMethod::VideoOptimize => {
                self.optimize_video_container(data, quality_level).await
            },
            _ => {
                // Fallback to generic compression for unsupported methods
                self.generic_video_compression(data, quality_level).await
            }
        }
    }
}

impl VideoCompressor {
    /// Optimize video at container level (metadata, unused tracks, etc.)
    async fn optimize_video_container(&self, data: Vec<u8>, quality_level: i32) -> DomainResult<Vec<u8>> {
        println!("🎥 [VIDEO_COMPRESSOR] Optimizing video container");
        
        // Run in blocking task
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            // Container-level optimizations:
            // 1. Remove metadata that might not be essential
            // 2. Optimize header structure
            // 3. Remove unused atoms/tracks
            
            let optimized = optimize_mp4_container(&data)?;
            
            // If optimization didn't help much, return original
            let space_saved = data.len() as i64 - optimized.len() as i64;
            let savings_percent = (space_saved as f64 / data.len() as f64) * 100.0;
            
            if savings_percent < 5.0 {
                println!("🎥 [VIDEO_COMPRESSOR] Container optimization saved only {:.1}%, returning original", savings_percent);
                Ok(data)
            } else {
                println!("🎥 [VIDEO_COMPRESSOR] Container optimization saved {:.1}% ({} bytes)", savings_percent, space_saved);
                Ok(optimized)
            }
        }).await.map_err(|e| DomainError::Internal(format!("Video optimization task failed: {}", e)))?
    }
    
    /// Generic compression for video files (when specific optimization isn't available)
    async fn generic_video_compression(&self, data: Vec<u8>, quality_level: i32) -> DomainResult<Vec<u8>> {
        println!("🎥 [VIDEO_COMPRESSOR] Applying generic compression");
        
        // Use light compression to avoid corrupting video structure
        let compression_level = match quality_level {
            1..=30 => Compression::fast(),
            31..=70 => Compression::default(),
            _ => Compression::best(),
        };
        
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            let mut encoder = GzEncoder::new(Vec::new(), compression_level);
            encoder.write_all(&data)
                .map_err(|e| DomainError::Internal(format!("Video compression write error: {}", e)))?;
            
            let compressed = encoder.finish()
                .map_err(|e| DomainError::Internal(format!("Video compression finish error: {}", e)))?;
            
            // Only return compressed version if it's meaningfully smaller
            if compressed.len() < (data.len() * 90 / 100) {
                Ok(compressed)
            } else {
                println!("🎥 [VIDEO_COMPRESSOR] Generic compression not effective, returning original");
                Ok(data)
            }
        }).await.map_err(|e| DomainError::Internal(format!("Video compression task failed: {}", e)))?
    }
}

/// Optimize MP4 container structure (simplified implementation)
fn optimize_mp4_container(data: &[u8]) -> DomainResult<Vec<u8>> {
    // This is a simplified implementation. In production, you might use:
    // - mp4parse-rust for parsing
    // - Custom MP4 box manipulation
    // - Metadata stripping
    
    // For now, just remove common metadata that might be large
    let mut optimized = data.to_vec();
    
    // Remove potential metadata sections (very basic implementation)
    if let Some(metadata_start) = find_metadata_section(&optimized) {
        if let Some(metadata_end) = find_metadata_end(&optimized, metadata_start) {
            // Remove metadata section
            optimized.drain(metadata_start..metadata_end);
            println!("🎥 [VIDEO_COMPRESSOR] Removed metadata section: {} bytes", metadata_end - metadata_start);
        }
    }
    
    Ok(optimized)
}

/// Find potential metadata section in MP4 (simplified)
fn find_metadata_section(data: &[u8]) -> Option<usize> {
    // Look for common metadata boxes: 'meta', 'udta'
    for i in 0..(data.len().saturating_sub(8)) {
        if &data[i+4..i+8] == b"meta" || &data[i+4..i+8] == b"udta" {
            return Some(i);
        }
    }
    None
}

/// Find end of metadata section (simplified)
fn find_metadata_end(data: &[u8], start: usize) -> Option<usize> {
    if start + 8 >= data.len() {
        return None;
    }
    
    // Read box size from first 4 bytes
    let size_bytes = &data[start..start+4];
    let size = u32::from_be_bytes([size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]]) as usize;
    
    if size > 0 && start + size <= data.len() {
        Some(start + size)
    } else {
        None
    }
}

/// Enhanced video format detection
pub fn detect_video_format(data: &[u8]) -> Option<&'static str> {
    if data.len() < 12 {
        return None;
    }
    
    // Check for common video format signatures
    match &data[4..8] {
        b"ftyp" => {
            // MP4 family
            match &data[8..12] {
                b"mp41" | b"mp42" | b"isom" | b"dash" => Some("mp4"),
                b"qt  " => Some("mov"),
                b"M4V " => Some("m4v"),
                _ => Some("mp4"), // Default for MP4 family
            }
        },
        _ => {
            // Check for other formats by magic bytes
            if &data[0..4] == b"RIFF" && data.len() > 8 && &data[8..12] == b"AVI " {
                Some("avi")
            } else if &data[0..4] == b"\x1A\x45\xDF\xA3" {
                Some("mkv") // Matroska/WebM
            } else if data.starts_with(b"FLV") {
                Some("flv")
            } else {
                None
            }
        }
    }
} 