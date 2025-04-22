// Re-export DbConnection from the infrastructure layer
pub use crate::infrastructure::database::connection::DbConnection;

// Export the dependency checker
pub mod dependency_checker;
pub use dependency_checker::DependencyChecker; 