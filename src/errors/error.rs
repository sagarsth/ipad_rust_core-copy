use std::fmt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use crate::domains::core::file_storage_service::FileStorageError;

/// Database errors
#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),

    #[error("Connection pool error: {0}")]
    ConnectionPool(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Query error: {0}")]
    Query(String),

    #[error("Error executing statement: {0}")]
    Execution(String),

    #[error("Record not found: {0} with ID {1}")]
    NotFound(String, String),
    
    #[error("Conflict error: {0}")]
    Conflict(String),
    
    #[error("Database is locked")]
    Locked,
    
    #[error("Migration error: {0}")]
    Migration(String),
    
    #[error("Database error: {0}")]
    Other(String),
}

impl serde::Serialize for DbError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("DbError", 2)?;
        match self {
            DbError::Sqlx(err) => {
                state.serialize_field("type", "Sqlx")?;
                state.serialize_field("message", &err.to_string())?;
            }
            DbError::ConnectionPool(s) => {
                state.serialize_field("type", "ConnectionPool")?;
                state.serialize_field("message", s)?;
            }
            DbError::Transaction(s) => {
                state.serialize_field("type", "Transaction")?;
                state.serialize_field("message", s)?;
            }
            DbError::Query(s) => {
                state.serialize_field("type", "Query")?;
                state.serialize_field("message", s)?;
            }
            DbError::Execution(s) => {
                state.serialize_field("type", "Execution")?;
                state.serialize_field("message", s)?;
            }
            DbError::NotFound(s1, s2) => {
                state.serialize_field("type", "NotFound")?;
                state.serialize_field("message", &format!("Record not found: {} with ID {}", s1, s2))?;
            }
            DbError::Conflict(s) => {
                state.serialize_field("type", "Conflict")?;
                state.serialize_field("message", s)?;
            }
            DbError::Locked => {
                state.serialize_field("type", "Locked")?;
                state.serialize_field("message", "Database is locked")?;
            }
            DbError::Migration(s) => {
                state.serialize_field("type", "Migration")?;
                state.serialize_field("message", s)?;
            }
            DbError::Other(s) => {
                state.serialize_field("type", "Other")?;
                state.serialize_field("message", s)?;
            }
        }
        state.end()
    }
}

/// Manual Clone implementation for DbError
impl Clone for DbError {
    fn clone(&self) -> Self {
        match self {
            DbError::Sqlx(err) => DbError::Other(format!("SQLx error: {}", err.to_string())),
            DbError::ConnectionPool(s) => DbError::ConnectionPool(s.clone()),
            DbError::Transaction(s) => DbError::Transaction(s.clone()),
            DbError::Query(s) => DbError::Query(s.clone()),
            DbError::Execution(s) => DbError::Execution(s.clone()),
            DbError::NotFound(s1, s2) => DbError::NotFound(s1.clone(), s2.clone()),
            DbError::Conflict(s) => DbError::Conflict(s.clone()),
            DbError::Locked => DbError::Locked,
            DbError::Migration(s) => DbError::Migration(s.clone()),
            DbError::Other(s) => DbError::Other(s.clone()),
        }
    }
}

/// Domain-level errors
#[derive(Debug, Error, Clone, Serialize)]
pub enum DomainError {
    #[error("Database error: {0}")]
    Database(#[from] DbError),
    
    #[error("Authorization failed: {0}")]
    AuthorizationFailed(String),
    
    #[error("Invalid UUID: {0}")]
    InvalidUuid(String),
    
    #[error("Entity not found: {0} with ID {1}")]
    EntityNotFound(String, Uuid),
    
    #[error("Cannot delete {entity_type} with ID {id} due to dependent records in: {}", .dependencies.join(", "))]
    DependentRecordsExist {
        entity_type: String,
        id: Uuid,
        dependencies: Vec<String>,
    },

    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
    
    #[error("Sync error: {0}")]
    Sync(#[from] SyncError),
    
    #[error("LWW conflict: {field} in {entity_type} {id}")]
    LwwConflict {
        entity_type: String,
        id: Uuid,
        field: String,
    },
    
    #[error("Cannot perform operation on deleted entity: {0} with ID {1}")]
    DeletedEntity(String, Uuid),
    
    #[error("File error: {0}")]
    File(String),
    
    #[error("Compression error: {0}")]
    Compression(String),
    
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("External error: {0}")]
    External(String),
}

impl From<FileStorageError> for DomainError {
    fn from(error: FileStorageError) -> Self {
        DomainError::External(format!("File storage error: {}", error))
    }
}

/// Service-level errors (application specific)
#[derive(Debug, Error, Clone, Serialize)]
pub enum ServiceError {
    #[error("Domain error: {0}")]
    Domain(#[from] DomainError),
    
    #[error("Cannot delete record due to dependencies in: {}", .0.join(", "))]
    DependenciesPreventDeletion(Vec<String>),
    
    #[error("User interface error: {0}")]
    Ui(String),
    
    #[error("Authentication error: {0}")]
    Authentication(String),
    
    #[error("Session expired")]
    SessionExpired,
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Feature not available in offline mode: {0}")]
    OfflineFeatureUnavailable(String),
    
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("External service error: {0}")]
    ExternalService(String),
}

/// Sync-specific errors
#[derive(Debug, Error, Clone, Serialize)]
pub enum SyncError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    
    #[error("Record conflict: {0}")]
    RecordConflict(String),
    
    #[error("Conflict during sync: {}", .conflict.message)]
    Conflict {
        conflict: SyncConflict,
    },
    
    #[error("Tombstone conflict: Entity {entity_type} with ID {id} was previously deleted")]
    TombstoneConflict {
        entity_type: String,
        id: Uuid,
    },
    
    #[error("Server error: {0}")]
    ServerError(String),
    
    #[error("Local database error: {0}")]
    LocalDatabase(#[from] DbError),
    
    #[error("Remote entity not found: {0}")]
    RemoteEntityNotFound(String),
    
    #[error("Entity type mismatch: {0}")]
    EntityTypeMismatch(String),
    
    #[error("Missing required fields: {0}")]
    MissingRequiredFields(String),
    
    #[error("Invalid batch: {0}")]
    InvalidBatch(String),
    
    #[error("Sync interrupted")]
    Interrupted,
    
    #[error("Sync timeout")]
    Timeout,
    
    #[error("Insufficient storage")]
    InsufficientStorage,
    
    #[error("Sync error: {0}")]
    Other(String),
}

/// Detailed information about a sync conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConflict {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub field_name: Option<String>,
    pub local_timestamp: DateTime<Utc>,
    pub remote_timestamp: DateTime<Utc>,
    pub message: String,
}

/// Validation errors
#[derive(Debug, Error, Clone, Serialize)]
pub enum ValidationError {
    #[error("Field '{field}' is required")]
    Required {
        field: String,
    },
    
    #[error("Field '{field}' must be at least {min} characters")]
    MinLength {
        field: String,
        min: usize,
    },
    
    #[error("Field '{field}' cannot exceed {max} characters")]
    MaxLength {
        field: String,
        max: usize,
    },
    
    #[error("Field '{field}' must be between {min} and {max}")]
    Range {
        field: String,
        min: String,
        max: String,
    },
    
    #[error("Field '{field}' contains invalid format: {reason}")]
    Format {
        field: String,
        reason: String,
    },
    
    #[error("Field '{field}' must be unique")]
    Unique {
        field: String,
    },
    
    #[error("Field '{field}' contains an invalid value: {reason}")]
    InvalidValue {
        field: String,
        reason: String,
    },
    
    #[error("Entity is invalid: {0}")]
    Entity(String),
    
    #[error("Relationship error: {0}")]
    Relationship(String),
    
    #[error("Validation error: {0}")]
    Custom(String),
}

impl ValidationError {
    pub fn required(field: &str) -> Self {
        Self::Required {
            field: field.to_string(),
        }
    }
    
    pub fn min_length(field: &str, min: usize) -> Self {
        Self::MinLength {
            field: field.to_string(),
            min,
        }
    }
    
    pub fn max_length(field: &str, max: usize) -> Self {
        Self::MaxLength {
            field: field.to_string(),
            max,
        }
    }
    
    pub fn range<T: fmt::Display>(field: &str, min: T, max: T) -> Self {
        Self::Range {
            field: field.to_string(),
            min: min.to_string(),
            max: max.to_string(),
        }
    }
    
    pub fn format(field: &str, reason: &str) -> Self {
        Self::Format {
            field: field.to_string(),
            reason: reason.to_string(),
        }
    }
    
    pub fn unique(field: &str) -> Self {
        Self::Unique {
            field: field.to_string(),
        }
    }
    
    pub fn invalid_value(field: &str, reason: &str) -> Self {
        Self::InvalidValue {
            field: field.to_string(),
            reason: reason.to_string(),
        }
    }
    
    pub fn entity(message: &str) -> Self {
        Self::Entity(message.to_string())
    }
    
    pub fn relationship(message: &str) -> Self {
        Self::Relationship(message.to_string())
    }
    
    pub fn custom(message: &str) -> Self {
        Self::Custom(message.to_string())
    }
}