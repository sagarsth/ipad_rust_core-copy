pub mod context;
pub mod service;
mod repository;
pub mod jwt;

// Re-export public items
pub use context::AuthContext;
pub use service::{AuthService, LoginResult};

// Export internal items for use within auth module
pub(crate) use repository::AuthRepository;