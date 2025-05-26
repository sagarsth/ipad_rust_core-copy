use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use crate::globals;
use std::path::Path;
use std::fs;

/// List of schema migration files in order
const MIGRATION_FILES: [&str; 22] = [
    "20240320000000_basic.sql",
    "20240404000000_cascade.sql",
    "20240407000000_tombstone.sql",
    "20240412000000_sync.sql",
    "20240417000000_nullable.sql",
    "20240420000000_add_evaluations.sql",
    "20240421000000_document_updates.sql",
    "20240422000000_add_sync_priority.sql",
    "20240423000000_add_temp_doc_link.sql",
    "20240730100000_add_sync_priority_to_activities_donors.sql",
    "20240801000000_add_document_ref_columns.sql",
    "20250421170655_add_total_files_skipped_to_compression_stats.sql",
    "20250422061436_update_document_type_compression_check.sql",
    "20250423000001_update_document_schema_fields.sql",
    "20250424000000_document_enhancements.sql",
    "20250425000000_add_revoked_tokens_table.sql",
    "20250502040000_merge_sync_types.sql",
    "20250503000000_standardize_sync_priority.sql",
    "20250503000001_update_donors_sync_priority.sql",
    "20250516000000_device_id.sql",
    "20250517000000_add_source_of_change_to_media_documents.sql",
    "20250523000000_create_export_jobs.sql",
];

/// Initialize the database with migrations (async version)
pub async fn initialize_database() -> FFIResult<()> {
    let pool = globals::get_db_pool()?;
    
    // Create migrations table if it doesn't exist
    create_migrations_table(&pool).await?;
    
    // Get last applied migration
    let last_migration = get_last_migration(&pool).await?;
    
    // Apply missing migrations
    apply_pending_migrations(&pool, last_migration).await?;
    
    Ok(())
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