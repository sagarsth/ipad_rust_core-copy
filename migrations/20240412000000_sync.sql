-- Device Sync Migration
-- Created at: 2024-04-12
-- Description: Adds device sync state tracking

-- Enable foreign keys
PRAGMA foreign_keys = OFF;

-- Create device sync state table
CREATE TABLE IF NOT EXISTS device_sync_state (
    device_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    last_upload_timestamp TEXT,
    last_download_timestamp TEXT,
    last_sync_status TEXT CHECK(last_sync_status IN ('success', 'partial_success', 'failed', 'in_progress')),
    last_sync_attempt_at TEXT,  -- When a sync was last attempted regardless of outcome
    server_version INTEGER DEFAULT 0,  -- To track server-side version vector
    sync_enabled INTEGER DEFAULT 1,    -- Allow disabling sync on specific devices
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
);

-- Add indices for efficiency
CREATE INDEX IF NOT EXISTS idx_device_sync_state_user_id ON device_sync_state(user_id);
CREATE INDEX IF NOT EXISTS idx_device_sync_state_updated_at ON device_sync_state(updated_at);


CREATE TABLE IF NOT EXISTS app_connection_settings (
    id TEXT PRIMARY KEY CHECK(id = 'cloud'),
    api_endpoint TEXT NOT NULL,  -- Base URL for your cloud API
    api_version TEXT,            -- API version for compatibility
    connection_timeout INTEGER DEFAULT 30000,  -- Timeout in milliseconds
    offline_mode_enabled INTEGER DEFAULT 0,
    retry_count INTEGER DEFAULT 3,             -- Number of retries on failure
    retry_delay INTEGER DEFAULT 5000,          -- Delay between retries
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);


-- Add device tracking to sync_batches table
-- Since SQLite doesn't support adding NOT NULL columns with a default value,
-- we need to recreate the table
CREATE TABLE sync_batches_new (
    batch_id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL, -- New column for device tracking
    direction TEXT NOT NULL CHECK (direction IN ('upload', 'download')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'processing', 'completed', 'failed', 'partially_failed')),
    item_count INTEGER DEFAULT 0,
    total_size INTEGER DEFAULT 0,
    priority INTEGER DEFAULT 5,
    attempts INTEGER DEFAULT 0,
    last_attempt_at TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT
);

-- Copy existing data (adding a default device_id for existing records)
INSERT INTO sync_batches_new (
    batch_id, device_id, direction, status, item_count, 
    total_size, priority, attempts, last_attempt_at,
    error_message, created_at, completed_at
)
SELECT 
    batch_id, 'legacy_device', direction, status, item_count,
    total_size, priority, attempts, last_attempt_at,
    error_message, created_at, completed_at
FROM sync_batches;

-- Drop old table
DROP TABLE sync_batches;

-- Rename new table to original name
ALTER TABLE sync_batches_new RENAME TO sync_batches;

-- Recreate indices
CREATE INDEX IF NOT EXISTS idx_sync_batches_status_priority ON sync_batches(status, priority, created_at);
CREATE INDEX IF NOT EXISTS idx_sync_batches_direction ON sync_batches(direction, status);
-- Add new device-specific index
CREATE INDEX IF NOT EXISTS idx_sync_batches_device ON sync_batches(device_id, status);

-- Add device metadata table for tracking known devices
CREATE TABLE IF NOT EXISTS device_metadata (
    device_id TEXT PRIMARY KEY,
    name TEXT NOT NULL,           -- User-friendly device name
    platform TEXT NOT NULL,       -- iOS, iPadOS, macOS, etc.
    model TEXT,                   -- Device model information
    os_version TEXT,              -- OS version information
    app_version TEXT NOT NULL,    -- App version when last used
    last_active_at TEXT,          -- When the device was last active
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Add index for last active devices
CREATE INDEX IF NOT EXISTS idx_device_metadata_last_active ON device_metadata(last_active_at DESC);

-- Add a field to track which device created a change_log entry (if not already populated)
-- SQLite doesn't support checking if a column exists, so we'll rely on the migration tool
-- to handle this properly

-- Enable foreign keys again
PRAGMA foreign_keys = ON;