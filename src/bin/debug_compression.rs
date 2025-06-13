use std::env;
use std::path::Path;
use sqlx::{SqlitePool, Row};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Compression Debug Tool");
    println!("========================");
    
    // Try to find the database
    let db_path = find_database_path().unwrap_or_else(|| {
        eprintln!("❌ Could not find actionaid.db database");
        eprintln!("   Searched in:");
        eprintln!("   - ./storage/actionaid.db");
        eprintln!("   - ./actionaid.db");
        eprintln!("   - $IOS_DOCUMENTS_DIR/actionaid.db");
        eprintln!("   - iOS Simulator directories");
        std::process::exit(1);
    });
    
    println!("📍 Found database: {}", db_path);
    
    // Connect to database
    let database_url = format!("sqlite:{}", db_path);
    let pool = SqlitePool::connect(&database_url).await?;
    
    // Run debug analysis
    get_compression_overview(&pool).await?;
    get_compressed_documents(&pool).await?;
    get_failed_compressions(&pool).await?;
    get_compression_queue_status(&pool).await?;
    get_document_types_analysis(&pool).await?;
    check_storage_directory(&pool).await?;
    
    println!("\n✅ DEBUG SESSION COMPLETED");
    println!("==========================");
    
    Ok(())
}

fn find_database_path() -> Option<String> {
    // Check current directory first
    let candidates = vec![
        "./storage/actionaid.db",
        "./actionaid.db",
    ];
    
    for path in &candidates {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    
    // Check iOS Documents directory if set
    if let Ok(ios_docs) = env::var("IOS_DOCUMENTS_DIR") {
        let db_path = format!("{}/actionaid.db", ios_docs);
        if Path::new(&db_path).exists() {
            return Some(db_path);
        }
    }
    
    // Try to find in iOS simulator directories
    if let Ok(home) = env::var("HOME") {
        use std::process::Command;
        
        let output = Command::new("find")
            .args(&[
                &format!("{}/Library/Developer/CoreSimulator/Devices", home),
                "-name", "actionaid.db",
                "-type", "f"
            ])
            .output()
            .ok()?;
        
        if output.status.success() {
            let db_path = String::from_utf8(output.stdout).ok()?;
            let db_path = db_path.trim();
            if !db_path.is_empty() {
                return Some(db_path.to_string());
            }
        }
    }
    
    None
}

async fn get_compression_overview(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 COMPRESSION OVERVIEW");
    println!("========================");
    
    let rows = sqlx::query(r#"
        SELECT 
            compression_status,
            COUNT(*) as count,
            SUM(size_bytes) as total_original_size,
            SUM(CASE WHEN compressed_size_bytes IS NOT NULL THEN compressed_size_bytes ELSE 0 END) as total_compressed_size
        FROM media_documents 
        WHERE file_path != 'ERROR'
        GROUP BY compression_status
        ORDER BY count DESC
    "#)
    .fetch_all(pool)
    .await?;
    
    println!("\n📊 Status Breakdown:");
    let mut total_docs = 0;
    let mut total_original = 0i64;
    let mut total_compressed = 0i64;
    
    for row in &rows {
        let status: String = row.get("compression_status");
        let count: i64 = row.get("count");
        let orig_size: Option<i64> = row.get("total_original_size");
        let comp_size: Option<i64> = row.get("total_compressed_size");
        
        total_docs += count;
        total_original += orig_size.unwrap_or(0);
        total_compressed += comp_size.unwrap_or(0);
        
        println!("   {:<12}: {:>3} documents | {:>10} original | {:>10} compressed", 
                 status, count, 
                 format_bytes(orig_size.unwrap_or(0)), 
                 format_bytes(comp_size.unwrap_or(0)));
    }
    
    println!("\n📈 Totals: {} documents | {} original | {} compressed", 
             total_docs, format_bytes(total_original), format_bytes(total_compressed));
    
    if total_compressed > 0 && total_original > 0 {
        let savings = total_original - total_compressed;
        let percentage = (savings as f64 / total_original as f64) * 100.0;
        println!("💾 Space Saved: {} ({:.1}%)", format_bytes(savings), percentage);
    }
    
    Ok(())
}

async fn get_compressed_documents(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n✅ SUCCESSFULLY COMPRESSED DOCUMENTS");
    println!("=====================================");
    
    let rows = sqlx::query(r#"
        SELECT 
            id,
            original_filename,
            mime_type,
            size_bytes,
            compressed_size_bytes,
            file_path,
            compressed_file_path,
            created_at,
            related_table
        FROM media_documents 
        WHERE compression_status = 'completed' 
            AND compressed_file_path IS NOT NULL
            AND file_path != 'ERROR'
        ORDER BY created_at DESC
        LIMIT 20
    "#)
    .fetch_all(pool)
    .await?;
    
    if rows.is_empty() {
        println!("❌ No compressed documents found!");
        return Ok(());
    }
    
    println!("\n🎯 Found {} compressed documents (showing first 20):", rows.len());
    
    for row in rows {
        let doc_id: String = row.get("id");
        let filename: String = row.get("original_filename");
        let mime_type: String = row.get("mime_type");
        let orig_size: i64 = row.get("size_bytes");
        let comp_size: Option<i64> = row.get("compressed_size_bytes");
        let orig_path: String = row.get("file_path");
        let comp_path: Option<String> = row.get("compressed_file_path");
        let created: String = row.get("created_at");
        let table: String = row.get("related_table");
        
        let comp_size = comp_size.unwrap_or(orig_size);
        let savings = orig_size - comp_size;
        let percentage = if orig_size > 0 { (savings as f64 / orig_size as f64) * 100.0 } else { 0.0 };
        
        println!("\n📄 {}", filename);
        println!("   🆔 ID: {}...", &doc_id[..8.min(doc_id.len())]);
        println!("   🗂️ Type: {} ({})", mime_type, table);
        println!("   📏 Size: {} → {}", format_bytes(orig_size), format_bytes(comp_size));
        println!("   💾 Saved: {} ({:.1}%)", format_bytes(savings), percentage);
        println!("   📁 Original: {}", orig_path);
        if let Some(comp_path) = comp_path {
            println!("   🗜️ Compressed: {}", comp_path);
        }
        println!("   📅 Created: {}", created);
    }
    
    Ok(())
}

async fn get_failed_compressions(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n❌ FAILED COMPRESSIONS");
    println!("======================");
    
    let rows = sqlx::query(r#"
        SELECT 
            id,
            original_filename,
            mime_type,
            size_bytes,
            file_path,
            error_message,
            has_error
        FROM media_documents 
        WHERE compression_status = 'failed' 
            OR has_error = 1
            AND file_path != 'ERROR'
        ORDER BY created_at DESC
        LIMIT 10
    "#)
    .fetch_all(pool)
    .await?;
    
    if rows.is_empty() {
        println!("✅ No failed compressions found!");
        return Ok(());
    }
    
    println!("\n🚨 Found {} failed documents (showing first 10):", rows.len());
    for row in rows {
        let doc_id: String = row.get("id");
        let filename: String = row.get("original_filename");
        let mime_type: String = row.get("mime_type");
        let size: i64 = row.get("size_bytes");
        let path: String = row.get("file_path");
        let error: Option<String> = row.get("error_message");
        
        println!("\n📄 {}", filename);
        println!("   🆔 ID: {}...", &doc_id[..8.min(doc_id.len())]);
        println!("   🗂️ Type: {}", mime_type);
        println!("   📏 Size: {}", format_bytes(size));
        println!("   ❌ Error: {}", error.unwrap_or_else(|| "Unknown error".to_string()));
        println!("   📁 Path: {}", path);
    }
    
    Ok(())
}

async fn get_compression_queue_status(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔄 COMPRESSION QUEUE STATUS");
    println!("===========================");
    
    // Check if compression_queue table exists
    let table_exists = sqlx::query(r#"
        SELECT name FROM sqlite_master WHERE type='table' AND name='compression_queue'
    "#)
    .fetch_optional(pool)
    .await?;
    
    if table_exists.is_none() {
        println!("ℹ️ No compression_queue table found");
        return Ok(());
    }
    
    let rows = sqlx::query(r#"
        SELECT 
            document_id,
            priority,
            status,
            queued_at,
            started_at,
            completed_at,
            error_message,
            attempts
        FROM compression_queue 
        ORDER BY queued_at DESC
        LIMIT 20
    "#)
    .fetch_all(pool)
    .await?;
    
    if rows.is_empty() {
        println!("📭 Queue is empty");
        return Ok(());
    }
    
    println!("\n📋 Found {} queue entries (showing latest 20):", rows.len());
    for row in rows {
        let doc_id: String = row.get("document_id");
        let priority: String = row.get("priority");
        let status: String = row.get("status");
        let queued: String = row.get("queued_at");
        let started: Option<String> = row.get("started_at");
        let completed: Option<String> = row.get("completed_at");
        let error: Option<String> = row.get("error_message");
        let attempts: i64 = row.get("attempts");
        
        println!("\n🔄 Document: {}...", &doc_id[..8.min(doc_id.len())]);
        println!("   🚦 Status: {}", status);
        println!("   ⚡ Priority: {}", priority);
        println!("   📅 Queued: {}", queued);
        println!("   🏃 Started: {}", started.unwrap_or_else(|| "Not started".to_string()));
        println!("   ✅ Completed: {}", completed.unwrap_or_else(|| "Not completed".to_string()));
        println!("   🔄 Attempts: {}", attempts);
        if let Some(error) = error {
            println!("   ❌ Error: {}", error);
        }
    }
    
    Ok(())
}

async fn get_document_types_analysis(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📊 DOCUMENT TYPES COMPRESSION ANALYSIS");
    println!("=======================================");
    
    let rows = sqlx::query(r#"
        SELECT 
            dt.name as type_name,
            dt.compression_level,
            dt.compression_method,
            dt.min_size_for_compression,
            COUNT(md.id) as doc_count,
            SUM(CASE WHEN md.compression_status = 'completed' THEN 1 ELSE 0 END) as compressed_count,
            SUM(CASE WHEN md.compression_status = 'failed' THEN 1 ELSE 0 END) as failed_count,
            SUM(CASE WHEN md.compression_status = 'skipped' THEN 1 ELSE 0 END) as skipped_count,
            AVG(md.size_bytes) as avg_size,
            SUM(md.size_bytes) as total_original_size,
            SUM(CASE WHEN md.compressed_size_bytes IS NOT NULL THEN md.compressed_size_bytes ELSE 0 END) as total_compressed_size
        FROM document_types dt
        LEFT JOIN media_documents md ON dt.id = md.type_id AND md.file_path != 'ERROR'
        GROUP BY dt.id, dt.name
        ORDER BY doc_count DESC
    "#)
    .fetch_all(pool)
    .await?;
    
    println!("\n📋 Document Type Analysis:");
    
    for row in rows {
        let type_name: String = row.get("type_name");
        let comp_level: i64 = row.get("compression_level");
        let comp_method: Option<String> = row.get("compression_method");
        let min_size: Option<i64> = row.get("min_size_for_compression");
        let doc_count: i64 = row.get("doc_count");
        let compressed: i64 = row.get("compressed_count");
        let failed: i64 = row.get("failed_count");
        let skipped: i64 = row.get("skipped_count");
        let avg_size: Option<f64> = row.get("avg_size");
        let total_orig: Option<i64> = row.get("total_original_size");
        let total_comp: Option<i64> = row.get("total_compressed_size");
        
        if doc_count == 0 {
            continue;
        }
        
        println!("\n📂 {}", type_name);
        println!("   🗜️ Compression: Level {}, Method: {}", 
                 comp_level, comp_method.unwrap_or_else(|| "none".to_string()));
        println!("   📏 Min size for compression: {}", format_bytes(min_size.unwrap_or(0)));
        println!("   📊 Documents: {} total", doc_count);
        println!("   ✅ Compressed: {}", compressed);
        println!("   ❌ Failed: {}", failed);
        println!("   ⏭️ Skipped: {}", skipped);
        println!("   📐 Average size: {}", format_bytes(avg_size.unwrap_or(0.0) as i64));
        
        let total_orig = total_orig.unwrap_or(0);
        let total_comp = total_comp.unwrap_or(0);
        if total_comp > 0 && total_orig > 0 {
            let savings = total_orig - total_comp;
            let percentage = (savings as f64 / total_orig as f64) * 100.0;
            println!("   💾 Total savings: {} ({:.1}%)", format_bytes(savings), percentage);
        }
    }
    
    Ok(())
}

async fn check_storage_directory(_pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📁 STORAGE DIRECTORY ANALYSIS");
    println!("==============================");
    
    // Try to determine storage path from environment or find the database path
    let storage_path = if let Ok(ios_docs) = env::var("IOS_DOCUMENTS_DIR") {
        ios_docs
    } else if let Some(db_path) = find_database_path() {
        Path::new(&db_path).parent().unwrap_or_else(|| Path::new("./")).to_string_lossy().to_string()
    } else {
        "./storage".to_string()
    };
    
    println!("🔍 Checking storage path: {}", storage_path);
    
    let storage_base = Path::new(&storage_path);
    let original_dir = storage_base.join("original");
    let compressed_dir = storage_base.join("compressed");
    
    println!("\n📂 Directory structure:");
    println!("   📁 Original: {}", if original_dir.exists() { "✅ exists" } else { "❌ missing" });
    println!("   🗜️ Compressed: {}", if compressed_dir.exists() { "✅ exists" } else { "❌ missing" });
    
    if original_dir.exists() {
        scan_directory(&original_dir, "📄 Original").await;
    }
    
    if compressed_dir.exists() {
        scan_directory(&compressed_dir, "🗜️ Compressed").await;
    }
    
    Ok(())
}

async fn scan_directory(dir: &Path, label: &str) {
    match std::fs::read_dir(dir) {
        Ok(entries) => {
            let mut file_count = 0;
            let mut total_size = 0u64;
            let mut examples = Vec::new();
            
            for entry in entries.flatten() {
                if entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                    let file_name = entry.file_name().to_string_lossy().to_string();
                    if !file_name.starts_with('.') {  // Skip hidden files
                        if let Ok(metadata) = entry.metadata() {
                            file_count += 1;
                            total_size += metadata.len();
                            
                            if examples.len() < 5 {
                                examples.push((file_name, metadata.len()));
                            }
                        }
                    }
                }
            }
            
            println!("\n   {} files: {} files, {}", label, file_count, format_bytes(total_size as i64));
            
            if !examples.is_empty() {
                println!("   Examples:");
                for (filename, size) in examples {
                    println!("     📄 {} ({})", filename, format_bytes(size as i64));
                }
                if file_count > 5 {
                    println!("     ... and {} more files", file_count - 5);
                }
            }
        },
        Err(e) => {
            println!("   ❌ Error reading {}: {}", label, e);
        }
    }
}

fn format_bytes(bytes: i64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    
    if bytes == 0 {
        return "0 B".to_string();
    }
    
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    format!("{:.1} {}", size, UNITS[unit_index])
} 