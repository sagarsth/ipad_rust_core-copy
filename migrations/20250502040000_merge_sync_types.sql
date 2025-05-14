-- Migration: Merge Sync Types and Add New Sync Tables
-- Timestamp: 20250502040000 (Example)

PRAGMA foreign_keys=OFF;

-- --------------------------------------------
-- Modify Existing Tables
-- --------------------------------------------

-- Add entity_data to change_log
ALTER TABLE change_log ADD COLUMN entity_data TEXT NULL;

-- Add push tracking to tombstones
ALTER TABLE tombstones ADD COLUMN pushed_at TEXT NULL;
ALTER TABLE tombstones ADD COLUMN sync_batch_id TEXT NULL;
-- Optional: Add FK constraint if sync_batches table always exists when this runs
-- ALTER TABLE tombstones ADD CONSTRAINT fk_tombstones_sync_batch FOREIGN KEY (sync_batch_id) REFERENCES sync_batches(batch_id) ON DELETE SET NULL;

-- Add new INTEGER sync_priority_level column to tables that had sync_priority
-- We default to 5 (Normal in DetailedSyncPriority)
ALTER TABLE strategic_goals ADD COLUMN sync_priority_level INTEGER DEFAULT 5 NOT NULL;
ALTER TABLE projects ADD COLUMN sync_priority_level INTEGER DEFAULT 5 NOT NULL;
ALTER TABLE participants ADD COLUMN sync_priority_level INTEGER DEFAULT 5 NOT NULL;
ALTER TABLE livelihoods ADD COLUMN sync_priority_level INTEGER DEFAULT 5 NOT NULL;
ALTER TABLE subsequent_grants ADD COLUMN sync_priority_level INTEGER DEFAULT 5 NOT NULL;
ALTER TABLE workshops ADD COLUMN sync_priority_level INTEGER DEFAULT 5 NOT NULL;

-- Handle media_documents sync_priority (more complex due to CHECK constraint)
-- 1. Add the new column
ALTER TABLE media_documents ADD COLUMN sync_priority_level INTEGER DEFAULT 5 NOT NULL; -- Default to Normal

-- 2. Data Migration (Example - adapt based on DetailedSyncPriority mapping)
-- Update based on existing TEXT values (assuming File 2's mapping was used)
UPDATE media_documents SET sync_priority_level = 8 WHERE sync_priority = 'high'; -- High
UPDATE media_documents SET sync_priority_level = 5 WHERE sync_priority = 'normal'; -- Normal
UPDATE media_documents SET sync_priority_level = 3 WHERE sync_priority = 'low'; -- Low
-- UPDATE media_documents SET sync_priority_level = 0 WHERE sync_priority = 'never'; -- Match to something? maybe 0 or 1? Let's use 1 (Background)
UPDATE media_documents SET sync_priority_level = 1 WHERE sync_priority = 'never';


-- Note: Dropping the old sync_priority TEXT column and its CHECK constraint
-- usually requires recreating the table in SQLite. This is often done in a separate
-- maintenance migration script. For now, we keep both columns.


-- --------------------------------------------
-- Create New Tables
-- --------------------------------------------

-- Sync Configuration (based on File 2's SyncConfig struct)
CREATE TABLE IF NOT EXISTS sync_configs (
    id TEXT PRIMARY KEY, -- UUID
    user_id TEXT NOT NULL UNIQUE,
    sync_interval_minutes INTEGER NOT NULL DEFAULT 60,
    background_sync_enabled INTEGER NOT NULL DEFAULT 1,
    wifi_only INTEGER NOT NULL DEFAULT 1,
    charging_only INTEGER NOT NULL DEFAULT 0,
    sync_priority_threshold INTEGER NOT NULL DEFAULT 1, -- Corresponds to DetailedSyncPriority::Background or higher
    document_sync_enabled INTEGER NOT NULL DEFAULT 1,
    metadata_sync_enabled INTEGER NOT NULL DEFAULT 1,
    server_token TEXT NULL, -- Server's last known sync point for this client
    last_sync_timestamp TEXT NULL, -- Client's record of last successful sync completion
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Sync Conflicts (based on File 1's detailed SyncConflictRow)
CREATE TABLE IF NOT EXISTS sync_conflicts (
    conflict_id TEXT PRIMARY KEY, -- UUID
    entity_table TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    field_name TEXT NULL,
    local_change_op_id TEXT NOT NULL, -- FK to change_log operation_id
    remote_change_op_id TEXT NOT NULL, -- ID of the conflicting remote operation (how to store?) - Maybe TEXT is better here
    resolution_status TEXT NOT NULL DEFAULT 'unresolved' CHECK(resolution_status IN ('resolved', 'unresolved', 'manual', 'ignored')),
    resolution_strategy TEXT NULL CHECK(resolution_strategy IS NULL OR resolution_strategy IN ('server_wins', 'client_wins', 'last_write_wins', 'merge_prioritize_server', 'merge_prioritize_client', 'manual')),
    resolved_by_user_id TEXT NULL,
    resolved_at TEXT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    details TEXT NULL, -- Storing local/remote values might be better here if change log entries can be purged
    FOREIGN KEY (resolved_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (local_change_op_id) REFERENCES change_log(operation_id) ON DELETE CASCADE -- If local change gone, conflict context lost
    -- Note: No FK for remote_change_op_id unless we store remote ops reliably
);

-- Sync Operation Logs (based on File 2's log_sync_operation call)
CREATE TABLE IF NOT EXISTS sync_operation_logs (
    id TEXT PRIMARY KEY, -- UUID
    batch_id TEXT NOT NULL,
    operation TEXT NOT NULL, -- e.g., 'upload', 'download', 'apply_remote_change'
    entity_id TEXT NULL,
    entity_type TEXT NULL,
    status TEXT NOT NULL CHECK(status IN ('started', 'completed', 'failed', 'skipped')),
    error_message TEXT NULL,
    blob_key TEXT NULL, -- For document operations
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (batch_id) REFERENCES sync_batches(batch_id) ON DELETE CASCADE
);

-- Sync Sessions (based on File 1's SyncSessionRow)
CREATE TABLE IF NOT EXISTS sync_sessions (
    session_id TEXT PRIMARY KEY, -- Maybe UUID? File 1 uses String
    user_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    start_time TEXT NOT NULL,
    end_time TEXT NULL,
    sync_mode TEXT NOT NULL CHECK(sync_mode IN ('full', 'incremental', 'minimal', 'selective')),
    status TEXT NOT NULL CHECK(status IN ('success', 'partial_success', 'failed', 'in_progress')),
    error_message TEXT NULL,
    changes_uploaded INTEGER NULL,
    changes_downloaded INTEGER NULL,
    conflicts_encountered INTEGER NULL,
    bytes_transferred INTEGER NULL,
    network_type TEXT NULL,
    duration_seconds REAL NULL,
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
    -- Maybe FK to device_metadata?
);

-- Sync Queue (based on File 2's SyncQueueItem struct)
CREATE TABLE IF NOT EXISTS sync_queue (
    id TEXT PRIMARY KEY, -- UUID
    sync_batch_id TEXT NULL,
    entity_id TEXT NOT NULL,
    entity_type TEXT NOT NULL,
    operation_type TEXT NOT NULL, -- "upload", "download", "merge" ?
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'processing', 'completed', 'failed')),
    blob_key TEXT NULL, -- For document operations
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT NULL,
    error_message TEXT NULL,
    retry_count INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (sync_batch_id) REFERENCES sync_batches(batch_id) ON DELETE SET NULL
);

-- --------------------------------------------
-- Create Indexes for New/Modified Columns/Tables
-- --------------------------------------------

-- change_log
CREATE INDEX IF NOT EXISTS idx_change_log_entity_data ON change_log(entity_data) WHERE entity_data IS NOT NULL; -- If searchable

-- tombstones
CREATE INDEX IF NOT EXISTS idx_tombstones_pushed_at ON tombstones(pushed_at) WHERE pushed_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_tombstones_sync_batch_id ON tombstones(sync_batch_id);

-- sync_priority_level indexes (add for all modified tables)
CREATE INDEX IF NOT EXISTS idx_strategic_goals_sync_priority_level ON strategic_goals(sync_priority_level);
CREATE INDEX IF NOT EXISTS idx_projects_sync_priority_level ON projects(sync_priority_level);
CREATE INDEX IF NOT EXISTS idx_participants_sync_priority_level ON participants(sync_priority_level);
CREATE INDEX IF NOT EXISTS idx_livelihoods_sync_priority_level ON livelihoods(sync_priority_level);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_sync_priority_level ON subsequent_grants(sync_priority_level);
CREATE INDEX IF NOT EXISTS idx_workshops_sync_priority_level ON workshops(sync_priority_level);
CREATE INDEX IF NOT EXISTS idx_media_documents_sync_priority_level ON media_documents(sync_priority_level);


-- sync_configs
CREATE UNIQUE INDEX IF NOT EXISTS idx_sync_configs_user_id ON sync_configs(user_id);

-- sync_conflicts
CREATE INDEX IF NOT EXISTS idx_sync_conflicts_entity ON sync_conflicts(entity_table, entity_id);
CREATE INDEX IF NOT EXISTS idx_sync_conflicts_status ON sync_conflicts(resolution_status);
CREATE INDEX IF NOT EXISTS idx_sync_conflicts_local_op ON sync_conflicts(local_change_op_id);

-- sync_operation_logs
CREATE INDEX IF NOT EXISTS idx_sync_operation_logs_batch_id ON sync_operation_logs(batch_id);
CREATE INDEX IF NOT EXISTS idx_sync_operation_logs_entity ON sync_operation_logs(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_sync_operation_logs_status ON sync_operation_logs(status);

-- sync_sessions
CREATE INDEX IF NOT EXISTS idx_sync_sessions_user_device ON sync_sessions(user_id, device_id, start_time DESC);
CREATE INDEX IF NOT EXISTS idx_sync_sessions_status ON sync_sessions(status);

-- sync_queue
CREATE INDEX IF NOT EXISTS idx_sync_queue_status_created ON sync_queue(status, created_at) WHERE status = 'pending' OR status = 'failed';
CREATE INDEX IF NOT EXISTS idx_sync_queue_entity ON sync_queue(entity_type, entity_id);
CREATE INDEX IF NOT EXISTS idx_sync_queue_batch ON sync_queue(sync_batch_id);



PRAGMA foreign_keys=ON; 