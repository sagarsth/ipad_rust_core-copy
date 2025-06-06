-- Document Handling System Enhancement Migration
-- Created: April 24, 2025
-- Description: Enhances document tracking, deletion, and error handling capabilities
-- Compatible with SQLite limitations

-- Disable foreign keys during migration
PRAGMA foreign_keys=off;

-- ========================================================================
-- 1. Enhance the tombstones table with additional metadata
-- ========================================================================

-- Add additional_metadata column to tombstones
ALTER TABLE tombstones ADD COLUMN additional_metadata TEXT NULL;

-- ========================================================================
-- 2. Enhance media_documents table with error handling fields
-- ========================================================================

-- First check what columns exist in the current table
PRAGMA table_info(media_documents);

-- Create a new media_documents table with enhanced error fields and cleanup
CREATE TABLE media_documents_new (
    id TEXT PRIMARY KEY NOT NULL,
    related_table TEXT NOT NULL,
    related_id TEXT NULL,           -- Nullable to support temporary documents
    temp_related_id TEXT NULL,      -- For documents uploaded before entity creation
    type_id TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    file_path TEXT NOT NULL,
    compressed_file_path TEXT NULL,
    compressed_size_bytes INTEGER NULL,
    field_identifier TEXT NULL,     -- For linking to specific form fields
    title TEXT NULL,
    description TEXT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    
    -- Error handling fields
    has_error INTEGER NOT NULL DEFAULT 0,       -- Boolean flag for error state
    error_message TEXT NULL,                    -- Error details
    error_type TEXT NULL CHECK(error_type IS NULL OR error_type IN (
        'storage_failure',          -- File system errors
        'conversion_failure',       -- Format conversion errors
        'compression_failure',      -- Compression errors
        'sync_failure',             -- Sync-related errors
        'permission_failure',       -- Access permission issues
        'upload_failure',           -- Initial upload failures
        'other'                     -- Miscellaneous errors
    )),
    
    -- Status fields
    compression_status TEXT NOT NULL DEFAULT 'pending',
    blob_status TEXT NOT NULL DEFAULT 'pending',
    blob_key TEXT NULL,
    sync_priority TEXT NOT NULL DEFAULT 'normal' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
    
    -- Sync tracking
    last_sync_attempt_at TEXT NULL,
    sync_attempt_count INTEGER NOT NULL DEFAULT 0,
    
    -- Standard tracking fields
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_by_user_id TEXT NULL,
    updated_by_user_id TEXT NULL,
    deleted_at TEXT NULL,
    deleted_by_user_id TEXT NULL,
    
    -- Device ID columns (for compatibility with later migrations)
    created_by_device_id TEXT NULL,
    updated_by_device_id TEXT NULL,
    deleted_by_device_id TEXT NULL,
    title_updated_at TEXT NULL,
    title_updated_by_user_id TEXT NULL,
    title_updated_by_device_id TEXT NULL,
    description_updated_at TEXT NULL,
    description_updated_by_user_id TEXT NULL,
    description_updated_by_device_id TEXT NULL,
    
    FOREIGN KEY (type_id) REFERENCES document_types(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL
);

-- Copy existing data to the new table, handling missing columns
INSERT INTO media_documents_new (
    id, related_table, related_id, temp_related_id, type_id, original_filename,
    file_path, compressed_file_path, compressed_size_bytes, field_identifier, 
    title, description, mime_type, size_bytes, 
    -- Error fields with derived values
    has_error, error_message, error_type,
    -- Status fields
    compression_status, blob_sync_status, blob_key, sync_priority,
    -- Sync tracking (new fields)
    last_sync_attempt_at, sync_attempt_count,
    -- Standard tracking
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id,
    -- Device tracking columns (new fields)
    created_by_device_id, updated_by_device_id, deleted_by_device_id,
    title_updated_at, title_updated_by_user_id, title_updated_by_device_id,
    description_updated_at, description_updated_by_user_id, description_updated_by_device_id
)
SELECT 
    id, related_table, related_id, temp_related_id, type_id, original_filename,
    file_path, compressed_file_path, compressed_size_bytes, field_identifier,
    title, description, mime_type, size_bytes,
    -- Derive error state from file_path
    CASE WHEN file_path = 'ERROR' THEN 1 ELSE 0 END as has_error,
    -- Use description as error_message for existing error records
    CASE WHEN file_path = 'ERROR' THEN description ELSE NULL END as error_message,
    -- Default error type for existing errors
    CASE WHEN file_path = 'ERROR' THEN 'upload_failure' ELSE NULL END as error_type,
    -- Status fields
    compression_status, 
    blob_sync_status, -- Use existing column
    NULL as blob_key, 
    'normal' as sync_priority, -- Default value
    -- New sync tracking fields
    NULL as last_sync_attempt_at, 0 as sync_attempt_count,
    -- Standard tracking
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id,
    -- Device tracking columns (set to NULL for existing data)
    NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL
FROM media_documents;

-- Drop old table and rename new one
DROP TABLE media_documents;
ALTER TABLE media_documents_new RENAME TO media_documents;

-- Recreate indices for media_documents
CREATE INDEX idx_media_documents_related ON media_documents(related_table, related_id);
CREATE INDEX idx_media_documents_temp_related_id ON media_documents(temp_related_id) 
    WHERE temp_related_id IS NOT NULL;
CREATE INDEX idx_media_documents_type ON media_documents(type_id);
CREATE INDEX idx_media_documents_compression ON media_documents(compression_status);
CREATE INDEX idx_media_documents_blob_sync ON media_documents(blob_sync_status);
CREATE INDEX idx_media_documents_updated_at ON media_documents(updated_at);
CREATE INDEX idx_media_documents_deleted_at ON media_documents(deleted_at);
CREATE INDEX idx_media_documents_sync_priority ON media_documents(sync_priority);
CREATE INDEX idx_media_documents_has_error ON media_documents(has_error);

-- ========================================================================
-- 3. Enhance document_access_logs to support more access types
-- ========================================================================

-- Update document_access_logs check constraint to support more access types
CREATE TABLE document_access_logs_new (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    access_type TEXT NOT NULL,
    access_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    details TEXT,
    FOREIGN KEY (document_id) REFERENCES media_documents(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Copy existing data
INSERT INTO document_access_logs_new SELECT * FROM document_access_logs;

-- Drop old table and rename new one
DROP TABLE document_access_logs;
ALTER TABLE document_access_logs_new RENAME TO document_access_logs;

-- Recreate indices
CREATE INDEX idx_document_access_logs_document ON document_access_logs(document_id);
CREATE INDEX idx_document_access_logs_user ON document_access_logs(user_id);
CREATE INDEX idx_document_access_logs_date ON document_access_logs(access_date);
CREATE INDEX idx_document_access_logs_type ON document_access_logs(access_type);

-- ========================================================================
-- 4. Create active_file_usage table for tracking files in use
-- ========================================================================

CREATE TABLE active_file_usage (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    last_active_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    use_type TEXT NOT NULL DEFAULT 'view',
    FOREIGN KEY (document_id) REFERENCES media_documents(id) ON DELETE CASCADE,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(document_id, user_id, device_id)
);

CREATE INDEX idx_active_file_usage_document ON active_file_usage(document_id);
CREATE INDEX idx_active_file_usage_user ON active_file_usage(user_id);
CREATE INDEX idx_active_file_usage_active ON active_file_usage(last_active_at);

-- ========================================================================
-- 5. Create file_deletion_queue for deferred file cleanup
-- ========================================================================

CREATE TABLE file_deletion_queue (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL,        -- For reference, even after document record deleted
    file_path TEXT NOT NULL,          -- Original file to delete
    compressed_file_path TEXT NULL,   -- Compressed version if it exists
    requested_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    requested_by TEXT NOT NULL,
    grace_period_seconds INTEGER DEFAULT 86400,  -- 24 hours by default
    attempts INTEGER DEFAULT 0,
    last_attempt_at TEXT NULL,
    completed_at TEXT NULL,
    error_message TEXT NULL,
    FOREIGN KEY (requested_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX idx_file_deletion_queue_pending ON file_deletion_queue(requested_at)
    WHERE completed_at IS NULL;
CREATE INDEX idx_file_deletion_queue_document ON file_deletion_queue(document_id);

-- ========================================================================
-- 6. Update change_log to support extra document information
-- ========================================================================

-- Add document_metadata to change_log
ALTER TABLE change_log ADD COLUMN document_metadata TEXT NULL;

-- Re-enable foreign keys
PRAGMA foreign_keys=on;