use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use crate::globals;
use std::path::Path;
use std::fs;
use tokio::runtime::Runtime;

/// List of schema migration files in order
const MIGRATION_FILES: [&str; 4] = [
    "v1_schema.sql",
    "v2_schema.sql",
    "v3_tombstone.sql",
    "v4_syncapi.sql",
];

/// Initialize the database with migrations (synchronous wrapper)
pub fn initialize_database() -> FFIResult<()> {
    // Create a temporary runtime to block on async operations
    let rt = Runtime::new().map_err(|e| FFIError::internal(format!("Failed to create Tokio runtime: {}", e)))?;

    // Block on the async initialization logic
    rt.block_on(async {
        let pool = globals::get_db_pool()?;
        
        // Create migrations table if it doesn't exist
        create_migrations_table(&pool).await?;
        
        // Get last applied migration
        let last_migration = get_last_migration(&pool).await?;
        
        // Apply missing migrations
        apply_pending_migrations(&pool, last_migration).await?;
        
        Ok(())
    })
}

/// Create migrations table if it doesn't exist
async fn create_migrations_table(pool: &SqlitePool) -> FFIResult<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS migrations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        )"
    )
    .execute(pool)
    .await
    .map_err(|e| FFIError::internal(format!("Failed to create migrations table: {}", e)))?;
    
    Ok(())
}

/// Get the last applied migration
async fn get_last_migration(pool: &SqlitePool) -> FFIResult<Option<String>> {
    let result = sqlx::query_scalar::<_, String>(
        "SELECT name FROM migrations ORDER BY id DESC LIMIT 1"
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| FFIError::internal(format!("Failed to get last migration: {}", e)))?;
    
    Ok(result)
}

/// Apply pending migrations
async fn apply_pending_migrations(pool: &SqlitePool, last_migration: Option<String>) -> FFIResult<()> {
    // Determine which migrations need to be applied
    let pending_migrations = get_pending_migrations(last_migration)?;
    
    if pending_migrations.is_empty() {
        return Ok(());
    }
    
    // Begin transaction
    let mut tx = pool.begin().await
        .map_err(|e| FFIError::internal(format!("Failed to begin transaction: {}", e)))?;
    
    // Apply each migration (Iterate by consuming the Vec)
    for migration_name in pending_migrations { 
        let migration_name_clone = migration_name.clone(); // Clone for the error closure
        // Load migration SQL
        let migration_sql = load_migration_file(&migration_name)?; // Sync call
        
        // Apply migration
        sqlx::query(&migration_sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| FFIError::internal(format!("Failed to apply migration {}: {}", migration_name_clone, e)))?; // Use clone here
        
        let migration_name_clone_2 = migration_name.clone(); // Clone again for the second error closure
        // Record migration
        let now = chrono::Utc::now().to_rfc3339(); // Ensure chrono is in scope via use
        sqlx::query(
            "INSERT INTO migrations (name, applied_at) VALUES (?, ?)"
        )
        .bind(&migration_name) // Borrowing original String is fine for bind
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| FFIError::internal(format!("Failed to record migration {}: {}", migration_name_clone_2, e)))?; // Use clone here
        
        // Log migration
        println!("Applied migration: {}", migration_name); // Use original here is fine
    }
    
    // Commit transaction
    tx.commit().await
        .map_err(|e| FFIError::internal(format!("Failed to commit transaction: {}", e)))?;
    
    Ok(())
}

/// Determine which migrations need to be applied
fn get_pending_migrations(last_migration: Option<String>) -> FFIResult<Vec<String>> {
    let mut pending = Vec::new();
    let mut should_include = last_migration.is_none();
    
    for &migration_name in &MIGRATION_FILES {
        if should_include {
            pending.push(migration_name.to_string());
        } else if Some(migration_name.to_string()) == last_migration {
            // Found the last applied migration, include all subsequent ones
            should_include = true;
        }
    }
    
    Ok(pending)
}

/// Load migration file from the migrations directory
fn load_migration_file(filename: &str) -> FFIResult<String> {
    // In a real app, you'd embed these as resources or load from a specific path
    // This is a simplified example
    let migrations_dir = std::env::var("MIGRATIONS_DIR")
        .unwrap_or_else(|_| "./migrations".to_string());
    
    let path = Path::new(&migrations_dir).join(filename);
    
    fs::read_to_string(&path)
        .map_err(|e| FFIError::internal(format!("Failed to read migration file {}: {}", filename, e)))
}