#!/bin/bash

# Quick compression debug script for iPad simulator
# Finds the database and storage paths automatically

set -e

echo "🔍 COMPRESSION DEBUG SESSION"
echo "==============================="
date

# Try to find database
DB_PATH=""
STORAGE_PATH=""

# Check current directory first
if [ -f "./storage/actionaid.db" ]; then
    DB_PATH="./storage/actionaid.db"
    STORAGE_PATH="./storage"
elif [ -f "./actionaid.db" ]; then
    DB_PATH="./actionaid.db"
    STORAGE_PATH="./storage"
# Check iOS Documents directory if set
elif [ ! -z "$IOS_DOCUMENTS_DIR" ] && [ -f "$IOS_DOCUMENTS_DIR/actionaid.db" ]; then
    DB_PATH="$IOS_DOCUMENTS_DIR/actionaid.db"
    STORAGE_PATH="$IOS_DOCUMENTS_DIR"
# Check simulator directories
else
    SIM_DB=$(find ~/Library/Developer/CoreSimulator/Devices -name "actionaid.db" 2>/dev/null | head -1)
    if [ ! -z "$SIM_DB" ]; then
        DB_PATH="$SIM_DB"
        STORAGE_PATH=$(dirname "$SIM_DB")
    fi
fi

if [ -z "$DB_PATH" ] || [ ! -f "$DB_PATH" ]; then
    echo "❌ Could not find actionaid.db database"
    echo "   Checked:"
    echo "   - ./storage/actionaid.db"
    echo "   - ./actionaid.db"
    echo "   - \$IOS_DOCUMENTS_DIR/actionaid.db"
    echo "   - iOS Simulator directories"
    exit 1
fi

echo "📍 Found database: $DB_PATH"
echo "📁 Storage path: $STORAGE_PATH"

# Function to format bytes
format_bytes() {
    local bytes=$1
    if [ -z "$bytes" ] || [ "$bytes" = "NULL" ]; then
        echo "0 B"
        return
    fi
    
    if [ "$bytes" -lt 1024 ]; then
        echo "${bytes} B"
    elif [ "$bytes" -lt 1048576 ]; then
        echo "$((bytes / 1024)) KB"
    elif [ "$bytes" -lt 1073741824 ]; then
        echo "$((bytes / 1048576)) MB"
    else
        echo "$((bytes / 1073741824)) GB"
    fi
}

echo ""
echo "📊 COMPRESSION STATUS OVERVIEW"
echo "==============================="

# Check compression status breakdown
sqlite3 "$DB_PATH" << EOF
.mode column
.headers on
SELECT 
    compression_status,
    COUNT(*) as count,
    printf('%.2f MB', SUM(size_bytes) / 1024.0 / 1024.0) as original_size,
    printf('%.2f MB', SUM(CASE WHEN compressed_size_bytes IS NOT NULL THEN compressed_size_bytes ELSE 0 END) / 1024.0 / 1024.0) as compressed_size
FROM media_documents 
WHERE file_path != 'ERROR'
GROUP BY compression_status
ORDER BY count DESC;
EOF

echo ""
echo "✅ SUCCESSFULLY COMPRESSED DOCUMENTS"
echo "====================================="

# Show compressed documents
COMPRESSED_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM media_documents WHERE compression_status = 'completed' AND compressed_file_path IS NOT NULL AND file_path != 'ERROR';")

if [ "$COMPRESSED_COUNT" -eq 0 ]; then
    echo "❌ No compressed documents found!"
else
    echo "🎯 Found $COMPRESSED_COUNT compressed documents:"
    echo ""
    
    sqlite3 "$DB_PATH" << EOF
.mode column
.headers on
SELECT 
    substr(id, 1, 8) || '...' as doc_id,
    original_filename,
    mime_type,
    printf('%.1f KB', size_bytes / 1024.0) as original_size,
    printf('%.1f KB', compressed_size_bytes / 1024.0) as compressed_size,
    printf('%.1f%%', (size_bytes - compressed_size_bytes) * 100.0 / size_bytes) as savings_percent
FROM media_documents 
WHERE compression_status = 'completed' 
    AND compressed_file_path IS NOT NULL
    AND file_path != 'ERROR'
ORDER BY created_at DESC
LIMIT 10;
EOF
fi

echo ""
echo "❌ FAILED COMPRESSIONS"
echo "======================"

FAILED_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM media_documents WHERE compression_status = 'failed' OR has_error = 1 AND file_path != 'ERROR';")

if [ "$FAILED_COUNT" -eq 0 ]; then
    echo "✅ No failed compressions found!"
else
    echo "🚨 Found $FAILED_COUNT failed documents:"
    echo ""
    
    sqlite3 "$DB_PATH" << EOF
.mode column
.headers on
SELECT 
    substr(id, 1, 8) || '...' as doc_id,
    original_filename,
    mime_type,
    printf('%.1f KB', size_bytes / 1024.0) as size,
    COALESCE(error_message, 'Unknown error') as error
FROM media_documents 
WHERE compression_status = 'failed' 
    OR has_error = 1
    AND file_path != 'ERROR'
ORDER BY created_at DESC
LIMIT 5;
EOF
fi

echo ""
echo "🔄 COMPRESSION QUEUE STATUS"
echo "=========================="

QUEUE_EXISTS=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='compression_queue';")

if [ "$QUEUE_EXISTS" -eq 0 ]; then
    echo "ℹ️ No compression_queue table found"
else
    QUEUE_COUNT=$(sqlite3 "$DB_PATH" "SELECT COUNT(*) FROM compression_queue;")
    
    if [ "$QUEUE_COUNT" -eq 0 ]; then
        echo "📭 Queue is empty"
    else
        echo "📋 Found $QUEUE_COUNT queue entries (showing latest 5):"
        echo ""
        
        sqlite3 "$DB_PATH" << EOF
.mode column
.headers on
SELECT 
    substr(document_id, 1, 8) || '...' as doc_id,
    priority,
    status,
    queued_at,
    attempts
FROM compression_queue 
ORDER BY queued_at DESC
LIMIT 5;
EOF
    fi
fi

echo ""
echo "📁 STORAGE DIRECTORY CHECK"
echo "========================="

if [ ! -d "$STORAGE_PATH" ]; then
    echo "❌ Storage directory does not exist: $STORAGE_PATH"
else
    echo "📂 Storage directory: $STORAGE_PATH"
    
    ORIGINAL_DIR="$STORAGE_PATH/original"
    COMPRESSED_DIR="$STORAGE_PATH/compressed"
    
    if [ -d "$ORIGINAL_DIR" ]; then
        ORIG_COUNT=$(find "$ORIGINAL_DIR" -type f ! -name ".*" | wc -l)
        ORIG_SIZE=$(find "$ORIGINAL_DIR" -type f ! -name ".*" -exec du -bc {} + 2>/dev/null | tail -1 | cut -f1 || echo "0")
        echo "📄 Original files: $ORIG_COUNT files ($(format_bytes $ORIG_SIZE))"
    else
        echo "❌ Original directory not found: $ORIGINAL_DIR"
    fi
    
    if [ -d "$COMPRESSED_DIR" ]; then
        COMP_COUNT=$(find "$COMPRESSED_DIR" -type f ! -name ".*" | wc -l)
        COMP_SIZE=$(find "$COMPRESSED_DIR" -type f ! -name ".*" -exec du -bc {} + 2>/dev/null | tail -1 | cut -f1 || echo "0")
        echo "🗜️ Compressed files: $COMP_COUNT files ($(format_bytes $COMP_SIZE))"
        
        if [ "$COMP_COUNT" -gt 0 ] && [ "$COMP_COUNT" -le 5 ]; then
            echo "   Examples:"
            find "$COMPRESSED_DIR" -type f ! -name ".*" | head -5 | while read file; do
                filename=$(basename "$file")
                size=$(stat -f%z "$file" 2>/dev/null || stat -c%s "$file" 2>/dev/null || echo "0")
                echo "   🗜️ $filename ($(format_bytes $size))"
            done
        fi
    else
        echo "❌ Compressed directory not found: $COMPRESSED_DIR"
    fi
fi

echo ""
echo "📊 DOCUMENT TYPES ANALYSIS"
echo "========================="

sqlite3 "$DB_PATH" << EOF
.mode column
.headers on
SELECT 
    dt.name as type_name,
    COUNT(md.id) as total_docs,
    SUM(CASE WHEN md.compression_status = 'completed' THEN 1 ELSE 0 END) as compressed,
    SUM(CASE WHEN md.compression_status = 'failed' THEN 1 ELSE 0 END) as failed,
    SUM(CASE WHEN md.compression_status = 'skipped' THEN 1 ELSE 0 END) as skipped,
    printf('%.1f KB', AVG(md.size_bytes) / 1024.0) as avg_size
FROM document_types dt
LEFT JOIN media_documents md ON dt.id = md.type_id AND md.file_path != 'ERROR'
GROUP BY dt.id, dt.name
HAVING total_docs > 0
ORDER BY total_docs DESC;
EOF

echo ""
echo "✅ DEBUG SESSION COMPLETED"
echo "=========================="
date 