-- Updated Schema Migration
-- Created at: 2025-04-04 
-- Based on original schema from 2024-03-20
-- With foreign key constraint updates

-- Enable foreign keys
PRAGMA foreign_keys = OFF;

-- #############################################################
-- ##          Phase 1: Update Foreign Key Constraints         ##
-- #############################################################

-- 1. For projects table (change CASCADE to RESTRICT for strategic_goal_id)
CREATE TABLE projects_new (
    id TEXT PRIMARY KEY,
    strategic_goal_id TEXT NOT NULL,
    name TEXT NOT NULL,
    name_updated_at TEXT,
    name_updated_by TEXT,
    objective TEXT,
    objective_updated_at TEXT,
    objective_updated_by TEXT,
    outcome TEXT,
    outcome_updated_at TEXT,
    outcome_updated_by TEXT,
    status_id INTEGER,
    status_id_updated_at TEXT,
    status_id_updated_by TEXT,
    timeline TEXT,
    timeline_updated_at TEXT,
    timeline_updated_by TEXT,
    responsible_team TEXT,
    responsible_team_updated_at TEXT,
    responsible_team_updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    -- Change CASCADE to RESTRICT for strategic_goal_id
    FOREIGN KEY (strategic_goal_id) REFERENCES strategic_goals(id) ON DELETE RESTRICT,
    FOREIGN KEY (status_id) REFERENCES status_types(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (name_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (objective_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (outcome_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (timeline_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (responsible_team_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

-- Copy data from old table to new table
INSERT INTO projects_new SELECT * FROM projects;

-- Drop the old table
DROP TABLE projects;

-- Rename the new table to the original name
ALTER TABLE projects_new RENAME TO projects;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_projects_strategic_goal ON projects(strategic_goal_id);
CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at);
CREATE INDEX IF NOT EXISTS idx_projects_deleted_at ON projects(deleted_at);
CREATE INDEX IF NOT EXISTS idx_projects_created_by ON projects(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_by ON projects(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);

-- 2. For workshops table (change CASCADE to RESTRICT for project_id)
CREATE TABLE workshops_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    purpose TEXT,
    purpose_updated_at TEXT,
    purpose_updated_by TEXT,
    event_date TEXT,
    event_date_updated_at TEXT,
    event_date_updated_by TEXT,
    location TEXT,
    location_updated_at TEXT,
    location_updated_by TEXT,
    budget REAL,
    budget_updated_at TEXT,
    budget_updated_by TEXT,
    actuals REAL,
    actuals_updated_at TEXT,
    actuals_updated_by TEXT,
    participant_count INTEGER DEFAULT 0,
    participant_count_updated_at TEXT,
    participant_count_updated_by TEXT,
    local_partner TEXT,
    local_partner_updated_at TEXT,
    local_partner_updated_by TEXT,
    partner_responsibility TEXT,
    partner_responsibility_updated_at TEXT,
    partner_responsibility_updated_by TEXT,
    partnership_success TEXT,
    partnership_success_updated_at TEXT,
    partnership_success_updated_by TEXT,
    capacity_challenges TEXT,
    capacity_challenges_updated_at TEXT,
    capacity_challenges_updated_by TEXT,
    strengths TEXT,
    strengths_updated_at TEXT,
    strengths_updated_by TEXT,
    outcomes TEXT,
    outcomes_updated_at TEXT,
    outcomes_updated_by TEXT,
    recommendations TEXT,
    recommendations_updated_at TEXT,
    recommendations_updated_by TEXT,
    challenge_resolution TEXT,
    challenge_resolution_updated_at TEXT,
    challenge_resolution_updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    -- Change CASCADE to RESTRICT for project_id
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (participant_count_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (purpose_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (event_date_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (location_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (budget_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (actuals_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (local_partner_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (partner_responsibility_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (partnership_success_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (capacity_challenges_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (strengths_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (outcomes_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (recommendations_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (challenge_resolution_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

INSERT INTO workshops_new SELECT * FROM workshops;
DROP TABLE workshops;
ALTER TABLE workshops_new RENAME TO workshops;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_workshops_project ON workshops(project_id);
CREATE INDEX IF NOT EXISTS idx_workshops_event_date ON workshops(event_date);
CREATE INDEX IF NOT EXISTS idx_workshops_location ON workshops(location);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_at ON workshops(updated_at);
CREATE INDEX IF NOT EXISTS idx_workshops_deleted_at ON workshops(deleted_at);
CREATE INDEX IF NOT EXISTS idx_workshops_created_by ON workshops(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_by ON workshops(updated_by_user_id);

-- 3. For project_funding table (change CASCADE to RESTRICT for project_id)
CREATE TABLE project_funding_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    project_id_updated_at TEXT,
    project_id_updated_by TEXT,
    donor_id TEXT NOT NULL,
    donor_id_updated_at TEXT,
    donor_id_updated_by TEXT,
    grant_id TEXT,
    grant_id_updated_at TEXT,
    grant_id_updated_by TEXT,
    amount REAL,
    amount_updated_at TEXT,
    amount_updated_by TEXT,
    currency TEXT DEFAULT 'AUD',
    currency_updated_at TEXT,
    currency_updated_by TEXT,
    start_date TEXT,
    start_date_updated_at TEXT,
    start_date_updated_by TEXT,
    end_date TEXT,
    end_date_updated_at TEXT,
    end_date_updated_by TEXT,
    status TEXT,
    status_updated_at TEXT,
    status_updated_by TEXT,
    reporting_requirements TEXT,
    reporting_requirements_updated_at TEXT,
    reporting_requirements_updated_by TEXT,
    notes TEXT,
    notes_updated_at TEXT,
    notes_updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    -- Change CASCADE to RESTRICT for project_id
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY (donor_id) REFERENCES donors(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (project_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (donor_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (grant_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (amount_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (currency_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (start_date_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (end_date_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (reporting_requirements_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (notes_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

INSERT INTO project_funding_new SELECT * FROM project_funding;
DROP TABLE project_funding;
ALTER TABLE project_funding_new RENAME TO project_funding;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_project_funding_project ON project_funding(project_id);
CREATE INDEX IF NOT EXISTS idx_project_funding_donor ON project_funding(donor_id);
CREATE INDEX IF NOT EXISTS idx_project_funding_status ON project_funding(status);
CREATE INDEX IF NOT EXISTS idx_project_funding_updated_at ON project_funding(updated_at);
CREATE INDEX IF NOT EXISTS idx_project_funding_deleted_at ON project_funding(deleted_at);
CREATE INDEX IF NOT EXISTS idx_project_funding_created_by ON project_funding(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_project_funding_updated_by ON project_funding(updated_by_user_id);

-- 4. For livelihoods table (change CASCADE to RESTRICT for project_id)
CREATE TABLE livelihoods_new (
    id TEXT PRIMARY KEY,
    participant_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    grant_amount REAL,
    grant_amount_updated_at TEXT,
    grant_amount_updated_by TEXT,
    purpose TEXT,
    purpose_updated_at TEXT,
    purpose_updated_by TEXT,
    progress1 TEXT,
    progress1_updated_at TEXT,
    progress1_updated_by TEXT,
    progress2 TEXT,
    progress2_updated_at TEXT,
    progress2_updated_by TEXT,
    outcome TEXT,
    outcome_updated_at TEXT,
    outcome_updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    -- Keep CASCADE for participant_id but change project_id to RESTRICT
    FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (grant_amount_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (purpose_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (progress1_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (progress2_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (outcome_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

INSERT INTO livelihoods_new SELECT * FROM livelihoods;
DROP TABLE livelihoods;
ALTER TABLE livelihoods_new RENAME TO livelihoods;

-- Recreate indexes
CREATE INDEX IF NOT EXISTS idx_livelihoods_participant ON livelihoods(participant_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_project ON livelihoods(project_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_at ON livelihoods(updated_at);
CREATE INDEX IF NOT EXISTS idx_livelihoods_deleted_at ON livelihoods(deleted_at);
CREATE INDEX IF NOT EXISTS idx_livelihoods_created_by ON livelihoods(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_by ON livelihoods(updated_by_user_id);

-- #############################################################
-- ##          Summarize the changes                          ##
-- #############################################################
-- The following foreign key constraint changes were made:
--
-- 1. Changed strategic_goal_id in projects table from ON DELETE CASCADE to ON DELETE RESTRICT
-- 2. Changed project_id in workshops table from ON DELETE CASCADE to ON DELETE RESTRICT
-- 3. Changed project_id in project_funding table from ON DELETE CASCADE to ON DELETE RESTRICT
-- 4. Changed project_id in livelihoods table from ON DELETE CASCADE to ON DELETE RESTRICT
--
-- The following foreign key constraints were kept as ON DELETE CASCADE:
-- - activities.project_id referencing projects.id
-- - workshop_participants.workshop_id referencing workshops.id
-- - workshop_participants.participant_id referencing participants.id
-- - livelihoods.participant_id referencing participants.id
-- - subsequent_grants.livelihood_id referencing livelihoods.id

-- Turn foreign keys back on
PRAGMA foreign_keys = ON;