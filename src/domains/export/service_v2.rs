use crate::domains::export::types::*;
use crate::domains::export::writer::*;
use crate::domains::export::writers::{csv_writer::*, parquet_writer::*};
use crate::domains::export::schemas::parquet::get_cached_schema;
use crate::domains::export::repository::{ExportJobRepository, TransactionalJobRepository};
use crate::domains::export::repository_v2::{StreamingExportRepository, ExportEntity, SqliteStreamingRepository};
use crate::domains::export::queue_manager::{ExportQueueManager, ExportJob as QueueJob, JobPriority};
use crate::domains::export::ios::background_v2::ModernBackgroundExporter;
use crate::domains::export::service::{export_strategic_goals_by_ids, create_zip_from_dir};
use crate::globals;
use tempfile::TempDir;
use crate::auth::AuthContext;
use crate::errors::{ServiceError, ServiceResult};
use async_trait::async_trait;
use futures::stream::{Stream, StreamExt};
use futures::StreamExt as FuturesStreamExt;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use tokio::fs::File;
use tokio::io::{AsyncWriteExt, BufWriter};
use uuid::Uuid;
use chrono::Utc;
use serde_json;
use sqlx::{Transaction, Sqlite};
use log;

// Type alias for enhanced CSV writer that includes document metadata columns
type EnhancedCsvWriterWithDocuments<W> = StreamingCsvWriter<W>;

/// Trait for processing export jobs
#[async_trait]
pub trait JobProcessor: Send + Sync {
    async fn process(&self, job: QueueJob) -> Result<ExportSummary, ExportError>;
}



/// Modern export service with streaming support
pub struct ExportServiceV2 {
    job_repo: Arc<dyn ExportJobRepository>,
    streaming_repo: Arc<SqliteStreamingRepository>,
    file_storage: Arc<dyn crate::domains::core::file_storage_service::FileStorageService>,
    export_manager: Arc<ExportQueueManager>,
    background_exporter: Arc<ModernBackgroundExporter>,
}

impl std::fmt::Debug for ExportServiceV2 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExportServiceV2")
            .field("job_repo", &"<ExportJobRepository>")
            .field("streaming_repo", &"<SqliteStreamingRepository>")
            .field("file_storage", &"<FileStorageService>")
            .field("export_manager", &"<ExportQueueManager>")
            .field("background_exporter", &"<ModernBackgroundExporter>")
            .finish()
    }
}

impl ExportServiceV2 {
    pub fn new(
        job_repo: Arc<dyn ExportJobRepository>,
        streaming_repo: Arc<SqliteStreamingRepository>,
        file_storage: Arc<dyn crate::domains::core::file_storage_service::FileStorageService>,
    ) -> Arc<Self> {
        Arc::new_cyclic(|weak_self| {
            let job_processor = weak_self.clone() as Weak<dyn JobProcessor>;
            let export_manager = Arc::new(ExportQueueManager::new(job_processor));
            Self {
                job_repo: job_repo.clone(),
                streaming_repo: streaming_repo.clone(),
                file_storage: file_storage.clone(),
                export_manager,
                background_exporter: Arc::new(ModernBackgroundExporter::new()),
            }
        })
    }
    
    /// Modern streaming export with backpressure
    pub async fn export_streaming(
        &self,
        request: ExportRequest,
        auth: &AuthContext,
    ) -> ServiceResult<ExportSummary> {
        // Permission check
        auth.authorize(crate::types::Permission::ExportData)?;

        // For simple exports (single filter, small data), execute directly
        // This avoids the complex queue system that's causing hangs
        if self.should_execute_directly(&request) {
            let (progress_tx, _) = tokio::sync::mpsc::channel(100);
            return self.export_with_progress(request, auth, progress_tx).await;
        }

        // Create export job in database for complex exports
        let job = ExportJob {
            id: Uuid::new_v4(),
            requested_by_user_id: Some(auth.user_id),
            requested_at: Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Running,
            local_path: None,
            total_entities: None,
            total_bytes: None,
            error_message: None,
        };

        let job_id = job.id;
        self.job_repo.create_job(&job).await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        // Queue the export job
        let queue_job = QueueJob {
            id: job_id,
            request: request.clone(),
            priority: self.determine_priority(&request),
            created_at: Utc::now(),
            status: crate::domains::export::queue_manager::JobStatus::Queued,
            retry_count: 0,
        };

        let handle = self.export_manager.enqueue(queue_job).await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        // Return summary immediately
        Ok(ExportSummary {
            job: job,
        })
    }

    /// Determine if we should execute export directly instead of queuing
    fn should_execute_directly(&self, request: &ExportRequest) -> bool {
        // Set strict limits for performance and stability
        if request.filters.len() != 1 {
            return false;
        }

        match &request.filters[0] {
            EntityFilter::StrategicGoalsByIds { ids } => {
                // Limit: 1000 items max for strategic goals
                if request.include_blobs {
                    ids.len() <= 1000 // With attachments: 1000 max
                } else {
                    ids.len() <= 1000 // Without attachments: 1000 max
                }
            }
            EntityFilter::ProjectsByIds { ids } => {
                // Limit: 1000 items max for projects
                if request.include_blobs {
                    ids.len() <= 1000 // With attachments: 1000 max
                } else {
                    ids.len() <= 1000 // Without attachments: 1000 max
                }
            }
            EntityFilter::ParticipantsByIds { ids } => {
                // Limit: 1000 items max for participants
                if request.include_blobs {
                    ids.len() <= 1000 // With attachments: 1000 max
                } else {
                    ids.len() <= 1000 // Without attachments: 1000 max
                }
            }
            EntityFilter::ActivitiesByIds { ids } => {
                // Limit: 1000 items max for activities
                if request.include_blobs {
                    ids.len() <= 1000 // With attachments: 1000 max
                } else {
                    ids.len() <= 1000 // Without attachments: 1000 max
                }
            }
            EntityFilter::StrategicGoals { .. } => {
                // All strategic goals exports: limit to 1000 via streaming
                true
            }
            EntityFilter::ProjectsAll | 
            EntityFilter::ActivitiesAll | 
            EntityFilter::DonorsAll | 
            EntityFilter::FundingAll | 
            EntityFilter::LivelihoodsAll => {
                // Other domain exports: limit to 1000 without blobs
                !request.include_blobs
            }
            EntityFilter::ParticipantsAll => {
                // Participants: limit to 1000 without blobs
                !request.include_blobs
            }
            EntityFilter::WorkshopsAll { .. } => {
                // Workshops: limit to 1000 without blobs
                !request.include_blobs
            }
            // Other filters: only simple cases without blobs
            _ => !request.include_blobs && matches!(&request.format, Some(ExportFormat::Csv { .. }) | None)
        }
    }
    
    /// Export with progress tracking for background tasks
    pub async fn export_with_progress(
        &self,
        request: ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<ExportSummary> {
        // Run the actual export logic
        match &request.format.clone().unwrap_or_default() {
            ExportFormat::Csv { .. } => {
                self.export_csv_streaming(&request, auth, progress_tx).await
            }
            ExportFormat::Parquet { .. } => {
                self.export_parquet_streaming(&request, auth, progress_tx).await
            }
            ExportFormat::JsonLines => {
                self.export_jsonl_streaming(&request, auth, progress_tx).await
            }
        }
    }
    
    /// Export to CSV using streaming writer with ZIP structure when include_blobs is true
    async fn export_csv_streaming(
        &self,
        request: &ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<ExportSummary> {
        log::debug!("Starting CSV export with include_blobs={}", request.include_blobs);
        
        // Only create ZIP structure if include_blobs is true
        if request.include_blobs {
            log::debug!("Using ZIP structure for CSV export with blobs");
            return self.export_csv_with_zip_structure(request, auth, progress_tx).await;
        }

        log::debug!("Using simple CSV export without blobs");

        // Create export job
        let job_id = Uuid::new_v4();
        let job = ExportJob {
            id: job_id,
            requested_by_user_id: Some(auth.user_id),
            requested_at: Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Running,
            local_path: None,
            total_entities: None,
            total_bytes: None,
            error_message: None,
        };

        // For simple CSV exports, don't save job to database - just process directly
        let output_path = self.generate_export_path(request);
        log::debug!("Exporting to path: {}", output_path.display());
        
        let csv_file = File::create(&output_path).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create CSV file: {}", e)))?;

        let config = match &request.format {
            Some(ExportFormat::Csv { delimiter, quote_char, escape_char, .. }) => CsvConfig {
                    delimiter: *delimiter,
                    quote_char: *quote_char,
                    escape_char: *escape_char,
                compress: false,
                batch_size: 1000, // Increased batch size for better performance
            },
            _ => CsvConfig::default(),
        };

        log::debug!("Using CSV config: delimiter={}, batch_size={}", config.delimiter as char, config.batch_size);

        let mut writer = StreamingCsvWriter::new(csv_file, config);
        
        // Create entity stream with optimized batch size
        let stream = self.create_entity_stream(&request.filters, progress_tx.clone()).await?;
        
        log::debug!("Writing entities to CSV");
        let stats = writer.write_json_stream(Box::new(stream)).await
            .map_err(|e| ServiceError::InternalError(format!("CSV writing failed: {}", e)))?;
        
        writer.flush().await
            .map_err(|e| ServiceError::InternalError(format!("CSV flush failed: {}", e)))?;
        
        log::debug!("CSV export completed. Entities written: {}, bytes: {}", stats.entities_written, stats.bytes_written);

        // Get file size
        let file_size = std::fs::metadata(&output_path)
            .map(|m| m.len() as i64)
            .unwrap_or(0);

        let completed_job = ExportJob {
            id: job_id,
            requested_by_user_id: Some(auth.user_id),
            requested_at: Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Completed,
            local_path: Some(output_path.to_string_lossy().to_string()),
            total_entities: Some(stats.entities_written as i64),
            total_bytes: Some(file_size),
            error_message: None,
        };

        Ok(ExportSummary { job: completed_job })
    }

    /// Export CSV with ZIP structure containing documents and CSV files
    async fn export_csv_with_zip_structure(
        &self,
        request: &ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<ExportSummary> {
        let temp_dir = TempDir::new().map_err(|e| ServiceError::InternalError(format!("Failed to create temp dir: {}", e)))?;
        let temp_path = temp_dir.path();

        // 🔧 FIX: Generic entity type and filename determination
        let (entity_ids, entity_type) = match request.filters.first() {
            Some(EntityFilter::StrategicGoalsByIds { ids }) => (ids.clone(), "strategic_goals"),
            Some(EntityFilter::ProjectsByIds { ids }) => (ids.clone(), "projects"),
            Some(EntityFilter::ParticipantsByIds { ids }) => (ids.clone(), "participants"),
            Some(EntityFilter::ActivitiesByIds { ids }) => (ids.clone(), "activities"),
            _ => {
                log::warn!("CSV with ZIP structure only supports entity ID-based filters currently");
                return Err(ServiceError::ValidationError("CSV ZIP export only supports entity ID-based filters currently".to_string()));
            }
        };
        let csv_filename = format!("{}.csv", entity_type);

        let csv_path = temp_path.join(&csv_filename);
        
        log::debug!("Creating enhanced CSV export with document associations at: {}", csv_path.display());

        // Create CSV file with document metadata columns
        let csv_file = File::create(&csv_path).await.map_err(|e| ServiceError::InternalError(e.to_string()))?;

        let config = match &request.format {
            Some(ExportFormat::Csv { delimiter, quote_char, escape_char, .. }) => CsvConfig {
                    delimiter: *delimiter,
                    quote_char: *quote_char,
                    escape_char: *escape_char,
                compress: false, // Don't compress inside ZIP
                    batch_size: 1000,
            },
            _ => CsvConfig::default(),
        };

        // Use enhanced streaming CSV writer that includes document columns
        let mut writer = EnhancedCsvWriterWithDocuments::new(csv_file, config);

        log::debug!("Fetching documents for {} {} entities", entity_ids.len(), entity_type);

        // Pre-fetch all documents for efficient lookup
        let media_doc_repo = globals::get_media_document_repo()
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
        
        let all_documents = media_doc_repo
            .find_by_related_entities(entity_type, &entity_ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        // Group documents by entity ID for efficient lookup
        let mut documents_by_entity: std::collections::HashMap<uuid::Uuid, Vec<crate::domains::document::types::MediaDocument>> = std::collections::HashMap::new();
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                documents_by_entity.entry(related_id).or_insert_with(Vec::new).push(document);
            }
        }

        log::debug!("Found {} entities with documents", documents_by_entity.len());

        // Create the entity stream but enhance each entity with document metadata
        let stream = self.create_entity_stream(&request.filters, progress_tx.clone()).await?;
        
        // Process stream and add document metadata to each record
        let docs_by_entity = documents_by_entity.clone(); // Clone for move into closure
        let entity_type_copy = entity_type.to_string(); // Clone for move into closure
        let enhanced_stream = stream.map(move |result| {
            match result {
                Ok(mut entity) => {
                    // Extract entity ID from the entity
                    if let Some(entity_id_value) = entity.get("id") {
                        if let Some(entity_id_str) = entity_id_value.as_str() {
                            if let Ok(entity_id) = uuid::Uuid::parse_str(entity_id_str) {
                                // Get documents for this entity
                                let entity_documents = docs_by_entity.get(&entity_id).cloned().unwrap_or_default();
                                
                                // Add document metadata to the entity
                                entity["document_count"] = serde_json::Value::Number(serde_json::Number::from(entity_documents.len()));
                                entity["has_documents"] = serde_json::Value::Bool(!entity_documents.is_empty());
                                
                                // Add document filenames as a semicolon-separated list
                                let document_filenames: Vec<String> = entity_documents.iter()
                                    .map(|doc| doc.original_filename.clone())
                                    .collect();
                                entity["document_filenames"] = serde_json::Value::String(document_filenames.join("; "));
                                
                                // Add document types (using type_id since document_type_name doesn't exist)
                                let document_type_ids: Vec<String> = entity_documents.iter()
                                    .map(|doc| doc.type_id.to_string())
                                    .collect();
                                entity["document_type_ids"] = serde_json::Value::String(document_type_ids.join("; "));
                                
                                // Add file paths relative to the organized ZIP structure
                                let document_paths: Vec<String> = entity_documents.iter()
                                    .map(|doc| format!("files/{}/{}/{}", entity_type_copy, entity_id, doc.original_filename))
                                    .collect();
                                entity["document_paths"] = serde_json::Value::String(document_paths.join("; "));
                            }
                        }
                    }
                    Ok(entity)
                }
                Err(e) => Err(e)
            }
        });

        let stats = writer.write_json_stream(Box::new(enhanced_stream)).await.map_err(|e| ServiceError::InternalError(e.to_string()))?;
        writer.flush().await.map_err(|e| ServiceError::InternalError(e.to_string()))?;
        
        let mut total_file_size = std::fs::metadata(&csv_path).map(|m| m.len()).unwrap_or(0);

        // Copy documents to files directory
        if request.include_blobs {
            log::debug!("Copying documents to ZIP structure");
            let documents_size = match entity_type {
                "strategic_goals" => self.copy_documents_for_strategic_goals(temp_path, &entity_ids).await?,
                "projects" => self.copy_documents_for_projects(temp_path, &entity_ids).await?,
                "participants" => self.copy_documents_for_participants(temp_path, &entity_ids).await?,
                "activities" => self.copy_documents_for_activities(temp_path, &entity_ids).await?,
                _ => 0
            };
            total_file_size += documents_size;
            log::debug!("Copied {} bytes of documents", documents_size);
        }
        
        // Create README.txt to explain the export structure
        let readme_path = temp_path.join("README.txt");
        let entity_display_name = match entity_type {
            "strategic_goals" => "Strategic Goals",
            "projects" => "Projects",
            "participants" => "Participants",
            "activities" => "Activities",
            _ => "Entities"
        };
        let entity_singular = match entity_type {
            "strategic_goals" => "goal",
            "projects" => "project",
            "participants" => "participant",
            "activities" => "activity",
            _ => "entity"
        };
        let readme_content = format!(
            "ActionAid {} Export\n\
            =====================================\n\n\
            This export contains:\n\
            - {}: Main data file with document associations\n\
            - files/{}/[entity_id]/: Organized folders containing documents for each {}\n\n\
            Folder Structure:\n\
            - files/{}/[uuid]/: Documents for each {} (organized by {} ID)\n\
            - Each {} has its own folder containing only its documents\n\
            - Document filenames are preserved as uploaded\n\n\
            CSV Columns for Document Association:\n\
            - document_count: Number of documents attached to this {}\n\
            - has_documents: Boolean indicating if documents are attached\n\
            - document_filenames: List of document filenames (separated by '; ')\n\
            - document_type_ids: List of document type UUIDs (separated by '; ')\n\
            - document_paths: File paths relative to this ZIP (separated by '; ')\n\n\
            Example document path: files/{}/123e4567-e89b-12d3-a456-426614174000/report.pdf\n\n\
            Export created: {}\n\
            Records exported: {}\n\
            Documents included: {}\n",
            entity_display_name,
            csv_filename,
            entity_type, entity_singular,
            entity_type, entity_singular, entity_singular,
            entity_singular,
            entity_singular,
            entity_type,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            stats.entities_written,
            documents_by_entity.values().map(|docs| docs.len()).sum::<usize>()
        );
        
        tokio::fs::write(&readme_path, readme_content).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create README: {}", e)))?;

        let output_path = self.generate_export_path(request);
        let zip_path = if output_path.extension().and_then(|s| s.to_str()) == Some("zip") {
            output_path
        } else {
            output_path.with_extension("zip")
        };
        
        create_zip_from_dir(temp_path, &zip_path)
            .map_err(|e| ServiceError::InternalError(format!("Failed to create ZIP: {}", e)))?;

        let job = ExportJob {
            id: Uuid::new_v4(),
            requested_by_user_id: Some(auth.user_id),
            requested_at: Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Completed,
            local_path: Some(zip_path.to_string_lossy().to_string()),
            total_entities: Some(stats.entities_written as i64),
            total_bytes: Some(total_file_size as i64),
            error_message: None,
        };

        log::info!("CSV export with ZIP structure completed: {} entities, {} bytes", stats.entities_written, total_file_size);

        Ok(ExportSummary { job })
    }
    
    /// Export to Parquet using streaming writer with ZIP structure when include_blobs is true
    async fn export_parquet_streaming(
        &self,
        request: &ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<ExportSummary> {
        if request.include_blobs {
            // Use organized ZIP structure with documents (same as CSV/JSONL)
            return self.export_parquet_with_zip_structure(request, auth, progress_tx).await;
        }

        // Simple Parquet export without documents - FIXED implementation
        let output_path = self.generate_export_path(request);
        
        // Get schema for the primary domain being exported
        let schema = self.get_schema_for_filters(&request.filters)?;
        
        let mut writer = IOSParquetWriter::new_ios_optimized(&output_path, schema).await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        // Create Arrow RecordBatch stream for Parquet writer
        let arrow_stream = self.create_arrow_stream(&request.filters, progress_tx).await?;
        
        // Use the streaming interface to write Arrow batches
        let stats = writer.write_batch_stream(Box::new(arrow_stream)).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to write Parquet batches: {}", e)))?;

        // Finalize the writer
        let metadata = Box::new(writer).finalize().await
            .map_err(|e| ServiceError::InternalError(format!("Failed to finalize Parquet file: {}", e)))?;

        let file_size = tokio::fs::metadata(&output_path).await
            .map(|m| m.len())
            .unwrap_or(0);

        let job = ExportJob {
            id: uuid::Uuid::new_v4(),
            requested_by_user_id: Some(auth.user_id),
            requested_at: chrono::Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Completed,
            local_path: Some(output_path.to_string_lossy().to_string()),
            total_entities: Some(stats.entities_written as i64),
            total_bytes: Some(file_size as i64),
            error_message: None,
        };

        log::info!("Parquet export completed: {} entities, {} bytes", stats.entities_written, file_size);
        Ok(ExportSummary { job })
    }

    /// Export Parquet with organized ZIP structure containing documents (mirrors CSV/JSONL implementation)
    async fn export_parquet_with_zip_structure(
        &self,
        request: &ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<ExportSummary> {
        let temp_dir = TempDir::new().map_err(|e| ServiceError::InternalError(format!("Failed to create temp dir: {}", e)))?;
        let temp_path = temp_dir.path();

        // 🔧 FIX: Generic entity type and filename determination
        let (entity_ids, entity_type) = match request.filters.first() {
            Some(EntityFilter::StrategicGoalsByIds { ids }) => (ids.clone(), "strategic_goals"),
            Some(EntityFilter::ProjectsByIds { ids }) => (ids.clone(), "projects"),
            Some(EntityFilter::ParticipantsByIds { ids }) => (ids.clone(), "participants"),
            Some(EntityFilter::ActivitiesByIds { ids }) => (ids.clone(), "activities"),
            _ => {
                return Err(ServiceError::ValidationError("Parquet ZIP export only supports entity ID-based filters".to_string()));
            }
        };
        let parquet_filename = format!("{}.parquet", entity_type);

        let parquet_path = temp_path.join(&parquet_filename);
        
        log::debug!("Creating Parquet export with organized document structure at: {}", parquet_path.display());

        // Get documents by entity for association
        let documents_by_entity = if request.include_blobs {
            let media_doc_repo = globals::get_media_document_repo()
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

            let all_documents = media_doc_repo
                .find_by_related_entities(entity_type, &entity_ids)
                .await
                .map_err(|e| ServiceError::InternalError(e.to_string()))?;

            let mut docs_by_entity: std::collections::HashMap<Uuid, Vec<crate::domains::document::types::MediaDocument>> = std::collections::HashMap::new();
            for doc in all_documents {
                if let Some(entity_id) = doc.related_id {
                    docs_by_entity.entry(entity_id).or_insert_with(Vec::new).push(doc);
                }
            }
            docs_by_entity
        } else {
            std::collections::HashMap::new()
        };

        let parquet_path = temp_path.join(&parquet_filename);
        
        // Create Parquet file with enhanced schema including document metadata
        let schema = self.get_schema_for_filters(&request.filters)?;
        
        // Create mobile-optimized Parquet writer with smaller row groups and compression
        let mut writer = IOSParquetWriter::new_ios_optimized(&parquet_path, schema.clone()).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create Parquet writer: {}", e)))?;
        
        // Create enhanced Arrow stream with document metadata
        let arrow_stream = self.create_enhanced_arrow_stream_with_documents(&request.filters, &documents_by_entity, progress_tx.clone()).await?;
        
        // Use the streaming interface to write Arrow batches
        let stats = writer.write_batch_stream(Box::new(arrow_stream)).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to write Parquet batches: {}", e)))?;

        // Finalize the Parquet writer
        let _metadata = Box::new(writer).finalize().await
            .map_err(|e| ServiceError::InternalError(format!("Failed to finalize Parquet file: {}", e)))?;

        let parquet_file_size = tokio::fs::metadata(&parquet_path).await
            .map(|m| m.len())
            .unwrap_or(0);
        let mut total_file_size = parquet_file_size;

        // Copy documents using the same organized structure as CSV/JSONL
        if request.include_blobs {
            log::debug!("Copying documents to organized ZIP structure");
            let documents_size = match entity_type {
                "strategic_goals" => self.copy_documents_for_strategic_goals(temp_path, &entity_ids).await?,
                "projects" => self.copy_documents_for_projects(temp_path, &entity_ids).await?,
                "participants" => self.copy_documents_for_participants(temp_path, &entity_ids).await?,
                "activities" => self.copy_documents_for_activities(temp_path, &entity_ids).await?,
                _ => 0
            };
            total_file_size += documents_size;
            log::debug!("Copied {} bytes of documents", documents_size);
        }
        
        // Create README.txt to explain the export structure (same as CSV/JSONL)
        let readme_path = temp_path.join("README.txt");
        let entity_display_name = match entity_type {
            "strategic_goals" => "Strategic Goals",
            "projects" => "Projects",
            "participants" => "Participants",
            "activities" => "Activities",
            _ => "Entities"
        };
        let entity_singular = match entity_type {
            "strategic_goals" => "goal",
            "projects" => "project",
            "participants" => "participant",
            "activities" => "activity",
            _ => "entity"
        };
        let readme_content = format!(
            "ActionAid {} Export (Parquet Format)\n\
            ================================================\n\n\
            This export contains:\n\
            - {}: Main data file with document associations (columnar format)\n\
            - files/{}/[entity_id]/: Organized folders containing documents for each {}\n\n\
            Folder Structure:\n\
            - files/{}/[uuid]/: Documents for each {} (organized by {} ID)\n\
            - Each {} has its own folder containing only its documents\n\
            - Document filenames are preserved as uploaded\n\n\
            Parquet Columns for Document Association:\n\
            - document_count: Number of documents attached to this {}\n\
            - has_documents: Boolean indicating if documents are attached\n\
            - document_filenames: List of document filenames (separated by '; ')\n\
            - document_type_ids: List of document type UUIDs (separated by '; ')\n\
            - document_paths: File paths relative to this ZIP (separated by '; ')\n\n\
            Example document path: files/{}/123e4567-e89b-12d3-a456-426614174000/report.pdf\n\n\
            Export created: {}\n\
            Records exported: {}\n\
            Documents included: {}\n\
            Note: Parquet format provides efficient columnar storage and compression.\n\
            Note: Document metadata is not embedded in Parquet file, but documents are organized in folders.\n",
            entity_display_name,
            parquet_filename,
            entity_type, entity_singular,
            entity_type, entity_singular, entity_singular,
            entity_singular,
            entity_singular,
            entity_type,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            stats.entities_written,
            documents_by_entity.values().map(|docs| docs.len()).sum::<usize>()
        );
        
        tokio::fs::write(&readme_path, readme_content).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create README: {}", e)))?;

        let output_path = self.generate_export_path(request);
        let zip_path = if output_path.extension().and_then(|s| s.to_str()) == Some("zip") {
            output_path
        } else {
            output_path.with_extension("zip")
        };
        
        create_zip_from_dir(temp_path, &zip_path)
            .map_err(|e| ServiceError::InternalError(format!("Failed to create ZIP: {}", e)))?;

        let job = ExportJob {
            id: Uuid::new_v4(),
            requested_by_user_id: Some(auth.user_id),
            requested_at: Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Completed,
            local_path: Some(zip_path.to_string_lossy().to_string()),
            total_entities: Some(stats.entities_written as i64),
            total_bytes: Some(total_file_size as i64),
            error_message: None,
        };

        log::info!("Parquet export with organized ZIP structure completed: {} entities, {} bytes", stats.entities_written, total_file_size);

        Ok(ExportSummary { job })
    }
    
    /// Export with ZIP structure containing documents and files (restores previous behavior)
    async fn export_with_zip_structure(
        &self,
        request: &ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
        format: ExportFormat,
    ) -> ServiceResult<ExportSummary> {
        // Create temporary directory for organizing export structure
        let temp_dir = TempDir::new()
            .map_err(|e| ServiceError::InternalError(format!("Failed to create temp dir: {}", e)))?;
        
        // Use the existing comprehensive export logic from service.rs
        let file_storage = self.file_storage.clone();
        
                // Delegate to the original export service that creates proper ZIP structure
        match &request.filters.first() {
            Some(EntityFilter::StrategicGoalsByIds { ids }) => {
                let count = crate::domains::export::service::export_strategic_goals_by_ids(
                    temp_dir.path(), 
                    ids, 
                    request.include_blobs, 
                    &file_storage
                ).await.map_err(|e| ServiceError::InternalError(e))?;
                
                // Create ZIP from the structured directory
                let original_path = self.generate_export_path(request);
                let zip_name = if let Some(stem) = original_path.file_stem() {
                    format!("{}.zip", stem.to_string_lossy())
                } else {
                    format!("strategic_goals_export_{}.zip", uuid::Uuid::new_v4())
                };
                let output_path = original_path.parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(&zip_name);
                
                crate::domains::export::service::create_zip_from_dir(temp_dir.path(), &output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create ZIP: {}", e)))?;
                
                // Create summary
                let metadata = std::fs::metadata(&output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to stat ZIP: {}", e)))?;
                
                let job = ExportJob {
                    id: uuid::Uuid::new_v4(),
                    requested_by_user_id: Some(auth.user_id),
                    requested_at: chrono::Utc::now(),
                    include_blobs: request.include_blobs,
                    status: ExportStatus::Completed,
                    local_path: Some(output_path.to_string_lossy().to_string()),
                    total_entities: Some(count),
                    total_bytes: Some(metadata.len() as i64),
                    error_message: None,
                };
                
                Ok(ExportSummary { job })
            }
            Some(EntityFilter::ProjectsByIds { ids }) => {
                let count = crate::domains::export::service::export_projects_by_ids_with_options(
                    temp_dir.path(), 
                    ids, 
                    request.include_blobs, 
                    &file_storage
                ).await.map_err(|e| ServiceError::InternalError(e))?;
                
                // Create ZIP from the structured directory
                let original_path = self.generate_export_path(request);
                let zip_name = if let Some(stem) = original_path.file_stem() {
                    format!("{}.zip", stem.to_string_lossy())
                } else {
                    format!("projects_export_{}.zip", uuid::Uuid::new_v4())
                };
                let output_path = original_path.parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(&zip_name);
                
                crate::domains::export::service::create_zip_from_dir(temp_dir.path(), &output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create ZIP: {}", e)))?;
                
                // Create summary
                let metadata = std::fs::metadata(&output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to stat ZIP: {}", e)))?;
                
                let job = ExportJob {
                    id: uuid::Uuid::new_v4(),
                    requested_by_user_id: Some(auth.user_id),
                    requested_at: chrono::Utc::now(),
                    include_blobs: request.include_blobs,
                    status: ExportStatus::Completed,
                    local_path: Some(output_path.to_string_lossy().to_string()),
                    total_entities: Some(count),
                    total_bytes: Some(metadata.len() as i64),
                    error_message: None,
                };
                
                Ok(ExportSummary { job })
            }
            Some(EntityFilter::ParticipantsByIds { ids }) => {
                let count = crate::domains::export::service::export_participants_by_ids_with_documents(
                    temp_dir.path(), 
                    ids, 
                    request.include_blobs, 
                    &file_storage
                ).await.map_err(|e| ServiceError::InternalError(e))?;
                
                // Create ZIP from the structured directory
                let original_path = self.generate_export_path(request);
                let zip_name = if let Some(stem) = original_path.file_stem() {
                    format!("{}.zip", stem.to_string_lossy())
                } else {
                    format!("participants_export_{}.zip", uuid::Uuid::new_v4())
                };
                let output_path = original_path.parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(&zip_name);
                
                crate::domains::export::service::create_zip_from_dir(temp_dir.path(), &output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create ZIP: {}", e)))?;
                
                // Create summary
                let metadata = std::fs::metadata(&output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to stat ZIP: {}", e)))?;
                
                let job = ExportJob {
                    id: uuid::Uuid::new_v4(),
                    requested_by_user_id: Some(auth.user_id),
                    requested_at: chrono::Utc::now(),
                    include_blobs: request.include_blobs,
                    status: ExportStatus::Completed,
                    local_path: Some(output_path.to_string_lossy().to_string()),
                    total_entities: Some(count),
                    total_bytes: Some(metadata.len() as i64),
                    error_message: None,
                };
                
                Ok(ExportSummary { job })
            }
            Some(EntityFilter::ActivitiesByIds { ids }) => {
                let count = crate::domains::export::service::export_activities_by_ids_with_documents(
                    temp_dir.path(), 
                    ids, 
                    request.include_blobs, 
                    &file_storage
                ).await.map_err(|e| ServiceError::InternalError(e))?;
                
                // Create ZIP from the structured directory
                let original_path = self.generate_export_path(request);
                let zip_name = if let Some(stem) = original_path.file_stem() {
                    format!("{}.zip", stem.to_string_lossy())
                } else {
                    format!("activities_export_{}.zip", uuid::Uuid::new_v4())
                };
                let output_path = original_path.parent()
                    .unwrap_or_else(|| std::path::Path::new("."))
                    .join(&zip_name);
                
                crate::domains::export::service::create_zip_from_dir(temp_dir.path(), &output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create ZIP: {}", e)))?;
                
                // Create summary
                let metadata = std::fs::metadata(&output_path)
                    .map_err(|e| ServiceError::InternalError(format!("Failed to stat ZIP: {}", e)))?;
                
                let job = ExportJob {
                    id: uuid::Uuid::new_v4(),
                    requested_by_user_id: Some(auth.user_id),
                    requested_at: chrono::Utc::now(),
                    include_blobs: request.include_blobs,
                    status: ExportStatus::Completed,
                    local_path: Some(output_path.to_string_lossy().to_string()),
                    total_entities: Some(count),
                    total_bytes: Some(metadata.len() as i64),
                    error_message: None,
                };
                
                Ok(ExportSummary { job })
            }
            _ => {
                // For other filters, fall back to simple JSONL export
                Err(ServiceError::ValidationError("ZIP structure export only supports StrategicGoalsByIds, ProjectsByIds, ParticipantsByIds, and ActivitiesByIds filters currently".to_string()))
            }
        }
    }

    /// Export to JSONL using existing streaming approach with ZIP structure when include_blobs is true
    async fn export_jsonl_streaming(
        &self,
        request: &ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<ExportSummary> {
        if request.include_blobs {
            // Use organized ZIP structure with documents (same as CSV)
            return self.export_jsonl_with_zip_structure(request, auth, progress_tx).await;
        }

        // Simple JSONL export without documents - FIXED implementation
        let output_path = self.generate_export_path(request);
        let file = File::create(&output_path).await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;

        let mut writer = BufWriter::new(file);
        let stream = self.create_entity_stream(&request.filters, progress_tx).await?;
        
        let mut entities_written = 0u64;
        let mut bytes_written = 0u64;
        
        tokio::pin!(stream);
        
        while let Some(result) = stream.next().await {
            match result {
                Ok(entity) => {
                    let json_line = serde_json::to_string(&entity)
                        .map_err(|e| ServiceError::InternalError(format!("JSON serialization failed: {}", e)))?;
                    let line_with_newline = format!("{}\n", json_line);
                    
                    writer.write_all(line_with_newline.as_bytes()).await
                        .map_err(|e| ServiceError::InternalError(format!("Write failed: {}", e)))?;
                    
                    entities_written += 1;
                    bytes_written += line_with_newline.len() as u64;
                }
                Err(e) => {
                    return Err(ServiceError::InternalError(format!("Stream error: {}", e)));
                }
            }
        }
        
        writer.flush().await
            .map_err(|e| ServiceError::InternalError(format!("Flush failed: {}", e)))?;

        let job = ExportJob {
            id: uuid::Uuid::new_v4(),
            requested_by_user_id: Some(auth.user_id),
            requested_at: chrono::Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Completed,
            local_path: Some(output_path.to_string_lossy().to_string()),
            total_entities: Some(entities_written as i64),
            total_bytes: Some(bytes_written as i64),
            error_message: None,
        };

        log::info!("JSONL export completed: {} entities, {} bytes", entities_written, bytes_written);
        Ok(ExportSummary { job })
    }

    /// Export JSONL with organized ZIP structure containing documents (mirrors CSV implementation)
    async fn export_jsonl_with_zip_structure(
        &self,
        request: &ExportRequest,
        auth: &AuthContext,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<ExportSummary> {
        let temp_dir = TempDir::new().map_err(|e| ServiceError::InternalError(format!("Failed to create temp dir: {}", e)))?;
        let temp_path = temp_dir.path();

        // 🔧 FIX: Generic entity type and filename determination  
        let (entity_ids, entity_type) = match request.filters.first() {
            Some(EntityFilter::StrategicGoalsByIds { ids }) => (ids.clone(), "strategic_goals"),
            Some(EntityFilter::ProjectsByIds { ids }) => (ids.clone(), "projects"),
            Some(EntityFilter::ParticipantsByIds { ids }) => (ids.clone(), "participants"),
            Some(EntityFilter::ActivitiesByIds { ids }) => (ids.clone(), "activities"),
            _ => {
                return Err(ServiceError::ValidationError("JSONL ZIP export only supports entity ID-based filters".to_string()));
            }
        };
        let jsonl_filename = format!("{}.jsonl", entity_type);

        let jsonl_path = temp_path.join(&jsonl_filename);
        
        log::debug!("Creating JSONL export with organized document structure at: {}", jsonl_path.display());

        // Get documents by entity for association
        let documents_by_entity = if request.include_blobs {
            let media_doc_repo = globals::get_media_document_repo()
                .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
            let all_documents = media_doc_repo
                .find_by_related_entities(entity_type, &entity_ids)
                .await
                .map_err(|e| ServiceError::InternalError(e.to_string()))?;

            let mut docs_by_entity: std::collections::HashMap<Uuid, Vec<crate::domains::document::types::MediaDocument>> = std::collections::HashMap::new();
            for doc in all_documents {
                if let Some(entity_id) = doc.related_id {
                    docs_by_entity.entry(entity_id).or_insert_with(Vec::new).push(doc);
                }
            }
            docs_by_entity
        } else {
            std::collections::HashMap::new()
        };

        let jsonl_path = temp_path.join(&jsonl_filename);
        
        // Create JSONL file with document metadata
        let file = File::create(&jsonl_path).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create JSONL file: {}", e)))?;
        let mut writer = BufWriter::new(file);
        
        let stream = self.create_entity_stream(&request.filters, progress_tx.clone()).await?;
        tokio::pin!(stream);
        
        let mut entities_written = 0u64;
        let mut total_file_size = 0u64;

        // Process stream and add document metadata to each record (same logic as CSV)
        let docs_by_entity = documents_by_entity.clone();
        let entity_type_copy = entity_type.to_string();
        while let Some(result) = stream.next().await {
            match result {
                Ok(mut entity) => {
                    // Extract entity ID from the entity
                    if let Some(entity_id_value) = entity.get("id") {
                        if let Some(entity_id_str) = entity_id_value.as_str() {
                            if let Ok(entity_id) = uuid::Uuid::parse_str(entity_id_str) {
                                // Get documents for this entity
                                let entity_documents = docs_by_entity.get(&entity_id).cloned().unwrap_or_default();
                                
                                // Add document metadata to the entity (same as CSV)
                                entity["document_count"] = serde_json::Value::Number(serde_json::Number::from(entity_documents.len()));
                                entity["has_documents"] = serde_json::Value::Bool(!entity_documents.is_empty());
                                
                                // Add document filenames as a semicolon-separated list
                                let document_filenames: Vec<String> = entity_documents.iter()
                                    .map(|doc| doc.original_filename.clone())
                                    .collect();
                                entity["document_filenames"] = serde_json::Value::String(document_filenames.join("; "));
                                
                                // Add document type IDs
                                let document_type_ids: Vec<String> = entity_documents.iter()
                                    .map(|doc| doc.type_id.to_string())
                                    .collect();
                                entity["document_type_ids"] = serde_json::Value::String(document_type_ids.join("; "));
                                
                                // Add file paths relative to the organized ZIP structure
                                let document_paths: Vec<String> = entity_documents.iter()
                                    .map(|doc| format!("files/{}/{}/{}", entity_type_copy, entity_id, doc.original_filename))
                                    .collect();
                                entity["document_paths"] = serde_json::Value::String(document_paths.join("; "));
                            }
                        }
                    }

                    let json_line = serde_json::to_string(&entity)
                        .map_err(|e| ServiceError::InternalError(format!("JSON serialization failed: {}", e)))?;
                    let line_with_newline = format!("{}\n", json_line);
                    
                    writer.write_all(line_with_newline.as_bytes()).await
                        .map_err(|e| ServiceError::InternalError(format!("Write failed: {}", e)))?;
                    
                    entities_written += 1;
                    total_file_size += line_with_newline.len() as u64;
                }
                Err(e) => {
                    return Err(ServiceError::InternalError(format!("Stream error: {}", e)));
                }
            }
        }

        writer.flush().await
            .map_err(|e| ServiceError::InternalError(format!("Flush failed: {}", e)))?;

        // Copy documents using the same organized structure as CSV
        if request.include_blobs {
            log::debug!("Copying documents to organized ZIP structure");
            let documents_size = match entity_type {
                "strategic_goals" => self.copy_documents_for_strategic_goals(temp_path, &entity_ids).await?,
                "projects" => self.copy_documents_for_projects(temp_path, &entity_ids).await?,
                "participants" => self.copy_documents_for_participants(temp_path, &entity_ids).await?,
                "activities" => self.copy_documents_for_activities(temp_path, &entity_ids).await?,
                _ => 0
            };
            total_file_size += documents_size;
            log::debug!("Copied {} bytes of documents", documents_size);
        }
        
        // Create README.txt to explain the export structure (same as CSV)
        let readme_path = temp_path.join("README.txt");
        let entity_display_name = match entity_type {
            "strategic_goals" => "Strategic Goals",
            "projects" => "Projects",
            "participants" => "Participants",
            "activities" => "Activities",
            _ => "Entities"
        };
        let entity_singular = match entity_type {
            "strategic_goals" => "goal",
            "projects" => "project",
            "participants" => "participant",
            "activities" => "activity",
            _ => "entity"
        };
        let readme_content = format!(
            "ActionAid {} Export (JSONL Format)\n\
            ===============================================\n\n\
            This export contains:\n\
            - {}: Main data file with document associations (one JSON object per line)\n\
            - files/{}/[entity_id]/: Organized folders containing documents for each {}\n\n\
            Folder Structure:\n\
            - files/{}/[uuid]/: Documents for each {} (organized by {} ID)\n\
            - Each {} has its own folder containing only its documents\n\
            - Document filenames are preserved as uploaded\n\n\
            JSONL Fields for Document Association:\n\
            - document_count: Number of documents attached to this {}\n\
            - has_documents: Boolean indicating if documents are attached\n\
            - document_filenames: List of document filenames (separated by '; ')\n\
            - document_type_ids: List of document type UUIDs (separated by '; ')\n\
            - document_paths: File paths relative to this ZIP (separated by '; ')\n\n\
            Example document path: files/{}/123e4567-e89b-12d3-a456-426614174000/report.pdf\n\n\
            Export created: {}\n\
            Records exported: {}\n\
            Documents included: {}\n",
            entity_display_name,
            jsonl_filename,
            entity_type, entity_singular,
            entity_type, entity_singular, entity_singular,
            entity_singular,
            entity_singular,
            entity_type,
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC"),
            entities_written,
            documents_by_entity.values().map(|docs| docs.len()).sum::<usize>()
        );
        
        tokio::fs::write(&readme_path, readme_content).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create README: {}", e)))?;

        let output_path = self.generate_export_path(request);
        let zip_path = if output_path.extension().and_then(|s| s.to_str()) == Some("zip") {
            output_path
        } else {
            output_path.with_extension("zip")
        };
        
        create_zip_from_dir(temp_path, &zip_path)
            .map_err(|e| ServiceError::InternalError(format!("Failed to create ZIP: {}", e)))?;

        let job = ExportJob {
            id: Uuid::new_v4(),
            requested_by_user_id: Some(auth.user_id),
            requested_at: Utc::now(),
            include_blobs: request.include_blobs,
            status: ExportStatus::Completed,
            local_path: Some(zip_path.to_string_lossy().to_string()),
            total_entities: Some(entities_written as i64),
            total_bytes: Some(total_file_size as i64),
            error_message: None,
        };

        log::info!("JSONL export with organized ZIP structure completed: {} entities, {} bytes", entities_written, total_file_size);

        Ok(ExportSummary { job })
    }
    
    /// Create entity stream for CSV/JSON export using streaming repository with iOS optimization
    async fn create_entity_stream(
        &self,
        filters: &[EntityFilter],
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<impl Stream<Item = Result<serde_json::Value, ExportError>>> {
        // Use the first filter for now (in real implementation you'd handle multiple filters)
        let filter = filters.first().ok_or_else(|| {
            ServiceError::ValidationError("No filters provided".to_string())
        })?.clone();
        
        log::debug!("Creating entity stream for filter: {:?}", filter);
        
        // Optimized batch sizes for performance with 1000 item limit
        let (batch_size, max_total) = match &filter {
            EntityFilter::StrategicGoalsByIds { ids } => {
                let limited_ids = if ids.len() > 1000 {
                    log::warn!("Strategic goals export limited to first 1000 items (requested: {})", ids.len());
                    ids[..1000].to_vec()
                } else {
                    ids.clone()
                };
                
                // Create new filter with limited IDs
                let limited_filter = EntityFilter::StrategicGoalsByIds { ids: limited_ids.clone() };
                (50, limited_ids.len()) // Batch size 50, max 1000 total
            }
            EntityFilter::ProjectsByIds { ids } => {
                let limited_ids = if ids.len() > 1000 {
                    log::warn!("Projects export limited to first 1000 items (requested: {})", ids.len());
                    ids[..1000].to_vec()
                } else {
                    ids.clone()
                };
                
                (50, limited_ids.len()) // Batch size 50, max 1000 total
            }
            EntityFilter::ParticipantsByIds { ids } => {
                let limited_ids = if ids.len() > 1000 {
                    log::warn!("Participants export limited to first 1000 items (requested: {})", ids.len());
                    ids[..1000].to_vec()
                } else {
                    ids.clone()
                };
                
                (50, limited_ids.len()) // Batch size 50, max 1000 total
            }
            EntityFilter::ActivitiesByIds { ids } => {
                let limited_ids = if ids.len() > 1000 {
                    log::warn!("Activities export limited to first 1000 items (requested: {})", ids.len());
                    ids[..1000].to_vec()
                } else {
                    ids.clone()
                };
                
                (50, limited_ids.len()) // Batch size 50, max 1000 total
            }
            EntityFilter::StrategicGoals { .. } => (50, 1000), // Limit to 1000 strategic goals
            EntityFilter::ProjectsAll => (50, 1000), // Limit to 1000 projects
            EntityFilter::ActivitiesAll => (50, 1000), // Limit to 1000 activities
            EntityFilter::ParticipantsAll => (50, 1000), // Limit to 1000 participants
            EntityFilter::WorkshopsAll { .. } => (25, 1000), // Limit to 1000 workshops
            _ => (50, 1000), // Default: 1000 limit
        };
        
        log::debug!("Using batch size: {}, max total: {}", batch_size, max_total);
        
        // Create JSON stream using the streaming repository with limits
        let stream = self.streaming_repo.create_json_stream(filter, batch_size);
        
        // Convert stream with enhanced data and progress tracking
        let progress_sender = progress_tx.clone();
        let mut processed = 0;
        let max_items = max_total;
        
        // Convert stream with enhanced data, limits, and progress tracking
        let converted_stream = stream.map(move |result| {
            match result {
                Ok(mut json) => {
                    processed += 1;
                    
                    // Enforce 1000 item limit
                    if processed > max_items {
                        log::debug!("Reached export limit of {} items", max_items);
                        return Err(ExportError::Unknown("Export limit reached".to_string()));
                    }
                    
                    // Add calculated fields for strategic goals
                    if let Some(obj) = json.as_object_mut() {
                        // Add progress_percentage calculation
                        if let (Some(target), Some(actual)) = (
                            obj.get("target_value").and_then(|v| v.as_f64()),
                            obj.get("actual_value").and_then(|v| v.as_f64())
                        ) {
                            if target > 0.0 {
                                let progress = (actual / target) * 100.0;
                                obj.insert("progress_percentage".to_string(), serde_json::Value::Number(
                                    serde_json::Number::from_f64(progress).unwrap_or_else(|| serde_json::Number::from(0))
                                ));
                            } else {
                                obj.insert("progress_percentage".to_string(), serde_json::Value::Null);
                            }
                        } else {
                            obj.insert("progress_percentage".to_string(), serde_json::Value::Null);
                        }
                        
                        // Add missing fields with default values to prevent empty columns
                        let default_fields = vec![
                            ("sync_priority", serde_json::Value::String("normal".to_string())),
                            ("created_by_user_id", serde_json::Value::Null),
                            ("updated_by_user_id", serde_json::Value::Null),
                            ("deleted_at", serde_json::Value::Null),
                            ("last_synced_at", serde_json::Value::Null),
                        ];
                        
                        for (field_name, default_value) in default_fields {
                            if !obj.contains_key(field_name) {
                                obj.insert(field_name.to_string(), default_value);
                            }
                        }
                    }
                    
                    // Send progress every 50 items for better performance
                    if processed % 50 == 0 {
                        let progress = ExportProgress {
                            job_id: Uuid::new_v4(),
                            completed_bytes: processed * 512, // Better estimate
                            total_bytes: max_items * 512, // Better estimate
                            entities_processed: processed as u64,
                            current_domain: "export".to_string(),
                            estimated_time_remaining: 0.0,
                            status: ExportStatus::Running,
                        };
                        let _ = progress_sender.try_send(progress);
                    }
                    
                    Ok(json)
                }
                Err(e) => {
                    log::error!("Stream error: {}", e);
                    Err(ExportError::Database(e.to_string()))
                }
            }
        });
        
        Ok(converted_stream)
    }
    
    /// Create Arrow RecordBatch stream for Parquet export
    async fn create_arrow_stream(
        &self,
        filters: &[EntityFilter],
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<impl Stream<Item = Result<arrow::record_batch::RecordBatch, ExportError>>> {
        use arrow::array::*;
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use std::sync::Arc;
        
        // Get the schema for this domain
        let schema = self.get_schema_for_filters(filters)?;
        
        // Create JSON entity stream
        let json_stream = self.create_entity_stream(filters, progress_tx).await?;
        
        // Convert JSON stream to Arrow RecordBatch stream
        let arrow_stream = json_stream.chunks(1000).map(move |chunk_result| {
            // Process a chunk of JSON entities into a RecordBatch
            let entities: Result<Vec<_>, _> = chunk_result.into_iter().collect();
            match entities {
                Ok(entities) => {
                    if entities.is_empty() {
                        return Err(ExportError::Unknown("Empty chunk".to_string()));
                    }
                    
                    // Convert JSON entities to Arrow arrays
                    Self::json_entities_to_record_batch(&entities, schema.clone())
                        .map_err(|e| ExportError::Serialization(format!("Arrow conversion failed: {}", e)))
                }
                Err(e) => Err(e)
            }
        });
        
        Ok(arrow_stream)
    }
    
    /// Convert JSON entities to Arrow RecordBatch
    fn json_entities_to_record_batch(
        entities: &[serde_json::Value],
        schema: Arc<arrow::datatypes::Schema>,
    ) -> Result<arrow::record_batch::RecordBatch, Box<dyn std::error::Error + Send + Sync>> {
        use arrow::array::*;
        use arrow::datatypes::DataType;
        use arrow::record_batch::RecordBatch;
        
        if entities.is_empty() {
            return Err("No entities to convert".into());
        }
        
        let mut arrays: Vec<ArrayRef> = Vec::new();
        
        // Build arrays for each field in the schema
        for field in schema.fields() {
            let field_name = field.name();
            
            match field.data_type() {
                DataType::Utf8 => {
                    let mut builder = StringBuilder::new();
                    for entity in entities {
                        if let Some(value) = entity.get(field_name) {
                            match value {
                                serde_json::Value::String(s) => builder.append_value(s),
                                serde_json::Value::Null => builder.append_null(),
                                _ => builder.append_value(&value.to_string()),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrays.push(Arc::new(builder.finish()));
                }
                DataType::Int64 => {
                    let mut builder = Int64Builder::new();
                    for entity in entities {
                        if let Some(value) = entity.get(field_name) {
                            match value {
                                serde_json::Value::Number(n) => {
                                    if let Some(i) = n.as_i64() {
                                        builder.append_value(i);
                                    } else {
                                        builder.append_null();
                                    }
                                }
                                serde_json::Value::Null => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrays.push(Arc::new(builder.finish()));
                }
                DataType::Float64 => {
                    let mut builder = Float64Builder::new();
                    for entity in entities {
                        if let Some(value) = entity.get(field_name) {
                            match value {
                                serde_json::Value::Number(n) => {
                                    if let Some(f) = n.as_f64() {
                                        builder.append_value(f);
                                    } else {
                                        builder.append_null();
                                    }
                                }
                                serde_json::Value::Null => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrays.push(Arc::new(builder.finish()));
                }
                DataType::Boolean => {
                    let mut builder = BooleanBuilder::new();
                    for entity in entities {
                        if let Some(value) = entity.get(field_name) {
                            match value {
                                serde_json::Value::Bool(b) => builder.append_value(*b),
                                serde_json::Value::Null => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrays.push(Arc::new(builder.finish()));
                }
                DataType::Timestamp(arrow::datatypes::TimeUnit::Millisecond, _) => {
                    let mut builder = TimestampMillisecondBuilder::new();
                    for entity in entities {
                        if let Some(value) = entity.get(field_name) {
                            match value {
                                serde_json::Value::String(s) => {
                                    // Try to parse ISO 8601 timestamp
                                    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
                                        builder.append_value(dt.timestamp_millis());
                                    } else {
                                        builder.append_null();
                                    }
                                }
                                serde_json::Value::Number(n) => {
                                    // Assume it's already a timestamp in milliseconds
                                    if let Some(ts) = n.as_i64() {
                                        builder.append_value(ts);
                                    } else {
                                        builder.append_null();
                                    }
                                }
                                serde_json::Value::Null => builder.append_null(),
                                _ => builder.append_null(),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrays.push(Arc::new(builder.finish()));
                }
                _ => {
                    // Fallback to string for unknown types
                    let mut builder = StringBuilder::new();
                    for entity in entities {
                        if let Some(value) = entity.get(field_name) {
                            match value {
                                serde_json::Value::Null => builder.append_null(),
                                _ => builder.append_value(&value.to_string()),
                            }
                        } else {
                            builder.append_null();
                        }
                    }
                    arrays.push(Arc::new(builder.finish()));
                }
            }
        }
        
        RecordBatch::try_new(schema, arrays).map_err(|e| e.into())
    }
    
    /// Create enhanced Arrow stream with document metadata for Parquet ZIP exports
    async fn create_enhanced_arrow_stream_with_documents(
        &self,
        filters: &[EntityFilter],
        documents_by_entity: &std::collections::HashMap<Uuid, Vec<crate::domains::document::types::MediaDocument>>,
        progress_tx: mpsc::Sender<ExportProgress>,
    ) -> ServiceResult<impl Stream<Item = Result<arrow::record_batch::RecordBatch, ExportError>>> {
        use arrow::array::*;
        use arrow::datatypes::{DataType, Field, Schema};
        use arrow::record_batch::RecordBatch;
        use std::sync::Arc;
        
        // Get the enhanced schema with document metadata fields
        let schema = self.get_schema_for_filters(filters)?;
        
        // Create JSON entity stream
        let json_stream = self.create_entity_stream(filters, progress_tx).await?;
        
        // Clone documents_by_entity for move into closure
        let docs_by_entity = documents_by_entity.clone();
        
        // Determine entity type for path generation
        let entity_type = match filters.first() {
            Some(EntityFilter::StrategicGoalsByIds { .. }) => "strategic_goals",
            Some(EntityFilter::ProjectsByIds { .. }) => "projects",
            Some(EntityFilter::ParticipantsByIds { .. }) => "participants",
            Some(EntityFilter::ActivitiesByIds { .. }) => "activities",
            _ => "entities"
        };
        let entity_type_copy = entity_type.to_string();
        
        // Enhance JSON stream with document metadata (same logic as CSV/JSONL)
        let enhanced_stream = json_stream.map(move |result| {
            match result {
                Ok(mut entity) => {
                    // Extract entity ID from the entity and add document metadata
                    if let Some(entity_id_value) = entity.get("id") {
                        if let Some(entity_id_str) = entity_id_value.as_str() {
                            if let Ok(entity_id) = uuid::Uuid::parse_str(entity_id_str) {
                                // Get documents for this entity
                                let entity_documents = docs_by_entity.get(&entity_id).cloned().unwrap_or_default();
                                
                                // Add document metadata to the entity (same as CSV/JSONL)
                                entity["document_count"] = serde_json::Value::Number(serde_json::Number::from(entity_documents.len()));
                                entity["has_documents"] = serde_json::Value::Bool(!entity_documents.is_empty());
                                
                                // Add document filenames as a semicolon-separated list
                                let document_filenames: Vec<String> = entity_documents.iter()
                                    .map(|doc| doc.original_filename.clone())
                                    .collect();
                                entity["document_filenames"] = serde_json::Value::String(document_filenames.join("; "));
                                
                                // Add document type IDs
                                let document_type_ids: Vec<String> = entity_documents.iter()
                                    .map(|doc| doc.type_id.to_string())
                                    .collect();
                                entity["document_type_ids"] = serde_json::Value::String(document_type_ids.join("; "));
                                
                                // Add file paths relative to the organized ZIP structure
                                let document_paths: Vec<String> = entity_documents.iter()
                                    .map(|doc| format!("files/{}/{}/{}", entity_type_copy, entity_id, doc.original_filename))
                                    .collect();
                                entity["document_paths"] = serde_json::Value::String(document_paths.join("; "));
                            }
                        }
                    }
                    Ok(entity)
                }
                Err(e) => Err(e)
            }
        });
        
        // Convert enhanced JSON stream to Arrow RecordBatch stream
        let arrow_stream = enhanced_stream.chunks(1000).map(move |chunk_result| {
            // Process a chunk of enhanced JSON entities into a RecordBatch
            let entities: Result<Vec<_>, _> = chunk_result.into_iter().collect();
            match entities {
                Ok(entities) => {
                    if entities.is_empty() {
                        return Err(ExportError::Unknown("Empty chunk".to_string()));
                    }
                    
                    // Convert enhanced JSON entities to Arrow arrays
                    Self::json_entities_to_record_batch(&entities, schema.clone())
                        .map_err(|e| ExportError::Serialization(format!("Enhanced Arrow conversion failed: {}", e)))
                }
                Err(e) => Err(e)
            }
        });
        
        Ok(arrow_stream)
    }
    
    /// Export with background processing support
    pub async fn export_with_background(
        &self,
        request: ExportRequest,
        auth: &AuthContext,
    ) -> ServiceResult<ExportSummary> {
        // Use background exporter for long-running tasks
        self.background_exporter.export_with_resume(request).await
            .map_err(|e| ServiceError::InternalError(e.to_string()))
    }
    
    /// Get the status of an export job by ID
    pub async fn get_export_status(&self, export_id: Uuid) -> ServiceResult<ExportSummary> {
        let job = self.job_repo.find_by_id(export_id).await.map_err(ServiceError::Domain)?;
        Ok(ExportSummary { job })
    }
    
    /// Get appropriate schema for the filter types with optional document metadata fields
    fn get_schema_for_filters(&self, filters: &[EntityFilter]) -> ServiceResult<Arc<arrow::datatypes::Schema>> {
        use arrow::datatypes::{DataType, Field, Schema};
        
        // For strategic goals, create a dynamic schema that matches actual data
        for filter in filters {
            match filter {
                EntityFilter::StrategicGoals { .. } | EntityFilter::StrategicGoalsByIds { .. } => {
                    return self.create_dynamic_strategic_goals_schema();
                }
                EntityFilter::ProjectsAll | EntityFilter::ProjectsByIds { .. } => {
                    return self.create_dynamic_projects_schema();
                }
                EntityFilter::ParticipantsAll | EntityFilter::ParticipantsByIds { .. } => {
                    return self.create_dynamic_participants_schema();
                }
                EntityFilter::WorkshopsAll { .. } => {
                    return self.create_dynamic_workshops_schema();
                }
                // Add other cases as needed
                _ => continue,
            }
        }
        
        // Default to dynamic strategic goals schema
        self.create_dynamic_strategic_goals_schema()
    }
    

    
    /// Create dynamic schema for strategic goals that matches actual database structure
    fn create_dynamic_strategic_goals_schema(&self) -> ServiceResult<Arc<arrow::datatypes::Schema>> {
        use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
        
        let fields = vec![
            // Core fields from actual database
            Arc::new(Field::new("id", DataType::Utf8, true)),
            Arc::new(Field::new("objective_code", DataType::Utf8, true)),
            Arc::new(Field::new("outcome", DataType::Utf8, true)),
            Arc::new(Field::new("kpi", DataType::Utf8, true)),
            Arc::new(Field::new("target_value", DataType::Float64, true)),
            Arc::new(Field::new("actual_value", DataType::Float64, true)),
            Arc::new(Field::new("status_id", DataType::Int64, true)), // This was the mismatch - should be Int64, not Int32
            Arc::new(Field::new("responsible_team", DataType::Utf8, true)),
            Arc::new(Field::new("sync_priority", DataType::Utf8, true)),
            Arc::new(Field::new("created_at", DataType::Utf8, true)), // Keep as string for simplicity
            Arc::new(Field::new("updated_at", DataType::Utf8, true)),
            Arc::new(Field::new("created_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("updated_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("deleted_at", DataType::Utf8, true)),
            
            // Document metadata fields
            Arc::new(Field::new("document_count", DataType::Int64, true)),
            Arc::new(Field::new("has_documents", DataType::Boolean, true)),
            Arc::new(Field::new("document_filenames", DataType::Utf8, true)),
            Arc::new(Field::new("document_type_ids", DataType::Utf8, true)),
            Arc::new(Field::new("document_paths", DataType::Utf8, true)),
        ];
        
        Ok(Arc::new(Schema::new(fields)))
    }
    
    /// Create dynamic schema for projects
    fn create_dynamic_projects_schema(&self) -> ServiceResult<Arc<arrow::datatypes::Schema>> {
        use arrow::datatypes::{DataType, Field, Schema};
        
        let fields = vec![
            Arc::new(Field::new("id", DataType::Utf8, true)),
            Arc::new(Field::new("strategic_goal_id", DataType::Utf8, true)),
            Arc::new(Field::new("name", DataType::Utf8, true)),
            Arc::new(Field::new("objective", DataType::Utf8, true)),
            Arc::new(Field::new("outcome", DataType::Utf8, true)),
            Arc::new(Field::new("status_id", DataType::Int64, true)),
            Arc::new(Field::new("timeline", DataType::Utf8, true)),
            Arc::new(Field::new("responsible_team", DataType::Utf8, true)),
            Arc::new(Field::new("sync_priority", DataType::Utf8, true)),
            Arc::new(Field::new("created_at", DataType::Utf8, true)),
            Arc::new(Field::new("updated_at", DataType::Utf8, true)),
            Arc::new(Field::new("created_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("updated_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("deleted_at", DataType::Utf8, true)),
        ];
        
        Ok(Arc::new(Schema::new(fields)))
    }
    
    /// Create dynamic schema for workshops  
    fn create_dynamic_workshops_schema(&self) -> ServiceResult<Arc<arrow::datatypes::Schema>> {
        use arrow::datatypes::{DataType, Field, Schema};
        
        let fields = vec![
            Arc::new(Field::new("id", DataType::Utf8, true)),
            Arc::new(Field::new("project_id", DataType::Utf8, true)),
            Arc::new(Field::new("title", DataType::Utf8, true)),
            Arc::new(Field::new("description", DataType::Utf8, true)),
            Arc::new(Field::new("start_date", DataType::Utf8, true)),
            Arc::new(Field::new("end_date", DataType::Utf8, true)),
            Arc::new(Field::new("location", DataType::Utf8, true)),
            Arc::new(Field::new("status", DataType::Utf8, true)),
            Arc::new(Field::new("created_at", DataType::Utf8, true)),
            Arc::new(Field::new("updated_at", DataType::Utf8, true)),
            Arc::new(Field::new("created_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("updated_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("deleted_at", DataType::Utf8, true)),
        ];
        
        Ok(Arc::new(Schema::new(fields)))
    }
    
    /// Create dynamic schema for participants
    fn create_dynamic_participants_schema(&self) -> ServiceResult<Arc<arrow::datatypes::Schema>> {
        use arrow::datatypes::{DataType, Field, Schema};
        
        let fields = vec![
            Arc::new(Field::new("id", DataType::Utf8, true)),
            Arc::new(Field::new("name", DataType::Utf8, true)),
            Arc::new(Field::new("gender", DataType::Utf8, true)),
            Arc::new(Field::new("disability", DataType::Boolean, true)),
            Arc::new(Field::new("disability_type", DataType::Utf8, true)),
            Arc::new(Field::new("age_group", DataType::Utf8, true)),
            Arc::new(Field::new("location", DataType::Utf8, true)),
            Arc::new(Field::new("sync_priority", DataType::Utf8, true)),
            Arc::new(Field::new("created_at", DataType::Utf8, true)),
            Arc::new(Field::new("updated_at", DataType::Utf8, true)),
            Arc::new(Field::new("created_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("created_by_device_id", DataType::Utf8, true)),
            Arc::new(Field::new("updated_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("updated_by_device_id", DataType::Utf8, true)),
            Arc::new(Field::new("deleted_at", DataType::Utf8, true)),
            Arc::new(Field::new("deleted_by_user_id", DataType::Utf8, true)),
            Arc::new(Field::new("deleted_by_device_id", DataType::Utf8, true)),
        ];
        
        Ok(Arc::new(Schema::new(fields)))
    }
    
    /// Get entity count for progress estimation
    async fn get_entity_count(&self, filters: &[EntityFilter]) -> ServiceResult<usize> {
        if let Some(filter) = filters.first() {
            self.streaming_repo.count_entities(filter).await
        } else {
            Ok(0)
        }
    }
    
    /// Generate output path for export
    fn generate_export_path(&self, request: &ExportRequest) -> PathBuf {
        let format_ext = match &request.format {
            Some(ExportFormat::Csv { .. }) => "csv",
            Some(ExportFormat::Parquet { .. }) => "parquet",
            _ => "jsonl",
        };
        
        request.target_path.clone().unwrap_or_else(|| {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
            PathBuf::from(format!("export_{}_{}.{}", Uuid::new_v4(), timestamp, format_ext))
        })
    }
    
    /// Create summary from export metadata
    async fn create_summary_from_metadata(
        &self,
        metadata: ExportMetadata,
        stats: ExportStats,
    ) -> ServiceResult<ExportSummary> {
        let job = ExportJob {
            id: Uuid::new_v4(),
            requested_by_user_id: None,
            requested_at: Utc::now(),
            include_blobs: false,
            status: ExportStatus::Completed,
            local_path: metadata.file_paths.first().map(|p| p.to_string_lossy().to_string()),
            total_entities: Some(stats.entities_written as i64),
            total_bytes: Some(stats.bytes_written as i64),
            error_message: None,
        };
        
        Ok(ExportSummary { job })
    }
    
    /// Determine job priority based on request characteristics
    fn determine_priority(&self, request: &ExportRequest) -> JobPriority {
        match &request.format {
            Some(ExportFormat::Parquet { .. }) => JobPriority::High, // Parquet exports are more expensive
            Some(ExportFormat::Csv { compress: true, .. }) => JobPriority::Normal,
            _ => JobPriority::Low,
        }
    }

    /// Copies documents for projects to a destination directory.
    async fn copy_documents_for_projects(&self, dest_dir: &Path, ids: &[Uuid]) -> ServiceResult<u64> {
        // Early bailout if no IDs
        if ids.is_empty() {
            log::debug!("No project IDs provided, skipping document copy");
            return Ok(0);
        }

        log::debug!("Checking for documents related to {} projects", ids.len());
        
        let media_doc_repo = globals::get_media_document_repo()
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
        
        // Quick count check first to avoid unnecessary work
        let document_count = media_doc_repo
            .count_by_related_entities("projects", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if document_count == 0 {
            log::debug!("No documents found for projects, skipping file operations");
            return Ok(0);
        }

        log::debug!("Found {} documents to copy", document_count);
        
        let all_documents = media_doc_repo
            .find_by_related_entities("projects", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if all_documents.is_empty() {
            log::debug!("Document query returned empty results");
            return Ok(0);
        }

        // Create organized folder structure: files/projects/project_id/
        let files_dir = dest_dir.join("files");
        let projects_dir = files_dir.join("projects");
        tokio::fs::create_dir_all(&projects_dir).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create projects directory: {}", e)))?;
        
        let mut total_bytes = 0;
        let mut copied_count = 0;

        // Group documents by project ID and copy to organized folders
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                // Create project-specific folder
                let project_dir = projects_dir.join(related_id.to_string());
                tokio::fs::create_dir_all(&project_dir).await
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create project directory {}: {}", related_id, e)))?;

                let (source_file_path, _) = crate::domains::export::service::select_best_file_path_with_storage(&document, &self.file_storage);
                let abs_source_path = self.file_storage.get_absolute_path(source_file_path);

                if abs_source_path.exists() {
                    let dest_file_path = project_dir.join(&document.original_filename);
                    match tokio::fs::copy(&abs_source_path, &dest_file_path).await {
                        Ok(bytes_copied) => {
                            total_bytes += bytes_copied;
                            copied_count += 1;
                            log::debug!("Copied document {} to project folder {}", document.original_filename, related_id);
                        }
                        Err(e) => {
                            log::error!("Failed to copy document {} to project folder {}: {}", document.original_filename, related_id, e);
                        }
                    }
                } else {
                    log::warn!("Document file not found: {}", source_file_path);
                }
            }
        }

        log::info!("Copied {} documents ({} bytes) for {} projects", copied_count, total_bytes, ids.len());
        Ok(total_bytes)
    }

    /// Copies documents for strategic goals to a destination directory.
    async fn copy_documents_for_strategic_goals(&self, dest_dir: &Path, ids: &[Uuid]) -> ServiceResult<u64> {
        // Early bailout if no IDs
        if ids.is_empty() {
            log::debug!("No strategic goal IDs provided, skipping document copy");
            return Ok(0);
        }

        log::debug!("Checking for documents related to {} strategic goals", ids.len());
        
        let media_doc_repo = globals::get_media_document_repo()
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
        
        // Quick count check first to avoid unnecessary work
        let document_count = media_doc_repo
            .count_by_related_entities("strategic_goals", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if document_count == 0 {
            log::debug!("No documents found for strategic goals, skipping file operations");
            return Ok(0);
        }

        log::debug!("Found {} documents to copy", document_count);
        
        let all_documents = media_doc_repo
            .find_by_related_entities("strategic_goals", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if all_documents.is_empty() {
            log::debug!("Document query returned empty results");
            return Ok(0);
        }

        // Create organized folder structure: files/strategic_goals/goal_id/
        let files_dir = dest_dir.join("files");
        let strategic_goals_dir = files_dir.join("strategic_goals");
        tokio::fs::create_dir_all(&strategic_goals_dir).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create strategic goals directory: {}", e)))?;
        
        let mut total_bytes = 0;
        let mut copied_count = 0;

        // Group documents by goal ID and copy to organized folders
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                // Create goal-specific folder
                let goal_dir = strategic_goals_dir.join(related_id.to_string());
                tokio::fs::create_dir_all(&goal_dir).await
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create goal directory {}: {}", related_id, e)))?;

                let (source_file_path, _) = crate::domains::export::service::select_best_file_path_with_storage(&document, &self.file_storage);
                let abs_source_path = self.file_storage.get_absolute_path(source_file_path);

                if abs_source_path.exists() {
                    let dest_file_path = goal_dir.join(&document.original_filename);
                    match tokio::fs::copy(&abs_source_path, &dest_file_path).await {
                        Ok(bytes_copied) => {
                            total_bytes += bytes_copied;
                            copied_count += 1;
                            log::debug!("Copied document {} to goal folder {}", document.original_filename, related_id);
                        }
                        Err(e) => {
                            log::warn!("Failed to copy document {} to goal {}: {}", document.original_filename, related_id, e);
                        }
                    }
                } else {
                    log::warn!("Source file does not exist: {}", abs_source_path.display());
                }
            } else {
                log::warn!("Document {} has no related_id, skipping", document.original_filename);
            }
        }
        
        log::debug!("Successfully copied {} documents ({} bytes) to organized goal folders", copied_count, total_bytes);
        Ok(total_bytes)
    }

    /// Copies documents for participants to a destination directory.
    async fn copy_documents_for_participants(&self, dest_dir: &Path, ids: &[Uuid]) -> ServiceResult<u64> {
        // Early bailout if no IDs
        if ids.is_empty() {
            log::debug!("No participant IDs provided, skipping document copy");
            return Ok(0);
        }

        log::debug!("Checking for documents related to {} participants", ids.len());
        
        let media_doc_repo = globals::get_media_document_repo()
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
        
        // Quick count check first to avoid unnecessary work
        let document_count = media_doc_repo
            .count_by_related_entities("participants", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if document_count == 0 {
            log::debug!("No documents found for participants, skipping file operations");
            return Ok(0);
        }

        log::debug!("Found {} documents to copy", document_count);
        
        let all_documents = media_doc_repo
            .find_by_related_entities("participants", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if all_documents.is_empty() {
            log::debug!("Document query returned empty results");
            return Ok(0);
        }

        // Create organized folder structure: files/participants/participant_id/
        let files_dir = dest_dir.join("files");
        let participants_dir = files_dir.join("participants");
        tokio::fs::create_dir_all(&participants_dir).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create participants directory: {}", e)))?;
        
        let mut total_bytes = 0;
        let mut copied_count = 0;

        // Group documents by participant ID and copy to organized folders
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                // Create participant-specific folder
                let participant_dir = participants_dir.join(related_id.to_string());
                tokio::fs::create_dir_all(&participant_dir).await
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create participant directory {}: {}", related_id, e)))?;

                let (source_file_path, _) = crate::domains::export::service::select_best_file_path_with_storage(&document, &self.file_storage);
                let abs_source_path = self.file_storage.get_absolute_path(source_file_path);

                if abs_source_path.exists() {
                    let dest_file_path = participant_dir.join(&document.original_filename);
                    match tokio::fs::copy(&abs_source_path, &dest_file_path).await {
                        Ok(bytes_copied) => {
                            total_bytes += bytes_copied;
                            copied_count += 1;
                            log::debug!("Copied document {} to participant folder {}", document.original_filename, related_id);
                        }
                        Err(e) => {
                            log::error!("Failed to copy document {} to participant folder {}: {}", document.original_filename, related_id, e);
                        }
                    }
                } else {
                    log::warn!("Document file not found: {}", source_file_path);
                }
            }
        }

        log::info!("Copied {} documents ({} bytes) for {} participants", copied_count, total_bytes, ids.len());
        Ok(total_bytes)
    }

    /// Copies documents for activities to a destination directory.
    async fn copy_documents_for_activities(&self, dest_dir: &Path, ids: &[Uuid]) -> ServiceResult<u64> {
        // Early bailout if no IDs
        if ids.is_empty() {
            log::debug!("No activity IDs provided, skipping document copy");
            return Ok(0);
        }

        log::debug!("Checking for documents related to {} activities", ids.len());
        
        let media_doc_repo = globals::get_media_document_repo()
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
        
        // Quick count check first to avoid unnecessary work
        let document_count = media_doc_repo
            .count_by_related_entities("activities", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if document_count == 0 {
            log::debug!("No documents found for activities, skipping file operations");
            return Ok(0);
        }

        log::debug!("Found {} documents to copy", document_count);
        
        let all_documents = media_doc_repo
            .find_by_related_entities("activities", &ids)
            .await
            .map_err(|e| ServiceError::InternalError(e.to_string()))?;
            
        if all_documents.is_empty() {
            log::debug!("Document query returned empty results");
            return Ok(0);
        }

        // Create organized folder structure: files/activities/activity_id/
        let files_dir = dest_dir.join("files");
        let activities_dir = files_dir.join("activities");
        tokio::fs::create_dir_all(&activities_dir).await
            .map_err(|e| ServiceError::InternalError(format!("Failed to create activities directory: {}", e)))?;
        
        let mut total_bytes = 0;
        let mut copied_count = 0;

        // Group documents by activity ID and copy to organized folders
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                // Create activity-specific folder
                let activity_dir = activities_dir.join(related_id.to_string());
                tokio::fs::create_dir_all(&activity_dir).await
                    .map_err(|e| ServiceError::InternalError(format!("Failed to create activity directory {}: {}", related_id, e)))?;

                let (source_file_path, _) = crate::domains::export::service::select_best_file_path_with_storage(&document, &self.file_storage);
                let abs_source_path = self.file_storage.get_absolute_path(source_file_path);

                if abs_source_path.exists() {
                    let dest_file_path = activity_dir.join(&document.original_filename);
                    match tokio::fs::copy(&abs_source_path, &dest_file_path).await {
                        Ok(bytes_copied) => {
                            total_bytes += bytes_copied;
                            copied_count += 1;
                            log::debug!("Copied document {} to activity folder {}", document.original_filename, related_id);
                        }
                        Err(e) => {
                            log::error!("Failed to copy document {} to activity folder {}: {}", document.original_filename, related_id, e);
                        }
                    }
                } else {
                    log::warn!("Document file not found: {}", source_file_path);
                }
            }
        }

        log::info!("Copied {} documents ({} bytes) for {} activities", copied_count, total_bytes, ids.len());
        Ok(total_bytes)
    }
}

/// Export progress information
#[derive(Debug, Clone)]
pub struct ExportProgress {
    pub job_id: Uuid,
    pub completed_bytes: usize,
    pub total_bytes: usize,
    pub entities_processed: u64,
    pub current_domain: String,
    pub estimated_time_remaining: f64,
    pub status: ExportStatus,
}

/// Implementation of JobProcessor for ExportServiceV2
#[async_trait]
impl JobProcessor for ExportServiceV2 {
    async fn process(&self, job: QueueJob) -> Result<ExportSummary, ExportError> {
        let job_id = job.id;
        
        // Process export with atomic transaction
        let result = self.export_with_transaction(job).await;
        
        match &result {
            Ok(summary) => {
                // Log successful completion
                println!("Export job {} completed successfully", job_id);
            }
            Err(e) => {
                // Log failure - status already updated in transaction
                eprintln!("Export job {} failed: {}", job_id, e);
            }
        }
        
        result
    }
}

/// Atomic export processing with transaction integrity
impl ExportServiceV2 {
    async fn export_with_transaction(&self, job: QueueJob) -> Result<ExportSummary, ExportError> {
        let job_id = job.id;
        
        // Create progress channel
        let (progress_tx, _progress_rx) = mpsc::channel(100);
        
        // Create system auth context for background jobs
        let auth = AuthContext::internal_system_context();
        
        // Begin database transaction for atomic operations
        let transactional_repo = self.job_repo.as_transactional()
            .ok_or_else(|| ExportError::Database("Repository doesn't support transactions".to_string()))?;
        let mut tx = transactional_repo.begin_transaction().await
            .map_err(|e| ExportError::Database(e.to_string()))?;
        
        // Update job status to running within transaction
        transactional_repo.update_status_tx(
            &mut tx,
            job_id,
            ExportStatus::Running,
            None, None, None, None
        ).await.map_err(|e| ExportError::Database(e.to_string()))?;
        
        // Process the export
        let export_result = self.export_with_progress(job.request, &auth, progress_tx).await;
        
        // Update final status atomically within same transaction
        match export_result {
            Ok(summary) => {
                // Update job as completed within transaction
                let file_path = summary.job.local_path.clone();
                let entities = summary.job.total_entities.unwrap_or(0);
                let bytes = summary.job.total_bytes.unwrap_or(0);
                
                transactional_repo.update_status_tx(
                    &mut tx,
                    job_id,
                    ExportStatus::Completed,
                    None,
                    file_path,
                    Some(entities),
                    Some(bytes)
                ).await.map_err(|e| ExportError::Database(e.to_string()))?;
                
                // Commit transaction
                tx.commit().await.map_err(|e| ExportError::Database(e.to_string()))?;
                
                Ok(summary)
            }
            Err(e) => {
                // Update job as failed within transaction
                transactional_repo.update_status_tx(
                    &mut tx,
                    job_id,
                    ExportStatus::Failed,
                    Some(e.to_string()),
                    None, None, None
                ).await.map_err(|db_err| ExportError::Database(db_err.to_string()))?;
                
                // Commit failure status
                tx.commit().await.map_err(|commit_err| ExportError::Database(commit_err.to_string()))?;
                
                Err(ExportError::JobFailed(e.to_string()))
            }
        }
    }
}

/// Helper methods for job status updates  
impl ExportServiceV2 {
    async fn update_job_success(
        &self, 
        job_id: Uuid, 
        summary: &ExportSummary
    ) -> Result<(), ExportError> {
        let file_path = summary.job.local_path.clone();
        let entities = summary.job.total_entities.unwrap_or(0);
        let bytes = summary.job.total_bytes.unwrap_or(0);
        
        self.job_repo.update_status(
            job_id,
            ExportStatus::Completed,
            None,
            file_path,
            Some(entities),
            Some(bytes)
        ).await.map_err(|e| ExportError::Database(e.to_string()))
    }

    async fn update_job_failure(
        &self, 
        job_id: Uuid, 
        error: &ExportError
    ) -> Result<(), ExportError> {
        self.job_repo.update_status(
            job_id,
            ExportStatus::Failed,
            Some(error.to_string()),
            None, None, None
        ).await.map_err(|e| ExportError::Database(e.to_string()))
    }
}