use crate::domains::export::types::*;
use crate::domains::export::writer::*;
use crate::domains::export::ios::memory::*;
use crate::domains::export::csv_record::CsvRecord;
use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use serde::Serialize;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;
use csv::ByteRecord;
use flate2::write::ZlibEncoder;
use flate2::Compression;

/// Modern streaming CSV writer with iOS optimizations
pub struct StreamingCsvWriter<W: AsyncWrite + Unpin + Send> {
    inner: W,
    config: CsvConfig,
    header_written: AtomicBool,
    stats: ExportStats,
    start_time: Instant,
    memory_observer: MemoryPressureObserver,
    adaptive_buffer: AdaptiveBuffer,
}

#[derive(Clone)]
pub struct CsvConfig {
    pub delimiter: u8,
    pub quote_char: u8,
    pub escape_char: Option<u8>,
    pub compress: bool,
    pub batch_size: usize,
}

impl Default for CsvConfig {
    fn default() -> Self {
        Self {
            delimiter: b',',
            quote_char: b'"',
            escape_char: None,
            compress: false,
            batch_size: 1000,
        }
    }
}

impl<W: AsyncWrite + Unpin + Send> StreamingCsvWriter<W> {
    pub fn new(writer: W, config: CsvConfig) -> Self {
        Self {
            inner: writer,
            config,
            header_written: AtomicBool::new(false),
            stats: ExportStats {
                entities_written: 0,
                bytes_written: 0,
                duration_ms: 0,
                memory_peak_mb: 0,
                compression_ratio: None,
            },
            start_time: Instant::now(),
            memory_observer: MemoryPressureObserver::new(),
            adaptive_buffer: AdaptiveBuffer::new(),
        }
    }
    
    async fn write_headers_for_strategic_goals(&mut self) -> Result<(), ExportError> {
        if self.header_written.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        
        // Add UTF-8 BOM for Excel compatibility
        self.inner.write_all(b"\xEF\xBB\xBF").await.map_err(|e| ExportError::Io(e.to_string()))?;
        
        let mut buffer = Vec::new();
        {
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(self.config.delimiter)
                .quote(self.config.quote_char)
                .from_writer(&mut buffer);
            
            // Use CsvRecord trait headers
            let headers = crate::domains::strategic_goal::types::StrategicGoalResponse::headers();
            wtr.write_record(&headers).map_err(|e| ExportError::Serialization(e.to_string()))?;
            wtr.flush().map_err(|e| ExportError::Io(e.to_string()))?;
        }
        
        self.inner.write_all(&buffer).await.map_err(|e| ExportError::Io(e.to_string()))?;
        self.stats.bytes_written += buffer.len() + 3; // Include BOM bytes
        
        Ok(())
    }
    
    async fn write_csv_record<T: CsvRecord>(&mut self, record: &T) -> Result<(), ExportError> {
        let mut buffer = self.adaptive_buffer.get_buffer().await;
        
        // Use CsvRecord trait directly - no JSON conversion
        {
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(self.config.delimiter)
                .quote(self.config.quote_char)
                .from_writer(&mut buffer);
            
            let csv_row = record.to_csv();
            wtr.write_record(&csv_row).map_err(|e| ExportError::Serialization(e.to_string()))?;
            wtr.flush().map_err(|e| ExportError::Io(e.to_string()))?;
        }
        
        // Write directly without iOS-specific escaping (which was corrupting data)
        self.inner.write_all(&buffer).await.map_err(|e| ExportError::Io(e.to_string()))?;
        
        self.stats.entities_written += 1;
        self.stats.bytes_written += buffer.len();
        
        self.adaptive_buffer.release(buffer);
        
        Ok(())
    }
    
    // Keep the legacy JSON method for backward compatibility but fix it
    async fn write_json_record(&mut self, record: &serde_json::Value) -> Result<(), ExportError> {
        // Try to deserialize as StrategicGoalResponse first (most common)
        if let Ok(strategic_goal) = serde_json::from_value::<crate::domains::strategic_goal::types::StrategicGoalResponse>(record.clone()) {
            return self.write_csv_record(&strategic_goal).await;
        }
        
        // Try to deserialize as StrategicGoal
        if let Ok(strategic_goal) = serde_json::from_value::<crate::domains::strategic_goal::types::StrategicGoal>(record.clone()) {
            return self.write_csv_record(&strategic_goal).await;
        }
        
        // Fallback to generic JSON handling
        let mut buffer = self.adaptive_buffer.get_buffer().await;
        
        // Extract values in the correct order for CSV
        let csv_record = self.extract_csv_fields_from_json(record)?;
        
        // Write as CSV record
        {
            let mut wtr = csv::WriterBuilder::new()
                .delimiter(self.config.delimiter)
                .quote(self.config.quote_char)
                .from_writer(&mut buffer);
            
            wtr.write_record(&csv_record).map_err(|e| ExportError::Serialization(e.to_string()))?;
            wtr.flush().map_err(|e| ExportError::Io(e.to_string()))?;
        }
        
        // Write directly without corrupting escaping
        self.inner.write_all(&buffer).await.map_err(|e| ExportError::Io(e.to_string()))?;
        
        self.stats.entities_written += 1;
        self.stats.bytes_written += buffer.len();
        
        self.adaptive_buffer.release(buffer);
        
        Ok(())
    }
    
    fn extract_csv_fields_from_json(&self, json_value: &serde_json::Value) -> Result<Vec<String>, ExportError> {
        let obj = json_value.as_object()
            .ok_or_else(|| ExportError::Serialization("Expected JSON object".to_string()))?;
        
        // Extract fields in the expected order for strategic goals (matching database schema)
        let fields = vec![
            "id",
            "objective_code", 
            "outcome",
            "kpi",
            "target_value",
            "actual_value", 
            "progress_percentage", // Calculated field
            "status_id",
            "responsible_team",
            "sync_priority",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "updated_by_user_id",
            "deleted_at",
            "last_synced_at" // Not in database, will be empty
        ];
        
        let mut csv_record = Vec::new();
        for field in fields {
            let value = obj.get(field)
                .map(|v| self.format_csv_value(v))
                .unwrap_or_else(|| String::new());
            csv_record.push(value);
        }
        
        Ok(csv_record)
    }
    
    fn format_csv_value(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::Null => String::new(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => {
                // Convert array to comma-separated string
                arr.iter()
                    .map(|v| self.format_csv_value(v))
                    .collect::<Vec<_>>()
                    .join("; ")
            }
            serde_json::Value::Object(_) => {
                // For nested objects, convert to JSON string
                serde_json::to_string(value).unwrap_or_default()
            }
        }
    }
}

#[async_trait]
impl<W: AsyncWrite + Unpin + Send + Sync + 'static> StreamingExportWriter for StreamingCsvWriter<W> {
    async fn write_json_stream(&mut self, mut stream: Box<dyn Stream<Item = Result<serde_json::Value, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError>
    {
        let mut batch = Vec::with_capacity(self.config.batch_size);
        let mut first_item = true;
        
        while let Some(result) = stream.next().await {
            let item = result?;
            
            // Write headers on first item
            if first_item {
                self.write_headers_for_strategic_goals().await?;
                first_item = false;
            }
            
            // Check memory pressure
            if self.memory_observer.is_critical() {
                self.flush().await?;
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
            
            batch.push(item);
            
            // Write batch when full
            if batch.len() >= self.config.batch_size {
                for item in &batch {
                    self.write_json_record(item).await?;
                }
                batch.clear();
                
                // Yield for iOS background processing
                tokio::task::yield_now().await;
            }
        }
        
        // Write remaining items
        for item in &batch {
            self.write_json_record(item).await?;
        }
        
        self.flush().await?;
        
        self.stats.duration_ms = self.start_time.elapsed().as_millis() as u64;
        Ok(self.stats.clone())
    }
    
    async fn write_batch_stream(&mut self, _stream: Box<dyn Stream<Item = Result<arrow::record_batch::RecordBatch, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError> {
        Err(ExportError::InvalidConfig("CSV writer doesn't support Arrow batches".to_string()))
    }
    
    async fn flush(&mut self) -> Result<(), ExportError> {
        self.inner.flush().await.map_err(|e| ExportError::Io(e.to_string()))?;
        Ok(())
    }
    
    async fn finalize(mut self: Box<Self>) -> Result<ExportMetadata, ExportError> {
        self.flush().await?;
        
        Ok(ExportMetadata {
            format: ExportFormat::Csv {
                delimiter: self.config.delimiter,
                quote_char: self.config.quote_char,
                escape_char: self.config.escape_char,
                compress: self.config.compress,
            },
            stats: self.stats,
            file_paths: vec![], // Will be set by service layer
            schema_version: 1,
            checksum: None,
        })
    }
    
    fn format(&self) -> ExportFormat {
        ExportFormat::Csv {
            delimiter: self.config.delimiter,
            quote_char: self.config.quote_char,
            escape_char: self.config.escape_char,
            compress: self.config.compress,
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
        crate::domains::export::writer::DeviceCapabilities::optimal_batch_size(self.format())
    }
}

/// Compressed CSV writer for large exports using our existing compression
pub struct CompressedCsvWriter<W: AsyncWrite + Unpin + Send> {
    inner: StreamingCsvWriter<Vec<u8>>,
    output: W,
}

impl<W: AsyncWrite + Unpin + Send + 'static> CompressedCsvWriter<W> {
    pub fn new(writer: W, config: CsvConfig) -> Self {
        let mut csv_config = config;
        csv_config.compress = true;
        
        Self {
            inner: StreamingCsvWriter::new(Vec::new(), csv_config),
            output: writer,
        }
    }
}

#[async_trait]
impl<W: AsyncWrite + Unpin + Send + Sync + 'static> StreamingExportWriter for CompressedCsvWriter<W> {
    async fn write_json_stream(&mut self, stream: Box<dyn Stream<Item = Result<serde_json::Value, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError> {
        let stats = self.inner.write_json_stream(stream).await?;
        
        // Write the raw CSV data directly (compression handled by service layer)
        let uncompressed_data = std::mem::take(&mut self.inner.inner);
        self.output.write_all(&uncompressed_data).await
            .map_err(|e| ExportError::Io(e.to_string()))?;
        
        Ok(stats)
    }
    
    async fn write_batch_stream(&mut self, _stream: Box<dyn Stream<Item = Result<arrow::record_batch::RecordBatch, ExportError>> + Send + Unpin>) -> Result<ExportStats, ExportError> {
        Err(ExportError::InvalidConfig("Compressed CSV writer doesn't support Arrow batches".to_string()))
    }
    
    async fn flush(&mut self) -> Result<(), ExportError> {
        self.output.flush().await.map_err(|e| ExportError::Io(e.to_string()))?;
        Ok(())
    }
    
    async fn finalize(mut self: Box<Self>) -> Result<ExportMetadata, ExportError> {
        self.flush().await?;
        Box::new(self.inner).finalize().await
    }
    
    fn format(&self) -> ExportFormat {
        self.inner.format()
    }
    
    fn can_handle_pressure(&self, level: MemoryPressureLevel) -> bool {
        self.inner.can_handle_pressure(level)
    }
    
    fn optimal_batch_size(&self) -> usize {
        self.inner.optimal_batch_size()
    }
} 