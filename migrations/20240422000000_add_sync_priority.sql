-- Add sync_priority column to media_documents
ALTER TABLE media_documents ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 1; -- Default to Normal

-- Add sync_priority column to strategic_goals
ALTER TABLE strategic_goals ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 1; -- Default to Normal

-- Add sync_priority column to projects
ALTER TABLE projects ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 1; -- Default to Normal

-- Add sync_priority column to workshops
ALTER TABLE workshops ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 1; -- Default to Normal

-- Add sync_priority column to participants
ALTER TABLE participants ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 1; -- Default to Normal

-- Add sync_priority column to livelihoods
ALTER TABLE livelihoods ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 1; -- Default to Normal

-- Add sync_priority column to subsequent_grants
ALTER TABLE subsequent_grants ADD COLUMN sync_priority INTEGER NOT NULL DEFAULT 1; -- Default to Normal

-- NOTE: Add to other relevant tables if necessary (e.g., activities, donors) 