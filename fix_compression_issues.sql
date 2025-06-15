-- ===================================================================
-- COMPREHENSIVE COMPRESSION SYSTEM FIX
-- ===================================================================
-- This script fixes stale data and resets the compression system
-- Usage: sqlite3 /path/to/actionaid_core.sqlite < fix_compression_issues.sql

.echo on
.headers on

-- Step 1: Identify stale documents (stuck for more than 1 hour)
SELECT '=== IDENTIFYING STALE DOCUMENTS ===' as step;

SELECT 
    id,
    original_filename,
    compression_status,
    (julianday('now') - julianday(updated_at)) * 24 * 60 as minutes_stale
FROM media_documents 
WHERE compression_status IN ('processing', 'pending')
AND (julianday('now') - julianday(updated_at)) * 24 * 60 > 60
ORDER BY minutes_stale DESC;

-- Step 2: Reset stuck documents to pending (older than 1 hour)
SELECT '=== RESETTING STUCK DOCUMENTS ===' as step;

UPDATE media_documents 
SET 
    compression_status = 'pending',
    updated_at = datetime('now'),
    has_error = 0,
    error_message = NULL
WHERE compression_status = 'processing'
AND (julianday('now') - julianday(updated_at)) * 24 * 60 > 60;

SELECT 'Reset ' || changes() || ' stuck documents to pending status' as result;

-- Step 3: Clean up orphaned compression queue entries
SELECT '=== CLEANING ORPHANED QUEUE ENTRIES ===' as step;

DELETE FROM compression_queue 
WHERE document_id NOT IN (
    SELECT id FROM media_documents
);

SELECT 'Removed ' || changes() || ' orphaned queue entries' as result;

-- Step 4: Reset failed queue entries to pending for retry
SELECT '=== RESETTING FAILED QUEUE ENTRIES ===' as step;

UPDATE compression_queue 
SET 
    status = 'pending',
    attempts = 0,
    error_message = NULL,
    updated_at = datetime('now')
WHERE status IN ('failed', 'processing')
AND (julianday('now') - julianday(updated_at)) * 24 * 60 > 10;

SELECT 'Reset ' || changes() || ' failed/stuck queue entries' as result;

-- Step 5: Ensure all pending documents have queue entries
SELECT '=== ENSURING QUEUE COMPLETENESS ===' as step;

INSERT INTO compression_queue (id, document_id, priority, status, attempts, created_at, updated_at)
SELECT 
    lower(hex(randomblob(4))) || '-' || 
    lower(hex(randomblob(2))) || '-' || 
    '4' || substr(lower(hex(randomblob(2))), 2) || '-' || 
    substr('89ab', abs(random()) % 4 + 1, 1) || 
    substr(lower(hex(randomblob(2))), 2) || '-' || 
    lower(hex(randomblob(6))) as id,
    md.id as document_id,
    5 as priority,  -- Normal priority
    'pending' as status,
    0 as attempts,
    datetime('now') as created_at,
    datetime('now') as updated_at
FROM media_documents md
LEFT JOIN compression_queue cq ON md.id = cq.document_id
WHERE md.compression_status = 'pending'
AND cq.document_id IS NULL
AND md.file_path != 'ERROR'
AND md.has_error != 1;

SELECT 'Added ' || changes() || ' missing queue entries' as result;

-- Step 6: Show current state after cleanup
SELECT '=== CURRENT STATE SUMMARY ===' as step;

SELECT 
    compression_status,
    COUNT(*) as document_count,
    printf('%.2f MB', SUM(size_bytes) / 1024.0 / 1024.0) as total_size
FROM media_documents 
WHERE file_path != 'ERROR'
GROUP BY compression_status
ORDER BY document_count DESC;

SELECT '=== QUEUE STATUS ===' as step;

SELECT 
    status,
    COUNT(*) as queue_count
FROM compression_queue
GROUP BY status
ORDER BY queue_count DESC;

-- Step 7: Show next documents ready for processing
SELECT '=== NEXT DOCUMENTS FOR PROCESSING ===' as step;

SELECT 
    cq.document_id,
    md.original_filename,
    md.compression_status as doc_status,
    cq.status as queue_status,
    cq.priority,
    cq.attempts,
    printf('%.1f MB', md.size_bytes / 1024.0 / 1024.0) as file_size
FROM compression_queue cq
JOIN media_documents md ON cq.document_id = md.id
WHERE cq.status = 'pending'
ORDER BY cq.priority DESC, cq.created_at ASC
LIMIT 10;

SELECT '=== COMPRESSION CLEANUP COMPLETE ===' as final_message; 