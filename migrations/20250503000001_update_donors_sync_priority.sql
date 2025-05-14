-- Migration: Standardize Sync Priority for Donors table
-- Timestamp: 20250503000001

PRAGMA foreign_keys=OFF;

-- --------------------------------------------
-- Table: donors
-- Description: Change sync_priority from INTEGER to TEXT and map existing values.
-- Existing sync_priority was INTEGER DEFAULT 5 (meaning 'normal').
-- New sync_priority is TEXT with values 'high', 'normal', 'low', 'never'.
-- Default for new records will be 'high'.
-- --------------------------------------------
CREATE TABLE donors_new (
    id TEXT PRIMARY KEY, -- UUID
    name TEXT NOT NULL,
    name_updated_at TEXT,
    name_updated_by TEXT,
    type TEXT,
    type_updated_at TEXT,
    type_updated_by TEXT,
    contact_person TEXT,
    contact_person_updated_at TEXT,
    contact_person_updated_by TEXT,
    email TEXT,
    email_updated_at TEXT,
    email_updated_by TEXT,
    phone TEXT,
    phone_updated_at TEXT,
    phone_updated_by TEXT,
    country TEXT,
    country_updated_at TEXT,
    country_updated_by TEXT,
    first_donation_date TEXT,
    first_donation_date_updated_at TEXT,
    first_donation_date_updated_by TEXT,
    notes TEXT,
    notes_updated_at TEXT,
    notes_updated_by TEXT,
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')), -- Changed type, new default
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
    FOREIGN KEY (type_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (contact_person_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (email_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (phone_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (country_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (first_donation_date_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (notes_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

-- Copy data from old table to new table, transforming sync_priority
INSERT INTO donors_new (
    id, name, name_updated_at, name_updated_by, type, type_updated_at, type_updated_by,
    contact_person, contact_person_updated_at, contact_person_updated_by, email, email_updated_at, email_updated_by,
    phone, phone_updated_at, phone_updated_by, country, country_updated_at, country_updated_by,
    first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by,
    notes, notes_updated_at, notes_updated_by,
    sync_priority, -- Target column for transformed value
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, name, name_updated_at, name_updated_by, type, type_updated_at, type_updated_by,
    contact_person, contact_person_updated_at, contact_person_updated_by, email, email_updated_at, email_updated_by,
    phone, phone_updated_at, phone_updated_by, country, country_updated_at, country_updated_by,
    first_donation_date, first_donation_date_updated_at, first_donation_date_updated_by,
    notes, notes_updated_at, notes_updated_by,
    CASE
        WHEN sync_priority = 5 THEN 'normal' -- Map old INTEGER value 5 to 'normal'
        ELSE 'high' -- Default for any other existing INTEGER values or if it was NULL
    END,
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM donors;

-- Drop the old donors table
DROP TABLE donors;

-- Rename the new table to the original name
ALTER TABLE donors_new RENAME TO donors;

-- Recreate Indexes for donors table
CREATE INDEX IF NOT EXISTS idx_donors_name ON donors(name);
CREATE INDEX IF NOT EXISTS idx_donors_type ON donors(type);
CREATE INDEX IF NOT EXISTS idx_donors_country ON donors(country);
CREATE INDEX IF NOT EXISTS idx_donors_updated_at ON donors(updated_at);
CREATE INDEX IF NOT EXISTS idx_donors_deleted_at ON donors(deleted_at);
CREATE INDEX IF NOT EXISTS idx_donors_created_by ON donors(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_donors_updated_by ON donors(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_donors_sync_priority ON donors(sync_priority); -- Index on the new TEXT column

PRAGMA foreign_keys=ON; 