use async_trait::async_trait;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{DateTime, Utc};
use tokio::task;
use tempfile::TempDir;
use zip::{ZipWriter, write::FileOptions};
use std::io::Write;
use serde_json::Value;
use futures::future::join_all;
use chrono::TimeZone;

use crate::auth::AuthContext;
use crate::errors::{ServiceError, ServiceResult};
use crate::domains::core::file_storage_service::FileStorageService;
use crate::domains::export::types::{EntityFilter};

use super::repository::ExportJobRepository;
use super::types::{ExportRequest, ExportSummary, ExportJob, ExportStatus};
use crate::globals;
use crate::types::PaginationParams;
use std::path::{PathBuf, Path};

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
    let mut total_entities: i64 = 0;
    let mut total_bytes: i64 = 0;

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
                    }
                    EntityFilter::ProjectsAll => {
                        export_projects(&temp_dir_path).await?;
                    }
                    EntityFilter::ActivitiesAll => {
                        export_activities(&temp_dir_path).await?;
                    }
                    EntityFilter::DonorsAll => {
                        export_donors(&temp_dir_path).await?;
                    }
                    EntityFilter::FundingAll => {
                        export_fundings(&temp_dir_path).await?;
                    }
                    EntityFilter::LivelihoodsAll => {
                        export_livelihoods(&temp_dir_path).await?;
                    }
                    EntityFilter::WorkshopsAll => {
                        export_workshops(&temp_dir_path).await?;
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
                    }

                    // --- Date-range variants ---
                    EntityFilter::StrategicGoalsByDateRange { start_date, end_date, status_id } => {
                        export_strategic_goals_by_date_range(&temp_dir_path, start_date, end_date, status_id).await?;
                    }
                    EntityFilter::ProjectsByDateRange { start_date, end_date } => {
                        export_projects_by_date_range(&temp_dir_path, start_date, end_date).await?;
                    }
                    EntityFilter::ActivitiesByDateRange { start_date, end_date } => {
                        export_activities_by_date_range(&temp_dir_path, start_date, end_date).await?;
                    }
                    EntityFilter::DonorsByDateRange { start_date, end_date } => {
                        export_donors_by_date_range(&temp_dir_path, start_date, end_date).await?;
                    }
                    EntityFilter::FundingByDateRange { start_date, end_date } => {
                        export_fundings_by_date_range(&temp_dir_path, start_date, end_date).await?;
                    }
                    EntityFilter::LivelihoodsByDateRange { start_date, end_date } => {
                        export_livelihoods_by_date_range(&temp_dir_path, start_date, end_date).await?;
                    }
                    EntityFilter::WorkshopsByDateRange { start_date, end_date } => {
                        export_workshops_by_date_range(&temp_dir_path, start_date, end_date).await?;
                    }
                    EntityFilter::MediaDocumentsByDateRange { start_date, end_date } => {
                        export_media_documents_by_date_range(
                            &temp_dir_path,
                            start_date,
                            end_date,
                            include_blobs,
                            &file_storage_clone2,
                        ).await?;
                    }

                    // Unified variants
                    EntityFilter::UnifiedAllDomains { include_type_tags } => {
                        export_unified(&temp_dir_path, None, include_type_tags, include_blobs, &file_storage_clone2).await?;
                    }
                    EntityFilter::UnifiedByDateRange { start_date, end_date, include_type_tags } => {
                        export_unified(&temp_dir_path, Some((start_date, end_date)), include_type_tags, include_blobs, &file_storage_clone2).await?;
                    }
                }
                Ok::<(), String>(())
            };
            tasks.push(task_fut);
        }

        // Wait for all tasks to finish
        let results = join_all(tasks).await;
        for res in results {
            if let Err(e) = res {
                return Err(e);
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
        total_bytes = metadata.len() as i64;

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
                    Some(total_entities),
                    Some(total_bytes),
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
                    Some(total_entities),
                    Some(total_bytes),
                )
                .await;
        }
    }
}

// Export helper implementations ----------------------------------

async fn export_strategic_goals(dest_dir: &Path, status_id: Option<i64>) -> Result<(), String> {
    let repo = globals::get_strategic_goal_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("strategic_goals.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let mut page: u32 = 1;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = if let Some(status) = status_id {
            repo.find_by_status_id(status, params).await.map_err(|e| e.to_string())?
        } else {
            repo.find_all(params).await.map_err(|e| e.to_string())?
        };

        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }

        if page >= page_result.total_pages {
            break;
        }
        page += 1;
    }
    Ok(())
}

async fn export_projects(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_project_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("projects.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;

    let mut page: u32 = 1;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages {
            break;
        }
        page += 1;
    }
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

async fn export_workshops(dest_dir: &Path) -> Result<(), String> {
    let repo = globals::get_workshop_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("workshops.jsonl");
    let mut file = std::fs::File::create(&file_path).map_err(|e| e.to_string())?;
    let mut page = 1u32;
    let per_page = 200;
    loop {
        let params = PaginationParams { page, per_page };
        let page_result = repo.find_all(params, None).await.map_err(|e| e.to_string())?;
        for entity in page_result.items {
            let json = serde_json::to_string(&entity).map_err(|e| e.to_string())?;
            writeln!(file, "{}", json).map_err(|e| e.to_string())?;
        }
        if page >= page_result.total_pages { break; }
        page += 1;
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
                let rel_path = &doc.file_path;
                let abs = file_storage.get_absolute_path(rel_path);
                if abs.exists() {
                    let blobs_dir = dest_dir.join("blobs");
                    std::fs::create_dir_all(&blobs_dir).map_err(|e| e.to_string())?;
                    let file_name = abs.file_name().ok_or("bad file name")?;
                    std::fs::copy(&abs, blobs_dir.join(file_name)).map_err(|e| e.to_string())?;
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

fn create_zip_from_dir(src_dir: &Path, dest_zip: &Path) -> Result<(), String> {
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
                    let rel_path = &doc.file_path;
                    let abs = file_storage.get_absolute_path(rel_path);
                    if abs.exists() {
                        let blobs_dir = dest_dir.join("blobs");
                        std::fs::create_dir_all(&blobs_dir).map_err(|e| e.to_string())?;
                        let file_name = abs.file_name().ok_or("bad file name")?;
                        std::fs::copy(&abs, blobs_dir.join(file_name)).map_err(|e| e.to_string())?;
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

async fn export_workshops_by_date_range(dest_dir: &Path, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<(), String> {
    let repo = globals::get_workshop_repo().map_err(|e| e.to_string())?;
    let file_path = dest_dir.join("workshops.jsonl");
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
                let rel_path = &doc.file_path;
                let abs = file_storage.get_absolute_path(rel_path);
                if abs.exists() {
                    let blobs_dir = dest_dir.join("blobs");
                    std::fs::create_dir_all(&blobs_dir).map_err(|e| e.to_string())?;
                    let file_name = abs.file_name().ok_or("bad file name")?;
                    std::fs::copy(&abs, blobs_dir.join(file_name)).map_err(|e| e.to_string())?;
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