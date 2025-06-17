 //! Office document compression implementation

use async_trait::async_trait;
use std::io::{Cursor, Read, Write};
use tokio::task;
use zip::{ZipArchive, ZipWriter, write::FileOptions};
use std::collections::HashMap;

use crate::errors::{DomainError, DomainResult};
use super::{Compressor, get_extension};
use crate::domains::compression::types::CompressionMethod;

/// Office document compressor for DOCX, XLSX, PPTX files
pub struct OfficeCompressor {
    image_compressor: super::image_compressor::ImageCompressor,
}

impl OfficeCompressor {
    pub fn new() -> Self {
        Self {
            image_compressor: super::image_compressor::ImageCompressor,
        }
    }
}

#[async_trait]
impl Compressor for OfficeCompressor {
    async fn can_handle(&self, mime_type: &str, extension: Option<&str>) -> bool {
        matches!(mime_type, 
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" |
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" |
            "application/vnd.openxmlformats-officedocument.presentationml.presentation" |
            "application/msword" |
            "application/vnd.ms-excel" |
            "application/vnd.ms-powerpoint"
        ) || matches!(extension, 
            Some("docx") | Some("xlsx") | Some("pptx") | 
            Some("doc") | Some("xls") | Some("ppt")
        )
    }
    
    async fn compress(
        &self,
        data: Vec<u8>,
        method: CompressionMethod,
        quality_level: i32,
    ) -> DomainResult<Vec<u8>> {
        let image_compressor = self.image_compressor.clone();
        
        // 🔧 FIX: Extract ALL data first to avoid Send issues with ZipArchive
        let mut image_files: Vec<(String, Vec<u8>)> = Vec::new();
        let mut other_files: Vec<(String, Vec<u8>)> = Vec::new();
        
        // Extract all files from ZIP first (synchronous, no Send issues)
        {
            let mut archive = ZipArchive::new(Cursor::new(&data))
                .map_err(|e| DomainError::Internal(format!("Failed to read Office document as ZIP: {}", e)))?;
            
            let image_extensions = ["png", "jpg", "jpeg", "gif", "bmp"];
            
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)
                    .map_err(|e| DomainError::Internal(format!("Failed to read file in ZIP: {}", e)))?;
                
                let name = file.name().to_string();
                let mut file_data = Vec::new();
                file.read_to_end(&mut file_data)
                    .map_err(|e| DomainError::Internal(format!("Failed to read file data: {}", e)))?;
                
                let ext = get_extension(&name).map(|e| e.to_lowercase());
                let is_image = ext.map(|e| image_extensions.contains(&e.as_str())).unwrap_or(false);
                
                if is_image && method != CompressionMethod::None {
                    image_files.push((name, file_data));
                } else {
                    other_files.push((name, file_data));
                }
            }
        } // ZipArchive is dropped here
        
        // Check if there are any images to compress
        if image_files.is_empty() {
            println!("📄 [OFFICE_COMPRESSOR] No images found in document, skipping compression");
            return Err(DomainError::Validation(crate::errors::ValidationError::custom("Office document contains no compressible images")));
        }
        
        // Now compress images asynchronously (no Send issues)
        let mut compressed_images: HashMap<String, Vec<u8>> = HashMap::new();
        for (name, image_data) in image_files {
            println!("📷 [OFFICE_COMPRESSOR] Compressing embedded image: {}", name);
            let compressed_image = image_compressor.compress(
                image_data, 
                CompressionMethod::Lossy, 
                quality_level
            ).await?;
            compressed_images.insert(name, compressed_image);
        }
        
        // Finally, reconstruct ZIP in blocking task
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            // 🔧 FIX: Use Vec<u8> directly instead of Cursor for better control
            let mut compressed_data = Vec::new();
            {
                let mut zip_writer = ZipWriter::new(Cursor::new(&mut compressed_data));
                
                let options = FileOptions::default()
                    .compression_method(zip::CompressionMethod::Deflated)
                    .compression_level(Some(9));
                
                // Add compressed images
                for (name, image_data) in compressed_images {
                    println!("📷 [OFFICE_COMPRESSOR] Adding compressed image: {} ({} bytes)", name, image_data.len());
                    zip_writer.start_file(&name, options)
                        .map_err(|e| DomainError::Internal(format!("Failed to create file in ZIP: {}", e)))?;
                    zip_writer.write_all(&image_data)
                        .map_err(|e| DomainError::Internal(format!("Failed to write image to ZIP: {}", e)))?;
                }
                
                // Add other files
                for (name, file_data) in other_files {
                    println!("📄 [OFFICE_COMPRESSOR] Adding file: {} ({} bytes)", name, file_data.len());
                    zip_writer.start_file(&name, options)
                        .map_err(|e| DomainError::Internal(format!("Failed to create file in ZIP: {}", e)))?;
                    zip_writer.write_all(&file_data)
                        .map_err(|e| DomainError::Internal(format!("Failed to write file to ZIP: {}", e)))?;
                }
                
                // 🔧 FIX: Properly finish the ZIP writer
                zip_writer.finish()
                    .map_err(|e| DomainError::Internal(format!("Failed to finalize ZIP: {}", e)))?;
                
            } // ZipWriter is dropped here, data is written to compressed_data
            
            println!("✅ [OFFICE_COMPRESSOR] Successfully compressed office document: {} bytes output", compressed_data.len());
            
            // 🔧 FIX: Validate output size
            if compressed_data.is_empty() {
                return Err(DomainError::Internal("Office compression produced empty output".to_string()));
            }
            
            Ok(compressed_data)
        }).await.map_err(|e| DomainError::Internal(format!("Task join error: {}", e)))?
    }
}