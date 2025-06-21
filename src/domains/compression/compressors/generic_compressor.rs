 //! Generic compression implementation for any file type

use async_trait::async_trait;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;
use tokio::task;

use crate::errors::{DomainError, DomainResult};
use super::Compressor;
use crate::domains::compression::types::CompressionMethod;

/// Generic compressor using flate2 for lossless compression
pub struct GenericCompressor;

#[async_trait]
impl Compressor for GenericCompressor {
    async fn can_handle(&self, _mime_type: &str, _extension: Option<&str>) -> bool {
        // Fallback compressor - handles any file type
        true
    }
    
    fn compressor_name(&self) -> &'static str {
        "GenericCompressor"
    }
    
    async fn compress(
        &self,
        data: Vec<u8>,
        method: CompressionMethod,
        quality_level: i32,
    ) -> DomainResult<Vec<u8>> {
        println!("🗜️ [GENERIC_COMPRESSOR] Starting generic compression: {} bytes, method: {:?}, quality: {}", 
                 data.len(), method, quality_level);
        
        // Don't try to compress if method is None
        if method == CompressionMethod::None {
            println!("🗜️ [GENERIC_COMPRESSOR] Method is None, returning original data");
            return Ok(data);
        }
        
        // Map quality level (1-100) to compression level (0-9)
        let level = match quality_level {
            1..=10 => 1,
            11..=20 => 2,
            21..=30 => 3,
            31..=40 => 4,
            41..=50 => 5,
            51..=60 => 6,
            61..=70 => 7,
            71..=80 => 8,
            _ => 9,
        };
        
        println!("🗜️ [GENERIC_COMPRESSOR] Using compression level: {}", level);
        
        // Run compression in a blocking task
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            println!("🗜️ [GENERIC_COMPRESSOR] Creating gzip encoder...");
            let mut encoder = GzEncoder::new(Vec::new(), Compression::new(level));
            
            println!("🗜️ [GENERIC_COMPRESSOR] Writing data to encoder...");
            encoder.write_all(&data)
                .map_err(|e| {
                    println!("❌ [GENERIC_COMPRESSOR] Compression write error: {}", e);
                    DomainError::Internal(format!("Compression write error: {}", e))
                })?;
                
            println!("🗜️ [GENERIC_COMPRESSOR] Finishing compression...");
            let compressed_data = encoder.finish()
                .map_err(|e| {
                    println!("❌ [GENERIC_COMPRESSOR] Compression finish error: {}", e);
                    DomainError::Internal(format!("Compression finish error: {}", e))
                })?;
            
            println!("✅ [GENERIC_COMPRESSOR] Compression successful: {} bytes -> {} bytes", 
                     data.len(), compressed_data.len());
            
            Ok(compressed_data)
        }).await.map_err(|e| {
            println!("❌ [GENERIC_COMPRESSOR] Task join error: {}", e);
            DomainError::Internal(format!("Task join error: {}", e))
        })?
    }
}