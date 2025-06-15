use std::env;
use std::path::Path;
use sqlx::{SqlitePool, Row};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Compression Debug Tool");
    println!("========================");
    
    // Try to find the database (look for actionaid_core.sqlite)
    let db_path = find_database_path().unwrap_or_else(|| {
        eprintln!("❌ Could not find actionaid_core.sqlite database");
        eprintln!("   Searched in:");
        eprintln!("   - ./storage/actionaid_core.sqlite");
        eprintln!("   - ./actionaid_core.sqlite");
        eprintln!("   - $IOS_DOCUMENTS_DIR/actionaid_core.sqlite");
        eprintln!("   - iOS Simulator directories");
        std::process::exit(1);
    });
    
    println!("📍 Found database: {}", db_path);
    
    // Connect to database
    let database_url = format!("sqlite:{}", db_path);
    let pool = SqlitePool::connect(&database_url).await?;
    
    // Run debug analysis
    get_compression_overview(&pool).await?;
    get_recent_documents(&pool).await?;
    get_failed_compressions(&pool).await?;
    get_compression_queue_status(&pool).await?;
    get_document_types_analysis(&pool).await?;
    check_storage_directory(&pool).await?;
    
    println!("\n✅ DEBUG SESSION COMPLETED");
    println!("==========================");
    
    Ok(())
}

fn find_database_path() -> Option<String> {
    // Check current directory first (for actionaid_core.sqlite)
    let candidates = vec![
        "./storage/actionaid_core.sqlite",
        "./actionaid_core.sqlite",
    ];
    
    for path in &candidates {
        if Path::new(path).exists() {
            return Some(path.to_string());
        }
    }
    
    // Check iOS Documents directory if set
    if let Ok(ios_docs) = env::var("IOS_DOCUMENTS_DIR") {
        let db_path = format!("{}/actionaid_core.sqlite", ios_docs);
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
                "-name", "actionaid_core.sqlite",
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

async fn get_recent_documents(pool: &SqlitePool) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n📋 RECENT DOCUMENT ACTIVITY");
    println!("============================");
    
    let rows = sqlx::query(r#"
        SELECT 
            id,
            original_filename,
            compression_status,
            size_bytes,
            compressed_size_bytes,
            created_at,
            updated_at
        FROM media_documents 
        WHERE file_path != 'ERROR'
        ORDER BY updated_at DESC
        LIMIT 10
    "#)
    .fetch_all(pool)
    .await?;
    
    if rows.is_empty() {
        println!("❌ No documents found!");
        return Ok(());
    }
    
    println!("\n🎯 Most recent 10 documents:");
    
    for row in rows {
        let doc_id: String = row.get("id");
        let filename: String = row.get("original_filename");
        let status: String = row.get("compression_status");
        let orig_size: i64 = row.get("size_bytes");
        let comp_size: Option<i64> = row.get("compressed_size_bytes");
        let updated: String = row.get("updated_at");
        
        let status_icon = match status.as_str() {
            "completed" => "✅",
            "skipped" => "⏭️",
            "pending" => "⏳",
            "processing" => "🔄",
            "failed" => "❌",
            _ => "❓",
        };
        
        println!("\n📄 {}", filename);
        println!("   🆔 ID: {}...", &doc_id[..8.min(doc_id.len())]);
        println!("   {} Status: {}", status_icon, status);
        if let Some(comp_size) = comp_size {
            let savings = orig_size - comp_size;
            let percentage = if orig_size > 0 { (savings as f64 / orig_size as f64) * 100.0 } else { 0.0 };
            println!("   📏 Size: {} → {} ({:.1}% saved)", format_bytes(orig_size), format_bytes(comp_size), percentage);
        } else {
            println!("   📏 Size: {}", format_bytes(orig_size));
        }
        println!("   📅 Updated: {}", updated);
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
        ORDER BY updated_at DESC
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
        let error: Option<String> = row.get("error_message");
        
        println!("\n📄 {}", filename);
        println!("   🆔 ID: {}...", &doc_id[..8.min(doc_id.len())]);
        println!("   🗂️ Type: {}", mime_type);
        println!("   📏 Size: {}", format_bytes(size));
        println!("   ❌ Error: {}", error.unwrap_or_else(|| "Unknown error".to_string()));
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
            cq.document_id,
            md.original_filename,
            cq.status,
            cq.priority,
            cq.attempts,
            cq.created_at,
            cq.updated_at,
            cq.error_message,
            printf('%.1f MB', md.size_bytes / 1024.0 / 1024.0) as file_size
        FROM compression_queue cq
        JOIN media_documents md ON cq.document_id = md.id
        ORDER BY cq.status, cq.priority DESC, cq.created_at ASC
        LIMIT 20
    "#)
    .fetch_all(pool)
    .await?;
    
    if rows.is_empty() {
        println!("📭 Queue is empty");
        return Ok(());
    }
    
    println!("\n📋 Found {} queue entries (showing latest 20):", rows.len());
    
    let mut current_status = String::new();
    for row in rows {
        let filename: String = row.get("original_filename");
        let status: String = row.get("status");
        let priority: i32 = row.get("priority");
        let attempts: i32 = row.get("attempts");
        let file_size: String = row.get("file_size");
        let error_message: Option<String> = row.get("error_message");
        
        if status != current_status {
            current_status = status.clone();
            let status_icon = match status.as_str() {
                "pending" => "⏳",
                "processing" => "🔄",
                "completed" => "✅",
                "failed" => "❌",
                "skipped" => "⏭️",
                _ => "❓",
            };
            println!("\n{} {} Jobs:", status_icon, status.to_uppercase());
        }
        
        println!("   📄 {} ({}) - Priority: {}, Attempts: {}", 
                 filename, file_size, priority, attempts);
        
        if let Some(err) = error_message {
            println!("      ❌ Error: {}", err);
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
            SUM(CASE WHEN md.compression_status = 'pending' THEN 1 ELSE 0 END) as pending_count,
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
        let pending: i64 = row.get("pending_count");
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
        println!("   📊 Documents: {} total (✅{} compressed, ❌{} failed, ⏭️{} skipped, ⏳{} pending)", 
                 doc_count, compressed, failed, skipped, pending);
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
    
    // Use the same logic as globals.rs to determine the correct storage path
    let storage_path = if cfg!(target_os = "ios") {
        println!("🔍 [STORAGE] Detected iOS target, checking IOS_DOCUMENTS_DIR...");
        match std::env::var("IOS_DOCUMENTS_DIR") {
            Ok(path) => {
                println!("✅ [STORAGE] IOS_DOCUMENTS_DIR found: '{}'", path);
                path
            },
            Err(e) => {
                println!("❌ [STORAGE] IOS_DOCUMENTS_DIR not found: {:?}, using fallback", e);
                "./storage".to_string()
            }
        }
    } else {
        println!("🔍 [STORAGE] Not iOS target, but checking IOS_DOCUMENTS_DIR anyway...");
        match std::env::var("IOS_DOCUMENTS_DIR") {
            Ok(path) => {
                println!("✅ [STORAGE] IOS_DOCUMENTS_DIR found even on non-iOS target: '{}'", path);
                path
            },
            Err(_) => {
                println!("📁 [STORAGE] IOS_DOCUMENTS_DIR not set, trying database path...");
                if let Some(db_path) = find_database_path() {
                    let db_parent = Path::new(&db_path).parent().unwrap_or_else(|| Path::new("./"));
                    let db_parent_str = db_parent.to_string_lossy().to_string();
                    println!("📍 [STORAGE] Using database parent directory: '{}'", db_parent_str);
                    db_parent_str
                } else {
                    println!("🔧 [STORAGE] Using default ./storage");
                    "./storage".to_string()
                }
            }
        }
    };
    
    println!("🗂️ [STORAGE] Final storage path: '{}'", storage_path);
    
    let storage_base = Path::new(&storage_path);
    let storage_subdir = storage_base.join("storage");
    let original_dir = storage_subdir.join("original");
    let compressed_dir = storage_subdir.join("compressed");
    
    println!("\n📂 Directory structure:");
    println!("   📁 Base: {} ({})", storage_path, if storage_base.exists() { "✅ exists" } else { "❌ missing" });
    println!("   📁 Original: {} ({})", original_dir.display(), if original_dir.exists() { "✅ exists" } else { "❌ missing" });
    println!("   🗜️ Compressed: {} ({})", compressed_dir.display(), if compressed_dir.exists() { "✅ exists" } else { "❌ missing" });
    
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