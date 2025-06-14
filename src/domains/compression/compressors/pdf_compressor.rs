//! PDF compression - DISABLED for efficiency
//! PDFs are already highly compressed and yield minimal savings (typically <0.1%)
//! This compressor now skips PDF files to save CPU cycles and processing time.

use async_trait::async_trait;

use crate::errors::{DomainError, DomainResult};
use super::Compressor;
use crate::domains::compression::types::CompressionMethod;

/// PDF compressor - DISABLED for efficiency
/// PDFs are already compressed and attempting further compression typically yields
/// minimal savings (often <1000 bytes on 10-20MB files = <0.01% savings)
#[derive(Clone)]
pub struct PdfCompressor {
    // Kept for compatibility but compression is disabled
}

impl PdfCompressor {
    pub fn new(_ghostscript_path: Option<String>) -> Self {
        Self {}
    }
}

#[async_trait]
impl Compressor for PdfCompressor {
    async fn can_handle(&self, mime_type: &str, extension: Option<&str>) -> bool {
        mime_type == "application/pdf" || extension == Some("pdf")
    }
    
    async fn compress(
        &self,
        data: Vec<u8>,
        _method: CompressionMethod,
        _quality_level: i32,
    ) -> DomainResult<Vec<u8>> {
        println!("📄 [PDF_COMPRESSOR] PDF compression SKIPPED: {} bytes", data.len());
        println!("   💡 PDFs are already compressed - skipping to save CPU cycles");
        
        // Return an error to indicate this file should be skipped
        // The compression service will catch this and mark the file as "skipped"
        Err(DomainError::Internal("PDF_SKIP_COMPRESSION".to_string()))
    }
}