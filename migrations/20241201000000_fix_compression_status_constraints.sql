-- Fix compression status constraints to include 'skipped'
-- Safe migration since tables are currently empty

-- 1. Drop and recreate compression_queue table with 'skipped' support
DROP TABLE compression_queue;

CREATE TABLE compression_queue (
    id TEXT PRIMARY KEY,
    document_id TEXT NOT NULL UNIQUE,
    priority INTEGER DEFAULT 5,
    attempts INTEGER DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'processing', 'completed', 'failed', 'skipped')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    error_message TEXT,
    
    FOREIGN KEY (document_id) REFERENCES media_documents(id) ON DELETE CASCADE
);

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_compression_queue_pending ON compression_queue(status, priority DESC, created_at ASC) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_compression_queue_document ON compression_queue(document_id);

-- 2. Add validation trigger for media_documents.compression_status 
-- (Since we can't easily add CHECK constraint to existing table)
CREATE TRIGGER IF NOT EXISTS validate_media_documents_compression_status
    BEFORE INSERT ON media_documents
    WHEN NEW.compression_status NOT IN ('pending', 'processing', 'completed', 'failed', 'skipped')
BEGIN
    SELECT RAISE(ABORT, 'Invalid compression_status. Must be one of: pending, processing, completed, failed, skipped');
END;

CREATE TRIGGER IF NOT EXISTS validate_media_documents_compression_status_update
    BEFORE UPDATE ON media_documents
    WHEN NEW.compression_status NOT IN ('pending', 'processing', 'completed', 'failed', 'skipped')
BEGIN
    SELECT RAISE(ABORT, 'Invalid compression_status. Must be one of: pending, processing, completed, failed, skipped');
END; 