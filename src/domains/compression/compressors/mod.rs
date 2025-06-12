 //! Different compressors for various file types

pub mod image_compressor;
pub mod pdf_compressor;
pub mod office_compressor;
pub mod video_compressor;
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
    // Enhanced lookup based on file extension
    match get_extension(filename).unwrap_or("").to_lowercase().as_str() {
        // Images - enhanced with more formats
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "tif" | "tiff" => "image/tiff",
        "bmp" => "image/bmp",
        "heic" | "heif" => "image/heic",
        "avif" => "image/avif",
        "svg" => "image/svg+xml",
        
        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "rtf" => "application/rtf",
        "odt" => "application/vnd.oasis.opendocument.text",
        "ods" => "application/vnd.oasis.opendocument.spreadsheet",
        "odp" => "application/vnd.oasis.opendocument.presentation",
        
        // Text files
        "txt" => "text/plain",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "yaml" | "yml" => "application/x-yaml",
        "csv" => "text/csv",
        "md" => "text/markdown",
        
        // Audio - enhanced
        "mp3" => "audio/mpeg",
        "m4a" => "audio/m4a",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "flac" => "audio/flac",
        "ogg" => "audio/ogg",
        "opus" => "audio/opus",
        "caf" => "audio/x-caf",
        
        // Video - significantly enhanced
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "m4v" => "video/x-m4v",
        "avi" => "video/x-msvideo",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "3gp" => "video/3gpp",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "ogv" => "video/ogg",
        
        // Archives
        "zip" => "application/zip",
        "rar" => "application/x-rar-compressed",
        "7z" => "application/x-7z-compressed",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "bz2" => "application/x-bzip2",
        
        // Code files
        "py" => "text/x-python",
        "rs" => "text/x-rust",
        "swift" => "text/x-swift",
        "java" => "text/x-java-source",
        "cpp" | "c" => "text/x-c",
        "h" => "text/x-c-header",
        "sql" => "application/sql",
        
        // Data files
        "db" | "sqlite" => "application/x-sqlite3",
        "backup" => "application/octet-stream",
        
        _ => "application/octet-stream",
    }
}