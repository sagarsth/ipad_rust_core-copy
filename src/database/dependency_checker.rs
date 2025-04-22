use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use crate::error::DbError;
use crate::database::DbConnection;

/// Manages checking for foreign key dependencies in the database
///
/// # Example
///
/// ```rust
/// use crate::database::{DbConnection, DependencyChecker};
/// use crate::domains::core::repository::DeleteResult;
/// use crate::error::DomainError;
/// use uuid::Uuid;
/// use crate::domains::user::types::User;
///
/// // In your repository implementation:
/// fn hard_delete(&self, id: Uuid, user: &User, conn: &mut DbConnection) -> Result<(), DbError> {
///     // Create a dependency checker
///     let checker = DependencyChecker::new(conn);
///     
///     // Get the table name from your entity
///     let table_name = "projects";  // or T::table_name() in generic context
///     
///     // Check if it can be hard deleted
///     if !checker.can_hard_delete(table_name, &id)? {
///         // Get the dependencies to report in the error
///         let dependencies = checker.get_dependency_table_names(table_name, &id)?;
///         
///         return Err(DbError::Conflict(format!(
///             "Cannot delete {} with ID {} due to dependencies: {}",
///             table_name, id, dependencies.join(", ")
///         )));
///     }
///     
///     // Proceed with the hard delete...
///     // ...
///     
///     Ok(())
/// }
///
/// // In your dependency-aware service method:
/// fn delete_with_dependency_check(
///     &self, 
///     id: Uuid, 
///     user: &User, 
///     fallback_to_soft_delete: bool,
///     conn: &mut DbConnection
/// ) -> Result<DeleteResult, DomainError> {
///     let checker = DependencyChecker::new(conn);
///     let table_name = "projects";
///     
///     // Get all dependencies with counts
///     let dependencies = checker.check_dependencies(table_name, &id)?;
///     
///     if dependencies.is_empty() {
///         // No dependencies, proceed with hard delete
///         self.hard_delete(id, user, conn)?;
///         Ok(DeleteResult::HardDeleted)
///     } else if fallback_to_soft_delete {
///         // Has dependencies but can fall back to soft delete
///         self.soft_delete(id, user, conn)?;
///         let dep_tables: Vec<String> = dependencies.keys().cloned().collect();
///         Ok(DeleteResult::SoftDeleted { dependencies: dep_tables })
///     } else {
///         // Has dependencies and cannot fall back
///         let dep_tables: Vec<String> = dependencies.keys().cloned().collect();
///         Ok(DeleteResult::DependenciesPrevented { dependencies: dep_tables })
///     }
/// }
/// ```
pub struct DependencyChecker<'a> {
    conn: &'a DbConnection,
    cascade_map: HashMap<String, HashSet<String>>,
}

impl<'a> DependencyChecker<'a> {
    /// Create a new dependency checker with the given database connection
    pub fn new(conn: &'a DbConnection) -> Self {
        let mut checker = Self {
            conn,
            cascade_map: HashMap::new(),
        };
        
        // Initialize the cascade map
        checker.init_cascade_map();
        
        checker
    }
    
    /// Initialize the map of tables that use CASCADE delete
    fn init_cascade_map(&mut self) {
        // These values should reflect your database schema
        self.add_cascade("projects", &["activities"]);
        self.add_cascade("livelihoods", &["subsequent_grants"]);
        
        // Note: In v2 schema, these are now RESTRICT, not CASCADE
        // - strategic_goals -> projects
        // - projects -> workshops
        // - projects -> livelihoods
        // - projects -> project_funding
        // - workshops -> workshop_participants
        // - participants -> workshop_participants
    }
    
    /// Add a CASCADE relationship to the map
    fn add_cascade(&mut self, from_table: &str, to_tables: &[&str]) {
        let entry = self.cascade_map
            .entry(from_table.to_string())
            .or_insert_with(HashSet::new);
            
        for table in to_tables {
            entry.insert(table.to_string());
        }
    }
    
    /// Get tables that have foreign keys to the given table
    pub fn get_dependent_tables(&self, table_name: &str) -> Vec<String> {
        // This is a static mapping of table relationships
        // Could be enhanced to read from database metadata
        
        match table_name {
            "strategic_goals" => vec!["projects".to_string()],
            "projects" => vec![
                "activities".to_string(),
                "workshops".to_string(),
                "livelihoods".to_string(),
                "project_funding".to_string()
            ],
            "participants" => vec![
                "workshop_participants".to_string(),
                "livelihoods".to_string()
            ],
            "workshops" => vec!["workshop_participants".to_string()],
            "livelihoods" => vec!["subsequent_grants".to_string()],
            "donors" => vec!["project_funding".to_string()],
            _ => Vec::new(),
        }
    }
    
    /// Check if a table uses CASCADE delete from the referenced table
    pub fn uses_cascade_delete(&self, child_table: &str, parent_table: &str) -> bool {
        self.cascade_map
            .get(parent_table)
            .map(|tables| tables.contains(child_table))
            .unwrap_or(false)
    }
    
    /// Check if a record has dependent records in other tables
    pub fn check_dependencies(
        &self,
        table_name: &str,
        id: &Uuid
    ) -> Result<HashMap<String, i64>, DbError> {
        let dependent_tables = self.get_dependent_tables(table_name);
        let mut dependencies = HashMap::new();
        
        for table in dependent_tables {
            // Skip tables that use CASCADE delete
            if self.uses_cascade_delete(&table, table_name) {
                continue;
            }
            
            // Check for records referencing this ID
            let count = self.conn.query_one::<i64>(
                &format!("SELECT COUNT(*) FROM {} WHERE {}_id = $1", table, table_name),
                &[id]
            )?;
            
            if count > 0 {
                dependencies.insert(table, count);
            }
        }
        
        Ok(dependencies)
    }
    
    /// Get a flat list of table names that have dependencies
    pub fn get_dependency_table_names(
        &self,
        table_name: &str,
        id: &Uuid
    ) -> Result<Vec<String>, DbError> {
        let dependencies = self.check_dependencies(table_name, id)?;
        
        Ok(dependencies.keys()
            .map(|k| k.clone())
            .collect())
    }
    
    /// Check if a record can be safely hard deleted
    pub fn can_hard_delete(
        &self,
        table_name: &str,
        id: &Uuid
    ) -> Result<bool, DbError> {
        let dependencies = self.check_dependencies(table_name, id)?;
        Ok(dependencies.is_empty())
    }
} 