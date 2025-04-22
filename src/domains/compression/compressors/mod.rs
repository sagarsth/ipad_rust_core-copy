 //! Different compressors for various file types

pub mod image_compressor;
pub mod pdf_compressor;
pub mod office_compressor;
pub mod generic_compressor;

use async_trait::async_trait;
use std::path::Path;
use crate::errors::DomainResult;
use super::types::CompressionMethod;

/// Common trait for all compressors
#[async_trait]
pub trait Compressor: Send + Sync {
    /// Check if this compressor can handle the given file
    async fn can_handle(&self, mime_type: &str, extension: Option<&str>) -> bool;
    
    /// Compress file data
    async fn compress(
        &self,
        data: Vec<u8>,
        method: CompressionMethod,
        quality_level: i32,
    ) -> DomainResult<Vec<u8>>;
}

/// Utility function to get file extension from filename
pub fn get_extension(filename: &str) -> Option<&str> {
    Path::new(filename).extension().and_then(|ext| ext.to_str())
}

/// Utility function to guess MIME type from extension
pub fn guess_mime_type(filename: &str) -> &'static str {
    // Simple lookup based on file extension
    match get_extension(filename).unwrap_or("").to_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "tif" | "tiff" => "image/tiff",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "csv" => "text/csv",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "zip" => "application/zip",
        _ => "application/octet-stream",
    }
}