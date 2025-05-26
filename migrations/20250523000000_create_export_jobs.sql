-- 20250523000000_create_export_jobs.sql
-- Migration to create export_jobs table for data export tracking

CREATE TABLE IF NOT EXISTS export_jobs (
    id TEXT PRIMARY KEY, -- UUID
    requested_by_user_id TEXT,
    requested_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    include_blobs INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL CHECK (status IN ('running','completed','failed')),
    local_path TEXT,
    total_entities INTEGER,
    total_bytes INTEGER,
    error_message TEXT,

    FOREIGN KEY (requested_by_user_id) REFERENCES users(id) ON DELETE SET NULL
); 