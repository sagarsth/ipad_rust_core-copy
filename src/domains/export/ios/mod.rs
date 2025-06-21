pub mod memory;
pub mod background_v2;

pub use memory::{
    MemoryPressureObserver,
    DeviceCapabilities,
    AdaptiveBuffer,
    ThermalMonitor,
    BackgroundTaskManager,
    MemoryPool,
    ios_memory_available,
    ios_device_tier,
    ios_active_processor_count,
};

pub use background_v2::{
    ModernBackgroundExporter,
    BGProcessingTask,
    ExportCheckpoint,
    CheckpointManager,
};