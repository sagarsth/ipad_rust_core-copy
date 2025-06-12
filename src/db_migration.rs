use crate::ffi::error::{FFIError, FFIResult};
use sqlx::SqlitePool;
use crate::globals;

// Embed the consolidated migration SQL file at compile time
const MIGRATION_CONSOLIDATED: &str = include_str!("../migrations/20240101000000_consolidated.sql");
const MIGRATION_COMPRESSION_STATUS_FIX: &str = include_str!("../migrations/20241201000000_fix_compression_status_constraints.sql");

// List of migrations with their names and SQL content.
// This now starts with the consolidated schema.
// Future migrations can be added here.
const MIGRATIONS: &[(&str, &str)] = &[
    ("20240101000000_consolidated.sql", MIGRATION_CONSOLIDATED),
    ("20241201000000_fix_compression_status_constraints.sql", MIGRATION_COMPRESSION_STATUS_FIX),
    // Add new migrations here in the future, for example:
    // ("20250601120000_new_feature.sql", include_str!("../migrations/20250601120000_new_feature.sql")),
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
