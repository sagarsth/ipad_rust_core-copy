use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Export formats supported by the system
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    JsonLines,
    Csv {
        delimiter: u8,
        quote_char: u8,
        escape_char: Option<u8>,
        compress: bool,
    },
    Parquet {
        compression: ParquetCompression,
        row_group_size: usize,
        enable_statistics: bool,
    },
}

// Custom deserializer to handle both Swift and Rust serialization formats
impl<'de> Deserialize<'de> for ExportFormat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, MapAccess, Visitor};
        use std::fmt;

        struct ExportFormatVisitor;

        impl<'de> Visitor<'de> for ExportFormatVisitor {
            type Value = ExportFormat;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an export format")
            }

            fn visit_str<E>(self, value: &str) -> Result<ExportFormat, E>
            where
                E: de::Error,
            {
                match value {
                    "jsonLines" | "JsonLines" => Ok(ExportFormat::JsonLines),
                    _ => Err(de::Error::unknown_variant(value, &["jsonLines", "csv", "parquet"])),
                }
            }

            fn visit_map<V>(self, mut map: V) -> Result<ExportFormat, V::Error>
            where
                V: MapAccess<'de>,
            {
                let key: String = map.next_key()?.ok_or_else(|| de::Error::missing_field("format type"))?;
                
                match key.as_str() {
                    "jsonLines" => {
                        // Consume the empty value for jsonLines
                        let _: serde_json::Value = map.next_value()?;
                        Ok(ExportFormat::JsonLines)
                    }
                    "csv" => {
                        // Handle Swift tuple serialization format: {"_0": {...}}
                        #[derive(Deserialize)]
                        struct CsvTupleWrapper {
                            #[serde(rename = "_0")]
                            data: CsvData,
                        }
                        
                        #[derive(Deserialize)]
                        struct CsvData {
                            delimiter: u8,
                            #[serde(rename = "quote_char")]
                            quote_char: u8,
                            #[serde(rename = "escape_char")]
                            escape_char: Option<u8>,
                            compress: bool,
                        }
                        
                        let csv_wrapper: CsvTupleWrapper = map.next_value()?;
                        let csv_data = csv_wrapper.data;
                        Ok(ExportFormat::Csv {
                            delimiter: csv_data.delimiter,
                            quote_char: csv_data.quote_char,
                            escape_char: csv_data.escape_char,
                            compress: csv_data.compress,
                        })
                    }
                    "parquet" => {
                        // Handle Swift tuple serialization format: {"_0": {...}}
                        #[derive(Deserialize)]
                        struct ParquetTupleWrapper {
                            #[serde(rename = "_0")]
                            data: ParquetData,
                        }
                        
                        #[derive(Deserialize)]
                        struct ParquetData {
                            compression: ParquetCompression,
                            #[serde(rename = "row_group_size")]
                            row_group_size: usize,
                            #[serde(rename = "enable_statistics")]
                            enable_statistics: bool,
                        }
                        
                        let parquet_wrapper: ParquetTupleWrapper = map.next_value()?;
                        let parquet_data = parquet_wrapper.data;
                        Ok(ExportFormat::Parquet {
                            compression: parquet_data.compression,
                            row_group_size: parquet_data.row_group_size,
                            enable_statistics: parquet_data.enable_statistics,
                        })
                    }
                    _ => Err(de::Error::unknown_variant(&key, &["jsonLines", "csv", "parquet"])),
                }
            }
        }

        deserializer.deserialize_any(ExportFormatVisitor)
    }
}

impl Default for ExportFormat {
    fn default() -> Self {
        Self::JsonLines
    }
}

impl ExportFormat {
    /// Get file extension for this format
    pub fn file_extension(&self) -> &'static str {
        match self {
            ExportFormat::JsonLines => "jsonl",
            ExportFormat::Csv { .. } => "csv",
            ExportFormat::Parquet { .. } => "parquet",
        }
    }
}

/// Parquet compression options
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParquetCompression {
    None,
    Snappy,
    Gzip,
    Lzo,
    Brotli,
    Lz4,
    Zstd,
}

/// Device tier for iOS optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceTier {
    Basic,    // iPad Air 2, etc.
    Standard, // iPad Air 3, iPad 9th gen
    Pro,      // iPad Pro models
    Max,      // iPad Pro M1/M2
}

/// Thermal state monitoring
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThermalState {
    Nominal,  // Normal operation
    Fair,     // Slight thermal pressure
    Serious,  // Significant thermal pressure
    Critical, // Severe thermal pressure
}

/// Memory pressure levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryPressureLevel {
    Normal,
    Warning,
    Critical,
}

/// Comprehensive error types for export operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportError {
    /// I/O related errors
    Io(String),
    /// Serialization errors
    Serialization(String),
    /// Memory pressure error
    MemoryPressure,
    /// Thermal throttling error
    ThermalThrottling,
    /// Background task expired
    BackgroundTaskExpired,
    /// Background time expired during long-running export
    BackgroundTimeExpired,
    /// Failed to save checkpoint data
    CheckpointSaveFailed,
    /// JSON serialization/deserialization error
    SerializationError(String),
    /// Communication channel closed unexpectedly
    ChannelClosed,
    /// Export job failed
    JobFailed(String),
    /// System is overloaded (memory/thermal pressure)
    SystemOverloaded(String),
    /// Export queue is full
    QueueFull,
    /// Compression error
    Compression(String),
    /// Schema validation error
    Schema(String),
    /// Permission error
    Permission(String),
    /// Network error (for remote exports)
    Network(String),
    /// Database error
    Database(String),
    /// Invalid configuration
    InvalidConfig(String),
    /// Cancelled by user or system
    Cancelled,
    /// Unknown error
    Unknown(String),
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::Io(msg) => write!(f, "I/O error: {}", msg),
            ExportError::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            ExportError::MemoryPressure => write!(f, "Memory pressure detected"),
            ExportError::ThermalThrottling => write!(f, "Thermal throttling active"),
            ExportError::BackgroundTaskExpired => write!(f, "Background task expired"),
            ExportError::BackgroundTimeExpired => write!(f, "Background time expired"),
            ExportError::CheckpointSaveFailed => write!(f, "Failed to save checkpoint"),
            ExportError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            ExportError::ChannelClosed => write!(f, "Communication channel closed"),
            ExportError::JobFailed(msg) => write!(f, "Export job failed: {}", msg),
            ExportError::SystemOverloaded(msg) => write!(f, "System overloaded: {}", msg),
            ExportError::QueueFull => write!(f, "Export queue is full"),
            ExportError::Compression(msg) => write!(f, "Compression error: {}", msg),
            ExportError::Schema(msg) => write!(f, "Schema error: {}", msg),
            ExportError::Permission(msg) => write!(f, "Permission error: {}", msg),
            ExportError::Network(msg) => write!(f, "Network error: {}", msg),
            ExportError::Database(msg) => write!(f, "Database error: {}", msg),
            ExportError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            ExportError::Cancelled => write!(f, "Operation cancelled"),
            ExportError::Unknown(msg) => write!(f, "Unknown error: {}", msg),
        }
    }
}

impl std::error::Error for ExportError {}

/// Export statistics for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportStats {
    pub entities_written: usize,
    pub bytes_written: usize,
    pub duration_ms: u64,
    pub memory_peak_mb: u32,
    pub compression_ratio: Option<f32>,
}

/// Export metadata for completed exports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportMetadata {
    pub format: ExportFormat,
    pub stats: ExportStats,
    pub file_paths: Vec<PathBuf>,
    pub schema_version: u32,
    pub checksum: Option<String>,
}

/// Export job statuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportStatus {
    Queued,
    Pending,
    Running,
    Completed,
    Failed,
}

/// Row mapped to the `export_jobs` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportJob {
    pub id: Uuid,
    pub requested_by_user_id: Option<Uuid>,
    pub requested_at: DateTime<Utc>,
    pub include_blobs: bool,
    pub status: ExportStatus,
    pub local_path: Option<String>,
    pub total_entities: Option<i64>,
    pub total_bytes: Option<i64>,
    pub error_message: Option<String>,
}

/// High-level request coming from UI / FFI describing what should be exported.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExportRequest {
    pub filters: Vec<EntityFilter>,
    pub include_blobs: bool,
    pub target_path: Option<PathBuf>,
    pub format: Option<ExportFormat>, // New field for format selection
    pub use_compression: bool,
    pub use_background: bool,
}

/// Summary returned to the caller after `create_export` or `get_export_status`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSummary {
    pub job: ExportJob,
}

/// Filter wrappers so that the export layer can stay repository-agnostic.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum EntityFilter {
    /// Export all strategic goals. Optionally restrict by `status_id`.
    StrategicGoals { status_id: Option<i64> },
    /// Export strategic goals by specific IDs
    StrategicGoalsByIds { ids: Vec<Uuid> },
    /// Export strategic goals within date range
    StrategicGoalsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc>,
        status_id: Option<i64> 
    },
    /// Export strategic goals using complex filter (matches UI filtering logic)
    StrategicGoalsByFilter { 
        filter: crate::domains::strategic_goal::types::StrategicGoalFilter 
    },
    /// Export all projects.
    ProjectsAll,
    /// Export projects by specific IDs
    ProjectsByIds { ids: Vec<Uuid> },
    /// Export projects within date range
    ProjectsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all activities.
    ActivitiesAll,
    /// Export activities by specific IDs
    ActivitiesByIds { ids: Vec<Uuid> },
    /// Export activities within date range
    ActivitiesByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all donors.
    DonorsAll,
    /// Export donors by specific IDs
    DonorsByIds { ids: Vec<Uuid> },
    /// Export donors within date range
    DonorsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all project funding records.
    FundingAll,
    /// Export funding by specific IDs
    FundingByIds { ids: Vec<Uuid> },
    /// Export funding within date range
    FundingByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all livelihoods.
    LivelihoodsAll,
    /// Export livelihoods by specific IDs
    LivelihoodsByIds { ids: Vec<Uuid> },
    /// Export livelihoods within date range
    LivelihoodsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// Export all workshops
    WorkshopsAll { include_participants: bool },
    /// Export workshops by specific IDs
    WorkshopsByIds { ids: Vec<Uuid>, include_participants: bool },
    /// Export workshops within date range
    WorkshopsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc>,
        include_participants: bool 
    },
    /// Export all workshop participants
    WorkshopParticipantsAll,
    /// Export workshop participants by specific IDs
    WorkshopParticipantsByIds { ids: Vec<Uuid> },
    /// Export media docs for a single related entity.
    MediaDocumentsByRelatedEntity { related_table: String, related_id: Uuid },
    /// Export media documents by specific IDs
    MediaDocumentsByIds { ids: Vec<Uuid> },
    /// Export media documents within date range
    MediaDocumentsByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc> 
    },
    /// NEW: Export all domains in a unified file with mixed records
    UnifiedAllDomains { 
        include_type_tags: bool 
    },
    /// NEW: Export all domains within date range in a unified file
    UnifiedByDateRange { 
        start_date: DateTime<Utc>, 
        end_date: DateTime<Utc>,
        include_type_tags: bool 
    },
} 