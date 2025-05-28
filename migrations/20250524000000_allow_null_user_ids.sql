-- Migration: Allow NULL user_ids for system operations
-- Date: 2025-05-24
-- Description: Remove NOT NULL constraints from user_id columns in change_log and tombstones
--              to allow system operations that don't have an associated user.

-- Fix change_log table to allow NULL user_id for system operations
CREATE TABLE IF NOT EXISTS change_log_temp (
    operation_id TEXT PRIMARY KEY, -- Unique ID for this specific change event (UUID)
    entity_table TEXT NOT NULL,    -- e.g., 'users', 'projects'
    entity_id TEXT NOT NULL,       -- The ID of the record changed
    operation_type TEXT NOT NULL CHECK (operation_type IN (
        'create',         -- Record created
        'update',         -- Field updated
        'delete',         -- Record soft deleted (set deleted_at)
        'hard_delete'     -- Record permanently deleted
       )),
    field_name TEXT,               -- Field name for 'update'. NULL otherwise.
    old_value TEXT,                -- Optional: Previous value (JSON encoded) for auditing/debugging
    new_value TEXT,                -- New value (JSON encoded) for 'create', 'update'. NULL for delete.
    timestamp TEXT NOT NULL,       -- High precision ISO8601 UTC timestamp of the change
    user_id TEXT,                  -- User performing the change (NULL for system operations)
    device_id TEXT,                -- Optional: ID of the device making the change
    priority INTEGER DEFAULT 5,
    document_metadata TEXT NULL,

    -- Sync processing state
    sync_batch_id TEXT,            -- ID of the sync batch this belongs to (upload or download)
    processed_at TEXT,             -- Timestamp when this incoming change was merged locally
    sync_error TEXT,               -- If processing failed

    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (sync_batch_id) REFERENCES sync_batches(batch_id) ON DELETE SET NULL
);

-- Copy data from old table
INSERT INTO change_log_temp 
SELECT 
    operation_id, entity_table, entity_id, operation_type, field_name,
    old_value, new_value, timestamp, user_id, device_id, priority, document_metadata,
    sync_batch_id, processed_at, sync_error
FROM change_log;

-- Drop old table and rename temp table
DROP TABLE change_log;
ALTER TABLE change_log_temp RENAME TO change_log;

-- Recreate indexes for change_log
CREATE INDEX IF NOT EXISTS idx_change_log_entity ON change_log(entity_table, entity_id);
CREATE INDEX IF NOT EXISTS idx_change_log_timestamp_user ON change_log(timestamp, user_id);
CREATE INDEX IF NOT EXISTS idx_change_log_operation ON change_log(operation_type);
CREATE INDEX IF NOT EXISTS idx_change_log_batch ON change_log(sync_batch_id);
CREATE INDEX IF NOT EXISTS idx_change_log_unprocessed_for_upload
ON change_log(entity_table, entity_id, timestamp)
WHERE processed_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_change_log_hard_delete
ON change_log(entity_table, entity_id)
WHERE operation_type = 'hard_delete';

-- Fix tombstones table to allow NULL deleted_by for system operations
CREATE TABLE IF NOT EXISTS tombstones_temp (
    id TEXT PRIMARY KEY, -- UUID for the tombstone itself
    entity_id TEXT NOT NULL, -- UUID of the deleted entity
    entity_type TEXT NOT NULL, -- Table name of the deleted entity
    deleted_by TEXT, -- User ID who performed the deletion (NULL for system operations)
    deleted_by_device_id TEXT, -- Device ID that performed the deletion
    deleted_at TEXT NOT NULL, -- Timestamp when deletion occurred
    operation_id TEXT NOT NULL, -- UUID for batch operations
    additional_metadata TEXT NULL,
    pushed_at TEXT NULL,
    sync_batch_id TEXT NULL,

    FOREIGN KEY (deleted_by) REFERENCES users(id) ON DELETE SET NULL
);

-- Copy data from old table
INSERT INTO tombstones_temp 
SELECT 
    id, entity_id, entity_type, deleted_by, deleted_by_device_id,
    deleted_at, operation_id, additional_metadata,
    pushed_at, sync_batch_id
FROM tombstones;

-- Drop old table and rename temp table
DROP TABLE tombstones;
ALTER TABLE tombstones_temp RENAME TO tombstones;

-- Recreate indexes for tombstones
CREATE INDEX IF NOT EXISTS idx_tombstones_entity ON tombstones(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_tombstones_deleted_by ON tombstones(deleted_by);
CREATE INDEX IF NOT EXISTS idx_tombstones_deleted_at ON tombstones(deleted_at);
CREATE INDEX IF NOT EXISTS idx_tombstones_pushed_at ON tombstones(pushed_at); 