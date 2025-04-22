-- Migration to add back evaluation columns to workshop_participants
-- Created at: 2024-04-20
-- Description: Adds pre_evaluation and post_evaluation columns back to workshop_participants table

PRAGMA foreign_keys=off;

-- Modify workshop_participants table to add evaluation columns
CREATE TABLE workshop_participants_new (
    id TEXT PRIMARY KEY,
    workshop_id TEXT NOT NULL,
    participant_id TEXT NULL,
    notes TEXT,
    notes_updated_at TEXT,
    notes_updated_by TEXT,
    pre_evaluation TEXT,
    pre_evaluation_updated_at TEXT,
    pre_evaluation_updated_by TEXT,
    post_evaluation TEXT,
    post_evaluation_updated_at TEXT,
    post_evaluation_updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
    FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE SET NULL,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (notes_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (pre_evaluation_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (post_evaluation_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    UNIQUE(workshop_id, participant_id) ON CONFLICT REPLACE
);

-- Copy existing data, preserving existing columns
INSERT INTO workshop_participants_new 
SELECT 
    id,
    workshop_id,
    participant_id,
    notes,
    notes_updated_at,
    notes_updated_by,
    NULL as pre_evaluation,
    NULL as pre_evaluation_updated_at,
    NULL as pre_evaluation_updated_by,
    NULL as post_evaluation,
    NULL as post_evaluation_updated_at,
    NULL as post_evaluation_updated_by,
    created_at,
    updated_at,
    created_by_user_id,
    updated_by_user_id,
    deleted_at,
    deleted_by_user_id
FROM workshop_participants;

DROP TABLE workshop_participants;
ALTER TABLE workshop_participants_new RENAME TO workshop_participants;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_workshop_participants_workshop ON workshop_participants(workshop_id);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_participant ON workshop_participants(participant_id);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_updated_at ON workshop_participants(updated_at);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_deleted_at ON workshop_participants(deleted_at);

PRAGMA foreign_keys=on; 