mod error;

pub use error::{DomainError, DbError, ServiceError, SyncError, ValidationError, SyncConflict};

/// Result type for database operations
pub type DbResult<T> = Result<T, DbError>;

/// Result type for domain operations
pub type DomainResult<T> = Result<T, DomainError>;

/// Result type for service operations
pub type ServiceResult<T> = Result<T, ServiceError>;

/// Result type for sync operations
pub type SyncResult<T> = Result<T, SyncError>;