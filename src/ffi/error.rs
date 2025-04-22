// Content for src/ffi/error.rs
use std::fmt;
use serde::{Deserialize, Serialize};
// Use the re-exported path for SyncConflict
use crate::errors::{DomainError, DbError, ServiceError, SyncError, ValidationError, SyncConflict};
use sqlx; // Make sure sqlx is imported if used in DbError::Sqlx details

/// Error codes for FFI boundary
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorCode {
    // Success (no error)
    Success = 0,

    // General errors (1-99)
    Unknown = 1,
    InvalidArgument = 2,
    NullPointer = 3,
    InvalidUtf8 = 4,
    InvalidUuid = 5,
    InternalError = 6,

    // Database errors (100-199)
    DatabaseGeneral = 100,
    DatabaseNotFound = 101,
    DatabaseConflict = 102,
    DatabaseLocked = 103,
    DatabaseConnection = 104,
    DatabaseTransaction = 105,
    DatabaseMigration = 106,

    // Domain errors (200-299)
    DomainGeneral = 200,
    EntityNotFound = 201,
    AuthorizationFailed = 202,
    DependentRecordsExist = 203,
    ValidationFailed = 204,
    LwwConflict = 205,
    DeletedEntity = 206,
    FileError = 207,
    CompressionError = 208,

    // Service errors (300-399)
    ServiceGeneral = 300,
    DependenciesPreventDeletion = 301,
    UiError = 302,
    AuthenticationFailed = 303,
    SessionExpired = 304,
    PermissionDenied = 305,
    OfflineFeatureUnavailable = 306,
    RateLimitExceeded = 307,
    NetworkError = 308,
    ServiceUnavailable = 309,
    ConfigurationError = 310,
    ExternalServiceError = 311,

    // Sync errors (400-499)
    SyncGeneral = 400,
    SyncNetworkError = 401,
    SyncAuthenticationFailed = 402,
    SyncConflict = 403,
    SyncTombstoneConflict = 404,
    SyncServerError = 405,
    SyncRemoteEntityNotFound = 406,
    SyncEntityTypeMismatch = 407,
    SyncMissingRequiredFields = 408,
    SyncInvalidBatch = 409,
    SyncInterrupted = 410,
    SyncTimeout = 411,
    SyncInsufficientStorage = 412,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} ({})", self, *self as i32)
    }
}

/// Error type for FFI boundary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FFIError {
    /// Error code for programmatic handling
    pub code: ErrorCode,

    /// Human-readable error message
    pub message: String,

    /// Optional additional details (JSON string)
    pub details: Option<String>,
}

impl fmt::Display for FFIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(details) = &self.details {
            write!(f, "{}: {} ({})", self.code, self.message, details)
        } else {
            write!(f, "{}: {}", self.code, self.message)
        }
    }
}

impl std::error::Error for FFIError {}

impl FFIError {
    pub fn new(code: ErrorCode, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            details: None,
        }
    }

    pub fn with_details(code: ErrorCode, message: &str, details: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            details: Some(details.to_string()),
        }
    }

    pub fn unknown(message: &str) -> Self {
        Self::new(ErrorCode::Unknown, message)
    }

    pub fn invalid_argument(message: &str) -> Self {
        Self::new(ErrorCode::InvalidArgument, message)
    }

    // Helper for internal errors
    pub fn internal(message: String) -> Self {
         Self::new(ErrorCode::InternalError, &message)
    }

    pub fn success() -> Self {
        Self::new(ErrorCode::Success, "Success")
    }

    // Helper for converting ServiceError, commonly needed in FFI layer
    pub fn from_service_error(err: ServiceError) -> Self {
        err.into()
    }
}

// Implement From traits for converting domain errors to FFI errors

impl From<DbError> for FFIError {
    fn from(err: DbError) -> Self {
        match err {
            DbError::Sqlx(sqlx_err) => {
                Self::new(ErrorCode::DatabaseGeneral, &sqlx_err.to_string())
            },
            DbError::NotFound(entity, id) => {
                Self::with_details(
                    ErrorCode::DatabaseNotFound,
                    &format!("Record not found: {} with ID {}", entity, id),
                    &format!("{{\"entity\":\"{}\",\"id\":\"{}\"}}", entity, id)
                )
            },
            DbError::Conflict(msg) => {
                Self::new(ErrorCode::DatabaseConflict, &msg)
            },
            DbError::Locked => {
                Self::new(ErrorCode::DatabaseLocked, "Database is locked")
            },
            DbError::ConnectionPool(msg) => {
                Self::new(ErrorCode::DatabaseConnection, &msg)
            },
            DbError::Transaction(msg) => {
                Self::new(ErrorCode::DatabaseTransaction, &msg)
            },
            DbError::Migration(msg) => {
                Self::new(ErrorCode::DatabaseMigration, &msg)
            },
            DbError::Query(msg) => {
                 Self::new(ErrorCode::DatabaseGeneral, &msg)
            },
            DbError::Execution(msg) => {
                 Self::new(ErrorCode::DatabaseGeneral, &msg)
            },
            DbError::Other(msg) => {
                 Self::new(ErrorCode::DatabaseGeneral, &msg)
            }
        }
    }
}


// --- From<DomainError> for FFIError ---
impl From<DomainError> for FFIError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::Database(db_err) => {
                db_err.into() // Delegate to From<DbError>
            },
            DomainError::EntityNotFound(entity, id) => {
                Self::with_details(
                    ErrorCode::EntityNotFound,
                    &format!("Entity not found: {} with ID {}", entity, id),
                    &format!("{{\"entity\":\"{}\",\"id\":\"{}\"}}", entity, id.to_string())
                )
            },
            DomainError::AuthorizationFailed(msg) => {
                Self::new(ErrorCode::AuthorizationFailed, &msg)
            },
            DomainError::InvalidUuid(uuid_str) => {
                Self::with_details(
                    ErrorCode::InvalidUuid,
                    &format!("Invalid UUID: {}", uuid_str),
                    &format!("{{\"uuid\":\"{}\"}}", uuid_str)
                )
            },
            DomainError::DependentRecordsExist { entity_type, id, dependencies } => {
                let dependencies_json = serde_json::to_string(&dependencies)
                    .unwrap_or_else(|_| dependencies.join(", "));

                Self::with_details(
                    ErrorCode::DependentRecordsExist,
                    &format!("Cannot delete {} with ID {} due to dependent records", entity_type, id),
                    &format!("{{\"entity\":\"{}\",\"id\":\"{}\",\"dependencies\":{}}}",
                        entity_type, id.to_string(), dependencies_json)
                )
            },
            DomainError::Validation(val_err) => {
                val_err.into() // Delegate to From<ValidationError>
            },
            DomainError::Sync(sync_err) => {
                sync_err.into() // Delegate to From<SyncError>
            },
            DomainError::LwwConflict { entity_type, id, field } => {
                Self::with_details(
                    ErrorCode::LwwConflict,
                    &format!("LWW conflict in field {} of {} {}", field, entity_type, id),
                    &format!("{{\"entity\":\"{}\",\"id\":\"{}\",\"field\":\"{}\"}}",
                        entity_type, id.to_string(), field)
                )
            },
            DomainError::DeletedEntity(entity, id) => {
                Self::with_details(
                    ErrorCode::DeletedEntity,
                    &format!("Cannot perform operation on deleted entity: {} with ID {}", entity, id),
                    &format!("{{\"entity\":\"{}\",\"id\":\"{}\"}}", entity, id.to_string())
                )
            },
            DomainError::File(msg) => {
                Self::new(ErrorCode::FileError, &msg)
            },
            DomainError::Compression(msg) => {
                Self::new(ErrorCode::CompressionError, &msg)
            },
            DomainError::Internal(msg) => {
                Self::new(ErrorCode::InternalError, &msg)
            },
            DomainError::External(msg) => {
                Self::new(ErrorCode::InternalError, &format!("External error: {}", msg))
            },
        }
    }
}


// --- From<ServiceError> for FFIError ---
impl From<ServiceError> for FFIError {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Domain(domain_err) => {
                domain_err.into() // Delegate
            },
            ServiceError::DependenciesPreventDeletion(dependencies) => {
                let dependencies_json = serde_json::to_string(&dependencies)
                    .unwrap_or_else(|_| dependencies.join(", "));

                Self::with_details(
                    ErrorCode::DependenciesPreventDeletion,
                    &format!("Cannot delete record due to dependencies: {}", dependencies.join(", ")),
                    &format!("{{\"dependencies\":{}}}", dependencies_json)
                )
            },
            ServiceError::Ui(msg) => {
                Self::new(ErrorCode::UiError, &msg)
            },
            ServiceError::Authentication(msg) => {
                Self::new(ErrorCode::AuthenticationFailed, &msg)
            },
            ServiceError::SessionExpired => {
                Self::new(ErrorCode::SessionExpired, "Session expired")
            },
            ServiceError::PermissionDenied(msg) => {
                Self::new(ErrorCode::PermissionDenied, &msg)
            },
            ServiceError::OfflineFeatureUnavailable(feature) => {
                Self::with_details(
                    ErrorCode::OfflineFeatureUnavailable,
                    &format!("Feature not available in offline mode: {}", feature),
                    &format!("{{\"feature\":\"{}\"}}", feature)
                )
            },
            ServiceError::RateLimitExceeded => {
                Self::new(ErrorCode::RateLimitExceeded, "Rate limit exceeded")
            },
            ServiceError::Network(msg) => {
                Self::new(ErrorCode::NetworkError, &msg)
            },
            ServiceError::ServiceUnavailable(msg) => {
                Self::new(ErrorCode::ServiceUnavailable, &msg)
            },
            ServiceError::Configuration(msg) => {
                Self::new(ErrorCode::ConfigurationError, &msg)
            },
            ServiceError::ExternalService(msg) => {
                Self::new(ErrorCode::ExternalServiceError, &msg)
            },
            // Ensure all ServiceError variants are handled or add a catch-all
            // _ => Self::new(ErrorCode::ServiceGeneral, &err.to_string()),
        }
    }
}

// --- From<SyncError> for FFIError ---
impl From<SyncError> for FFIError {
    fn from(err: SyncError) -> Self {
        match err {
            SyncError::Network(msg) => {
                Self::new(ErrorCode::SyncNetworkError, &msg)
            },
            SyncError::AuthenticationFailed(msg) => {
                Self::new(ErrorCode::SyncAuthenticationFailed, &msg)
            },
            SyncError::RecordConflict(msg) => {
                Self::new(ErrorCode::SyncConflict, &msg)
            },
            SyncError::Conflict { conflict } => {
                let details = serde_json::to_string(&conflict)
                    .unwrap_or_else(|_| format!("{{\"message\":\"{}\"}}", conflict.message));

                Self::with_details(
                    ErrorCode::SyncConflict,
                    &format!("Conflict during sync: {}", conflict.message),
                    &details
                )
            },
            SyncError::TombstoneConflict { entity_type, id } => {
                Self::with_details(
                    ErrorCode::SyncTombstoneConflict,
                    &format!("Tombstone conflict: Entity {} with ID {} was previously deleted", entity_type, id),
                    &format!("{{\"entity\":\"{}\",\"id\":\"{}\"}}", entity_type, id.to_string())
                )
            },
            SyncError::ServerError(msg) => {
                Self::new(ErrorCode::SyncServerError, &msg)
            },
            SyncError::LocalDatabase(db_err) => {
                db_err.into() // Delegate
            },
            SyncError::RemoteEntityNotFound(msg) => {
                Self::new(ErrorCode::SyncRemoteEntityNotFound, &msg)
            },
            SyncError::EntityTypeMismatch(msg) => {
                Self::new(ErrorCode::SyncEntityTypeMismatch, &msg)
            },
            SyncError::MissingRequiredFields(msg) => {
                Self::new(ErrorCode::SyncMissingRequiredFields, &msg)
            },
            SyncError::InvalidBatch(msg) => {
                Self::new(ErrorCode::SyncInvalidBatch, &msg)
            },
            SyncError::Interrupted => {
                Self::new(ErrorCode::SyncInterrupted, "Sync interrupted")
            },
            SyncError::Timeout => {
                Self::new(ErrorCode::SyncTimeout, "Sync timeout")
            },
            SyncError::InsufficientStorage => {
                Self::new(ErrorCode::SyncInsufficientStorage, "Insufficient storage for sync")
            },
            SyncError::Other(msg) => {
                Self::new(ErrorCode::SyncGeneral, &msg)
            }
        }
    }
}

// --- From<ValidationError> for FFIError ---
impl From<ValidationError> for FFIError {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::Required { field } => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Field '{}' is required", field),
                    &format!("{{\"field\":\"{}\",\"type\":\"required\"}}", field)
                )
            },
            ValidationError::MinLength { field, min } => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Field '{}' must be at least {} characters", field, min),
                    &format!("{{\"field\":\"{}\",\"type\":\"min_length\",\"min\":{}}}", field, min)
                )
            },
            ValidationError::MaxLength { field, max } => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Field '{}' cannot exceed {} characters", field, max),
                    &format!("{{\"field\":\"{}\",\"type\":\"max_length\",\"max\":{}}}", field, max)
                )
            },
            ValidationError::Range { field, min, max } => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Field '{}' must be between {} and {}", field, min, max),
                    &format!("{{\"field\":\"{}\",\"type\":\"range\",\"min\":\"{}\",\"max\":\"{}\"}}",
                        field, min, max)
                )
            },
            ValidationError::Format { field, reason } => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Field '{}' contains invalid format: {}", field, reason),
                    &format!("{{\"field\":\"{}\",\"type\":\"format\",\"reason\":\"{}\"}}",
                        field, reason)
                )
            },
            ValidationError::Unique { field } => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Field '{}' must be unique", field),
                    &format!("{{\"field\":\"{}\",\"type\":\"unique\"}}", field)
                )
            },
            ValidationError::InvalidValue { field, reason } => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Field '{}' contains an invalid value: {}", field, reason),
                    &format!("{{\"field\":\"{}\",\"type\":\"invalid_value\",\"reason\":\"{}\"}}",
                        field, reason)
                )
            },
            ValidationError::Entity(msg) => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Entity is invalid: {}", msg),
                    &format!("{{\"type\":\"entity\",\"message\":\"{}\"}}", msg)
                )
            },
            ValidationError::Relationship(msg) => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &format!("Relationship error: {}", msg),
                    &format!("{{\"type\":\"relationship\",\"message\":\"{}\"}}", msg)
                )
            },
            ValidationError::Custom(msg) => {
                Self::with_details(
                    ErrorCode::ValidationFailed,
                    &msg,
                    &format!("{{\"type\":\"custom\",\"message\":\"{}\"}}", msg)
                )
            },
        }
    }
}

// Implement From<std::ffi::NulError> for FFIError
impl From<std::ffi::NulError> for FFIError {
    fn from(_: std::ffi::NulError) -> Self {
        // Using InvalidUtf8 might be slightly inaccurate, but it's close
        // Or define a specific ErrorCode::InternalCStringError if needed
        Self::new(ErrorCode::InvalidUtf8, "String contains null bytes, cannot create CString")
    }
}


// FFI-specific helper functions (keep these)

/// Helper function to convert any Rust error to FFIError for C boundary
pub fn to_ffi_error<E: std::error::Error + 'static>(error: &E) -> FFIError {
    // Explicitly cast to &dyn Error before downcasting.
    let error_trait_object = error as &dyn std::error::Error;

    if let Some(ffi_err) = error_trait_object.downcast_ref::<FFIError>() {
        return ffi_err.clone();
    }
    // Handle DbError specifically without cloning
    if let Some(db_err) = error_trait_object.downcast_ref::<DbError>() {
        return match db_err {
            DbError::Sqlx(sqlx_err) => {
                FFIError::new(ErrorCode::DatabaseGeneral, &sqlx_err.to_string())
            },
            DbError::NotFound(entity, id) => {
                FFIError::with_details(
                    ErrorCode::DatabaseNotFound,
                    &format!("Record not found: {} with ID {}", entity, id),
                    &format!("{{\"entity\":\"{}\",\"id\":\"{}\"}}", entity, id)
                )
            },
            DbError::Conflict(msg) => {
                FFIError::new(ErrorCode::DatabaseConflict, msg)
            },
            DbError::Locked => {
                FFIError::new(ErrorCode::DatabaseLocked, "Database is locked")
            },
            DbError::ConnectionPool(msg) => {
                FFIError::new(ErrorCode::DatabaseConnection, msg)
            },
            DbError::Transaction(msg) => {
                FFIError::new(ErrorCode::DatabaseTransaction, msg)
            },
            DbError::Migration(msg) => {
                FFIError::new(ErrorCode::DatabaseMigration, msg)
            },
            DbError::Query(msg) => {
                 FFIError::new(ErrorCode::DatabaseGeneral, msg)
            },
            DbError::Execution(msg) => {
                 FFIError::new(ErrorCode::DatabaseGeneral, msg)
            },
            DbError::Other(msg) => {
                 FFIError::new(ErrorCode::DatabaseGeneral, msg)
            }
        };
    }
    // Handle other cloneable errors
    if let Some(service_err) = error_trait_object.downcast_ref::<ServiceError>() {
        return service_err.clone().into();
    }
    if let Some(domain_err) = error_trait_object.downcast_ref::<DomainError>() {
        return domain_err.clone().into();
    }
    if let Some(sync_err) = error_trait_object.downcast_ref::<SyncError>() {
        return sync_err.clone().into();
    }
    if let Some(validation_err) = error_trait_object.downcast_ref::<ValidationError>() {
        return validation_err.clone().into();
    }

    // Default case for unknown errors
    FFIError::unknown(&error.to_string())
}


/// Convert an FFIError to its i32 code representation for C
pub fn error_code_to_i32(error: &FFIError) -> i32 {
    error.code as i32
}

/// Create a success code (0)
pub fn success_code() -> i32 {
    ErrorCode::Success as i32
}

// Result type alias for FFI functions
pub type FFIResult<T> = Result<T, FFIError>;
