 //! PDF compression implementation

use async_trait::async_trait;
use std::process::{Command, Stdio};
use tokio::task;
use tempfile::NamedTempFile;
use std::io::Write;

use crate::errors::{DomainError, DomainResult};
use super::Compressor;
use crate::domains::compression::types::CompressionMethod;

/// PDF compressor using external tools (gs)
pub struct PdfCompressor {
    ghostscript_path: String,
}

impl PdfCompressor {
    pub fn new(ghostscript_path: Option<String>) -> Self {
        Self {
            ghostscript_path: ghostscript_path.unwrap_or_else(|| "gs".to_string()),
        }
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
        method: CompressionMethod,
        quality_level: i32,
    ) -> DomainResult<Vec<u8>> {
        let ghostscript_path = self.ghostscript_path.clone();
        
        println!("📄 [PDF_COMPRESSOR] Starting PDF compression: {} bytes, method: {:?}, quality: {}", 
                 data.len(), method, quality_level);
        
        // Convert quality level to PDF settings
        let settings = match method {
            CompressionMethod::PdfOptimize => {
                match quality_level {
                    1..=30 => "screen", // Low quality
                    31..=70 => "ebook", // Medium quality
                    _ => "printer",     // High quality
                }
            },
            CompressionMethod::Lossy => "screen", // Lossy compression uses screen quality
            _ => "prepress",           // Default to high quality
        };
        
        println!("📄 [PDF_COMPRESSOR] Using Ghostscript settings: {}", settings);
        
        // Run PDF operations in a blocking task
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            println!("📄 [PDF_COMPRESSOR] Creating temporary input file...");
            
            // Write PDF data to a temporary file
            let mut input_file = NamedTempFile::new()
                .map_err(|e| {
                    println!("❌ [PDF_COMPRESSOR] Failed to create temp file: {}", e);
                    DomainError::Internal(format!("Failed to create temp file: {}", e))
                })?;
                
            input_file.write_all(&data)
                .map_err(|e| {
                    println!("❌ [PDF_COMPRESSOR] Failed to write to temp file: {}", e);
                    DomainError::Internal(format!("Failed to write to temp file: {}", e))
                })?;
                
            let input_path = input_file.path();
            println!("📄 [PDF_COMPRESSOR] Input file written: {}", input_path.display());
            
            // Create a temporary file for output
            let output_file = NamedTempFile::new()
                .map_err(|e| {
                    println!("❌ [PDF_COMPRESSOR] Failed to create output temp file: {}", e);
                    DomainError::Internal(format!("Failed to create output temp file: {}", e))
                })?;
                
            let output_path = output_file.path();
            println!("📄 [PDF_COMPRESSOR] Output file path: {}", output_path.display());
            
            // Prepare Ghostscript command
            let args = vec![
                "-sDEVICE=pdfwrite".to_string(),
                format!("-dPDFSETTINGS=/{}", settings),
                "-dCompatibilityLevel=1.4".to_string(),
                "-dNOPAUSE".to_string(),
                "-dQUIET".to_string(),
                "-dBATCH".to_string(),
                format!("-sOutputFile={}", output_path.to_string_lossy()),
                input_path.to_string_lossy().to_string(),
            ];
            
            println!("📄 [PDF_COMPRESSOR] Executing Ghostscript: {} {:?}", ghostscript_path, args);
            
            // Run ghostscript to compress the PDF
            let output = Command::new(&ghostscript_path)
                .args(&args)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .output()
                .map_err(|e| {
                    println!("❌ [PDF_COMPRESSOR] Failed to execute Ghostscript ({}): {}", ghostscript_path, e);
                    println!("   This might indicate that Ghostscript is not installed or not in PATH");
                    DomainError::Internal(format!("Failed to execute ghostscript: {}", e))
                })?;
                
            println!("📄 [PDF_COMPRESSOR] Ghostscript finished with exit code: {:?}", output.status.code());
            
            if !output.status.success() {
                let error = String::from_utf8_lossy(&output.stderr);
                println!("❌ [PDF_COMPRESSOR] Ghostscript failed: {}", error);
                return Err(DomainError::Internal(format!("Ghostscript error: {}", error)));
            }
            
            // Read the compressed file
            println!("📄 [PDF_COMPRESSOR] Reading compressed PDF from: {}", output_path.display());
            let compressed_data = std::fs::read(output_path)
                .map_err(|e| {
                    println!("❌ [PDF_COMPRESSOR] Failed to read compressed PDF: {}", e);
                    DomainError::Internal(format!("Failed to read compressed PDF: {}", e))
                })?;
            
            println!("✅ [PDF_COMPRESSOR] Compression successful: {} bytes -> {} bytes", 
                     data.len(), compressed_data.len());
            
            Ok(compressed_data)
        }).await.map_err(|e| {
            println!("❌ [PDF_COMPRESSOR] Task join error: {}", e);
            DomainError::Internal(format!("Task join error: {}", e))
        })?
    }
}