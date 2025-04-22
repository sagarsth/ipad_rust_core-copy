-- SQLite does not directly support ALTER COLUMN to remove NOT NULL.
-- We need to recreate the table.

-- 1. Rename the existing table
ALTER TABLE media_documents RENAME TO media_documents_old;

-- 2. Create the new table with related_id as nullable and add temp_related_id
CREATE TABLE media_documents (
    id TEXT PRIMARY KEY NOT NULL,
    related_table TEXT NOT NULL,
    related_id TEXT NULL, -- Made nullable
    type_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    title TEXT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    field_identifier TEXT NULL,
    compression_status TEXT NOT NULL DEFAULT 'pending',
    compressed_file_path TEXT NULL,
    blob_sync_status TEXT NOT NULL DEFAULT 'pending',
    sync_priority INTEGER NOT NULL DEFAULT 1,
    temp_related_id TEXT NULL, -- New column
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    created_by_user_id TEXT NULL,
    updated_by_user_id TEXT NULL,
    deleted_at TEXT NULL,
    deleted_by_user_id TEXT NULL,
    FOREIGN KEY (type_id) REFERENCES document_types(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL
);

-- Add index for temp_related_id for faster lookups
CREATE INDEX idx_media_documents_temp_related_id ON media_documents(temp_related_id) WHERE temp_related_id IS NOT NULL;
-- Add index for related_id
CREATE INDEX idx_media_documents_related_id ON media_documents(related_id) WHERE related_id IS NOT NULL;


-- 3. Copy data from the old table to the new table
INSERT INTO media_documents (
    id, related_table, related_id, type_id, file_path, original_filename,
    title, mime_type, size_bytes, field_identifier,
    compression_status, compressed_file_path, blob_sync_status,
    sync_priority, temp_related_id, -- temp_related_id will be NULL initially
    created_at, updated_at, created_by_user_id, updated_by_user_id,
    deleted_at, deleted_by_user_id
)
SELECT
    id, related_table, related_id, type_id, file_path, original_filename,
    title, mime_type, size_bytes, field_identifier,
    compression_status, compressed_file_path, blob_sync_status,
    sync_priority, NULL, -- Set temp_related_id to NULL
    created_at, updated_at, created_by_user_id, updated_by_user_id,
    deleted_at, deleted_by_user_id
FROM media_documents_old;

-- 4. Drop the old table
DROP TABLE media_documents_old; 