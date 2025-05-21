use crate::errors::{ServiceError, ServiceResult, DomainError};
use crate::domains::sync::types::{
    FetchChangesResponse, PushChangesResponse, PushPayload,
};
use async_trait::async_trait;
use serde::{Serialize, Deserialize};
use std::path::Path;
use uuid::Uuid;
use log::{info, warn, error, debug};
use reqwest::Client;
use std::time::Duration;
use reqwest::multipart::{Form, Part};
use chrono::Utc;
use sha2::{Sha256, Digest};
use tokio::time::{sleep, Duration as TokioDuration};

/// Trait for cloud storage service operations
#[async_trait]
pub trait CloudStorageService: Send + Sync {
    /// Get changes from remote storage since a specific sync token
    async fn get_changes_since(&self, api_token: &str, device_id: Uuid, sync_token: Option<String>) -> ServiceResult<FetchChangesResponse>;
    
    /// Push local changes to remote storage
    async fn push_changes(&self, api_token: &str, payload: PushPayload) -> ServiceResult<PushChangesResponse>;
    
    /// Upload a document to cloud storage
    async fn upload_document(&self, device_id_of_uploader: Uuid, document_id: Uuid, local_path: &str, mime_type: &str, size_bytes: u64) -> ServiceResult<String>;
    
    /// Download a document from cloud storage
    async fn download_document(&self, document_id: Uuid, blob_key: &str) -> ServiceResult<(String, u64, bool)>;
}

/// Implementation of CloudStorageService that communicates with an API server
pub struct ApiCloudStorageService {
    client: Client,
    base_url: String,
    local_storage_path: String,
}

impl ApiCloudStorageService {
    /// Create a new API-based cloud storage service
    pub fn new(base_url: &str, local_storage_path: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120)) // 2 minute timeout
            .connect_timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
            
        Self {
            client,
            base_url: base_url.to_string(),
            local_storage_path: local_storage_path.to_string(),
        }
    }
    
    /// Get the authorization header
    fn auth_header(&self, api_token: &str) -> String {
        format!("Bearer {}", api_token)
    }
    
    /// Ensure the local storage directory exists
    fn ensure_storage_dir(&self) -> ServiceResult<()> {
        let path = Path::new(&self.local_storage_path);
        if !path.exists() {
            std::fs::create_dir_all(path)
                .map_err(|e| ServiceError::Domain(DomainError::File(format!("Failed to create local storage directory: {}", e))))?;
        }
        Ok(())
    }
}

const MAX_RETRIES: usize = 3;
const BASE_DELAY_MS: u64 = 1_000; // 1 second base delay for backoff

fn should_retry_status(status: reqwest::StatusCode) -> bool {
    status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
}

fn compute_sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let result = hasher.finalize();
    hex::encode(result)
}

#[async_trait]
impl CloudStorageService for ApiCloudStorageService {
    async fn get_changes_since(&self, api_token: &str, device_id: Uuid, sync_token: Option<String>) -> ServiceResult<FetchChangesResponse> {
        debug!("Fetching changes for device {} since token: {:?}", device_id, sync_token);

        if crate::globals::is_offline_mode() {
            warn!("Device is offline. Skipping get_changes_since API call.");
            return Err(ServiceError::ExternalService("Device is offline. Cannot fetch changes.".to_string()));
        }

        let device_id_str = device_id.to_string();
        let mut url_string = format!("{}/api/sync/changes?deviceId={}", self.base_url, device_id_str);

        if let Some(token) = &sync_token {
            url_string.push_str(&format!("&since={}", token));
        }

        let mut attempt = 0usize;
        loop {
            let resp_result = self.client.get(&url_string)
                .header("Authorization", self.auth_header(api_token))
                .send()
                .await;

            match resp_result {
                Ok(response) => {
                    if response.status().is_success() {
                        let decoded = response.json::<FetchChangesResponse>().await
                            .map_err(|e| ServiceError::ExternalService(format!("Failed to parse changes response: {}", e)))?;
                        return Ok(decoded);
                    } else if should_retry_status(response.status()) && attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Retrying fetch_changes (attempt {}) after {} ms (status {})", attempt, delay, response.status());
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_else(|_| "Unable to get error details".to_string());
                        return Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)));
                    }
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Network error fetching changes (attempt {}): {}. Retrying after {} ms", attempt, e, delay);
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    }
                    return Err(ServiceError::ExternalService(format!("Failed to fetch changes: {}", e)));
                }
            }
        }
    }
    
    async fn push_changes(&self, api_token: &str, payload: PushPayload) -> ServiceResult<PushChangesResponse> {
        debug!("Pushing {} changes and {} tombstones for device {}",
               payload.changes.len(),
               payload.tombstones.as_ref().map_or(0, |t| t.len()),
               payload.device_id);

        if crate::globals::is_offline_mode() {
            warn!("Device is offline. Skipping push_changes API call.");
            return Err(ServiceError::ExternalService("Device is offline. Cannot push changes.".to_string()));
        }

        let url = format!("{}/api/sync/push", self.base_url);

        let mut attempt = 0usize;
        loop {
            let resp_res = self.client.post(&url)
                .header("Authorization", self.auth_header(api_token))
                .json(&payload)
                .send()
                .await;

            match resp_res {
                Ok(response) => {
                    if response.status().is_success() {
                        let parsed = response.json::<PushChangesResponse>().await
                            .map_err(|e| ServiceError::ExternalService(format!("Failed to parse push response: {}", e)))?;
                        return Ok(parsed);
                    } else if should_retry_status(response.status()) && attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Retrying push_changes (attempt {}) after {} ms (status {})", attempt, delay, response.status());
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_else(|_| "Unable to get error details".to_string());
                        return Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)));
                    }
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Network error pushing changes (attempt {}): {}. Retrying after {} ms", attempt, e, delay);
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    }
                    return Err(ServiceError::ExternalService(format!("Failed to push changes: {}", e)));
                }
            }
        }
    }
    
    async fn upload_document(&self, device_id_of_uploader: Uuid, document_id: Uuid, local_path: &str, mime_type: &str, _size_bytes: u64) -> ServiceResult<String> {
        debug!("Uploading document {} from device {} from path {}", document_id, device_id_of_uploader, local_path);

        if crate::globals::is_offline_mode() {
            warn!("Device is offline. Skipping upload_document API call for document {}.", document_id);
            return Err(ServiceError::ExternalService(format!("Device is offline. Cannot upload document {}.", document_id)));
        }

        let doc_id_str = document_id.to_string();
        let uploader_device_id_str = device_id_of_uploader.to_string();
        let url = format!("{}/api/documents/upload/{}", self.base_url, doc_id_str);

        // Read the file
        let file_content = tokio::fs::read(local_path)
            .await
            .map_err(|e| ServiceError::Domain(DomainError::File(format!("Failed to read local file: {}", e))))?;

        let file_hash = compute_sha256_hex(&file_content);

        let mut attempt = 0usize;
        loop {
            let part = Part::bytes(file_content.clone())
                .file_name(Path::new(local_path).file_name().unwrap_or_default().to_string_lossy().into_owned())
                .mime_str(mime_type)
                .map_err(|e| ServiceError::Domain(DomainError::Internal(format!("Invalid MIME type for upload: {}", e))))?;

            let form = Form::new()
                .part("file", part)
                .text("documentId", doc_id_str.clone())
                .text("deviceId", uploader_device_id_str.clone());

            let resp_res = self.client.post(&url)
                .header("X-Content-Sha256", file_hash.as_str())
                .multipart(form)
                .send()
                .await;

            match resp_res {
                Ok(response) => {
                    if response.status().is_success() {
                        let server_hash_opt = response.headers().get("X-Verified-Sha256").and_then(|v| v.to_str().ok()).map(|s| s.to_string());

                        #[derive(Deserialize)]
                        struct UploadResponse { blob_key: String }
                        let upload_response = response.json::<UploadResponse>().await
                            .map_err(|e| ServiceError::ExternalService(format!("Failed to parse upload response: {}", e)))?;

                        if let Some(server_hash) = server_hash_opt {
                            if server_hash != file_hash {
                                return Err(ServiceError::Domain(DomainError::Internal("Checksum mismatch after upload".into())));
                            }
                        }

                        return Ok(upload_response.blob_key);
                    } else if should_retry_status(response.status()) && attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Retrying upload_document (attempt {}) after {} ms (status {})", attempt, delay, response.status());
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_else(|_| "Unable to get error details".to_string());
                        return Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)));
                    }
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Network error uploading document (attempt {}): {}. Retrying after {} ms", attempt, e, delay);
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    }
                    return Err(ServiceError::ExternalService(format!("Failed to upload document: {}", e)));
                }
            }
        }
    }
    
    async fn download_document(&self, document_id: Uuid, blob_key: &str) -> ServiceResult<(String, u64, bool)> {
        debug!("Downloading document {} with key {}", document_id, blob_key);

        if crate::globals::is_offline_mode() {
            warn!("Device is offline. Skipping download_document API call for document {} with key {}.", document_id, blob_key);
            return Err(ServiceError::ExternalService(format!("Device is offline. Cannot download document {} with key {}.", document_id, blob_key)));
        }

        self.ensure_storage_dir()?;

        let doc_id_str = document_id.to_string();
        let url = format!("{}/api/documents/download/{}", self.base_url, blob_key);

        let mut attempt = 0usize;
        loop {
            let resp_res = self.client.get(&url).send().await;

            match resp_res {
                Ok(response) => {
                    if response.status().is_success() {
                        // Determine compression via headers
                        let content_type = response.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("application/octet-stream");
                        let is_compressed = content_type.contains("compressed") || content_type.contains("zip") || response.headers().get("X-Compressed").and_then(|v| v.to_str().ok()).map(|v| v == "true").unwrap_or(false);

                        let size = response.content_length().unwrap_or(0);

                        let extension = if content_type.contains("jpeg") || content_type.contains("jpg") { "jpg" } else if content_type.contains("png") { "png" } else if content_type.contains("pdf") { "pdf" } else if content_type.contains("zip") || is_compressed { "zip" } else { "bin" };

                        let filename = if is_compressed { format!("{}_compressed.{}", doc_id_str, extension) } else { format!("{}.{}", doc_id_str, extension) };
                        let local_path = format!("{}/{}", self.local_storage_path, filename);

                        // Capture checksum header before consuming response
                        let server_hash_opt = response.headers()
                            .get("X-Content-Sha256")
                            .and_then(|v| v.to_str().ok())
                            .map(|s| s.to_string());
                        
                        let bytes = response.bytes().await.map_err(|e| ServiceError::Domain(DomainError::File(format!("Failed to read document bytes: {}", e))))?;
                        
                        // Verify checksum if header present
                        if let Some(expected_hash) = server_hash_opt {
                            let actual_hash = compute_sha256_hex(&bytes);
                            if actual_hash != expected_hash {
                                return Err(ServiceError::Domain(DomainError::Internal("Checksum mismatch in downloaded file".into())));
                            }
                        }

                        tokio::fs::write(&local_path, &bytes).await.map_err(|e| ServiceError::Domain(DomainError::File(format!("Failed to write document to disk: {}", e))))?;

                        return Ok((local_path, size, is_compressed));
                    } else if should_retry_status(response.status()) && attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Retrying download_document (attempt {}) after {} ms (status {})", attempt, delay, response.status());
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    } else {
                        let status = response.status();
                        let error_text = response.text().await.unwrap_or_else(|_| "Unable to get error details".to_string());
                        return Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)));
                    }
                }
                Err(e) => {
                    if attempt < MAX_RETRIES {
                        attempt += 1;
                        let delay = BASE_DELAY_MS * 2u64.pow(attempt as u32 - 1);
                        debug!("Network error downloading document (attempt {}): {}. Retrying after {} ms", attempt, e, delay);
                        sleep(TokioDuration::from_millis(delay)).await;
                        continue;
                    }
                    return Err(ServiceError::ExternalService(format!("Failed to download document: {}", e)));
                }
            }
        }
    }
}

/// Mock implementation for testing
#[cfg(test)]
pub struct MockCloudStorageService {
    base_path: String,
}

#[cfg(test)]
impl MockCloudStorageService {
    pub fn new(base_path: &str) -> Self {
        Self { base_path: base_path.to_string() }
    }
}

#[cfg(test)]
#[async_trait]
impl CloudStorageService for MockCloudStorageService {
    async fn get_changes_since(&self, _api_token: &str, _device_id: Uuid, _sync_token: Option<String>) -> ServiceResult<FetchChangesResponse> {
        // Return empty response for tests
        Ok(FetchChangesResponse {
            batch_id: Uuid::new_v4().to_string(), // Provide a mock batch_id
            changes: Vec::new(),
            tombstones: None,
            has_more: false,
            server_timestamp: Utc::now(),
            next_batch_hint: None,
        })
    }
    
    async fn push_changes(&self, _api_token: &str, payload: PushPayload) -> ServiceResult<PushChangesResponse> {
        // Mock successful push according to UploadChangesResponse structure
        let changes_accepted_count = payload.changes.len() as i64;
            
        Ok(PushChangesResponse {
            batch_id: payload.batch_id, // Directly use payload.batch_id as it's String
            changes_accepted: changes_accepted_count,
            changes_rejected: 0, // Mock: no rejected changes
            conflicts_detected: 0, // Mock: no conflicts detected (0 for false)
            conflicts: None,
            server_timestamp: Utc::now(),
        })
    }
    
    async fn upload_document(&self, _device_id_of_uploader: Uuid, document_id: Uuid, _local_path: &str, _mime_type: &str, _size_bytes: u64) -> ServiceResult<String> {
        // Mock successful upload by returning a fake blob key
        Ok(format!("mock_blob_key_for_{}", document_id))
    }
    
    async fn download_document(&self, document_id: Uuid, _blob_key: &str) -> ServiceResult<(String, u64, bool)> {
        // Generate a fake local path pointing to a non-existent file for testing
        let local_path = format!("{}/{}.bin", self.base_path, document_id);
        
        // Mock successful download (0 bytes, not compressed)
        Ok((local_path, 0, false))
    }
}