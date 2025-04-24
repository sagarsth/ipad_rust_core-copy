-- Begin transaction for restructuring media_documents

-- Create the new media_documents table with the updated schema
CREATE TABLE media_documents_new (
    id TEXT PRIMARY KEY NOT NULL,
    related_table TEXT NOT NULL,
    related_id TEXT NULL,
    type_id TEXT NOT NULL,
    original_filename TEXT NOT NULL, -- RENAMED from file_name
    file_path TEXT NOT NULL,
    compressed_file_path TEXT NULL,
    compressed_size_bytes INTEGER NULL, -- ADDED
    field_identifier TEXT NULL, -- RENAMED from linked_field_name
    title TEXT NULL,
    description TEXT NULL,
    mime_type TEXT NOT NULL, -- Made NOT NULL
    size_bytes INTEGER NOT NULL, -- RENAMED from file_size, Made NOT NULL
    compression_status TEXT NOT NULL DEFAULT 'PENDING', -- Added default
    blob_storage_key TEXT NULL,
    blob_sync_status TEXT NOT NULL DEFAULT 'PENDING', -- Added default
    sync_priority INTEGER NOT NULL DEFAULT 1, -- Added default (matches SyncPriority::Normal)
    temp_related_id TEXT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_by_user_id TEXT NULL,
    updated_by_user_id TEXT NULL,
    deleted_at TEXT NULL,
    deleted_by_user_id TEXT NULL,
    FOREIGN KEY (type_id) REFERENCES document_types(id) ON DELETE CASCADE
);

-- Copy data from the old table to the new table, handling renamed columns and NULLs
INSERT INTO media_documents_new (
    id, related_table, related_id, type_id, original_filename, file_path,
    compressed_file_path, compressed_size_bytes, field_identifier, title, description,
    mime_type, size_bytes, compression_status, blob_storage_key, blob_sync_status,
    sync_priority, temp_related_id, created_at, updated_at, created_by_user_id,
    updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, related_table, related_id, type_id, original_filename, file_path, -- FIXED: Use original_filename
    compressed_file_path, NULL, field_identifier, title, NULL, -- FIXED: Use field_identifier, Select NULL for description
    COALESCE(mime_type, 'application/octet-stream'), -- Handle potential NULL mime_type
    COALESCE(size_bytes, 0), -- FIXED: Use size_bytes
    compression_status, NULL, blob_sync_status, -- FIXED: Select NULL for blob_storage_key
    sync_priority, temp_related_id, created_at, updated_at, created_by_user_id,
    updated_by_user_id, deleted_at, deleted_by_user_id
FROM media_documents; -- Select from the original table

-- Drop the old table
DROP TABLE media_documents;

-- Rename the new table to the original name
ALTER TABLE media_documents_new RENAME TO media_documents;

-- Recreate indices that existed on the old table
CREATE INDEX idx_media_documents_related ON media_documents (related_table, related_id);
CREATE INDEX idx_media_documents_type ON media_documents (type_id);
CREATE INDEX idx_media_documents_temp_related_id ON media_documents (temp_related_id); 