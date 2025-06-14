// Declare submodules for the compression domain
pub mod types;
pub mod repository;
pub mod service;
pub mod worker;
pub mod compressors;
pub mod manager;

// Re-export key types including iOS-enhanced types
pub use types::{
    CompressionPriority, CompressionMethod, CompressionConfig, 
    CompressionResult, CompressionQueueEntry, CompressionQueueStatus,
    CompressionStats,
    // iOS-specific types
    IOSDeviceState, IOSThermalState, IOSAppState, IOSOptimizations, IOSWorkerStatus
};

pub use service::CompressionService;
pub use repository::CompressionRepository;
pub use manager::CompressionManager;
pub use worker::{CompressionWorker, CompressionWorkerMessage, WorkerStatus}; 