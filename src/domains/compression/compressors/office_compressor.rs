 //! Office document compression implementation

use async_trait::async_trait;
use std::io::{Cursor, Read, Write};
use tokio::task;
use tempfile::NamedTempFile;
use zip::{ZipArchive, ZipWriter, write::FileOptions};
use std::collections::HashSet;

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
        
        // Run office document operations in a blocking task
        task::spawn_blocking(move || -> DomainResult<Vec<u8>> {
            let mut archive = ZipArchive::new(Cursor::new(&data))
                .map_err(|e| DomainError::Internal(format!("Failed to read Office document as ZIP: {}", e)))?;
            
            // Create a new ZIP archive for the compressed result
            let mut compressed_data = Vec::new();
            let mut zip_writer = ZipWriter::new(Cursor::new(&mut compressed_data));
            
            // Options for storing files in the ZIP
            let options = FileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .compression_level(Some(9)); // Max compression
            
            // Process each file in the archive
            let mut file_names = Vec::with_capacity(archive.len());
            for i in 0..archive.len() {
                let mut file = archive.by_index(i)
                    .map_err(|e| DomainError::Internal(format!("Failed to read file in ZIP: {}", e)))?;
                
                let name = file.name().to_owned();
                file_names.push(name);
            }
            
            // Image extensions to compress
            let image_extensions = HashSet::from(["png", "jpg", "jpeg", "gif", "bmp"]);
            
            // Process each file
            for name in file_names {
                let mut file = archive.by_name(&name)
                    .map_err(|e| DomainError::Internal(format!("Failed to read file in ZIP: {}", e)))?;
                
                // Check if this is an image file
                let ext = get_extension(&name).map(|e| e.to_lowercase());
                let is_image = ext.map(|e| image_extensions.contains(e.as_str())).unwrap_or(false);
                
                if is_image && method != CompressionMethod::None {
                    // Read image data
                    let mut image_data = Vec::new();
                    file.read_to_end(&mut image_data)
                        .map_err(|e| DomainError::Internal(format!("Failed to read image data: {}", e)))?;
                    
                    // Compress the image by blocking the current thread
                    let handle = tokio::runtime::Handle::current();
                    let compressed_image = handle.block_on(image_compressor.compress(
                        image_data, CompressionMethod::Lossy, quality_level
                    ))?; // Apply ? to the Result returned by block_on
                    
                    // Add compressed image to the new ZIP
                    zip_writer.start_file(&name, options)
                        .map_err(|e| DomainError::Internal(format!("Failed to create file in ZIP: {}", e)))?;
                    
                    zip_writer.write_all(&compressed_image)
                        .map_err(|e| DomainError::Internal(format!("Failed to write to ZIP: {}", e)))?;
                } else {
                    // Copy other files as is
                    let mut file_data = Vec::new();
                    file.read_to_end(&mut file_data)
                        .map_err(|e| DomainError::Internal(format!("Failed to read file data: {}", e)))?;
                    
                    zip_writer.start_file(&name, options)
                        .map_err(|e| DomainError::Internal(format!("Failed to create file in ZIP: {}", e)))?;
                    
                    zip_writer.write_all(&file_data)
                        .map_err(|e| DomainError::Internal(format!("Failed to write to ZIP: {}", e)))?;
                }
            }
            
            // Finalize the ZIP
            let mut result = zip_writer.finish()
                .map_err(|e| DomainError::Internal(format!("Failed to finalize ZIP: {}", e)))?;
            
            // Extract the compressed data
            let mut output = Vec::new();
            result.read_to_end(&mut output)
                .map_err(|e| DomainError::Internal(format!("Failed to read compressed result: {}", e)))?;
            
            Ok(output)
        }).await.map_err(|e| DomainError::Internal(format!("Task join error: {}", e)))?
    }
}