pub mod types;
pub mod service;
pub mod repository;

// Re-export main items for other domains to use
pub use types::User;
pub use service::UserService;
pub use repository::UserRepository;
