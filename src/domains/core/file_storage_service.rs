use async_trait::async_trait;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs; // Use tokio::fs for async file operations
use uuid::Uuid;
use std::io;
use urlencoding;

#[derive(Debug, Error)]
pub enum FileStorageError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("File not found: {0}")]
    NotFound(String),
    #[error("Configuration error: {0}")]
    Configuration(String),
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    #[error("Storage limit exceeded")]
    LimitExceeded,
    #[error("Invalid path component: {0}")]
    InvalidPathComponent(String),
    #[error("Unknown storage error: {0}")]
    Other(String),
}

pub type FileStorageResult<T> = Result<T, FileStorageError>;

/// Service trait for abstracting file storage operations
#[async_trait]
pub trait FileStorageService: Send + Sync {
    /// Save file data to storage, returning the relative path and size.
    /// The implementation determines the final path structure.
    async fn save_file(
        &self,
        data: Vec<u8>,
        entity_type: &str, // e.g., "strategic_goals", "documents"
        entity_or_temp_id: &str,   // Associated entity ID (Uuid as string) or temp ID
        suggested_filename: &str, // Original filename for extension/naming hint
    ) -> FileStorageResult<(String, u64)>; // Returns (relative_path, size_bytes)

    /// iOS Optimized: Save file from path (no memory copy!)
    async fn save_file_from_path(
        &self,
        source_path: &str,
        entity_type: &str,
        entity_or_temp_id: &str,
        suggested_filename: &str,
    ) -> FileStorageResult<(String, u64)>;

    /// Delete a file from storage using its relative path.
    async fn delete_file(&self, relative_path: &str) -> FileStorageResult<()>;

    /// Get a readable stream or bytes for a file.
    /// (Using Vec<u8> for simplicity, could use streams like Tokio's AsyncRead)
    async fn get_file_data(&self, relative_path: &str) -> FileStorageResult<Vec<u8>>;
    
    /// Get the full absolute path for a given relative path (for internal use if needed)
    fn get_absolute_path(&self, relative_path: &str) -> PathBuf;

    /// Get the size of a file on disk without reading it into memory.
    async fn get_file_size(&self, relative_path: &str) -> FileStorageResult<u64>;
}

// --- Local File Storage Implementation ---

pub struct LocalFileStorageService {
    base_path: PathBuf,
    original_subdir: String,
    compressed_subdir: String,
}

impl LocalFileStorageService {
    /// Creates a new LocalFileStorageService.
    /// Ensures the base directory and subdirectories exist.
    pub fn new(base_path_str: &str) -> io::Result<Self> {
        let base_path = PathBuf::from(base_path_str);
        let original_subdir = "original".to_string();
        let compressed_subdir = "compressed".to_string();

        let original_path = base_path.join(&original_subdir);
        let compressed_path = base_path.join(&compressed_subdir);

        // Create directories synchronously during setup
        std::fs::create_dir_all(&original_path)?;
        std::fs::create_dir_all(&compressed_path)?;

        Ok(Self {
            base_path,
            original_subdir,
            compressed_subdir,
        })
    }

    /// Sanitizes a path component to prevent directory traversal issues.
    fn sanitize_component(component: &str) -> Result<String, FileStorageError> {
        // Handle empty or problematic components
        if component.is_empty() {
            return Err(FileStorageError::InvalidPathComponent("Empty component".to_string()));
        }
        
        // iOS OPTIMIZATION: Allow common iOS filename characters and handle special cases
        let sanitized = component
            .replace("/", "_")           // Replace path separators
            .replace("\\", "_")          // Replace Windows path separators
            .replace("\0", "")           // Remove null bytes
            .replace("..", "_")          // Replace parent directory references
            .trim_matches(|c: char| c.is_whitespace() || c == '.')  // Trim whitespace and dots
            .to_string();

        if sanitized.is_empty() {
            return Err(FileStorageError::InvalidPathComponent("Empty component after sanitization".to_string()));
        }

        // Check for remaining problematic characters (be more permissive for iOS)
        let has_invalid_chars = sanitized.chars().any(|c| {
            // Allow alphanumeric, spaces, hyphens, underscores, and common iOS filename characters
            !c.is_alphanumeric() && 
            !" -_@$+()[]{}.,;'\"".contains(c) &&
            !c.is_ascii_punctuation()  // Allow most ASCII punctuation
        });

        if has_invalid_chars {
            println!("⚠️ [FILE_STORAGE] Potentially problematic characters in component: {}", sanitized);
            // Don't fail, just log the warning - iOS filenames can be complex
        }

        // Additional check for reserved names (Windows compatibility)
        let reserved_names = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"];
        if reserved_names.contains(&sanitized.to_uppercase().as_str()) {
            return Err(FileStorageError::InvalidPathComponent(format!("Reserved name: {}", sanitized)));
        }

        Ok(sanitized)
    }

    /// Generates a unique filename based on suggestion and a new UUID.
    fn generate_unique_filename(suggested_filename: &str) -> String {
        let extension = Path::new(suggested_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| format!(".{}", ext))
            .unwrap_or_default();
        format!("{}{}", Uuid::new_v4(), extension)
    }
}

#[async_trait]
impl FileStorageService for LocalFileStorageService {
    async fn save_file(
        &self,
        data: Vec<u8>,
        entity_type: &str,
        entity_or_temp_id: &str,
        suggested_filename: &str,
    ) -> FileStorageResult<(String, u64)> {
        println!("🗂️ [FILE_STORAGE] Attempting to save file:");
        println!("   📁 Base path: {:?}", self.base_path);
        println!("   🏷️ Entity type: {}", entity_type);
        println!("   🆔 Entity ID: {}", entity_or_temp_id);
        println!("   📄 Filename: {}", suggested_filename);
        println!("   📊 Size: {} bytes", data.len());
        
        let sanitized_entity_type = Self::sanitize_component(entity_type)?;
        let sanitized_id = Self::sanitize_component(entity_or_temp_id)?;
        let unique_filename = Self::generate_unique_filename(suggested_filename);

        // Construct relative path: original/entity_type/entity_or_temp_id/unique_filename.ext
        let relative_path = Path::new(&self.original_subdir)
            .join(&sanitized_entity_type)
            .join(&sanitized_id)
            .join(&unique_filename);

        // Correctly get the relative path as a string slice for get_absolute_path
        let relative_path_str = relative_path.to_str().ok_or_else(|| FileStorageError::Other("Failed to convert relative path to string".to_string()))?;
        let absolute_path = self.get_absolute_path(relative_path_str);
        
        println!("   🔗 Relative path: {}", relative_path_str);
        println!("   🎯 Absolute path: {:?}", absolute_path);

        let parent_dir = absolute_path.parent().ok_or_else(|| FileStorageError::Other("Invalid path generated, no parent directory".to_string()))?;
        
        println!("   📂 Parent directory: {:?}", parent_dir);

        // Ensure the parent directory exists
        match fs::create_dir_all(parent_dir).await {
            Ok(_) => println!("   ✅ Directory structure created/verified"),
            Err(e) => {
                println!("   ❌ Failed to create directory structure: {}", e);
                return Err(FileStorageError::Io(e));
            }
        }

        let file_size = data.len() as u64;

        // Write the file asynchronously
        match fs::write(&absolute_path, data).await {
            Ok(_) => {
                println!("   ✅ File saved successfully: {} bytes", file_size);
                Ok((relative_path_str.to_string(), file_size))
            },
            Err(e) => {
                println!("   ❌ Failed to write file: {}", e);
                Err(FileStorageError::Io(e))
            }
        }
    }

    /// iOS Optimized: Save file from path (no memory loading!)
    async fn save_file_from_path(
        &self,
        source_path: &str,
        entity_type: &str,
        entity_or_temp_id: &str,
        suggested_filename: &str,
    ) -> FileStorageResult<(String, u64)> {
        println!("🚀 [FILE_STORAGE] iOS optimized path-based save:");
        println!("   📁 Source path: {}", source_path);
        println!("   🏷️ Entity type: {}", entity_type);
        println!("   🆔 Entity ID: {}", entity_or_temp_id);
        println!("   📄 Filename: {}", suggested_filename);
        
        // iOS OPTIMIZATION: Handle URL-encoded paths and special characters
        let decoded_source_path = match urlencoding::decode(source_path) {
            Ok(decoded) => {
                println!("   🔓 Decoded path: {}", decoded);
                decoded.to_string()
            },
            Err(_) => {
                println!("   ⚠️ Path decoding failed, using original: {}", source_path);
                source_path.to_string()
            }
        };
        
        // Verify source file exists and is accessible
        let source_metadata = fs::metadata(&decoded_source_path).await.map_err(|e| {
            println!("   ❌ Failed to read source file metadata from '{}': {}", decoded_source_path, e);
            println!("   🔍 Error kind: {:?}", e.kind());
            match e.kind() {
                io::ErrorKind::NotFound => FileStorageError::NotFound(format!("Source file not found: {}", decoded_source_path)),
                io::ErrorKind::PermissionDenied => FileStorageError::PermissionDenied(format!("Permission denied accessing: {}", decoded_source_path)),
                _ => FileStorageError::Io(e)
            }
        })?;
        
        let file_size = source_metadata.len();
        println!("   📊 Source file size: {} bytes", file_size);
        
        // Check if it's a regular file
        if !source_metadata.is_file() {
            return Err(FileStorageError::Other(format!("Source path is not a regular file: {}", decoded_source_path)));
        }
        
        let sanitized_entity_type = Self::sanitize_component(entity_type).map_err(|e| {
            println!("   ❌ Failed to sanitize entity type '{}': {}", entity_type, e);
            e
        })?;
        
        let sanitized_id = Self::sanitize_component(entity_or_temp_id).map_err(|e| {
            println!("   ❌ Failed to sanitize entity ID '{}': {}", entity_or_temp_id, e);
            e
        })?;
        
        let unique_filename = Self::generate_unique_filename(suggested_filename);
        println!("   🏷️ Generated unique filename: {}", unique_filename);

        // Construct relative path: original/entity_type/entity_or_temp_id/unique_filename.ext
        let relative_path = Path::new(&self.original_subdir)
            .join(&sanitized_entity_type)
            .join(&sanitized_id)
            .join(&unique_filename);

        let relative_path_str = relative_path.to_str().ok_or_else(|| {
            println!("   ❌ Failed to convert relative path to string");
            FileStorageError::Other("Failed to convert relative path to string".to_string())
        })?;
        
        let absolute_path = self.get_absolute_path(relative_path_str);
        
        println!("   🔗 Relative path: {}", relative_path_str);
        println!("   🎯 Absolute path: {:?}", absolute_path);

        let parent_dir = absolute_path.parent().ok_or_else(|| {
            println!("   ❌ Invalid path generated, no parent directory");
            FileStorageError::Other("Invalid path generated, no parent directory".to_string())
        })?;
        
        // Ensure the parent directory exists
        fs::create_dir_all(parent_dir).await.map_err(|e| {
            println!("   ❌ Failed to create directory structure at '{:?}': {}", parent_dir, e);
            FileStorageError::Io(e)
        })?;
        
        println!("   ✅ Directory structure created/verified: {:?}", parent_dir);

        // Use fs::copy for efficient file system operation (no memory loading!)
        match fs::copy(&decoded_source_path, &absolute_path).await {
            Ok(bytes_copied) => {
                println!("   ✅ File copied successfully: {} bytes (iOS optimized)", bytes_copied);
                
                // Verify the copied file
                match fs::metadata(&absolute_path).await {
                    Ok(dest_metadata) => {
                        let dest_size = dest_metadata.len();
                        if dest_size != file_size {
                            println!("   ⚠️ Size mismatch: source {} bytes, destination {} bytes", file_size, dest_size);
                        } else {
                            println!("   ✅ File integrity verified: {} bytes", dest_size);
                        }
                    },
                    Err(e) => {
                        println!("   ⚠️ Could not verify copied file: {}", e);
                    }
                }
                
                Ok((relative_path_str.to_string(), file_size))
            },
            Err(e) => {
                println!("   ❌ Failed to copy file from '{}' to '{:?}': {}", decoded_source_path, absolute_path, e);
                println!("   🔍 Copy error kind: {:?}", e.kind());
                Err(FileStorageError::Io(e))
            }
        }
    }

    async fn delete_file(&self, relative_path: &str) -> FileStorageResult<()> {
        let absolute_path = self.get_absolute_path(relative_path);

        // Basic check to prevent deleting outside the base path (though sanitize helps)
        if !absolute_path.starts_with(&self.base_path) {
            return Err(FileStorageError::PermissionDenied("Attempt to delete outside base path".to_string()));
        }

        match fs::remove_file(&absolute_path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                // Consider it success if the file is already gone
                Ok(())
            }
            Err(e) => Err(FileStorageError::Io(e)),
        }
        // TODO: Consider deleting empty parent directories? Maybe not necessary.
    }

    async fn get_file_data(&self, relative_path: &str) -> FileStorageResult<Vec<u8>> {
        let absolute_path = self.get_absolute_path(relative_path);

        // Basic check
        if !absolute_path.starts_with(&self.base_path) {
             return Err(FileStorageError::PermissionDenied("Attempt to read outside base path".to_string()));
        }

        match fs::read(&absolute_path).await {
            Ok(data) => Ok(data),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                 Err(FileStorageError::NotFound(relative_path.to_string()))
            }
            Err(e) => Err(FileStorageError::Io(e)),
        }
    }

    fn get_absolute_path(&self, relative_path: &str) -> PathBuf {
        // IMPORTANT: This assumes relative_path is ALREADY somewhat sanitized
        // or comes from a trusted source (like the DB). It cleans ".." etc.
        // normalize_path from the path_clean crate could be more robust if needed.
        let mut abs_path = self.base_path.clone();
        for component in Path::new(relative_path).components() {
             match component {
                std::path::Component::Normal(comp_str) => {
                    // Convert OsStr to str for checking. Handle potential non-UTF8 gracefully.
                    if let Some(s) = comp_str.to_str() {
                        if s.is_empty() || s.contains('/') || s.contains('\\') {
                            // Skip potentially problematic components
                            // Logging this might be useful in practice
                            continue;
                        }
                        abs_path.push(comp_str);
                    } else {
                        // Handle non-UTF8 path components if necessary, 
                        // for now, we might skip them or return an error.
                        // Skipping for simplicity.
                        continue; 
                    }
                },
                _ => { /* Skip RootDir, CurDir, ParentDir safely */ }
            }
        }
        abs_path
    }

    async fn get_file_size(&self, relative_path: &str) -> FileStorageResult<u64> {
        let absolute_path = self.get_absolute_path(relative_path);

        if !absolute_path.starts_with(&self.base_path) {
            return Err(FileStorageError::PermissionDenied("Attempt to stat outside base path".to_string()));
        }

        match tokio::fs::metadata(&absolute_path).await {
            Ok(meta) => Ok(meta.len()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(FileStorageError::NotFound(relative_path.to_string())),
            Err(e) => Err(FileStorageError::Io(e)),
        }
    }
} 