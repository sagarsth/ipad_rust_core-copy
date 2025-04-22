use crate::errors::DomainResult;
use async_trait::async_trait;
use sqlx::{Pool, Sqlite, query_as};
use uuid::Uuid;
use std::collections::HashMap;
use crate::errors::DbError;
use crate::errors::DomainError;

/// Dependency information
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Name of the table with dependent records
    pub table_name: String,
    
    /// Count of dependent records
    pub count: i64,
    
    /// Name of the foreign key column
    pub foreign_key_column: String,
    
    /// Whether the dependency is cascadable (ON DELETE CASCADE)
    pub is_cascadable: bool,
}

/// Trait for dependency checking
#[async_trait]
pub trait DependencyChecker: Send + Sync {
    /// Check for dependencies for an entity
    async fn check_dependencies(&self, table_name: &str, id: Uuid) -> DomainResult<Vec<Dependency>>;
    
    /// Get a simplified list of dependency tables
    async fn get_dependency_tables(&self, table_name: &str, id: Uuid) -> DomainResult<Vec<String>> {
        let dependencies = self.check_dependencies(table_name, id).await?;
        Ok(dependencies.into_iter().map(|dep| dep.table_name).collect())
    }
}

/// SQLite implementation of the DependencyChecker
pub struct SqliteDependencyChecker {
    pool: Pool<Sqlite>,
    /// Maps table name to its cascadable dependencies
    dependency_map: HashMap<String, Vec<(String, String, bool)>>,
}

impl SqliteDependencyChecker {
    /// Create a new SQLite dependency checker
    pub fn new(pool: Pool<Sqlite>) -> Self {
        let mut dependency_map = HashMap::new();
        
        // Define dependencies based on schema
        // Format: (table_name, [(dependent_table, foreign_key_column, is_cascadable)])
        
        // Strategic goals dependencies
        dependency_map.insert(
            "strategic_goals".to_string(), 
            vec![
                ("projects".to_string(), "strategic_goal_id".to_string(), false),
            ]
        );
        
        // Projects dependencies
        dependency_map.insert(
            "projects".to_string(), 
            vec![
                ("workshops".to_string(), "project_id".to_string(), false),
                ("activities".to_string(), "project_id".to_string(), true),
                ("livelihoods".to_string(), "project_id".to_string(), false),
                ("project_funding".to_string(), "project_id".to_string(), false),
            ]
        );
        
        // Workshops dependencies
        dependency_map.insert(
            "workshops".to_string(), 
            vec![
                ("workshop_participants".to_string(), "workshop_id".to_string(), true),
            ]
        );
        
        // Participants dependencies
        dependency_map.insert(
            "participants".to_string(), 
            vec![
                ("workshop_participants".to_string(), "participant_id".to_string(), true),
                ("livelihoods".to_string(), "participant_id".to_string(), true),
            ]
        );
        
        // Livelihoods dependencies
        dependency_map.insert(
            "livelihoods".to_string(), 
            vec![
                ("subsequent_grants".to_string(), "livelihood_id".to_string(), true),
            ]
        );
        
        // Donors dependencies
        dependency_map.insert(
            "donors".to_string(), 
            vec![
                ("project_funding".to_string(), "donor_id".to_string(), false),
            ]
        );

        // Document Types dependencies
        dependency_map.insert(
            "document_types".to_string(),
            vec![
                // MediaDocuments depend on DocumentTypes, but deletion is RESTRICTED by FK.
                // Hard deleting a DocumentType SHOULD be blocked if MediaDocuments use it.
                ("media_documents".to_string(), "type_id".to_string(), false),
            ]
        );

        // NOTE: We intentionally DO NOT add entries for other tables (projects, workshops, etc.)
        // pointing to media_documents here. The check for those dependencies is handled
        // differently in check_dependencies to allow the desired cascading behavior.
        
        Self { pool, dependency_map }
    }
}

/// Query result for dependency count
#[derive(Debug, sqlx::FromRow)]
struct DependencyCount {
    count: i64,
}

#[async_trait]
impl DependencyChecker for SqliteDependencyChecker {
    async fn check_dependencies(&self, table_name: &str, id: Uuid) -> DomainResult<Vec<Dependency>> {
        let mut dependencies = Vec::new();
        let id_str = id.to_string();

        // 1. Check defined dependencies from the map
        if let Some(dependent_tables) = self.dependency_map.get(table_name) {
            for (dependent_table, foreign_key, is_cascadable) in dependent_tables {
                // Build and execute query to count dependencies
                let query = format!(
                    "SELECT COUNT(*) as count FROM {} WHERE {} = ? AND deleted_at IS NULL", // Check only non-deleted dependents
                    dependent_table, 
                    foreign_key
                );
                
                let count_result: Result<DependencyCount, sqlx::Error> = query_as(&query)
                    .bind(&id_str)
                    .fetch_one(&self.pool)
                    .await;
                    
                let count = match count_result {
                    Ok(c) => c.count,
                    Err(sqlx::Error::RowNotFound) => 0, // Treat RowNotFound as 0 count
                    Err(e) => return Err(DomainError::Database(DbError::from(e))), // Propagate other errors
                };
                
                if count > 0 {
                    dependencies.push(Dependency {
                        table_name: dependent_table.clone(),
                        count: count,
                        foreign_key_column: foreign_key.clone(),
                        is_cascadable: *is_cascadable,
                    });
                }
            }
        }

        // 2. Special check for media_documents dependency for ALL tables EXCEPT document_types itself.
        // We want to know if documents exist, but NOT treat them as blocking for parents.
        // The service layer will handle the cascade.
        if table_name != "document_types" {
            let query = "SELECT COUNT(*) as count FROM media_documents WHERE related_table = ? AND related_id = ? AND deleted_at IS NULL";
            let count_result: Result<DependencyCount, sqlx::Error> = query_as(query)
                .bind(table_name) // Bind the parent table name
                .bind(&id_str)     // Bind the parent ID
                .fetch_one(&self.pool)
                .await;

            let count = match count_result {
                 Ok(c) => c.count,
                 Err(sqlx::Error::RowNotFound) => 0,
                 Err(e) => return Err(DomainError::Database(DbError::from(e))),
            };

            if count > 0 {
                // Add this dependency, but mark it as non-blocking (is_cascadable = true)
                // even though the FK isn't CASCADE. The service layer interprets this.
                dependencies.push(Dependency {
                    table_name: "media_documents".to_string(),
                    count: count,
                    foreign_key_column: "related_id".to_string(), // Indicate the relevant key
                    is_cascadable: true, // Signal to DeleteService this is handled by service-level cascade
                });
            }
        }
        
        Ok(dependencies)
    }
}