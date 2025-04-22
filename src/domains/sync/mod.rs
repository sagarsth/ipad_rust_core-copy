pub mod types;
pub mod repository;
pub mod conflict_resolver;

// Re-exports
pub use types::*;

// Re-export key types/traits if needed
pub use types::{Tombstone, ChangeLogEntry, ChangeOperationType};
// Potentially re-export repository traits if used directly elsewhere
// pub use repository::{TombstoneRepository, ChangeLogRepository};