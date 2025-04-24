// Declare submodules for the compression domain
pub mod types;
pub mod repository;
pub mod service;
pub mod worker;
pub mod compressors;
pub mod manager;

// Re-export key types/traits if needed later
// pub use types::{CompressionMethod, CompressionQueueEntry, ...};
// pub use service::CompressionService;
// pub use worker::CompressionWorker; 