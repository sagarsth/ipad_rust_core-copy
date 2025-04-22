-- Migration for Document Handling Updates
-- Created at: 2024-04-21
-- Description: Adds sync priority to change log, and compressed path/linked field to media documents.

PRAGMA foreign_keys=off; -- Recommended for schema changes

-- Add sync priority to the change_log table
ALTER TABLE change_log ADD COLUMN priority INTEGER DEFAULT 5; -- Add priority, default to normal (e.g., 5)

-- Add compressed file path and linked field name to media_documents table
-- ALTER TABLE media_documents ADD COLUMN compressed_file_path TEXT NULL; -- Commented out: Column now created in basic.sql
ALTER TABLE media_documents ADD COLUMN linked_field_name TEXT NULL;

-- No need to recreate indexes specifically for these added nullable columns usually
-- SQLite handles indexes on NULL values appropriately.

PRAGMA foreign_keys=on; 