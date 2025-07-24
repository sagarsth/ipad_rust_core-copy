use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::task;
use tempfile::TempDir;
use zip::{ZipWriter, write::FileOptions};
use std::io::{Write, BufWriter};
use serde_json::Value;
use futures::future::join_all;
use chrono::TimeZone;
use std::sync::atomic::{AtomicI64, Ordering};

use crate::auth::AuthContext;
use crate::errors::{ServiceError, ServiceResult};
use crate::domains::core::file_storage_service::FileStorageService;
use crate::domains::export::types::{EntityFilter, ExportFormat, ExportError, ExportStats, ExportMetadata};
use crate::domains::export::writer::{UnifiedExportWriter, WriterFactory, DeviceCapabilities};

use super::repository::ExportJobRepository;
use super::types::{ExportRequest, ExportSummary, ExportJob, ExportStatus};
use crate::globals;
use crate::types::PaginationParams;
use std::path::{PathBuf, Path};
use crate::domains::document::repository::MediaDocumentRepository;

// Performance-optimized JSON Lines writer
pub struct OptimizedJsonLWriter {
    writer: BufWriter<std::fs::File>,
    buffer: String,
    entities_written: usize,
}

impl OptimizedJsonLWriter {
    pub fn new(file_path: &Path) -> Result<Self, String> {
        let file = std::fs::File::create(file_path).map_err(|e| e.to_string())?;
        let writer = BufWriter::with_capacity(4 * 1024 * 1024, file); // 4MB buffer
        Ok(Self {
            writer,
            buffer: String::with_capacity(256 * 1024), // 256KB initial capacity
            entities_written: 0,
        })
    }

    fn write_entity<T: serde::Serialize>(&mut self, entity: &T) -> Result<(), String> {
        // Serialize entity to JSON
        let json_line = serde_json::to_string(entity).map_err(|e| e.to_string())?;
        
        // Check if adding this line would exceed buffer capacity
        if self.buffer.len() + json_line.len() + 1 > 512_000 && !self.buffer.is_empty() {
            // Flush current buffer first
            self.writer.write_all(self.buffer.as_bytes()).map_err(|e| e.to_string())?;
            self.buffer.clear();
        }
        
        // Add to buffer
        self.buffer.push_str(&json_line);
        self.buffer.push('\n');
        
        self.entities_written += 1;
        Ok(())
    }

    fn write_enhanced_entity<T: serde::Serialize>(&mut self, entity: &T, metadata: serde_json::Value) -> Result<(), String> {
        let mut entity_json = serde_json::to_value(entity).map_err(|e| e.to_string())?;
        if let (serde_json::Value::Object(ref mut entity_map), serde_json::Value::Object(meta_map)) = (&mut entity_json, metadata) {
            entity_map.extend(meta_map);
        }
        
        let json_line = serde_json::to_string(&entity_json).map_err(|e| e.to_string())?;
        
        // Check if adding this line would exceed buffer capacity
        if self.buffer.len() + json_line.len() + 1 > 512_000 && !self.buffer.is_empty() {
            // Flush current buffer first
            self.writer.write_all(self.buffer.as_bytes()).map_err(|e| e.to_string())?;
            self.buffer.clear();
        }
        
        // Add to buffer
        self.buffer.push_str(&json_line);
        self.buffer.push('\n');
        
        self.entities_written += 1;
        Ok(())
    }

    fn finalize(mut self) -> Result<usize, String> {
        // Flush remaining buffer
        if !self.buffer.is_empty() {
            self.writer.write_all(self.buffer.as_bytes()).map_err(|e| e.to_string())?;
        }
        self.writer.flush().map_err(|e| e.to_string())?;
        Ok(self.entities_written)
    }
}

// Implement UnifiedExportWriter for OptimizedJsonLWriter
#[async_trait]
impl UnifiedExportWriter for OptimizedJsonLWriter {
    async fn write_json_entity(&mut self, entity: &serde_json::Value) -> Result<(), ExportError> {
        self.write_entity(entity).map_err(|e| ExportError::Serialization(e))
    }
    
    async fn finalize(self: Box<Self>) -> Result<ExportMetadata, ExportError> {
        let entities_written = (*self).finalize().map_err(|e| ExportError::Io(e))?;
        
        let stats = ExportStats {
            entities_written,
            bytes_written: 0, // We don't track this in the current implementation
            duration_ms: 0,   // We don't track this in the current implementation
            memory_peak_mb: 0, // We don't track this in the current implementation
            compression_ratio: None,
        };
        
        Ok(ExportMetadata {
            format: ExportFormat::JsonLines,
            stats,
            file_paths: vec![], // We don't track file paths in the current implementation
            schema_version: 1,
            checksum: None,
        })
    }
    
    async fn flush(&mut self) -> Result<(), ExportError> {
        self.writer.flush().map_err(|e| ExportError::Io(e.to_string()))
    }
    
    fn format(&self) -> ExportFormat {
        ExportFormat::JsonLines
    }
}

// Legacy format abstraction - now using the one from types.rs

// Modernized approach using the new writer factory
fn create_export_writer(format: ExportFormat, file_path: &Path) -> Result<Box<dyn UnifiedExportWriter>, ExportError> {
    match format {
        ExportFormat::JsonLines => {
            let writer = OptimizedJsonLWriter::new(file_path)
                .map_err(|e| ExportError::Io(e.to_string()))?;
            Ok(Box::new(writer))
        }
        _ => Err(ExportError::InvalidConfig("Only JsonLines format supported in legacy writer".to_string()))
    }
}

// Parallel processing helper for large exports
async fn export_entities_parallel<T, F, Fut>(
    entities: &[T],
    chunk_size: usize,
    processor: F,
) -> Result<Vec<String>, String>
where
    T: Clone + Send + Sync + 'static,
    F: Fn(Vec<T>) -> Fut + Send + Sync + Clone + 'static,
    Fut: std::future::Future<Output = Result<String, String>> + Send + 'static,
{
    let chunks: Vec<Vec<T>> = entities.chunks(chunk_size).map(|chunk| chunk.to_vec()).collect();
    let tasks: Vec<_> = chunks.into_iter().map(|chunk| {
        let proc = processor.clone();
        tokio::spawn(async move { proc(chunk).await })
    }).collect();
    
    let mut results = Vec::new();
    for task in tasks {
        let result = task.await.map_err(|e| format!("Task join error: {}", e))?;
        results.push(result?);
    }
    
    Ok(results)
}

#[async_trait]
pub trait ExportService: Send + Sync {
    /// Begin a new export job. Returns immediately with a summary containing the job ID.
    async fn create_export(&self, request: ExportRequest, auth: &AuthContext) -> ServiceResult<ExportSummary>;

    /// Query an existing export job.
    async fn get_export_status(&self, export_id: Uuid) -> ServiceResult<ExportSummary>;
}

pub struct ExportServiceImpl {
    job_repo: Arc<dyn ExportJobRepository>,
    file_storage: Arc<dyn FileStorageService>,
}

impl ExportServiceImpl {
    pub fn new(
        job_repo: Arc<dyn ExportJobRepository>,
        file_storage: Arc<dyn FileStorageService>,
    ) -> Self {
        Self {
            job_repo,
            file_storage,
        }
    }
}

#[async_trait]
impl ExportService for ExportServiceImpl {
    async fn create_export(&self, request: ExportRequest, auth: &AuthContext) -> ServiceResult<ExportSummary> {
        // Permission check – only roles with ExportData permission may initiate exports
        auth.authorize(crate::types::Permission::ExportData)?;

        // 1. Build initial job row in "running" state
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

        // 2. Persist it
        self.job_repo.create_job(&job).await.map_err(ServiceError::Domain)?;

        // 3. Kick off background task that will do the heavy-lifting
        let job_repo_clone = self.job_repo.clone();
        let file_storage_clone = self.file_storage.clone();
        let filters_clone = request.filters.clone();
        let include_blobs = request.include_blobs;
        let target_path = request.target_path.clone();

        task::spawn(async move {
            perform_export_job(
                job_repo_clone,
                file_storage_clone,
                job.id,
                filters_clone,
                include_blobs,
                target_path,
            )
            .await;
        });

        let job_for_summary = job.clone();

        Ok(ExportSummary { job: job_for_summary })
    }

    async fn get_export_status(&self, export_id: Uuid) -> ServiceResult<ExportSummary> {
        let job = self.job_repo.find_by_id(export_id).await.map_err(ServiceError::Domain)?;
        Ok(ExportSummary { job })
    }
}

// Helper that executes the export work and updates the job status.
async fn perform_export_job(
    job_repo: Arc<dyn ExportJobRepository>,
    file_storage: Arc<dyn FileStorageService>,
    job_id: Uuid,
    filters: Vec<EntityFilter>,
    include_blobs: bool,
    target_path: Option<PathBuf>,
) {
    let total_entities = Arc::new(AtomicI64::new(0));
    let total_bytes = Arc::new(AtomicI64::new(0));

    // Wrap entire operation in its own error scope
    let result: Result<String, String> = async {
        // 1. Create temp directory
        let temp_dir = TempDir::new().map_err(|e| format!("failed to create temp dir: {e}"))?;

        // 2. Process each filter concurrently where safe
        let mut tasks = Vec::new();

        for filter in filters {
            let file_storage_clone2 = file_storage.clone();
            let temp_dir_path = temp_dir.path().to_path_buf();
            let task_fut = async move {
                match filter {
                    EntityFilter::StrategicGoals { status_id } => {
                        export_strategic_goals(&temp_dir_path, status_id).await?;
                        // FIXME: we don't yet get per-entity count accurately; leave 0 for now.
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::StrategicGoalsByIds { ids } => {
                        let count = export_strategic_goals_by_ids(&temp_dir_path, &ids, include_blobs, &file_storage_clone2).await?;
                        Ok::<i64, String>(count)
                    }
                    EntityFilter::ProjectsAll => {
                        export_projects(&temp_dir_path).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::ProjectsByIds { ids } => {
                        let count = export_projects_by_ids_with_options(&temp_dir_path, &ids, include_blobs, &file_storage_clone2).await?;
                        Ok::<i64, String>(count)
                    }
                    EntityFilter::ActivitiesAll => {
                        export_activities(&temp_dir_path).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::ActivitiesByIds { ids } => {
                        let count = export_activities_by_ids_with_documents(&temp_dir_path, &ids, include_blobs, &file_storage_clone2).await?;
                        Ok::<i64, String>(count)
                    }
                    EntityFilter::DonorsAll => {
                        export_donors(&temp_dir_path).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::DonorsByIds { ids } => {
                        export_donors_by_ids(&temp_dir_path, &ids).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::FundingAll => {
                        export_fundings(&temp_dir_path).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::FundingByIds { ids } => {
                        export_fundings_by_ids(&temp_dir_path, &ids).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::LivelihoodsAll => {
                        export_livelihoods(&temp_dir_path).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::LivelihoodsByIds { ids } => {
                        export_livelihoods_by_ids(&temp_dir_path, &ids).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::ParticipantsAll => {
                        export_participants(&temp_dir_path).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::ParticipantsByIds { ids } => {
                        let count = export_participants_by_ids_with_documents(&temp_dir_path, &ids, include_blobs, &file_storage_clone2).await?;
                        Ok::<i64, String>(count)
                    }
                    EntityFilter::WorkshopsAll { include_participants } => {
                        export_workshops(&temp_dir_path, include_participants).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::WorkshopsByIds { ids, include_participants } => {
                        export_workshops_by_ids(&temp_dir_path, &ids, include_participants).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::WorkshopParticipantsAll => {
                        export_workshop_participants(&temp_dir_path).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::WorkshopParticipantsByIds { ids } => {
                        export_workshop_participants_by_ids(&temp_dir_path, &ids).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::MediaDocumentsByRelatedEntity { related_table, related_id } => {
                        export_media_documents(
                            &temp_dir_path,
                            &related_table,
                            related_id,
                            include_blobs,
                            &file_storage_clone2,
                        )
                        .await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::MediaDocumentsByIds { ids } => {
                        export_media_documents_by_ids(
                            &temp_dir_path,
                            &ids,
                            include_blobs,
                            &file_storage_clone2,
                        )
                        .await?;
                        Ok::<i64, String>(0)
                    }

                    // --- Date-range variants ---
                    EntityFilter::StrategicGoalsByDateRange { start_date, end_date, status_id } => {
                        export_strategic_goals_by_date_range(&temp_dir_path, start_date, end_date, status_id).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::StrategicGoalsByFilter { filter } => {
                        export_strategic_goals_by_filter(&temp_dir_path, &filter).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::ProjectsByDateRange { start_date, end_date } => {
                        export_projects_by_date_range(&temp_dir_path, start_date, end_date).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::ActivitiesByDateRange { start_date, end_date } => {
                        export_activities_by_date_range(&temp_dir_path, start_date, end_date).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::DonorsByDateRange { start_date, end_date } => {
                        export_donors_by_date_range(&temp_dir_path, start_date, end_date).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::FundingByDateRange { start_date, end_date } => {
                        export_fundings_by_date_range(&temp_dir_path, start_date, end_date).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::LivelihoodsByDateRange { start_date, end_date } => {
                        export_livelihoods_by_date_range(&temp_dir_path, start_date, end_date).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::ParticipantsByDateRange { start_date, end_date } => {
                        export_participants_by_date_range(&temp_dir_path, start_date, end_date).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::WorkshopsByDateRange { start_date, end_date, include_participants } => {
                        export_workshops_by_date_range(&temp_dir_path, start_date, end_date, include_participants).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::MediaDocumentsByDateRange { start_date, end_date } => {
                        export_media_documents_by_date_range(
                            &temp_dir_path,
                            start_date,
                            end_date,
                            include_blobs,
                            &file_storage_clone2,
                        ).await?;
                        Ok::<i64, String>(0)
                    }

                    // Unified variants
                    EntityFilter::UnifiedAllDomains { include_type_tags } => {
                        export_unified(&temp_dir_path, None, include_type_tags, include_blobs, &file_storage_clone2).await?;
                        Ok::<i64, String>(0)
                    }
                    EntityFilter::UnifiedByDateRange { start_date, end_date, include_type_tags } => {
                        export_unified(&temp_dir_path, Some((start_date, end_date)), include_type_tags, include_blobs, &file_storage_clone2).await?;
                        Ok::<i64, String>(0)
                    }
                }
            };
            tasks.push(task_fut);
        }

        // Wait for all tasks to finish and collect entity counts
        let results = join_all(tasks).await;
        for res in results {
            match res {
                Ok(count) => {
                    total_entities.fetch_add(count, Ordering::Relaxed);
                },
                Err(e) => return Err(e),
            }
        }

        // 3. Zip the directory
        let zip_name = format!("{}.zip", job_id);
        let dest_path: PathBuf = if let Some(custom) = target_path {
            if custom.is_dir() {
                custom.join(&zip_name)
            } else {
                custom
            }
        } else {
            PathBuf::from(&zip_name)
        };

        create_zip_from_dir(&temp_dir.path(), &dest_path)
            .map_err(|e| format!("failed to create zip: {e}"))?;

        let metadata = std::fs::metadata(&dest_path)
            .map_err(|e| format!("failed to stat zip: {e}"))?;
        total_bytes.store(metadata.len() as i64, Ordering::Relaxed);

        Ok(dest_path.to_string_lossy().to_string())
    }
    .await;

    match result {
        Ok(path_str) => {
            let _ = job_repo
                .update_status(
                    job_id,
                    ExportStatus::Completed,
                    None,
                    Some(path_str),
                    Some(total_entities.load(Ordering::Relaxed)),
                    Some(total_bytes.load(Ordering::Relaxed)),
                )
                .await;
        }
        Err(err_msg) => {
            let _ = job_repo
                .update_status(
                    job_id,
                    ExportStatus::Failed,
                    Some(err_msg),
                    None,
                    Some(total_entities.load(Ordering::Relaxed)),
                    Some(total_bytes.load(Ordering::Relaxed)),
                )
                .await;
        }
    }
}

// Export helper implementations ----------------------------------

async fn export_strategic_goals(dest_dir: &Path, status_id: Option<i64>) -> Result<(), String> {
    let repo = globals::get_strategic_goal_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("strategic_goals.jsonl");
    let mut writer = OptimizedJsonLWriter::new(&file_path)?;

    let mut page: u32 = 1;
    let per_page = 500; // Increased page size for better performance
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = if let Some(status) = status_id {
            repo.find_by_status_id(status, params).await.map_err(|e| e.to_string())?
        } else {
            repo.find_all(params).await.map_err(|e| e.to_string())?
        };

        for entity in page_result.items {
            writer.write_entity(&entity)?;
        }

        if page >= page_result.total_pages {
            break;
        }
        page += 1;
    }
    
    writer.finalize()?;
    Ok(())
}

async fn export_projects(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_project_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("projects.jsonl");
    let mut writer = OptimizedJsonLWriter::new(&file_path)?;

    let mut page: u32 = 1;
    let per_page = 500;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            writer.write_entity(&entity)?;
        }
        if page >= page_result.total_pages {
            break;
        }
        page += 1;
    }
    
    writer.finalize()?;
    Ok(())
}

async fn export_activities(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_activity_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("activities.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_donors(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_donor_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("donors.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_fundings(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_project_funding_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("fundings.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_livelihoods(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_livelihood_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("livelihoods.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params, None, None).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_workshops(dest_dir: &Path, include_participants: bool) -> Result<(), String> {
    let workshop_repo = globals::get_workshop_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("workshops.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    
    let mut all_workshops = Vec::new();
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = workshop_repo.find_all(params, None).await.map_err(|e| e.to_string())?;
        for entity in &page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        all_workshops.extend(page_result.items);
        if page >= page_result.total_pages { break; }
        page += 1;
    }

    // Export associated workshop participants only if requested
    if include_participants {
        let workshop_participant_repo = globals::get_workshop_participant_repo().map_err(|e| e.to_string())?;
        let participants_file_path = dest_dir.join("workshop_participants.jsonl");
        let mut participants_file = std::fs::File::create(&participants_file_path).map_err(|e| e.to_string())?;

        for workshop in all_workshops {
            let participants = workshop_participant_repo.find_participants_for_workshop(workshop.id).await.map_err(|e| e.to_string())?;
            for participant in participants {
                let json = serde_json::to_string(&participant).map_err(|e| e.to_string())?;
                writeln!(participants_file, "{}", json).map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

async fn export_media_documents(
    dest_dir: &Path,
    related_table: &str,
    related_id: Uuid,
    include_blobs: bool,
    file_storage: &Arc<dyn FileStorageService>,
) -> Result<(), String> {
    let repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;

    let file_path = dest_dir.join("media_documents.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let mut page: u32 = 1;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let result = repo
            .find_by_related_entity(related_table, related_id, params)
            .await
            .map_err(|e| e.to_string())?;

        for doc in &result.items {
            let json = serde_json::to_string(&doc).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;

            if include_blobs {
                // Use helper function to select the best file path
                let (source_file_path, file_source_type) = select_best_file_path_with_storage(doc, file_storage);
                
                let abs_path = file_storage.get_absolute_path(source_file_path);
                if abs_path.exists() {
                    let blobs_dir = dest_dir.join("blobs");
                    std::fs::create_dir_all(&blobs_dir).map_err(|e| e.to_string())?;
                    if let Some(_filename) = abs_path.file_name() {
                        // Use original filename for consistency
                        let export_filename = std::ffi::OsString::from(&doc.original_filename);
                        match std::fs::copy(&abs_path, blobs_dir.join(export_filename)) {
                            Ok(_) => {
                                log::info!("Successfully copied {} file: {} -> {}", file_source_type, source_file_path, &doc.original_filename);
                            }
                            Err(e) => {
                                log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                            }
                        }
                    }
                } else {
                    log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
                }
            }
        }

        if page >= result.total_pages {
            break;
        }
        page += 1;
    }

    Ok(())
}

pub fn create_zip_from_dir(src_dir: &Path, dest_zip: &Path) -> Result<(), String> {
    let file = std::fs::File::create(dest_zip).map_err(|e| e.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    fn add_dir_recursively(
        zip: &mut ZipWriter<std::fs::File>,
        base_dir: &Path,
        path: &Path,
        options: FileOptions,
    ) -> Result<(), String> {
        for entry in std::fs::read_dir(path).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let name = path.strip_prefix(base_dir).map_err(|e| e.to_string())?;
            if path.is_file() {
                zip.start_file(name.to_string_lossy(), options)
                    .map_err(|e| e.to_string())?;
                let mut f = std::fs::File::open(&path).map_err(|e| e.to_string())?;
                std::io::copy(&mut f, zip).map_err(|e| e.to_string())?;
            } else if path.is_dir() {
                zip.add_directory(name.to_string_lossy(), options)
                    .map_err(|e| e.to_string())?;
                add_dir_recursively(zip, base_dir, &path, options)?;
            }
        }
        Ok(())
    }

    add_dir_recursively(&mut zip, src_dir, src_dir, options)?;
    zip.finish().map_err(|e| e.to_string())?;
    Ok(())
}

// Helper utility to write a JSONL line, optionally wrapping with type tag
fn write_jsonl_line<T: serde::Serialize>(
    file: &mut std::fs::File,
    entity: &T,
    type_name: &str,
    include_type_tag: bool,
) -> Result<(), String> {
    if include_type_tag {
        let wrapped = serde_json::json!({
            "type": type_name,
            "data": entity
        });
        writeln!(file, "{}", wrapped.to_string()).map_err(|e| e.to_string())
    } else {
        let json = serde_json::to_string(entity).map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())
    }
}

// Helper function to determine the best file path for export
pub fn select_best_file_path_with_storage<'a>(
    document: &'a crate::domains::document::types::MediaDocument,
    file_storage: &Arc<dyn FileStorageService>
) -> (&'a str, &'static str) {
    // First priority: compressed file if compression is completed and file exists
    if document.compression_status == "completed" {
        if let Some(ref compressed_path) = document.compressed_file_path {
            let abs_compressed = file_storage.get_absolute_path(compressed_path);
            if abs_compressed.exists() {
                log::info!("Using compressed file for export: {}", compressed_path);
                return (compressed_path.as_str(), "compressed");
            } else {
                log::warn!("Compressed file marked as completed but not found at: {}", abs_compressed.display());
                // Fall back to original if it exists
                let abs_original = file_storage.get_absolute_path(&document.file_path);
                if abs_original.exists() {
                    log::info!("Falling back to original file: {}", &document.file_path);
                    return (document.file_path.as_str(), "original_fallback");
                } else {
                    log::error!("Neither compressed ({}) nor original ({}) file exists for document {}", 
                        abs_compressed.display(), abs_original.display(), document.id);
                    return (document.file_path.as_str(), "missing");
                }
            }
        } else {
            log::warn!("Compression marked as completed but no compressed_file_path for document {}", document.id);
            return (document.file_path.as_str(), "original");
        }
    } else {
        // Compression not completed or in progress, use original
        log::info!("Using original file (compression status: {}): {}", document.compression_status, &document.file_path);
        return (document.file_path.as_str(), "original");
    }
}

// Legacy helper function for backward compatibility - now just delegates
fn select_best_file_path(document: &crate::domains::document::types::MediaDocument) -> (&str, &str) {
    // For functions that don't have access to file storage, default to original
    log::info!("Using original file (legacy path selection): {}", &document.file_path);
    return (document.file_path.as_str(), "original");
}

// NEW unified export helper
async fn export_unified(
    dest_dir: &Path,
    date_range: Option<(DateTime<Utc>, DateTime<Utc>)>,
    include_type_tags: bool,
    include_blobs: bool,
    file_storage: &Arc<dyn FileStorageService>,
) -> Result<(), String> {
    let file_path = dest_dir.join("unified_export.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let per_page = 200;

    // Strategic Goals
    {
        let repo = globals::get_strategic_goal_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "strategic_goal", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Projects
    {
        let repo = globals::get_project_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "project", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Activities
    {
        let repo = globals::get_activity_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "activity", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Donors
    {
        let repo = globals::get_donor_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "donor", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Project Funding
    {
        let repo = globals::get_project_funding_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "funding", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Livelihoods
    {
        let repo = globals::get_livelihood_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params, None, None).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "livelihood", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Participants
    {
        let repo = globals::get_participant_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "participant", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Workshops
    {
        let repo = globals::get_workshop_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = if let Some((start, end)) = date_range {
                repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?
            } else {
                repo.find_all(params, None).await.map_err(|e| e.to_string())?
            };
            for entity in &page_result.items {
                write_jsonl_line(&mut file, entity, "workshop", include_type_tags)?;
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    // Media Documents (optional, to copy blobs too)
    {
        let repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;
        let mut page = 1u32;
        loop {
            let params = PaginationParams { page, per_page };
            let page_result = repo.find_by_related_entity("%", Uuid::nil(), params) // placeholder wildcard; adjust as needed
                .await.map_err(|e| e.to_string())?;
            for doc in &page_result.items {
                write_jsonl_line(&mut file, doc, "media_document", include_type_tags)?;
                if include_blobs {
                    // Use helper function to select the best file path
                    let (source_file_path, file_source_type) = select_best_file_path_with_storage(doc, file_storage);
                    
                    let abs = file_storage.get_absolute_path(source_file_path);
                    if abs.exists() {
                        let blobs_dir = dest_dir.join("blobs");
                        std::fs::create_dir_all(&blobs_dir).map_err(|e| e.to_string())?;
                        let export_filename = std::ffi::OsString::from(&doc.original_filename);
                        match std::fs::copy(&abs, blobs_dir.join(export_filename)) {
                            Ok(_) => {
                                log::info!("Successfully copied {} file: {} -> {}", file_source_type, source_file_path, &doc.original_filename);
                            }
                            Err(e) => {
                                log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                            }
                        }
                    } else {
                        log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
                    }
                }
            }
            if page >= page_result.total_pages { break; }
            page += 1;
        }
    }

    Ok(())
}

// NEW: domain export helpers with date ranges
async fn export_projects_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), String> {
    let repo = globals::get_project_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("projects.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_activities_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), String> {
    let repo = globals::get_activity_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("activities.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_donors_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), String> {
    let repo = globals::get_donor_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("donors.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_fundings_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), String> {
    let repo = globals::get_project_funding_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("fundings.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_livelihoods_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), String> {
    let repo = globals::get_livelihood_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("livelihoods.jsonl");
    let mut writer = OptimizedJsonLWriter::new(&file_path)?;

    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            writer.write_entity(&entity)?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    
    writer.finalize()?;
    Ok(())
}

async fn export_workshops_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>, include_participants: bool) -> Result<(), String> {
    let workshop_repo = globals::get_workshop_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("workshops.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    
    let mut all_workshops = Vec::new();
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = workshop_repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        for entity in &page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        all_workshops.extend(page_result.items);
        if page >= page_result.total_pages { break; }
        page += 1;
    }

    // Export associated workshop participants only if requested
    if include_participants {
        let workshop_participant_repo = globals::get_workshop_participant_repo().map_err(|e| e.to_string())?;
        let participants_file_path = dest_dir.join("workshop_participants.jsonl");
        let mut participants_file = std::fs::File::create(&participants_file_path).map_err(|e| e.to_string())?;

        for workshop in all_workshops {
            let participants = workshop_participant_repo.find_participants_for_workshop(workshop.id).await.map_err(|e| e.to_string())?;
            for participant in participants {
                let json = serde_json::to_string(&participant).map_err(|e| e.to_string())?;
                writeln!(participants_file, "{}", json).map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(())
}

// Strategic goals by date range (with optional status filter)
async fn export_strategic_goals_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>, status_id: Option<i64>) -> Result<(), String> {
    let repo = globals::get_strategic_goal_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("strategic_goals.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let mut page_result = repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        if let Some(status) = status_id {
            page_result.items.retain(|g| g.status_id == Some(status));
        }
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    Ok(())
}

async fn export_media_documents_by_date_range(
    dest_dir: &Path,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    include_blobs: bool,
    file_storage: &Arc<dyn FileStorageService>,
) -> Result<(), String> {
    let repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("media_documents.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let mut page: u32 = 1;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo
            .find_by_date_range(start, end, params)
            .await
            .map_err(|e| e.to_string())?;

        for doc in &page_result.items {
            let json = serde_json::to_string(&doc).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;

            if include_blobs {
                
                // Use helper function to select the best file path
                let (source_file_path, file_source_type) = select_best_file_path_with_storage(doc, file_storage);
                
                let abs_path = file_storage.get_absolute_path(source_file_path);
                if abs_path.exists() {
                    let blobs_dir = dest_dir.join("blobs");
                    std::fs::create_dir_all(&blobs_dir).map_err(|e| e.to_string())?;
                    if let Some(_filename) = abs_path.file_name() {
                        // Use original filename for consistency
                        let export_filename = std::ffi::OsString::from(&doc.original_filename);
                        match std::fs::copy(&abs_path, blobs_dir.join(export_filename)) {
                            Ok(_) => {
                                log::info!("Successfully copied {} file: {} -> {}", file_source_type, source_file_path, &doc.original_filename);
                            }
                            Err(e) => {
                                log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                            }
                        }
                    }
                } else {
                    log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
                }
            }
        }

        if page >= page_result.total_pages {
            break;
        }
        page += 1;
    }
    Ok(())
}

// === ID-based Export Functions ===

pub async fn export_strategic_goals_by_ids(dest_dir: &Path, ids: &[Uuid], include_documents: bool, file_storage: &Arc<dyn FileStorageService>) -> Result<i64, String> {
    if ids.is_empty() {
        log::info!("Export strategic goals by IDs: empty IDs list");
        return Ok(0);
    }

    log::info!("Export strategic goals by IDs: {} IDs, include_documents: {}", ids.len(), include_documents);

    let strategic_goal_repo = globals::get_strategic_goal_repo().map_err(|e| e.to_string())?;
    
    // Get strategic goals
    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    log::info!("Calling repo.find_by_ids with params: page={}, per_page={}", params.page, params.per_page);
    let result = strategic_goal_repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    log::info!("Repository returned {} items, total={}", result.items.len(), result.total);

    // Export strategic goals using optimized writer
    let strategic_goals_file_path = dest_dir.join("strategic_goals.jsonl");
    let mut strategic_goals_writer = OptimizedJsonLWriter::new(&strategic_goals_file_path)?;

    let total_documents_exported = 0;
    let mut total_files_copied = 0;

    if include_documents {
        let media_doc_repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;
        
        // Get all goal IDs
        let goal_ids: Vec<Uuid> = result.items.iter().map(|g| g.id).collect();
        log::info!("Fetching all documents for {} strategic goals efficiently", goal_ids.len());
        
        // Fetch ALL documents for ALL goals in a single query - this solves the N+1 problem
        let all_documents = media_doc_repo
            .find_by_related_entities("strategic_goals", &goal_ids)
            .await
            .map_err(|e| e.to_string())?;
        
        // Group documents by related_id (goal_id) for efficient lookup
        let mut documents_by_goal: std::collections::HashMap<Uuid, Vec<crate::domains::document::types::MediaDocument>> = std::collections::HashMap::new();
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                documents_by_goal.entry(related_id).or_insert_with(Vec::new).push(document);
            }
        }
        
        log::info!("Found {} total documents across all goals", documents_by_goal.values().map(|v| v.len()).sum::<usize>());

        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing entity {} to file: id={}", i + 1, entity.id);
            
            // Get document count from our pre-fetched data
            let document_count = documents_by_goal.get(&entity.id).map(|docs| docs.len()).unwrap_or(0);
            let metadata = serde_json::json!({
                "document_count": document_count,
                "has_documents": document_count > 0
            });
            
            strategic_goals_writer.write_enhanced_entity(entity, metadata)?;
        }

        // Export associated media documents and files
        log::info!("Exporting associated media documents");
        let mut documents_writer_opt: Option<OptimizedJsonLWriter> = None;
        let mut files_dir_created = false;
        let files_dir = dest_dir.join("files");
        
        // Check if we actually have documents before processing
        let total_docs_count: usize = documents_by_goal.values().map(|docs| docs.len()).sum();
        log::info!("Total documents to export: {}", total_docs_count);
        
        if total_docs_count > 0 {
            // Process all documents we already fetched (no more database calls!)
            for documents in documents_by_goal.values() {
                for document in documents {
                    if !files_dir_created {
                        std::fs::create_dir_all(&files_dir).map_err(|e| e.to_string())?;
                        files_dir_created = true;
                    }

                    let (source_file_path, file_source_type) = select_best_file_path_with_storage(&document, file_storage);
                    let unique_filename = format!("{}_{}", document.id, document.original_filename);
                    let dest_file_path = files_dir.join(&unique_filename);
                    let abs_source_path = file_storage.get_absolute_path(source_file_path);

                    if abs_source_path.exists() {
                        match std::fs::copy(&abs_source_path, &dest_file_path) {
                            Ok(_) => {
                                log::info!("Successfully copied {} file: {} -> {}", file_source_type, source_file_path, unique_filename);
                                total_files_copied += 1;
                            }
                            Err(e) => {
                                log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                            }
                        }
                    } else {
                        log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
                    }
                }
            }
        } else {
            log::info!("No documents found, skipping document export files");
        }
    } else {
        // Export strategic goals without document metadata
        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing entity {} to file: id={}", i + 1, entity.id);
            strategic_goals_writer.write_entity(entity)?;
        }
    }
    
    // Finalize strategic goals writer
    strategic_goals_writer.finalize()?;
    
    log::info!("Export strategic goals by IDs completed: wrote {} goals, {} documents, {} files", result.items.len(), total_documents_exported, total_files_copied);
    Ok(result.items.len() as i64)
}

async fn export_projects_by_ids(dest_dir: &Path, ids: &[Uuid]) -> Result<(), String> {
    export_projects_by_ids_with_options(dest_dir, ids, false, &Arc::new(crate::globals::get_file_storage_service().map_err(|e| e.to_string())?)).await.map(|_| ())
}

pub async fn export_projects_by_ids_with_options(dest_dir: &Path, ids: &[Uuid], include_documents: bool, file_storage: &Arc<dyn FileStorageService>) -> Result<i64, String> {
    if ids.is_empty() {
        log::info!("Export projects by IDs: empty IDs list");
        return Ok(0);
    }

    log::info!("Export projects by IDs: {} IDs, include_documents: {}", ids.len(), include_documents);

    let project_repo = globals::get_project_repo().map_err(|e| e.to_string())?;
    
    // Get projects
    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    log::info!("Calling project repo.find_by_ids with params: page={}, per_page={}", params.page, params.per_page);
    let result = project_repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    log::info!("Repository returned {} items, total={}", result.items.len(), result.total);

    // Export projects using optimized writer
    let projects_file_path = dest_dir.join("projects.jsonl");
    let mut projects_writer = OptimizedJsonLWriter::new(&projects_file_path)?;

    let mut total_documents_exported = 0;
    let mut total_files_copied = 0;

    if include_documents {
        let media_doc_repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;
        
        // Get all project IDs
        let project_ids: Vec<Uuid> = result.items.iter().map(|p| p.id).collect();
        log::info!("Fetching all documents for {} projects efficiently", project_ids.len());
        
        // Fetch ALL documents for ALL projects in a single query - this solves the N+1 problem
        let all_documents = media_doc_repo
            .find_by_related_entities("projects", &project_ids)
            .await
            .map_err(|e| e.to_string())?;
        
        // Group documents by related_id (project_id) for efficient lookup
        let mut documents_by_project: std::collections::HashMap<Uuid, Vec<crate::domains::document::types::MediaDocument>> = std::collections::HashMap::new();
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                documents_by_project.entry(related_id).or_insert_with(Vec::new).push(document);
            }
        }
        
        log::info!("Found {} total documents across all projects", documents_by_project.values().map(|v| v.len()).sum::<usize>());

        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing entity {} to file: id={}", i + 1, entity.id);
            
            // Get document count from our pre-fetched data
            let document_count = documents_by_project.get(&entity.id).map(|docs| docs.len()).unwrap_or(0);
            let metadata = serde_json::json!({
                "document_count": document_count,
                "has_documents": document_count > 0
            });
            
            projects_writer.write_enhanced_entity(entity, metadata)?;
        }

        // Export associated media documents and files
        log::info!("Exporting associated media documents");
        let mut documents_writer_opt: Option<OptimizedJsonLWriter> = None;
        let mut files_dir_created = false;
        let files_dir = dest_dir.join("files");
        
        // Check if we actually have documents before processing
        let total_docs_count: usize = documents_by_project.values().map(|docs| docs.len()).sum();
        log::info!("Total documents to export: {}", total_docs_count);
        
        if total_docs_count > 0 {
            // Create organized directory structure: files/projects/{project_id}/
            let projects_dir = files_dir.join("projects");
            std::fs::create_dir_all(&projects_dir).map_err(|e| e.to_string())?;
            files_dir_created = true;
            
            // Process all documents we already fetched (no more database calls!)
            for (project_id, documents) in documents_by_project.iter() {
                if !documents.is_empty() {
                    // Create project-specific directory
                    let project_dir = projects_dir.join(project_id.to_string());
                    std::fs::create_dir_all(&project_dir).map_err(|e| e.to_string())?;
                    
                    for document in documents {
                        let (source_file_path, file_source_type) = select_best_file_path_with_storage(&document, file_storage);
                        let dest_file_path = project_dir.join(&document.original_filename);
                        let abs_source_path = file_storage.get_absolute_path(source_file_path);

                        if abs_source_path.exists() {
                            match std::fs::copy(&abs_source_path, &dest_file_path) {
                                Ok(_) => {
                                    log::info!("Successfully copied {} file: {} -> projects/{}/{}", file_source_type, source_file_path, project_id, document.original_filename);
                                    total_files_copied += 1;
                                }
                                Err(e) => {
                                    log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                                }
                            }
                        } else {
                            log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
                        }
                        
                        // Export document metadata to JSONL
                        if documents_writer_opt.is_none() {
                            let documents_file_path = dest_dir.join("media_documents.jsonl");
                            documents_writer_opt = Some(OptimizedJsonLWriter::new(&documents_file_path)?);
                        }
                        if let Some(ref mut documents_writer) = documents_writer_opt {
                            documents_writer.write_entity(&document)?;
                            total_documents_exported += 1;
                        }
                    }
                }
            }
            
            // Finalize documents writer
            if let Some(documents_writer) = documents_writer_opt {
                documents_writer.finalize()?;
            }
        } else {
            log::info!("No documents found, skipping document export files");
        }
    } else {
        // Export projects without document metadata
        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing entity {} to file: id={}", i + 1, entity.id);
            projects_writer.write_entity(entity)?;
        }
    }
    
    // Finalize projects writer
    projects_writer.finalize()?;
    
    log::info!("Export projects by IDs completed: wrote {} projects, {} documents, {} files", result.items.len(), total_documents_exported, total_files_copied);
    Ok(result.items.len() as i64)
}

async fn export_activities_by_ids(dest_dir: &Path, ids: &[Uuid]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let repo = globals::get_activity_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("activities.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    let result = repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    for entity in result.items {
        let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn export_donors_by_ids(dest_dir: &Path, ids: &[Uuid]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let repo = globals::get_donor_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("donors.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    let result = repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    for entity in result.items {
        let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn export_fundings_by_ids(dest_dir: &Path, ids: &[Uuid]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let repo = globals::get_project_funding_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("project_funding.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    let result = repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    for entity in result.items {
        let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

async fn export_livelihoods_by_ids(dest_dir: &Path, ids: &[Uuid]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let livelihood_repo = globals::get_livelihood_repo().map_err(|e| e.to_string())?;
    let subsequent_grant_repo = globals::get_subsequent_grant_repo().map_err(|e| e.to_string())?;
    
    // Export livelihoods
    let livelihoods_file_path = dest_dir.join("livelihoods.jsonl");
    let mut livelihoods_file = std::fs::File::create(&livelihoods_file_path).map_err(|e| e.to_string())?;

    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    let result = livelihood_repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    for entity in &result.items {
        let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
        writeln!(livelihoods_file, "{}", json).map_err(|e| e.to_string())?;
    }

    // Export associated subsequent grants for these livelihoods
    let grants_file_path = dest_dir.join("subsequent_grants.jsonl");
    let mut grants_file = std::fs::File::create(&grants_file_path).map_err(|e| e.to_string())?;

    for livelihood in &result.items {
        let grants = subsequent_grant_repo.find_by_livelihood_id(livelihood.id).await.map_err(|e| e.to_string())?;
        for grant in grants {
            let json = serde_json::to_string(&grant).map_err(|e| e.to_string())?;
            writeln!(grants_file, "{}", json).map_err(|e| e.to_string())?;
        }
    }
    
    Ok(())
}

async fn export_workshops_by_ids(dest_dir: &Path, ids: &[Uuid], include_participants: bool) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let workshop_repo = globals::get_workshop_repo().map_err(|e| e.to_string())?;
    let workshop_participant_repo = globals::get_workshop_participant_repo().map_err(|e| e.to_string())?;
    
    // Export workshops
    let workshops_file_path = dest_dir.join("workshops.jsonl");
    let mut workshops_file = std::fs::File::create(&workshops_file_path).map_err(|e| e.to_string())?;

    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    let result = workshop_repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    for entity in &result.items {
        let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
        writeln!(workshops_file, "{}", json).map_err(|e| e.to_string())?;
    }

    // Export associated workshop participants only if requested
    if include_participants {
        let participants_file_path = dest_dir.join("workshop_participants.jsonl");
        let mut participants_file = std::fs::File::create(&participants_file_path).map_err(|e| e.to_string())?;

        for workshop in &result.items {
            let participants = workshop_participant_repo.find_participants_for_workshop(workshop.id).await.map_err(|e| e.to_string())?;
            for participant in participants {
                let json = serde_json::to_string(&participant).map_err(|e| e.to_string())?;
                writeln!(participants_file, "{}", json).map_err(|e| e.to_string())?;
            }
        }
    }
    
    Ok(())
}

async fn export_media_documents_by_ids(
    dest_dir: &Path,
    ids: &[Uuid],
    include_blobs: bool,
    file_storage: &Arc<dyn FileStorageService>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("media_documents.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    for id in ids {
        let entity = match MediaDocumentRepository::find_by_id(repo.as_ref(), *id).await {
            Ok(doc) => doc,
            Err(_) => continue, // Skip missing documents
        };

        let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())?;

        // Handle blob export if requested and entity has file_path
        if include_blobs {
            // Use helper function to select the best file path
            let (source_file_path, file_source_type) = select_best_file_path_with_storage(&entity, file_storage);
            
            let abs_path = file_storage.get_absolute_path(source_file_path);
            if abs_path.exists() {
                let blobs_dir = dest_dir.join("blobs");
                std::fs::create_dir_all(&blobs_dir).map_err(|e| e.to_string())?;
                if let Some(_filename) = abs_path.file_name() {
                    // Use original filename for consistency
                    let export_filename = std::ffi::OsString::from(&entity.original_filename);
                    match std::fs::copy(&abs_path, blobs_dir.join(export_filename)) {
                        Ok(_) => {
                            log::info!("Successfully copied {} file: {} -> {}", file_source_type, source_file_path, &entity.original_filename);
                        }
                        Err(e) => {
                            log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                        }
                    }
                }
            } else {
                log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
            }
        }
    }
    Ok(())
}

async fn export_workshop_participants(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_workshop_participant_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("workshop_participants.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let mut page = 1;
    loop {
        let params = PaginationParams { page, per_page: 200 };
        let result = repo.find_all(params).await.map_err(|e| e.to_string())?;

        if result.items.is_empty() {
            break;
        }

        for entity in result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }

        page += 1;
    }
    Ok(())
}

async fn export_workshop_participants_by_ids(dest_dir: &Path, ids: &[Uuid]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let repo = globals::get_workshop_participant_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("workshop_participants.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    let result = repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    for entity in result.items {
        let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
        writeln!(file, "{}", json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// NEW: Helper function for strategic goals export by complex filter
async fn export_strategic_goals_by_filter(
    dest_dir: &Path,
    filter: &crate::domains::strategic_goal::types::StrategicGoalFilter,
) -> Result<(), String> {
    let repo = crate::globals::get_strategic_goal_repo().map_err(|e| e.to_string())?;
    
    // First, get all IDs that match the filter
    let matching_ids = repo.find_ids_by_filter(filter.clone()).await
        .map_err(|e| format!("Failed to get filtered IDs: {}", e))?;
    
    if matching_ids.is_empty() {
        // No matching goals, create empty file
        let file_path = dest_dir.join("strategic_goals.jsonl");
        let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
        // File is already empty, just return
        return Ok(());
    }
    
    // Now fetch the actual entities in batches using the IDs
    let file_path = dest_dir.join("strategic_goals.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let per_page = 200;
    
    for chunk in matching_ids.chunks(per_page) {
        let params = PaginationParams { 
            page: 1, // Always page 1 since we're passing specific IDs
            per_page: chunk.len() as u32 
        };
        
        let result = repo.find_by_ids(chunk, params).await
            .map_err(|e| format!("Failed to fetch goals by IDs: {}", e))?;
        
        for entity in result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
    }

    Ok(())
} 

// Generic optimized export helper for basic entity exports  
async fn export_entities_optimized<T, R>(
    dest_dir: &Path,
    filename: &str,
    repo_getter: impl Fn() -> Result<R, String>,
    fetcher: impl Fn(&R, PaginationParams) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<crate::types::PaginatedResult<T>, String>> + Send>>,
) -> Result<(), String>
where
    T: serde::Serialize + Send,
    R: Send,
{
    let repo = repo_getter()?;
    let file_path = dest_dir.join(filename);
    let mut writer = OptimizedJsonLWriter::new(&file_path)?;

    let mut page: u32 = 1;
    let per_page = 500; // Optimized page size
    
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = fetcher(&repo, params).await?;
        
        for entity in page_result.items {
            writer.write_entity(&entity)?;
        }
        
        if page >= page_result.total_pages {
            break;
        }
        page += 1;
    }
    
    writer.finalize()?;
    Ok(())
}

// Participants export functions
async fn export_participants(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_participant_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("participants.jsonl");
    let mut writer = OptimizedJsonLWriter::new(&file_path)?;

    let mut page: u32 = 1;
    let per_page = 500;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            writer.write_entity(&entity)?;
        }
        if page >= page_result.total_pages {
            break;
        }
        page += 1;
    }
    
    writer.finalize()?;
    Ok(())
}

async fn export_participants_by_ids(dest_dir: &Path, ids: &[Uuid]) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }

    let repo = globals::get_participant_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("participants.jsonl");
    let mut writer = OptimizedJsonLWriter::new(&file_path)?;

    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    let result = repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    for entity in result.items {
        writer.write_entity(&entity)?;
    }
    
    writer.finalize()?;
    Ok(())
}

async fn export_participants_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), String> {
    let repo = globals::get_participant_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("participants.jsonl");
    let mut writer = OptimizedJsonLWriter::new(&file_path)?;

    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_by_date_range(start, end, params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            writer.write_entity(&entity)?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
    }
    
    writer.finalize()?;
    Ok(())
}

/// Export participants by IDs with optional document support (similar to strategic goals)
pub async fn export_participants_by_ids_with_documents(dest_dir: &Path, ids: &[Uuid], include_documents: bool, file_storage: &Arc<dyn FileStorageService>) -> Result<i64, String> {
    if ids.is_empty() {
        log::info!("Export participants by IDs: empty IDs list");
        return Ok(0);
    }

    log::info!("Export participants by IDs: {} IDs, include_documents: {}", ids.len(), include_documents);

    let participant_repo = globals::get_participant_repo().map_err(|e| e.to_string())?;
    
    // Get participants
    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    log::info!("Calling participant repo.find_by_ids with params: page={}, per_page={}", params.page, params.per_page);
    let result = participant_repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    log::info!("Repository returned {} participants, total={}", result.items.len(), result.total);

    // Export participants using optimized writer
    let participants_file_path = dest_dir.join("participants.jsonl");
    let mut participants_writer = OptimizedJsonLWriter::new(&participants_file_path)?;

    let mut total_files_copied = 0;

    if include_documents {
        let media_doc_repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;
        
        // Get all participant IDs
        let participant_ids: Vec<Uuid> = result.items.iter().map(|p| p.id).collect();
        log::info!("Fetching all documents for {} participants efficiently", participant_ids.len());
        
        // Fetch ALL documents for ALL participants in a single query - this solves the N+1 problem
        let all_documents = media_doc_repo
            .find_by_related_entities("participants", &participant_ids)
            .await
            .map_err(|e| e.to_string())?;
        
        // Group documents by related_id (participant_id) for efficient lookup
        let mut documents_by_participant: std::collections::HashMap<Uuid, Vec<crate::domains::document::types::MediaDocument>> = std::collections::HashMap::new();
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                documents_by_participant.entry(related_id).or_insert_with(Vec::new).push(document);
            }
        }
        
        log::info!("Found {} total documents across all participants", documents_by_participant.values().map(|v| v.len()).sum::<usize>());

        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing participant {} to file: id={}", i + 1, entity.id);
            
            // Get document count from our pre-fetched data
            let document_count = documents_by_participant.get(&entity.id).map(|docs| docs.len()).unwrap_or(0);
            let metadata = serde_json::json!({
                "document_count": document_count,
                "has_documents": document_count > 0
            });
            
            participants_writer.write_enhanced_entity(entity, metadata)?;
        }

        // Export associated media documents and files
        log::info!("Exporting associated media documents");
        let mut files_dir_created = false;
        let files_dir = dest_dir.join("files");
        
        // Check if we actually have documents before processing
        let total_docs_count: usize = documents_by_participant.values().map(|docs| docs.len()).sum();
        log::info!("Total documents to export: {}", total_docs_count);
        
        if total_docs_count > 0 {
            // Process all documents we already fetched (no more database calls!)
            for documents in documents_by_participant.values() {
                for document in documents {
                    if !files_dir_created {
                        std::fs::create_dir_all(&files_dir).map_err(|e| e.to_string())?;
                        files_dir_created = true;
                    }

                    let (source_file_path, file_source_type) = select_best_file_path_with_storage(&document, file_storage);
                    let unique_filename = format!("{}_{}", document.id, document.original_filename);
                    let dest_file_path = files_dir.join(&unique_filename);
                    let abs_source_path = file_storage.get_absolute_path(source_file_path);

                    if abs_source_path.exists() {
                        match std::fs::copy(&abs_source_path, &dest_file_path) {
                            Ok(_) => {
                                log::info!("Successfully copied {} file: {} -> {}", file_source_type, source_file_path, unique_filename);
                                total_files_copied += 1;
                            }
                            Err(e) => {
                                log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                            }
                        }
                    } else {
                        log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
                    }
                }
            }
        } else {
            log::info!("No documents found, skipping document export files");
        }
    } else {
        // Export participants without document metadata
        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing participant {} to file: id={}", i + 1, entity.id);
            participants_writer.write_entity(entity)?;
        }
    }
    
    // Finalize participants writer
    participants_writer.finalize()?;
    
    log::info!("Export participants by IDs completed: wrote {} participants, {} files", result.items.len(), total_files_copied);
    Ok(result.items.len() as i64)
}

/// Export activities by IDs with optional document support (similar to strategic goals)
pub async fn export_activities_by_ids_with_documents(dest_dir: &Path, ids: &[Uuid], include_documents: bool, file_storage: &Arc<dyn FileStorageService>) -> Result<i64, String> {
    if ids.is_empty() {
        log::info!("Export activities by IDs: empty IDs list");
        return Ok(0);
    }

    log::info!("Export activities by IDs: {} IDs, include_documents: {}", ids.len(), include_documents);

    let activity_repo = globals::get_activity_repo().map_err(|e| e.to_string())?;
    
    // Get activities
    let params = PaginationParams { page: 1, per_page: ids.len() as u32 };
    log::info!("Calling activity repo.find_by_ids with params: page={}, per_page={}", params.page, params.per_page);
    let result = activity_repo.find_by_ids(ids, params).await.map_err(|e| e.to_string())?;

    log::info!("Repository returned {} activities, total={}", result.items.len(), result.total);

    // Export activities using optimized writer
    let activities_file_path = dest_dir.join("activities.jsonl");
    let mut activities_writer = OptimizedJsonLWriter::new(&activities_file_path)?;

    let mut total_files_copied = 0;

    if include_documents {
        let media_doc_repo = globals::get_media_document_repo().map_err(|e| e.to_string())?;
        
        // Get all activity IDs
        let activity_ids: Vec<Uuid> = result.items.iter().map(|a| a.id).collect();
        log::info!("Fetching all documents for {} activities efficiently", activity_ids.len());
        
        // Fetch ALL documents for ALL activities in a single query - this solves the N+1 problem
        let all_documents = media_doc_repo
            .find_by_related_entities("activities", &activity_ids)
            .await
            .map_err(|e| e.to_string())?;
        
        // Group documents by related_id (activity_id) for efficient lookup
        let mut documents_by_activity: std::collections::HashMap<Uuid, Vec<crate::domains::document::types::MediaDocument>> = std::collections::HashMap::new();
        for document in all_documents {
            if let Some(related_id) = document.related_id {
                documents_by_activity.entry(related_id).or_insert_with(Vec::new).push(document);
            }
        }
        
        log::info!("Found {} total documents across all activities", documents_by_activity.values().map(|v| v.len()).sum::<usize>());

        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing activity {} to file: id={}", i + 1, entity.id);
            
            // Get document count from our pre-fetched data
            let document_count = documents_by_activity.get(&entity.id).map(|docs| docs.len()).unwrap_or(0);
            let metadata = serde_json::json!({
                "document_count": document_count,
                "has_documents": document_count > 0
            });
            
            activities_writer.write_enhanced_entity(entity, metadata)?;
        }

        // Export associated media documents and files
        log::info!("Exporting associated media documents");
        let mut files_dir_created = false;
        let files_dir = dest_dir.join("files");
        
        // Check if we actually have documents before processing
        let total_docs_count: usize = documents_by_activity.values().map(|docs| docs.len()).sum();
        log::info!("Total documents to export: {}", total_docs_count);
        
        if total_docs_count > 0 {
            // Process all documents we already fetched (no more database calls!)
            for documents in documents_by_activity.values() {
                for document in documents {
                    if !files_dir_created {
                        std::fs::create_dir_all(&files_dir).map_err(|e| e.to_string())?;
                        files_dir_created = true;
                    }

                    let (source_file_path, file_source_type) = select_best_file_path_with_storage(&document, file_storage);
                    let unique_filename = format!("{}_{}", document.id, document.original_filename);
                    let dest_file_path = files_dir.join(&unique_filename);
                    let abs_source_path = file_storage.get_absolute_path(source_file_path);

                    if abs_source_path.exists() {
                        match std::fs::copy(&abs_source_path, &dest_file_path) {
                            Ok(_) => {
                                log::info!("Successfully copied {} file: {} -> {}", file_source_type, source_file_path, unique_filename);
                                total_files_copied += 1;
                            }
                            Err(e) => {
                                log::error!("Failed to copy {} file {}: {}", file_source_type, source_file_path, e);
                            }
                        }
                    } else {
                        log::error!("File not found for export: {} (type: {})", source_file_path, file_source_type);
                    }
                }
            }
        } else {
            log::info!("No documents found, skipping document export files");
        }
    } else {
        // Export activities without document metadata
        for (i, entity) in result.items.iter().enumerate() {
            log::info!("Writing activity {} to file: id={}", i + 1, entity.id);
            activities_writer.write_entity(entity)?;
        }
    }
    
    // Finalize activities writer
    activities_writer.finalize()?;
    
    log::info!("Export activities by IDs completed: wrote {} activities, {} files", result.items.len(), total_files_copied);
    Ok(result.items.len() as i64)
}