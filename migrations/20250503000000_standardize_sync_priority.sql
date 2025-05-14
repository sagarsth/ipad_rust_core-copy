-- Migration: Standardize Sync Priority and Remove sync_priority_level
-- Timestamp: 20250503000000

PRAGMA foreign_keys=OFF;

-- =====================================================================================
-- Step 0: Explicitly drop index dependent on sync_priority_level for media_documents
-- =====================================================================================
DROP INDEX IF EXISTS idx_media_documents_sync_priority_level;

-- =====================================================================================
-- Step 1: Drop sync_priority_level from tables where it was added
-- =====================================================================================

-- For media_documents (will be recreated later for type change, but drop level here)
ALTER TABLE media_documents DROP COLUMN sync_priority_level;

-- For other tables, they will be recreated, so sync_priority_level is implicitly removed.

-- =====================================================================================
-- Step 2 & 3: Recreate tables to change sync_priority type and add to activities
-- Defaulting existing records' sync_priority to 'high' for these entity tables.
-- =====================================================================================

-- --------------------------------------------
-- Table: strategic_goals
-- --------------------------------------------
CREATE TABLE strategic_goals_new (
    id TEXT PRIMARY KEY,
    objective_code TEXT NOT NULL,
    objective_code_updated_at TEXT,
    objective_code_updated_by TEXT,
    outcome TEXT,
    outcome_updated_at TEXT,
    outcome_updated_by TEXT,
    kpi TEXT,
    kpi_updated_at TEXT,
    kpi_updated_by TEXT,
    target_value REAL,
    target_value_updated_at TEXT,
    target_value_updated_by TEXT,
    actual_value REAL DEFAULT 0,
    actual_value_updated_at TEXT,
    actual_value_updated_by TEXT,
    status_id INTEGER,
    status_id_updated_at TEXT,
    status_id_updated_by TEXT,
    responsible_team TEXT,
    responsible_team_updated_at TEXT,
    responsible_team_updated_by TEXT,
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (status_id) REFERENCES status_types(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (objective_code_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (outcome_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (kpi_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (target_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (actual_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (responsible_team_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT INTO strategic_goals_new (
    id, objective_code, objective_code_updated_at, objective_code_updated_by, outcome, outcome_updated_at, outcome_updated_by,
    kpi, kpi_updated_at, kpi_updated_by, target_value, target_value_updated_at, target_value_updated_by,
    actual_value, actual_value_updated_at, actual_value_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    responsible_team, responsible_team_updated_at, responsible_team_updated_by,
    sync_priority, -- Set to 'high' for existing
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, objective_code, objective_code_updated_at, objective_code_updated_by, outcome, outcome_updated_at, outcome_updated_by,
    kpi, kpi_updated_at, kpi_updated_by, target_value, target_value_updated_at, target_value_updated_by,
    actual_value, actual_value_updated_at, actual_value_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    responsible_team, responsible_team_updated_at, responsible_team_updated_by,
    'high', -- Default existing to 'high'
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM strategic_goals;
DROP TABLE strategic_goals;
ALTER TABLE strategic_goals_new RENAME TO strategic_goals;

-- Recreate Indexes for strategic_goals
CREATE INDEX IF NOT EXISTS idx_strategic_goals_status ON strategic_goals(status_id);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_updated_at ON strategic_goals(updated_at);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_deleted_at ON strategic_goals(deleted_at);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_created_by ON strategic_goals(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_updated_by ON strategic_goals(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_objective_code ON strategic_goals(objective_code);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_sync_priority ON strategic_goals(sync_priority); -- New index

-- --------------------------------------------
-- Table: projects
-- --------------------------------------------
CREATE TABLE projects_new (
    id TEXT PRIMARY KEY,
    strategic_goal_id TEXT NULL,
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
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
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
INSERT INTO projects_new (
    id, strategic_goal_id, name, name_updated_at, name_updated_by, objective, objective_updated_at, objective_updated_by,
    outcome, outcome_updated_at, outcome_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    timeline, timeline_updated_at, timeline_updated_by, responsible_team, responsible_team_updated_at, responsible_team_updated_by,
    sync_priority, -- Set to 'high' for existing
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, strategic_goal_id, name, name_updated_at, name_updated_by, objective, objective_updated_at, objective_updated_by,
    outcome, outcome_updated_at, outcome_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    timeline, timeline_updated_at, timeline_updated_by, responsible_team, responsible_team_updated_at, responsible_team_updated_by,
    'high', -- Default existing to 'high'
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM projects;
DROP TABLE projects;
ALTER TABLE projects_new RENAME TO projects;

-- Recreate Indexes for projects
CREATE INDEX IF NOT EXISTS idx_projects_strategic_goal ON projects(strategic_goal_id);
CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at);
CREATE INDEX IF NOT EXISTS idx_projects_deleted_at ON projects(deleted_at);
CREATE INDEX IF NOT EXISTS idx_projects_created_by ON projects(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_by ON projects(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);
CREATE INDEX IF NOT EXISTS idx_projects_sync_priority ON projects(sync_priority); -- New index

-- --------------------------------------------
-- Table: activities
-- --------------------------------------------
CREATE TABLE activities_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NULL,
    description TEXT,
    description_updated_at TEXT,
    description_updated_by TEXT,
    kpi TEXT,
    kpi_updated_at TEXT,
    kpi_updated_by TEXT,
    target_value REAL,
    target_value_updated_at TEXT,
    target_value_updated_by TEXT,
    actual_value REAL DEFAULT 0,
    actual_value_updated_at TEXT,
    actual_value_updated_by TEXT,
    status_id INTEGER,
    status_id_updated_at TEXT,
    status_id_updated_by TEXT,
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (status_id) REFERENCES status_types(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (description_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (kpi_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (target_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (actual_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_id_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT INTO activities_new (
    id, project_id, description, description_updated_at, description_updated_by,
    kpi, kpi_updated_at, kpi_updated_by, target_value, target_value_updated_at, target_value_updated_by,
    actual_value, actual_value_updated_at, actual_value_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    sync_priority, -- Set to 'high' for existing
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, project_id, description, description_updated_at, description_updated_by,
    kpi, kpi_updated_at, kpi_updated_by, target_value, target_value_updated_at, target_value_updated_by,
    actual_value, actual_value_updated_at, actual_value_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    'high', -- Default existing to 'high'
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM activities;
DROP TABLE activities;
ALTER TABLE activities_new RENAME TO activities;

-- Recreate Indexes for activities
CREATE INDEX IF NOT EXISTS idx_activities_project ON activities(project_id);
CREATE INDEX IF NOT EXISTS idx_activities_status ON activities(status_id);
CREATE INDEX IF NOT EXISTS idx_activities_updated_at ON activities(updated_at);
CREATE INDEX IF NOT EXISTS idx_activities_deleted_at ON activities(deleted_at);
CREATE INDEX IF NOT EXISTS idx_activities_created_by ON activities(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_activities_updated_by ON activities(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_activities_sync_priority ON activities(sync_priority); -- New index

-- --------------------------------------------
-- Table: participants
-- --------------------------------------------
CREATE TABLE participants_new (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    name_updated_at TEXT,
    name_updated_by TEXT,
    gender TEXT,
    gender_updated_at TEXT,
    gender_updated_by TEXT,
    disability INTEGER DEFAULT 0,
    disability_updated_at TEXT,
    disability_updated_by TEXT,
    disability_type TEXT DEFAULT NULL,
    disability_type_updated_at TEXT,
    disability_type_updated_by TEXT,
    age_group TEXT,
    age_group_updated_at TEXT,
    age_group_updated_by TEXT,
    location TEXT,
    location_updated_at TEXT,
    location_updated_by TEXT,
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
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
    FOREIGN KEY (gender_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (disability_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (disability_type_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (age_group_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (location_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT INTO participants_new (
    id, name, name_updated_at, name_updated_by, gender, gender_updated_at, gender_updated_by,
    disability, disability_updated_at, disability_updated_by, disability_type, disability_type_updated_at, disability_type_updated_by,
    age_group, age_group_updated_at, age_group_updated_by, location, location_updated_at, location_updated_by,
    sync_priority, -- Set to 'high' for existing
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, name, name_updated_at, name_updated_by, gender, gender_updated_at, gender_updated_by,
    disability, disability_updated_at, disability_updated_by, disability_type, disability_type_updated_at, disability_type_updated_by,
    age_group, age_group_updated_at, age_group_updated_by, location, location_updated_at, location_updated_by,
    'high', -- Default existing to 'high'
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM participants;
DROP TABLE participants;
ALTER TABLE participants_new RENAME TO participants;

-- Recreate Indexes for participants
CREATE INDEX IF NOT EXISTS idx_participants_location ON participants(location);
CREATE INDEX IF NOT EXISTS idx_participants_gender_age ON participants(gender, age_group);
CREATE INDEX IF NOT EXISTS idx_participants_updated_at ON participants(updated_at);
CREATE INDEX IF NOT EXISTS idx_participants_deleted_at ON participants(deleted_at);
CREATE INDEX IF NOT EXISTS idx_participants_created_by ON participants(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_participants_updated_by ON participants(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_participants_name ON participants(name);
CREATE INDEX IF NOT EXISTS idx_participants_sync_priority ON participants(sync_priority); -- New index

-- --------------------------------------------
-- Table: workshops
-- --------------------------------------------
CREATE TABLE workshops_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NULL,
    title TEXT NOT NULL,
    title_updated_at TEXT,
    title_updated_by TEXT,
    objective TEXT,
    objective_updated_at TEXT,
    objective_updated_by TEXT,
    rationale TEXT,
    rationale_updated_at TEXT,
    rationale_updated_by TEXT,
    methodology TEXT,
    methodology_updated_at TEXT,
    methodology_updated_by TEXT,
    facilitator TEXT,
    facilitator_updated_at TEXT,
    facilitator_updated_by TEXT,
    event_date TEXT,
    event_date_updated_at TEXT,
    event_date_updated_by TEXT,
    start_time TEXT,
    start_time_updated_at TEXT,
    start_time_updated_by TEXT,
    end_time TEXT,
    end_time_updated_at TEXT,
    end_time_updated_by TEXT,
    location TEXT,
    location_updated_at TEXT,
    location_updated_by TEXT,
    total_male_participants INTEGER DEFAULT 0,
    total_male_participants_updated_at TEXT,
    total_male_participants_updated_by TEXT,
    total_female_participants INTEGER DEFAULT 0,
    total_female_participants_updated_at TEXT,
    total_female_participants_updated_by TEXT,
    total_other_participants INTEGER DEFAULT 0,
    total_other_participants_updated_at TEXT,
    total_other_participants_updated_by TEXT,
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (title_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (objective_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (rationale_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (methodology_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (facilitator_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (event_date_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (start_time_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (end_time_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (location_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (total_male_participants_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (total_female_participants_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (total_other_participants_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT INTO workshops_new (
    id, project_id, title, title_updated_at, title_updated_by, objective, objective_updated_at, objective_updated_by,
    rationale, rationale_updated_at, rationale_updated_by, methodology, methodology_updated_at, methodology_updated_by,
    facilitator, facilitator_updated_at, facilitator_updated_by, event_date, event_date_updated_at, event_date_updated_by,
    start_time, start_time_updated_at, start_time_updated_by, end_time, end_time_updated_at, end_time_updated_by,
    location, location_updated_at, location_updated_by, total_male_participants, total_male_participants_updated_at, total_male_participants_updated_by,
    total_female_participants, total_female_participants_updated_at, total_female_participants_updated_by,
    total_other_participants, total_other_participants_updated_at, total_other_participants_updated_by,
    sync_priority, -- Set to 'high' for existing
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, project_id, title, title_updated_at, title_updated_by, objective, objective_updated_at, objective_updated_by,
    rationale, rationale_updated_at, rationale_updated_by, methodology, methodology_updated_at, methodology_updated_by,
    facilitator, facilitator_updated_at, facilitator_updated_by, event_date, event_date_updated_at, event_date_updated_by,
    start_time, start_time_updated_at, start_time_updated_by, end_time, end_time_updated_at, end_time_updated_by,
    location, location_updated_at, location_updated_by, total_male_participants, total_male_participants_updated_at, total_male_participants_updated_by,
    total_female_participants, total_female_participants_updated_at, total_female_participants_updated_by,
    total_other_participants, total_other_participants_updated_at, total_other_participants_updated_by,
    'high', -- Default existing to 'high'
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM workshops;
DROP TABLE workshops;
ALTER TABLE workshops_new RENAME TO workshops;

-- Recreate Indexes for workshops
CREATE INDEX IF NOT EXISTS idx_workshops_project ON workshops(project_id);
CREATE INDEX IF NOT EXISTS idx_workshops_event_date ON workshops(event_date);
CREATE INDEX IF NOT EXISTS idx_workshops_location ON workshops(location);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_at ON workshops(updated_at);
CREATE INDEX IF NOT EXISTS idx_workshops_deleted_at ON workshops(deleted_at);
CREATE INDEX IF NOT EXISTS idx_workshops_created_by ON workshops(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_by ON workshops(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_workshops_sync_priority ON workshops(sync_priority); -- New index

-- --------------------------------------------
-- Table: livelihoods
-- --------------------------------------------
CREATE TABLE livelihoods_new (
    id TEXT PRIMARY KEY,
    participant_id TEXT NULL,
    project_id TEXT NULL,
    type TEXT NOT NULL,
    type_updated_at TEXT,
    type_updated_by TEXT,
    description TEXT,
    description_updated_at TEXT,
    description_updated_by TEXT,
    status_id INTEGER,
    status_id_updated_at TEXT,
    status_id_updated_by TEXT,
    initial_grant_date TEXT,
    initial_grant_date_updated_at TEXT,
    initial_grant_date_updated_by TEXT,
    initial_grant_amount REAL,
    initial_grant_amount_updated_at TEXT,
    initial_grant_amount_updated_by TEXT,
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE RESTRICT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT,
    FOREIGN KEY (status_id) REFERENCES status_types(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (type_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (description_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (initial_grant_date_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (initial_grant_amount_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT INTO livelihoods_new (
    id, participant_id, project_id, type, type_updated_at, type_updated_by,
    description, description_updated_at, description_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    initial_grant_date, initial_grant_date_updated_at, initial_grant_date_updated_by,
    initial_grant_amount, initial_grant_amount_updated_at, initial_grant_amount_updated_by,
    sync_priority, -- Set to 'high' for existing
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, participant_id, project_id, type, type_updated_at, type_updated_by,
    description, description_updated_at, description_updated_by, status_id, status_id_updated_at, status_id_updated_by,
    initial_grant_date, initial_grant_date_updated_at, initial_grant_date_updated_by,
    initial_grant_amount, initial_grant_amount_updated_at, initial_grant_amount_updated_by,
    'high', -- Default existing to 'high'
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM livelihoods;
DROP TABLE livelihoods;
ALTER TABLE livelihoods_new RENAME TO livelihoods;

-- Recreate Indexes for livelihoods
CREATE INDEX IF NOT EXISTS idx_livelihoods_participant ON livelihoods(participant_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_project ON livelihoods(project_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_at ON livelihoods(updated_at);
CREATE INDEX IF NOT EXISTS idx_livelihoods_deleted_at ON livelihoods(deleted_at);
CREATE INDEX IF NOT EXISTS idx_livelihoods_created_by ON livelihoods(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_by ON livelihoods(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_sync_priority ON livelihoods(sync_priority); -- New index

-- --------------------------------------------
-- Table: subsequent_grants
-- --------------------------------------------
CREATE TABLE subsequent_grants_new (
    id TEXT PRIMARY KEY,
    livelihood_id TEXT NOT NULL,
    amount REAL,
    amount_updated_at TEXT,
    amount_updated_by TEXT,
    purpose TEXT,
    purpose_updated_at TEXT,
    purpose_updated_by TEXT,
    grant_date TEXT,
    grant_date_updated_at TEXT,
    grant_date_updated_by TEXT,
    sync_priority TEXT NOT NULL DEFAULT 'high' CHECK(sync_priority IN ('high', 'normal', 'low', 'never')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (livelihood_id) REFERENCES livelihoods(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (amount_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (purpose_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (grant_date_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT INTO subsequent_grants_new (
    id, livelihood_id, amount, amount_updated_at, amount_updated_by,
    purpose, purpose_updated_at, purpose_updated_by, grant_date, grant_date_updated_at, grant_date_updated_by,
    sync_priority, -- Set to 'high' for existing
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
)
SELECT
    id, livelihood_id, amount, amount_updated_at, amount_updated_by,
    purpose, purpose_updated_at, purpose_updated_by, grant_date, grant_date_updated_at, grant_date_updated_by,
    'high', -- Default existing to 'high'
    created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id
FROM subsequent_grants;
DROP TABLE subsequent_grants;
ALTER TABLE subsequent_grants_new RENAME TO subsequent_grants;

-- Recreate Indexes for subsequent_grants
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_livelihood ON subsequent_grants(livelihood_id);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_date ON subsequent_grants(grant_date);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_updated_at ON subsequent_grants(updated_at);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_deleted_at ON subsequent_grants(deleted_at);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_created_by ON subsequent_grants(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_updated_by ON subsequent_grants(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_sync_priority ON subsequent_grants(sync_priority); -- New index

-- --------------------------------------------
-- Table: media_documents (already altered to drop sync_priority_level)
-- The sync_priority column type and default ('normal') are already correct.
-- No data migration needed for its sync_priority as it's already TEXT.
-- Just ensure indexes are correct after any implicit recreation by SQLite if other operations were done.
-- (No other operations here, so existing sync_priority index should be fine.)
-- --------------------------------------------

-- Ensure all relevant indexes for media_documents are present (especially sync_priority)
CREATE INDEX IF NOT EXISTS idx_media_documents_sync_priority ON media_documents(sync_priority);


-- =====================================================================================
-- Step 4: Re-enable Foreign Keys
-- =====================================================================================

PRAGMA foreign_keys=ON; 