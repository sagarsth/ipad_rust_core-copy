use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use crate::globals;

// Embed all migration SQL files at compile time
const MIGRATION_BASIC: &str = include_str!("../migrations/20240320000000_basic.sql");
const MIGRATION_CASCADE: &str = include_str!("../migrations/20240404000000_cascade.sql");
const MIGRATION_TOMBSTONE: &str = include_str!("../migrations/20240407000000_tombstone.sql");
const MIGRATION_SYNC: &str = include_str!("../migrations/20240412000000_sync.sql");
const MIGRATION_NULLABLE: &str = include_str!("../migrations/20240417000000_nullable.sql");
const MIGRATION_EVALUATIONS: &str = include_str!("../migrations/20240420000000_add_evaluations.sql");
const MIGRATION_DOC_UPDATES: &str = include_str!("../migrations/20240421000000_document_updates.sql");
const MIGRATION_SYNC_PRIORITY: &str = include_str!("../migrations/20240422000000_add_sync_priority.sql");
const MIGRATION_TEMP_DOC_LINK: &str = include_str!("../migrations/20240423000000_add_temp_doc_link.sql");
const MIGRATION_SYNC_PRIORITY_ACTIVITIES: &str = include_str!("../migrations/20240730100000_add_sync_priority_to_activities_donors.sql");
const MIGRATION_DOC_REF_COLUMNS: &str = include_str!("../migrations/20240801000000_add_document_ref_columns.sql");
const MIGRATION_COMPRESSION_STATS: &str = include_str!("../migrations/20250421170655_add_total_files_skipped_to_compression_stats.sql");
const MIGRATION_DOC_TYPE_COMPRESSION: &str = include_str!("../migrations/20250422061436_update_document_type_compression_check.sql");
const MIGRATION_DOC_SCHEMA_FIELDS: &str = include_str!("../migrations/20250423000001_update_document_schema_fields.sql");
const MIGRATION_DOC_ENHANCEMENTS: &str = include_str!("../migrations/20250424000000_document_enhancements.sql");
const MIGRATION_REVOKED_TOKENS: &str = include_str!("../migrations/20250425000000_add_revoked_tokens_table.sql");
const MIGRATION_MERGE_SYNC_TYPES: &str = include_str!("../migrations/20250502040000_merge_sync_types.sql");
const MIGRATION_STANDARDIZE_SYNC: &str = include_str!("../migrations/20250503000000_standardize_sync_priority.sql");
const MIGRATION_UPDATE_DONORS_SYNC: &str = include_str!("../migrations/20250503000001_update_donors_sync_priority.sql");
const MIGRATION_DEVICE_ID: &str = include_str!("../migrations/20250516000000_device_id.sql");
const MIGRATION_SOURCE_OF_CHANGE: &str = include_str!("../migrations/20250517000000_add_source_of_change_to_media_documents.sql");
const MIGRATION_EXPORT_JOBS: &str = include_str!("../migrations/20250523000000_create_export_jobs.sql");
const MIGRATION_FIX_FK_CONSTRAINTS: &str = include_str!("../migrations/20250523050000_fix_foreign_key_constraints.sql");
const MIGRATION_ALLOW_NULL_USER_IDS: &str = include_str!("../migrations/20250524000000_allow_null_user_ids.sql");

// List of migrations with their names and SQL content
const MIGRATIONS: &[(&str, &str)] = &[
    ("20240320000000_basic.sql", MIGRATION_BASIC),
    ("20240404000000_cascade.sql", MIGRATION_CASCADE),
    ("20240407000000_tombstone.sql", MIGRATION_TOMBSTONE),
    ("20240412000000_sync.sql", MIGRATION_SYNC),
    ("20240417000000_nullable.sql", MIGRATION_NULLABLE),
    ("20240420000000_add_evaluations.sql", MIGRATION_EVALUATIONS),
    ("20240421000000_document_updates.sql", MIGRATION_DOC_UPDATES),
    ("20240422000000_add_sync_priority.sql", MIGRATION_SYNC_PRIORITY),
    ("20240423000000_add_temp_doc_link.sql", MIGRATION_TEMP_DOC_LINK),
    ("20240730100000_add_sync_priority_to_activities_donors.sql", MIGRATION_SYNC_PRIORITY_ACTIVITIES),
    ("20240801000000_add_document_ref_columns.sql", MIGRATION_DOC_REF_COLUMNS),
    ("20250421170655_add_total_files_skipped_to_compression_stats.sql", MIGRATION_COMPRESSION_STATS),
    ("20250422061436_update_document_type_compression_check.sql", MIGRATION_DOC_TYPE_COMPRESSION),
    ("20250423000001_update_document_schema_fields.sql", MIGRATION_DOC_SCHEMA_FIELDS),
    ("20250424000000_document_enhancements.sql", MIGRATION_DOC_ENHANCEMENTS),
    ("20250425000000_add_revoked_tokens_table.sql", MIGRATION_REVOKED_TOKENS),
    ("20250502040000_merge_sync_types.sql", MIGRATION_MERGE_SYNC_TYPES),
    ("20250503000000_standardize_sync_priority.sql", MIGRATION_STANDARDIZE_SYNC),
    ("20250503000001_update_donors_sync_priority.sql", MIGRATION_UPDATE_DONORS_SYNC),
    ("20250516000000_device_id.sql", MIGRATION_DEVICE_ID),
    ("20250517000000_add_source_of_change_to_media_documents.sql", MIGRATION_SOURCE_OF_CHANGE),
    ("20250523000000_create_export_jobs.sql", MIGRATION_EXPORT_JOBS),
    ("20250523050000_fix_foreign_key_constraints.sql", MIGRATION_FIX_FK_CONSTRAINTS),
    ("20250524000000_allow_null_user_ids.sql", MIGRATION_ALLOW_NULL_USER_IDS),
];

/// Initialize the database with migrations (async version)
pub async fn initialize_database() -> FFIResult<()> {
    println!("🗄️ [DB_MIGRATION] Starting database migration process...");
    
    let pool = globals::get_db_pool()
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to get database pool: {}", e);
            e
        })?;
    
    println!("✅ [DB_MIGRATION] Database pool obtained");
    
    // Create migrations table if it doesn't exist
    println!("🔧 [DB_MIGRATION] Creating migrations table...");
    create_migrations_table(&pool).await
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to create migrations table: {}", e);
            e
        })?;
    
    // Get last applied migration
    println!("🔍 [DB_MIGRATION] Checking last applied migration...");
    let last_migration = get_last_migration(&pool).await
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to get last migration: {}", e);
            e
        })?;
    
    match &last_migration {
        Some(name) => println!("📋 [DB_MIGRATION] Last applied migration: {}", name),
        None => println!("📋 [DB_MIGRATION] No migrations applied yet"),
    }
    
    // Apply missing migrations
    println!("🚀 [DB_MIGRATION] Applying pending migrations...");
    apply_pending_migrations(&pool, last_migration).await
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to apply migrations: {}", e);
            e
        })?;
    
    println!("🎉 [DB_MIGRATION] Database migration process completed successfully");
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
    println!("🔍 [DB_MIGRATION] Determining pending migrations...");
    let pending_migrations = get_pending_migrations(last_migration)
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to get pending migrations: {}", e);
            e
        })?;
    
    if pending_migrations.is_empty() {
        println!("✅ [DB_MIGRATION] No pending migrations to apply");
        return Ok(());
    }
    
    println!("📋 [DB_MIGRATION] Found {} pending migrations", pending_migrations.len());
    for (name, _) in &pending_migrations {
        println!("  📄 [DB_MIGRATION] Pending: {}", name);
    }
    
    // Begin transaction
    println!("🔄 [DB_MIGRATION] Beginning transaction...");
    let mut tx = pool.begin().await
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to begin transaction: {}", e);
            FFIError::internal(format!("Failed to begin transaction: {}", e))
        })?;
    
    // Apply each migration
    for (migration_name, migration_sql) in pending_migrations {
        println!("🚀 [DB_MIGRATION] Applying migration: {}", migration_name);
        
        // Apply migration
        sqlx::query(migration_sql)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                println!("❌ [DB_MIGRATION] Failed to apply migration {}: {}", migration_name, e);
                FFIError::internal(format!("Failed to apply migration {}: {}", migration_name, e))
            })?;
        
        println!("✅ [DB_MIGRATION] Migration {} applied successfully", migration_name);
        
        // Record migration
        let now = chrono::Utc::now().to_rfc3339();
        println!("📝 [DB_MIGRATION] Recording migration {} in migrations table", migration_name);
        sqlx::query(
            "INSERT INTO migrations (name, applied_at) VALUES (?, ?)"
        )
        .bind(migration_name)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to record migration {}: {}", migration_name, e);
            FFIError::internal(format!("Failed to record migration {}: {}", migration_name, e))
        })?;
        
        // Log migration
        println!("✅ [DB_MIGRATION] Migration {} recorded successfully", migration_name);
    }
    
    // Commit transaction
    println!("💾 [DB_MIGRATION] Committing transaction...");
    tx.commit().await
        .map_err(|e| {
            println!("❌ [DB_MIGRATION] Failed to commit transaction: {}", e);
            FFIError::internal(format!("Failed to commit transaction: {}", e))
        })?;
    
    println!("🎉 [DB_MIGRATION] All migrations applied and committed successfully");
    Ok(())
}

/// Determine which migrations need to be applied
fn get_pending_migrations(last_migration: Option<String>) -> FFIResult<Vec<(&'static str, &'static str)>> {
    let mut pending = Vec::new();
    let mut should_include = last_migration.is_none();
    
    for &(migration_name, migration_sql) in MIGRATIONS {
        if should_include {
            pending.push((migration_name, migration_sql));
        } else if Some(migration_name.to_string()) == last_migration {
            // Found the last applied migration, include all subsequent ones
            should_include = true;
        }
    }
    
    Ok(pending)
}