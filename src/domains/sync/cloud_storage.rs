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

/// Trait for cloud storage service operations
#[async_trait]
pub trait CloudStorageService: Send + Sync {
    /// Get changes from remote storage since a specific sync token
    async fn get_changes_since(&self, api_token: &str, sync_token: Option<String>) -> ServiceResult<FetchChangesResponse>;
    
    /// Push local changes to remote storage
    async fn push_changes(&self, api_token: &str, payload: PushPayload) -> ServiceResult<PushChangesResponse>;
    
    /// Upload a document to cloud storage
    async fn upload_document(&self, document_id: Uuid, local_path: &str, mime_type: &str, size_bytes: u64) -> ServiceResult<String>;
    
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

#[async_trait]
impl CloudStorageService for ApiCloudStorageService {
    async fn get_changes_since(&self, api_token: &str, sync_token: Option<String>) -> ServiceResult<FetchChangesResponse> {
        debug!("Fetching changes since token: {:?}", sync_token);
        
        // Build the URL with optional sync token
        let url = if let Some(token) = &sync_token {
            format!("{}/api/sync/changes?since={}", self.base_url, token)
        } else {
            format!("{}/api/sync/changes", self.base_url)
        };
        
        // Make the API request
        let response = self.client.get(&url)
            .header("Authorization", self.auth_header(api_token))
            .send()
            .await
            .map_err(|e| ServiceError::ExternalService(format!("Failed to fetch changes: {}", e)))?;
            
        // Check status and parse response
        if response.status().is_success() {
            let changes_response = response.json::<FetchChangesResponse>()
                .await
                .map_err(|e| ServiceError::ExternalService(format!("Failed to parse changes response: {}", e)))?;
                
            Ok(changes_response)
        } else {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unable to get error details".to_string());
                
            Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)))
        }
    }
    
    async fn push_changes(&self, api_token: &str, payload: PushPayload) -> ServiceResult<PushChangesResponse> {
        debug!("Pushing {} changes and {} tombstones", payload.changes.len(), payload.tombstones.as_ref().map_or(0, |t| t.len()));
        
        let url = format!("{}/api/sync/push", self.base_url);
        
        // Make the API request
        let response = self.client.post(&url)
            .header("Authorization", self.auth_header(api_token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| ServiceError::ExternalService(format!("Failed to push changes: {}", e)))?;
            
        // Check status and parse response
        if response.status().is_success() {
            let push_response = response.json::<PushChangesResponse>()
                .await
                .map_err(|e| ServiceError::ExternalService(format!("Failed to parse push response: {}", e)))?;
                
            Ok(push_response)
        } else {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unable to get error details".to_string());
                
            Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)))
        }
    }
    
    async fn upload_document(&self, document_id: Uuid, local_path: &str, mime_type: &str, size_bytes: u64) -> ServiceResult<String> {
        debug!("Uploading document {} from {}", document_id, local_path);
        
        let doc_id_str = document_id.to_string();
        let url = format!("{}/api/documents/upload/{}", self.base_url, doc_id_str);
        
        // Read the file
        let file_content = tokio::fs::read(local_path)
            .await
            .map_err(|e| ServiceError::Domain(DomainError::File(format!("Failed to read local file: {}", e))))?;
            
        // Prepare the multipart form
        let part = Part::bytes(file_content)
            .file_name(Path::new(local_path).file_name().unwrap_or_default().to_string_lossy().into_owned())
            .mime_str(mime_type)
            .map_err(|e| ServiceError::Domain(DomainError::Internal(format!("Invalid MIME type for upload: {}", e))))?;
            
        let form = Form::new()
            .part("file", part)
            .text("documentId", doc_id_str.clone());
            
        // Make the API request
        let response = self.client.post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ServiceError::ExternalService(format!("Failed to upload document: {}", e)))?;
            
        // Check status and parse response
        if response.status().is_success() {
            #[derive(Deserialize)]
            struct UploadResponse {
                blob_key: String,
            }
            
            let upload_response = response.json::<UploadResponse>()
                .await
                .map_err(|e| ServiceError::ExternalService(format!("Failed to parse upload response: {}", e)))?;
                
            Ok(upload_response.blob_key)
        } else {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unable to get error details".to_string());
                
            Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)))
        }
    }
    
    async fn download_document(&self, document_id: Uuid, blob_key: &str) -> ServiceResult<(String, u64, bool)> {
        debug!("Downloading document {} with key {}", document_id, blob_key);
        
        // Ensure local storage directory exists
        self.ensure_storage_dir()?;
        
        let doc_id_str = document_id.to_string();
        let url = format!("{}/api/documents/download/{}", self.base_url, blob_key);
        
        // Make the API request
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| ServiceError::ExternalService(format!("Failed to download document: {}", e)))?;
            
        // Check status and process response
        if response.status().is_success() {
            // Get content type to determine compression
            let content_type = response.headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream");
                
            let is_compressed = content_type.contains("compressed") || 
                               content_type.contains("zip") ||
                               response.headers()
                                      .get("X-Compressed")
                                      .and_then(|v| v.to_str().ok())
                                      .map(|v| v == "true")
                                      .unwrap_or(false);
            
            // Get content length
            let size = response.content_length().unwrap_or(0);
            
            // Determine file extension from content type
            let extension = if content_type.contains("jpeg") || content_type.contains("jpg") {
                "jpg"
            } else if content_type.contains("png") {
                "png"
            } else if content_type.contains("pdf") {
                "pdf"
            } else if content_type.contains("zip") || is_compressed {
                "zip"
            } else {
                "bin" // Default binary extension
            };
            
            // Create local path
            let filename = if is_compressed {
                format!("{}_compressed.{}", doc_id_str, extension)
            } else {
                format!("{}.{}", doc_id_str, extension)
            };
            
            let local_path = format!("{}/{}", self.local_storage_path, filename);
            
            // Download and save the file
            let bytes = response.bytes()
                .await
                .map_err(|e| ServiceError::Domain(DomainError::File(format!("Failed to read document bytes: {}", e))))?;
                
            tokio::fs::write(&local_path, &bytes)
                .await
                .map_err(|e| ServiceError::Domain(DomainError::File(format!("Failed to write document to disk: {}", e))))?;
                
            Ok((local_path, size, is_compressed))
        } else {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unable to get error details".to_string());
                
            Err(ServiceError::ExternalService(format!("Server returned error {}: {}", status, error_text)))
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
    async fn get_changes_since(&self, _api_token: &str, _sync_token: Option<String>) -> ServiceResult<FetchChangesResponse> {
        // Return empty response for tests
        Ok(FetchChangesResponse {
            batch_id: String::new(),
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
    
    async fn upload_document(&self, document_id: Uuid, _local_path: &str, _mime_type: &str, _size_bytes: u64) -> ServiceResult<String> {
        // Mock successful upload by returning a fake blob key
        Ok(format!("test_blob_{}", document_id))
    }
    
    async fn download_document(&self, document_id: Uuid, _blob_key: &str) -> ServiceResult<(String, u64, bool)> {
        // Generate a fake local path pointing to a non-existent file for testing
        let local_path = format!("{}/{}.bin", self.base_path, document_id);
        
        // Mock successful download (0 bytes, not compressed)
        Ok((local_path, 0, false))
    }
}