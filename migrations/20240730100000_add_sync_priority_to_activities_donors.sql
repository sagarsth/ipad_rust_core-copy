-- Add sync_priority column to activities
ALTER TABLE activities ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 5; -- Default to Normal (5)

-- Add sync_priority column to donors
ALTER TABLE donors ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 5; -- Default to Normal (5) 