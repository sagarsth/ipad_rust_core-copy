-- Tombstones and Hard Delete Migration
-- Created at: 2024-04-07

-- Enable foreign keys
PRAGMA foreign_keys = OFF;

-- Drop the locks table since we're removing that feature
DROP TABLE IF EXISTS locks;

-- Create the tombstones table for tracking hard-deleted entities
CREATE TABLE IF NOT EXISTS tombstones (
    id TEXT PRIMARY KEY, -- UUID for the tombstone itself
    entity_id TEXT NOT NULL, -- UUID of the deleted entity
    entity_type TEXT NOT NULL, -- Table name of the deleted entity
    deleted_by TEXT NOT NULL, -- User ID who performed the deletion
    deleted_at TEXT NOT NULL, -- Timestamp when deletion occurred
    operation_id TEXT NOT NULL, -- UUID for batch operations
    
    FOREIGN KEY (deleted_by) REFERENCES users(id) ON DELETE SET NULL
);

-- Create indices for efficient tombstone lookups
CREATE INDEX IF NOT EXISTS idx_tombstones_entity ON tombstones(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_tombstones_deleted_by ON tombstones(deleted_by);
CREATE INDEX IF NOT EXISTS idx_tombstones_deleted_at ON tombstones(deleted_at);
CREATE INDEX IF NOT EXISTS idx_tombstones_operation ON tombstones(operation_id);

-- Update the change_log table to handle hard deletions
-- First, we need to remove the existing CHECK constraint

-- Create a new version of the change_log table with the updated constraint
CREATE TABLE IF NOT EXISTS change_log_new (
    operation_id TEXT PRIMARY KEY, -- Unique ID for this specific change event (UUID)
    entity_table TEXT NOT NULL,    -- e.g., 'users', 'projects'
    entity_id TEXT NOT NULL,       -- The ID of the record changed
    operation_type TEXT NOT NULL CHECK (operation_type IN (
        'create',       -- Record created
        'update',       -- Field updated
        'delete',       -- Record soft deleted (set deleted_at)
        'hard_delete'   -- Record permanently deleted
     )),
    field_name TEXT,               -- Field name for 'update'. NULL otherwise.
    old_value TEXT,                -- Optional: Previous value (JSON encoded) for auditing/debugging
    new_value TEXT,                -- New value (JSON encoded) for 'create', 'update'. NULL for delete.
    timestamp TEXT NOT NULL,       -- High precision ISO8601 UTC timestamp of the change
    user_id TEXT NOT NULL,         -- User performing the change
    device_id TEXT,                -- Optional: ID of the device making the change

    -- Sync processing state
    sync_batch_id TEXT,            -- ID of the sync batch this belongs to (upload or download)
    processed_at TEXT,             -- Timestamp when this incoming change was merged locally
    sync_error TEXT,               -- If processing failed

    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL, -- User might be deleted later
    FOREIGN KEY (sync_batch_id) REFERENCES sync_batches(batch_id) ON DELETE SET NULL -- Link to sync batch
);

-- Copy existing data
INSERT INTO change_log_new 
SELECT * FROM change_log;

-- Drop the old table
DROP TABLE change_log;

-- Rename the new table to change_log
ALTER TABLE change_log_new RENAME TO change_log;

-- Recreate indices
CREATE INDEX IF NOT EXISTS idx_change_log_entity ON change_log(entity_table, entity_id);
CREATE INDEX IF NOT EXISTS idx_change_log_timestamp_user ON change_log(timestamp, user_id);
CREATE INDEX IF NOT EXISTS idx_change_log_operation ON change_log(operation_type);
CREATE INDEX IF NOT EXISTS idx_change_log_batch ON change_log(sync_batch_id);
CREATE INDEX IF NOT EXISTS idx_change_log_unprocessed_for_upload 
ON change_log(entity_table, entity_id, timestamp) 
WHERE sync_batch_id IS NULL AND processed_at IS NULL;

-- Add a new index for hard_delete operations specifically
CREATE INDEX IF NOT EXISTS idx_change_log_hard_delete
ON change_log(entity_table, entity_id) 
WHERE operation_type = 'hard_delete';

-- Update the audit_logs table to ensure it also supports hard_delete operations
CREATE TABLE IF NOT EXISTS audit_logs_new (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN (
        'create', 'update', 'delete', 'hard_delete', -- Corresponds to change_log operations
        'login_success', 'login_fail', 'logout', -- Auth events
        'sync_upload_start', 'sync_upload_complete', 'sync_upload_fail', -- Sync events
        'sync_download_start', 'sync_download_complete', 'sync_download_fail',
        'merge_conflict_resolved', 'merge_conflict_detected', -- Sync details
        'permission_denied', 'data_export', 'data_import' -- Other actions
        )),
    entity_table TEXT, -- Table related to action (users, projects...)
    entity_id TEXT,    -- Record ID related to action
    field_name TEXT,   -- Field related to action (if applicable)
    details TEXT,      -- JSON blob for extra context (e.g., error msg, IP address, change diff summary)
    timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Copy existing data
INSERT INTO audit_logs_new
SELECT * FROM audit_logs;

-- Drop the old table
DROP TABLE audit_logs;

-- Rename the new table to audit_logs
ALTER TABLE audit_logs_new RENAME TO audit_logs;

-- Recreate indices
CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_target ON audit_logs(entity_table, entity_id);

-- Enable foreign keys again
PRAGMA foreign_keys = ON;

-- Done! 