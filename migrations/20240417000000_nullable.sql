-- Migration to make certain foreign key relationships optional (nullable)
-- Goal: Allow records in these tables to exist without being linked to their parent.
-- IMPORTANT: Ensure corresponding Rust types are updated to Option<Uuid>.

PRAGMA foreign_keys=off; -- Disable FK constraints during table modifications


-- 1. Modify 'projects' table: make 'strategic_goal_id' NULLable
CREATE TABLE projects_new (
    id TEXT PRIMARY KEY,
    strategic_goal_id TEXT NULL, -- Made NULLable
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
    FOREIGN KEY (strategic_goal_id) REFERENCES strategic_goals(id) ON DELETE RESTRICT, -- Keep RESTRICT
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
INSERT INTO projects_new (id, strategic_goal_id, name, name_updated_at, name_updated_by, objective, objective_updated_at, objective_updated_by, outcome, outcome_updated_at, outcome_updated_by, status_id, status_id_updated_at, status_id_updated_by, timeline, timeline_updated_at, timeline_updated_by, responsible_team, responsible_team_updated_at, responsible_team_updated_by, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id)
SELECT id, strategic_goal_id, name, name_updated_at, name_updated_by, objective, objective_updated_at, objective_updated_by, outcome, outcome_updated_at, outcome_updated_by, status_id, status_id_updated_at, status_id_updated_by, timeline, timeline_updated_at, timeline_updated_by, responsible_team, responsible_team_updated_at, responsible_team_updated_by, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id FROM projects;
DROP TABLE projects;
ALTER TABLE projects_new RENAME TO projects;

-- Recreate indexes for 'projects'
CREATE INDEX IF NOT EXISTS idx_projects_strategic_goal ON projects(strategic_goal_id);
CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at);
CREATE INDEX IF NOT EXISTS idx_projects_deleted_at ON projects(deleted_at);
CREATE INDEX IF NOT EXISTS idx_projects_created_by ON projects(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_by ON projects(updated_by_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);

-- 2. Modify 'activities' table: make 'project_id' NULLable
-- Assuming the dependency is activities -> projects based on schema
CREATE TABLE activities_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NULL, -- Made NULLable
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
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE, -- Keep CASCADE
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
INSERT INTO activities_new (id, project_id, description, description_updated_at, description_updated_by, kpi, kpi_updated_at, kpi_updated_by, target_value, target_value_updated_at, target_value_updated_by, actual_value, actual_value_updated_at, actual_value_updated_by, status_id, status_id_updated_at, status_id_updated_by, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id)
SELECT id, project_id, description, description_updated_at, description_updated_by, kpi, kpi_updated_at, kpi_updated_by, target_value, target_value_updated_at, target_value_updated_by, actual_value, actual_value_updated_at, actual_value_updated_by, status_id, status_id_updated_at, status_id_updated_by, created_at, updated_at, created_by_user_id, updated_by_user_id, deleted_at, deleted_by_user_id FROM activities;
DROP TABLE activities;
ALTER TABLE activities_new RENAME TO activities;

-- Recreate indexes for 'activities'
CREATE INDEX IF NOT EXISTS idx_activities_project ON activities(project_id);
CREATE INDEX IF NOT EXISTS idx_activities_status ON activities(status_id);
CREATE INDEX IF NOT EXISTS idx_activities_updated_at ON activities(updated_at);
CREATE INDEX IF NOT EXISTS idx_activities_deleted_at ON activities(deleted_at);
CREATE INDEX IF NOT EXISTS idx_activities_created_by ON activities(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_activities_updated_by ON activities(updated_by_user_id);


-- 3. Modify 'workshops' table: make 'project_id' NULLable
CREATE TABLE workshops_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NULL, -- Made NULLable
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

-- Fully corrected INSERT statement for workshops, selecting only columns that existed in basic.sql
-- and providing defaults/NULLs for columns added/changed in workshops_new.
INSERT INTO workshops_new (
    -- Columns present in workshops_new
    id, project_id, title, title_updated_at, title_updated_by, 
    objective, objective_updated_at, objective_updated_by, 
    rationale, rationale_updated_at, rationale_updated_by, 
    methodology, methodology_updated_at, methodology_updated_by, 
    facilitator, facilitator_updated_at, facilitator_updated_by, 
    event_date, event_date_updated_at, event_date_updated_by, 
    start_time, start_time_updated_at, start_time_updated_by, 
    end_time, end_time_updated_at, end_time_updated_by, 
    location, location_updated_at, location_updated_by, 
    total_male_participants, total_male_participants_updated_at, total_male_participants_updated_by, 
    total_female_participants, total_female_participants_updated_at, total_female_participants_updated_by, 
    total_other_participants, total_other_participants_updated_at, total_other_participants_updated_by, 
    created_at, updated_at, created_by_user_id, updated_by_user_id, 
    deleted_at, deleted_by_user_id
)
SELECT 
    -- Map from columns that ACTUALLY EXISTED in the old workshops table (from basic.sql)
    id, 
    project_id, 
    '' as title, -- New column, default to empty string (NOT NULL)
    NULL as title_updated_at, -- New column
    NULL as title_updated_by, -- New column
    NULL as objective, -- New column
    NULL as objective_updated_at, -- New column
    NULL as objective_updated_by, -- New column
    NULL as rationale, -- New column
    NULL as rationale_updated_at, -- New column
    NULL as rationale_updated_by, -- New column
    NULL as methodology, -- New column
    NULL as methodology_updated_at, -- New column
    NULL as methodology_updated_by, -- New column
    NULL as facilitator, -- New column
    NULL as facilitator_updated_at, -- New column
    NULL as facilitator_updated_by, -- New column
    event_date, -- Existed 
    event_date_updated_at, -- Existed
    event_date_updated_by, -- Existed
    NULL as start_time, -- New column
    NULL as start_time_updated_at, -- New column
    NULL as start_time_updated_by, -- New column
    NULL as end_time, -- New column
    NULL as end_time_updated_at, -- New column
    NULL as end_time_updated_by, -- New column
    location, -- Existed
    location_updated_at, -- Existed
    location_updated_by, -- Existed
    0 as total_male_participants, -- New column, default to 0
    NULL as total_male_participants_updated_at, -- New column
    NULL as total_male_participants_updated_by, -- New column
    0 as total_female_participants, -- New column, default to 0
    NULL as total_female_participants_updated_at, -- New column
    NULL as total_female_participants_updated_by, -- New column
    0 as total_other_participants, -- New column, default to 0
    NULL as total_other_participants_updated_at, -- New column
    NULL as total_other_participants_updated_by, -- New column
    created_at, -- Existed
    updated_at, -- Existed
    created_by_user_id, -- Existed
    updated_by_user_id, -- Existed
    deleted_at, -- Existed
    deleted_by_user_id -- Existed
    -- NOTE: We are IGNORING columns from the old table that are NOT in the new one:
    -- purpose, purpose_updated_at, purpose_updated_by
    -- budget, budget_updated_at, budget_updated_by
    -- actuals, actuals_updated_at, actuals_updated_by
    -- participant_count, participant_count_updated_at, participant_count_updated_by
    -- local_partner, local_partner_updated_at, local_partner_updated_by
    -- partner_responsibility, partner_responsibility_updated_at, partner_responsibility_updated_by
    -- partnership_success, partnership_success_updated_at, partnership_success_updated_by
    -- capacity_challenges, capacity_challenges_updated_at, capacity_challenges_updated_by
    -- strengths, strengths_updated_at, strengths_updated_by
    -- outcomes, outcomes_updated_at, outcomes_updated_by
    -- recommendations, recommendations_updated_at, recommendations_updated_by
    -- challenge_resolution, challenge_resolution_updated_at, challenge_resolution_updated_by
FROM workshops; 
DROP TABLE workshops;
ALTER TABLE workshops_new RENAME TO workshops;

-- Recreate indexes for 'workshops'
CREATE INDEX IF NOT EXISTS idx_workshops_project ON workshops(project_id);
CREATE INDEX IF NOT EXISTS idx_workshops_event_date ON workshops(event_date);
CREATE INDEX IF NOT EXISTS idx_workshops_location ON workshops(location);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_at ON workshops(updated_at);
CREATE INDEX IF NOT EXISTS idx_workshops_deleted_at ON workshops(deleted_at);
CREATE INDEX IF NOT EXISTS idx_workshops_created_by ON workshops(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_by ON workshops(updated_by_user_id);


-- 4. Modify 'workshop_participants' table: make 'workshop_id' and 'participant_id' NULLable
CREATE TABLE workshop_participants_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    workshop_id TEXT NULL, -- Made NULLable
    participant_id TEXT NULL, -- Made NULLable
    notes TEXT,
    notes_updated_at TEXT,
    notes_updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (workshop_id) REFERENCES workshops(id) ON DELETE RESTRICT, -- Keep RESTRICT
    FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE RESTRICT, -- Keep RESTRICT
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (notes_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT INTO workshop_participants_new (
    -- Columns from workshop_participants_new definition
    id, workshop_id, participant_id, notes, notes_updated_at, notes_updated_by, 
    created_at, updated_at, created_by_user_id, updated_by_user_id, 
    deleted_at, deleted_by_user_id
)
SELECT 
    -- Map from columns that ACTUALLY EXISTED in the old workshop_participants table (from basic.sql)
    id,                 -- Existed as PK
    workshop_id,        -- Existed
    participant_id,     -- Existed
    NULL as notes,      -- New column in _new table
    NULL as notes_updated_at, -- New column in _new table
    NULL as notes_updated_by, -- New column in _new table
    created_at,         -- Existed
    updated_at,         -- Existed
    created_by_user_id, -- Existed
    updated_by_user_id, -- Existed
    deleted_at,         -- Existed
    deleted_by_user_id  -- Existed
    -- NOTE: We are IGNORING columns from the old table that are NOT in the new one:
    -- pre_evaluation, pre_evaluation_updated_at, pre_evaluation_updated_by
    -- post_evaluation, post_evaluation_updated_at, post_evaluation_updated_by
FROM workshop_participants; 
DROP TABLE workshop_participants;
ALTER TABLE workshop_participants_new RENAME TO workshop_participants;

-- Recreate indexes for 'workshop_participants'
CREATE INDEX IF NOT EXISTS idx_workshop_participants_workshop ON workshop_participants(workshop_id);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_participant ON workshop_participants(participant_id);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_updated_at ON workshop_participants(updated_at);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_deleted_at ON workshop_participants(deleted_at);


-- 5. Modify 'livelihoods' table: make 'participant_id' and 'project_id' NULLable
CREATE TABLE livelihoods_new (
    id TEXT PRIMARY KEY,
    participant_id TEXT NULL, -- Made NULLable
    project_id TEXT NULL, -- Made NULLable
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
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,
    FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE RESTRICT, -- Keep RESTRICT
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT, -- Keep RESTRICT
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
    -- Columns from livelihoods_new definition
    id, participant_id, project_id, 
    type, type_updated_at, type_updated_by, 
    description, description_updated_at, description_updated_by, 
    status_id, status_id_updated_at, status_id_updated_by, 
    initial_grant_date, initial_grant_date_updated_at, initial_grant_date_updated_by, 
    initial_grant_amount, initial_grant_amount_updated_at, initial_grant_amount_updated_by, 
    created_at, updated_at, created_by_user_id, updated_by_user_id, 
    deleted_at, deleted_by_user_id
)
SELECT 
    -- Map from columns that ACTUALLY EXISTED in the old livelihoods table (from basic.sql)
    id, 
    participant_id, 
    project_id, 
    'Unknown' as type, -- New column (NOT NULL in new schema?), provide default
    NULL as type_updated_at, -- New column
    NULL as type_updated_by, -- New column
    NULL as description, -- New column
    NULL as description_updated_at, -- New column
    NULL as description_updated_by, -- New column
    NULL as status_id, -- New column
    NULL as status_id_updated_at, -- New column
    NULL as status_id_updated_by, -- New column
    NULL as initial_grant_date, -- New column
    NULL as initial_grant_date_updated_at, -- New column
    NULL as initial_grant_date_updated_by, -- New column
    NULL as initial_grant_amount, -- New column
    NULL as initial_grant_amount_updated_at, -- New column
    NULL as initial_grant_amount_updated_by, -- New column
    created_at, 
    updated_at, 
    created_by_user_id, 
    updated_by_user_id, 
    deleted_at, 
    deleted_by_user_id
    -- NOTE: We are IGNORING columns from the old table that are NOT in the new one:
    -- grant_amount, grant_amount_updated_at, grant_amount_updated_by
    -- purpose, purpose_updated_at, purpose_updated_by
    -- progress1, progress1_updated_at, progress1_updated_by
    -- progress2, progress2_updated_at, progress2_updated_by
    -- outcome, outcome_updated_at, outcome_updated_by
FROM livelihoods; 
DROP TABLE livelihoods;
ALTER TABLE livelihoods_new RENAME TO livelihoods;

-- Recreate indexes for 'livelihoods'
CREATE INDEX IF NOT EXISTS idx_livelihoods_participant ON livelihoods(participant_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_project ON livelihoods(project_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_at ON livelihoods(updated_at);
CREATE INDEX IF NOT EXISTS idx_livelihoods_deleted_at ON livelihoods(deleted_at);
CREATE INDEX IF NOT EXISTS idx_livelihoods_created_by ON livelihoods(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_by ON livelihoods(updated_by_user_id);


PRAGMA foreign_keys=on; -- Re-enable FK constraints