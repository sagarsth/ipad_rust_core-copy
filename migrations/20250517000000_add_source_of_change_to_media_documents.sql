-- Migration: Add source_of_change column to media_documents to track origin of changes
-- Timestamp: 20250517000000

PRAGMA foreign_keys=OFF;

ALTER TABLE media_documents ADD COLUMN source_of_change TEXT NOT NULL DEFAULT 'local';

-- Helpful index for filtering
CREATE INDEX IF NOT EXISTS idx_media_documents_source_of_change ON media_documents(source_of_change); 