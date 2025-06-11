use crate::domains::document::types::NewDocumentType;
use crate::errors::DomainResult;

/// Initialize all standard document type categories
/// These are file type categories, not semantic document types
/// Semantic meaning comes from field_identifier and user titles
pub fn initialize_standard_document_types() -> Vec<NewDocumentType> {
    vec![
        create_image_type(),
        create_document_type(),
        create_spreadsheet_type(),
        create_presentation_type(),
        create_video_type(),
        create_audio_type(),
        create_archive_type(),
        create_code_type(),
        create_data_type(),
    ]
}

/// Image files - photos, graphics, charts
/// High compression, smaller size limits for mobile efficiency
fn create_image_type() -> NewDocumentType {
    NewDocumentType {
        name: "Image".to_string(),
        description: Some("Photos, graphics, and image files".to_string()),
        icon: Some("photo".to_string()),
        default_priority: "normal".to_string(),
        // iOS compatible image formats
        allowed_extensions: "jpg,jpeg,png,heic,heif,webp,gif,bmp,tiff,svg".to_string(),
        max_size: 30_000_000, // 30MB - reasonable for high-res photos
        compression_level: 8, // High compression for images
        compression_method: Some("lossy".to_string()),
        min_size_for_compression: Some(500_000), // 500KB - compress anything larger
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Document files - PDFs, Word docs, text files
/// Medium compression, larger size limits for comprehensive reports
fn create_document_type() -> NewDocumentType {
    NewDocumentType {
        name: "Document".to_string(),
        description: Some("Text documents, PDFs, and reports".to_string()),
        icon: Some("doc.text".to_string()),
        default_priority: "normal".to_string(),
        // iOS compatible document formats
        allowed_extensions: "pdf,doc,docx,rtf,txt,md,pages,odt".to_string(),
        max_size: 150_000_000, // 150MB - large reports and proposals
        compression_level: 6, // Medium compression to preserve text quality
        compression_method: Some("lossless".to_string()),
        min_size_for_compression: Some(1_000_000), // 1MB
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Spreadsheet files - Excel, Numbers, CSV
/// Lower compression to preserve data integrity
fn create_spreadsheet_type() -> NewDocumentType {
    NewDocumentType {
        name: "Spreadsheet".to_string(),
        description: Some("Spreadsheets, data tables, and financial files".to_string()),
        icon: Some("tablecells".to_string()),
        default_priority: "normal".to_string(),
        // iOS compatible spreadsheet formats
        allowed_extensions: "xlsx,xls,numbers,csv,tsv,ods".to_string(),
        max_size: 100_000_000, // 10MB - large datasets
        compression_level: 4, // Lower compression for data integrity
        compression_method: Some("lossless".to_string()),
        min_size_for_compression: Some(2_000_000), // 2MB - only compress larger files
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Presentation files - PowerPoint, Keynote
/// Medium compression, moderate size limits
fn create_presentation_type() -> NewDocumentType {
    NewDocumentType {
        name: "Presentation".to_string(),
        description: Some("Presentation slides and training materials".to_string()),
        icon: Some("rectangle.on.rectangle".to_string()),
        default_priority: "normal".to_string(),
        // iOS compatible presentation formats
        allowed_extensions: "pptx,ppt,key,odp".to_string(),
        max_size: 75_000_000, // 75MB - presentations with media
        compression_level: 6, // Medium compression
        compression_method: Some("lossless".to_string()),
        min_size_for_compression: Some(5_000_000), // 5MB
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Video files - recordings, training videos, evidence videos
/// Lower compression (already compressed), large size limits
fn create_video_type() -> NewDocumentType {
    NewDocumentType {
        name: "Video".to_string(),
        description: Some("Video recordings, training materials, and evidence footage".to_string()),
        icon: Some("video".to_string()),
        default_priority: "high".to_string(), // Videos often important evidence
        // iOS compatible video formats
        allowed_extensions: "mp4,mov,m4v,avi,mkv,webm,3gp".to_string(),
        max_size: 500_000_000, // 500MB - long training videos
        compression_level: 2, // Minimal compression (videos already compressed)
        compression_method: Some("lossy".to_string()),
        min_size_for_compression: Some(50_000_000), // 50MB - only compress very large videos
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Audio files - recordings, interviews, training audio
/// Lower compression, moderate size limits
fn create_audio_type() -> NewDocumentType {
    NewDocumentType {
        name: "Audio".to_string(),
        description: Some("Audio recordings, interviews, and voice notes".to_string()),
        icon: Some("waveform".to_string()),
        default_priority: "normal".to_string(),
        // iOS compatible audio formats
        allowed_extensions: "mp3,m4a,wav,aac,flac,ogg,opus,caf".to_string(),
        max_size: 100_000_000, // 100MB - long interviews
        compression_level: 3, // Low compression for audio quality
        compression_method: Some("lossy".to_string()),
        min_size_for_compression: Some(10_000_000), // 10MB
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Archive files - compressed folders, backups
/// No additional compression (already compressed)
fn create_archive_type() -> NewDocumentType {
    NewDocumentType {
        name: "Archive".to_string(),
        description: Some("Compressed files, archives, and backup data".to_string()),
        icon: Some("archivebox".to_string()),
        default_priority: "low".to_string(),
        // iOS compatible archive formats
        allowed_extensions: "zip,rar,7z,tar,gz,bz2".to_string(),
        max_size: 200_000_000, // 200MB - document bundles
        compression_level: 0, // No compression (already compressed)
        compression_method: Some("none".to_string()),
        min_size_for_compression: None, // Never compress archives
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Code and markup files - for technical documentation
/// Minimal compression to preserve formatting
fn create_code_type() -> NewDocumentType {
    NewDocumentType {
        name: "Code".to_string(),
        description: Some("Code files, scripts, and markup documents".to_string()),
        icon: Some("chevron.left.forwardslash.chevron.right".to_string()),
        default_priority: "low".to_string(),
        // Text-based code formats
        allowed_extensions: "html,css,js,json,xml,yaml,yml,sql,py,rs,swift,java,cpp,c,h".to_string(),
        max_size: 10_000_000, // 10MB - code files are usually small
        compression_level: 5, // Medium compression
        compression_method: Some("lossless".to_string()),
        min_size_for_compression: Some(100_000), // 100KB - compress smaller files too
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Data files - JSON, XML, database exports
/// Minimal compression to preserve data integrity
fn create_data_type() -> NewDocumentType {
    NewDocumentType {
        name: "Data".to_string(),
        description: Some("Structured data files and database exports".to_string()),
        icon: Some("cylinder".to_string()),
        default_priority: "normal".to_string(),
        // Structured data formats
        allowed_extensions: "json,xml,yaml,yml,sql,db,sqlite,backup".to_string(),
        max_size: 1024_000_000_000, // 1TB - large data exports
        compression_level: 4, // Low-medium compression for data integrity
        compression_method: Some("lossless".to_string()),
        min_size_for_compression: Some(1_000_000), // 1MB
        related_tables: Some(r#"["all"]"#.to_string()),
    }
}

/// Get document type by file extension
/// Useful for auto-selecting type during upload
pub fn get_document_type_for_extension(extension: &str) -> Option<&'static str> {
    let ext = extension.to_lowercase();
    
    match ext.as_str() {
        // Images
        "jpg" | "jpeg" | "png" | "heic" | "heif" | "webp" | "gif" | "bmp" | "tiff" | "svg" => Some("Image"),
        
        // Documents  
        "pdf" | "doc" | "docx" | "rtf" | "txt" | "md" | "pages" | "odt" => Some("Document"),
        
        // Spreadsheets
        "xlsx" | "xls" | "numbers" | "csv" | "tsv" | "ods" => Some("Spreadsheet"),
        
        // Presentations
        "pptx" | "ppt" | "key" | "odp" => Some("Presentation"),
        
        // Videos
        "mp4" | "mov" | "m4v" | "avi" | "mkv" | "webm" | "3gp" => Some("Video"),
        
        // Audio
        "mp3" | "m4a" | "wav" | "aac" | "flac" | "ogg" | "opus" | "caf" => Some("Audio"),
        
        // Archives
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" => Some("Archive"),
        
        // Code
        "html" | "css" | "js" | "json" | "xml" | "yaml" | "yml" | "sql" | "py" | "rs" | "swift" | "java" | "cpp" | "c" | "h" => Some("Code"),
        
        // Data
        "db" | "sqlite" | "backup" => Some("Data"),
        
        _ => None,
    }
}

/// Validate that a file extension is supported
pub fn is_extension_supported(extension: &str) -> bool {
    get_document_type_for_extension(extension).is_some()
}

/// Get compression settings for a file type
pub fn get_compression_settings_for_type(type_name: &str) -> Option<(i32, String, Option<i64>)> {
    match type_name {
        "Image" => Some((8, "lossy".to_string(), Some(500_000))),
        "Document" => Some((6, "lossless".to_string(), Some(1_000_000))),
        "Spreadsheet" => Some((4, "lossless".to_string(), Some(2_000_000))),
        "Presentation" => Some((6, "lossless".to_string(), Some(5_000_000))),
        "Video" => Some((2, "lossy".to_string(), Some(50_000_000))),
        "Audio" => Some((3, "lossy".to_string(), Some(10_000_000))),
        "Archive" => Some((0, "none".to_string(), None)),
        "Code" => Some((5, "lossless".to_string(), Some(100_000))),
        "Data" => Some((4, "lossless".to_string(), Some(1_000_000))),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_document_types_created() {
        let types = initialize_standard_document_types();
        assert_eq!(types.len(), 9);
        
        let names: Vec<&str> = types.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"Image"));
        assert!(names.contains(&"Document"));
        assert!(names.contains(&"Video"));
    }

    #[test]
    fn test_extension_detection() {
        assert_eq!(get_document_type_for_extension("jpg"), Some("Image"));
        assert_eq!(get_document_type_for_extension("PDF"), Some("Document"));
        assert_eq!(get_document_type_for_extension("mp4"), Some("Video"));
        assert_eq!(get_document_type_for_extension("unknown"), None);
    }

    #[test]
    fn test_extension_support() {
        assert!(is_extension_supported("png"));
        assert!(is_extension_supported("docx"));
        assert!(!is_extension_supported("exe"));
    }

    #[test]
    fn test_compression_settings() {
        let (level, method, min_size) = get_compression_settings_for_type("Image").unwrap();
        assert_eq!(level, 8);
        assert_eq!(method, "lossy");
        assert_eq!(min_size, Some(500_000));
    }
} 