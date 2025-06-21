use crate::domains::export::types::*;
use crate::domains::export::writer::*;
use crate::domains::export::ios::memory::*;
use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use arrow::array::*;
use arrow::datatypes::{Schema, SchemaRef, DataType, Field};
use arrow::record_batch::RecordBatch;
use parquet::arrow::AsyncArrowWriter;
use parquet::file::properties::{WriterProperties, WriterVersion};
use parquet::basic::{Compression, Encoding};
use std::sync::Arc;
use std::time::Instant;
use tokio::fs::File;
use std::path::Path;
use tokio::sync::Mutex;

/// Parquet writer with iOS optimizations using our available compression
pub struct IOSParquetWriter {
    writer: Arc<Mutex<AsyncArrowWriter<File>>>,
    schema: SchemaRef,
    memory_observer: MemoryPressureObserver,
    stats: ExportStats,
    start_time: Instant,
    batch_builder: RecordBatchBuilder,
}

impl std::fmt::Debug for IOSParquetWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IOSParquetWriter")
            .field("schema", &self.schema)
            .field("stats", &self.stats)
            .field("writer", &"<AsyncArrowWriter>")
            .finish()
    }
}

impl IOSParquetWriter {
    pub async fn new_ios_optimized(path: &Path, schema: SchemaRef) -> Result<Self, ExportError> {
        let file = File::create(path).await
            .map_err(|e| ExportError::Io(e.to_string()))?;
        
        let props = WriterProperties::builder()
            .set_compression(Self::optimal_compression())
            .set_data_page_size_limit(Self::optimal_page_size())
            .set_dictionary_enabled(true)
            .set_writer_version(WriterVersion::PARQUET_2_0)
            .set_created_by("ipad-export-v2".to_string())
            // Use PLAIN encoding for all columns to avoid DELTA_BINARY_PACKED issues with byte arrays
            .set_encoding(Encoding::PLAIN)
            .build();
        
        let writer = AsyncArrowWriter::try_new(file, schema.clone(), Some(props))
            .map_err(|e| ExportError::Serialization(e.to_string()))?;
        
        Ok(Self {
            writer: Arc::new(Mutex::new(writer)),
            schema: schema.clone(),
            memory_observer: MemoryPressureObserver::new(),
            stats: ExportStats {
                entities_written: 0,
                bytes_written: 0,
                duration_ms: 0,
                memory_peak_mb: 0,
                compression_ratio: None,
            },
            start_time: Instant::now(),
            batch_builder: RecordBatchBuilder::new(schema),
        })
    }
    
    fn optimal_compression() -> Compression {
        match ios_device_tier() {
            DeviceTier::Max => Compression::SNAPPY, // Use Snappy for maximum performance
            DeviceTier::Pro => Compression::LZ4_RAW, // Use LZ4 for balanced performance
            _ => Compression::UNCOMPRESSED, // Skip compression for basic devices
        }
    }
    
    fn optimal_page_size() -> usize {
        match ios_memory_available() {
            0..=2_147_483_648 => 65_536,      // 64KB for < 2GB RAM
            2_147_483_649..=4_294_967_296 => 131_072,  // 128KB for 2-4GB
            _ => 262_144,                      // 256KB for > 4GB
        }
    }
    
    /// Write RecordBatch with iOS memory management
    pub async fn write_batch(&mut self, batch: RecordBatch) -> Result<(), ExportError> {
        // Check iOS memory pressure
        if self.memory_observer.is_critical() {
            self.writer.lock().await.flush().await
                .map_err(|e| ExportError::Io(e.to_string()))?;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Write batch
        self.writer.lock().await.write(&batch).await
            .map_err(|e| ExportError::Serialization(e.to_string()))?;
        
        self.stats.entities_written += batch.num_rows();
        self.stats.bytes_written += batch.get_array_memory_size();
        
        Ok(())
    }
}

#[async_trait]
impl StreamingExportWriter for IOSParquetWriter {
    async fn write_json_stream(&mut self, _stream: Box<dyn Stream<Item = Result<serde_json::Value, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError> {
        Err(ExportError::InvalidConfig("Parquet writer requires Arrow RecordBatch, not JSON".to_string()))
    }
    
    async fn write_batch_stream(&mut self, mut stream: Box<dyn Stream<Item = Result<RecordBatch, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError> {
        let mut row_count = 0;
        
        while let Some(result) = stream.next().await {
            let batch = result?;
            row_count += batch.num_rows();
            
            // iOS memory pressure check
            if self.memory_observer.is_critical() {
                self.writer.lock().await.flush().await
                    .map_err(|e| ExportError::Io(e.to_string()))?;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            // Write batch
            self.write_batch(batch).await?;
            
            // Yield for iOS background processing
            if row_count % 10_000 == 0 {
                tokio::task::yield_now().await;
            }
        }
        
        self.stats.duration_ms = self.start_time.elapsed().as_millis() as u64;
        Ok(self.stats.clone())
    }
    
    async fn flush(&mut self) -> Result<(), ExportError> {
        self.writer.lock().await.flush().await
            .map_err(|e| ExportError::Io(e.to_string()))?;
        Ok(())
    }
    
    async fn finalize(mut self: Box<Self>) -> Result<ExportMetadata, ExportError> {
        let writer = Arc::try_unwrap(self.writer)
            .map_err(|_| ExportError::Unknown("Failed to unwrap Arc".to_string()))?
            .into_inner();
        let file_metadata = writer.close().await
            .map_err(|e| ExportError::Serialization(e.to_string()))?;
        
        Ok(ExportMetadata {
            format: ExportFormat::Parquet {
                compression: Self::optimal_compression_type(),
                row_group_size: file_metadata.num_rows as usize,
                enable_statistics: true,
            },
            stats: self.stats,
            file_paths: vec![],
            schema_version: 1,
            checksum: None,
        })
    }
    
    fn format(&self) -> ExportFormat {
        ExportFormat::Parquet {
            compression: Self::optimal_compression_type(),
            row_group_size: 10_000,
            enable_statistics: true,
        }
    }
    
    fn can_handle_pressure(&self, level: MemoryPressureLevel) -> bool {
        match level {
            MemoryPressureLevel::Normal => true,
            MemoryPressureLevel::Warning => true,
            MemoryPressureLevel::Critical => false,
        }
    }
    
    fn optimal_batch_size(&self) -> usize {
        match (ios_device_tier(), self.memory_observer.current_level()) {
            (DeviceTier::Max, MemoryPressureLevel::Normal) => 10_000,
            (DeviceTier::Pro, MemoryPressureLevel::Normal) => 5_000,
            (_, MemoryPressureLevel::Warning) => 1_000,
            _ => 500,
        }
    }
}

impl IOSParquetWriter {
    fn optimal_compression_type() -> ParquetCompression {
        match ios_device_tier() {
            DeviceTier::Max => ParquetCompression::Snappy,
            DeviceTier::Pro => ParquetCompression::Lz4,
            _ => ParquetCompression::None,
        }
    }
}

/// Record batch builder for accumulating rows
pub struct RecordBatchBuilder {
    schema: SchemaRef,
    builders: Vec<Box<dyn ArrayBuilder>>,
    current_rows: usize,
}

impl RecordBatchBuilder {
    pub fn new(schema: SchemaRef) -> Self {
        let builders = schema
            .fields()
            .iter()
            .map(|field| Self::create_builder(field))
            .collect();
        
        Self {
            schema,
            builders,
            current_rows: 0,
        }
    }
    
    fn create_builder(field: &Field) -> Box<dyn ArrayBuilder> {
        match field.data_type() {
            DataType::Utf8 => Box::new(StringBuilder::new()),
            DataType::Int64 => Box::new(Int64Builder::new()),
            DataType::Float64 => Box::new(Float64Builder::new()),
            DataType::Boolean => Box::new(BooleanBuilder::new()),
            DataType::Timestamp(_, _) => Box::new(TimestampMillisecondBuilder::new()),
            _ => Box::new(StringBuilder::new()), // Fallback
        }
    }
    
    pub fn append_value(&mut self, column_index: usize, value: &str) -> Result<(), ExportError> {
        if column_index >= self.builders.len() {
            return Err(ExportError::Schema("Column index out of bounds".to_string()));
        }
        
        let field = &self.schema.fields()[column_index];
        match field.data_type() {
            DataType::Utf8 => {
                if let Some(builder) = self.builders[column_index].as_any_mut().downcast_mut::<StringBuilder>() {
                    builder.append_value(value);
                }
            },
            DataType::Int64 => {
                if let Some(builder) = self.builders[column_index].as_any_mut().downcast_mut::<Int64Builder>() {
                    let parsed: i64 = value.parse().map_err(|_| ExportError::Serialization("Invalid int64".to_string()))?;
                    builder.append_value(parsed);
                }
            },
            DataType::Float64 => {
                if let Some(builder) = self.builders[column_index].as_any_mut().downcast_mut::<Float64Builder>() {
                    let parsed: f64 = value.parse().map_err(|_| ExportError::Serialization("Invalid float64".to_string()))?;
                    builder.append_value(parsed);
                }
            },
            DataType::Boolean => {
                if let Some(builder) = self.builders[column_index].as_any_mut().downcast_mut::<BooleanBuilder>() {
                    let parsed: bool = value.parse().map_err(|_| ExportError::Serialization("Invalid boolean".to_string()))?;
                    builder.append_value(parsed);
                }
            },
            _ => {
                // Fallback to string
                if let Some(builder) = self.builders[column_index].as_any_mut().downcast_mut::<StringBuilder>() {
                    builder.append_value(value);
                }
            }
        }
        
        Ok(())
    }
    
    pub fn finish(&mut self) -> Result<RecordBatch, ExportError> {
        let arrays: Vec<ArrayRef> = self.builders
            .iter_mut()
            .map(|builder| builder.finish())
            .collect();
        
        self.current_rows = 0;
        
        // Recreate builders for next batch
        self.builders = self.schema
            .fields()
            .iter()
            .map(|field| Self::create_builder(field))
            .collect();
        
        RecordBatch::try_new(self.schema.clone(), arrays)
            .map_err(|e| ExportError::Schema(e.to_string()))
    }
} 