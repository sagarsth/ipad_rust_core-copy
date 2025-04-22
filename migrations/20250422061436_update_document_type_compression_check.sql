-- Add migration script here

-- SQLite does not support ALTER TABLE ADD CONSTRAINT CHECK directly.
-- We must recreate the table with the new constraint.

-- Step 1: Disable foreign keys (Necessary because we drop/recreate the table)
PRAGMA foreign_keys=off;

-- Step 2: Start transaction -- REMOVED (sqlx-cli handles the transaction)
-- BEGIN TRANSACTION; -- REMOVED

-- Step 3: Create the new table with the desired schema including the CHECK constraint
CREATE TABLE document_types_new (
    id TEXT PRIMARY KEY, -- User-defined key
    name TEXT NOT NULL,
    name_updated_at TEXT,
    name_updated_by TEXT,
    allowed_extensions TEXT NOT NULL,
    allowed_extensions_updated_at TEXT,
    allowed_extensions_updated_by TEXT,
    max_size INTEGER NOT NULL,
    max_size_updated_at TEXT,
    max_size_updated_by TEXT,
    compression_level INTEGER NOT NULL DEFAULT 6,
    compression_level_updated_at TEXT,
    compression_level_updated_by TEXT,
    -- Apply the new CHECK constraint
    compression_method TEXT DEFAULT 'lossless' 
        CHECK(compression_method IN ('lossless', 'lossy', 'pdf_optimize', 'office_optimize', 'none')),
    compression_method_updated_at TEXT,
    compression_method_updated_by TEXT,
    min_size_for_compression INTEGER DEFAULT 10240,
    min_size_for_compression_updated_at TEXT,
    min_size_for_compression_updated_by TEXT,
    description TEXT,
    description_updated_at TEXT,
    description_updated_by TEXT,
    default_priority TEXT NOT NULL DEFAULT 'normal' CHECK(default_priority IN ('high', 'normal', 'low', 'never')),
    default_priority_updated_at TEXT,
    default_priority_updated_by TEXT,
    icon TEXT,
    icon_updated_at TEXT,
    icon_updated_by TEXT,
    related_tables TEXT,
    related_tables_updated_at TEXT,
    related_tables_updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (name_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (allowed_extensions_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (max_size_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (compression_level_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (compression_method_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (min_size_for_compression_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (description_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (default_priority_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (icon_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (related_tables_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

-- Step 4: Copy data from the old table to the new table
-- Ensure all columns from the original table are included here
INSERT INTO document_types_new (
    id, name, name_updated_at, name_updated_by, 
    allowed_extensions, allowed_extensions_updated_at, allowed_extensions_updated_by,
    max_size, max_size_updated_at, max_size_updated_by,
    compression_level, compression_level_updated_at, compression_level_updated_by,
    compression_method, compression_method_updated_at, compression_method_updated_by,
    min_size_for_compression, min_size_for_compression_updated_at, min_size_for_compression_updated_by,
    description, description_updated_at, description_updated_by,
    default_priority, default_priority_updated_at, default_priority_updated_by,
    icon, icon_updated_at, icon_updated_by,
    related_tables, related_tables_updated_at, related_tables_updated_by,
    created_at, updated_at, created_by_user_id, updated_by_user_id,
    deleted_at, deleted_by_user_id
)
SELECT 
    id, name, name_updated_at, name_updated_by, 
    allowed_extensions, allowed_extensions_updated_at, allowed_extensions_updated_by,
    max_size, max_size_updated_at, max_size_updated_by,
    compression_level, compression_level_updated_at, compression_level_updated_by,
    compression_method, compression_method_updated_at, compression_method_updated_by,
    min_size_for_compression, min_size_for_compression_updated_at, min_size_for_compression_updated_by,
    description, description_updated_at, description_updated_by,
    default_priority, default_priority_updated_at, default_priority_updated_by,
    icon, icon_updated_at, icon_updated_by,
    related_tables, related_tables_updated_at, related_tables_updated_by,
    created_at, updated_at, created_by_user_id, updated_by_user_id,
    deleted_at, deleted_by_user_id
FROM document_types;

-- Step 5: Drop the old table
DROP TABLE document_types;

-- Step 6: Rename the new table to the original name
ALTER TABLE document_types_new RENAME TO document_types;

-- Step 7: Recreate indexes that were on the original table
CREATE UNIQUE INDEX IF NOT EXISTS idx_document_types_name ON document_types(name) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_document_types_updated_at ON document_types(updated_at);
CREATE INDEX IF NOT EXISTS idx_document_types_deleted_at ON document_types(deleted_at);
CREATE INDEX IF NOT EXISTS idx_document_types_created_by ON document_types(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_document_types_updated_by ON document_types(updated_by_user_id);

-- Step 8: Commit the transaction -- REMOVED (sqlx-cli handles the transaction)
-- COMMIT; -- REMOVED

-- Step 9: Re-enable foreign keys
PRAGMA foreign_keys=on;
