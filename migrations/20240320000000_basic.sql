-- Initial schema migration
-- Created at: 2024-03-20 00:00:00 

-- #############################################################
-- ##          Phase 1: Updated Schema SQL (incorporating LWW & Locking) ##
-- #############################################################

-- Ensure Foreign Keys are enabled
PRAGMA foreign_keys = ON;

-- Use TEXT for Timestamps to store ISO8601 format with precision
-- Example: 'YYYY-MM-DDTHH:MM:SS.sssZ' (UTC is generally recommended for sync)
-- Using datetime('now', 'localtime') for defaults as per original schema, but consider UTC.

-- ----------- Users Table (with Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY, -- UUID

    email TEXT NOT NULL UNIQUE,
    email_updated_at TEXT,          -- Field timestamp (ISO8601 UTC preferably)
    email_updated_by TEXT,          -- Field author (user_id)

    password_hash TEXT NOT NULL,
    -- password_hash_updated_at: Not typically needed for LWW sync

    name TEXT NOT NULL,
    name_updated_at TEXT,
    name_updated_by TEXT,

    -- Assuming 'field_tl' is a valid role now for locking
    role TEXT NOT NULL CHECK (role IN ('admin', 'field_tl', 'field')),
    role_updated_at TEXT,
    role_updated_by TEXT,

    last_login TEXT DEFAULT NULL,
    -- last_login_updated_at: Info only, not for LWW merge usually

    active INTEGER NOT NULL DEFAULT 1, -- 1=true, 0=false
    active_updated_at TEXT,
    active_updated_by TEXT,

    -- Core Timestamps/Authorship for the record itself
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- ISO8601 UTC
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- Main record timestamp (reflects the latest field update time)
    created_by_user_id TEXT, -- User who created this user record
    updated_by_user_id TEXT, -- User who last touched any field on this record

    -- Optional Soft Delete fields (can also be inferred from 'delete' in change_log)
    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (email_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (name_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (role_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (active_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);
CREATE INDEX IF NOT EXISTS idx_users_updated_at ON users(updated_at); -- Useful for general queries
CREATE INDEX IF NOT EXISTS idx_users_deleted_at ON users(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_users_created_by ON users(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_users_updated_by ON users(updated_by_user_id);


-- ----------- Status Types (Lookup Table - Now Syncable) -----------
CREATE TABLE IF NOT EXISTS status_types (
    id INTEGER PRIMARY KEY AUTOINCREMENT, -- Local immutable ID

    value TEXT NOT NULL UNIQUE,
    value_updated_at TEXT,          -- Timestamp for value change
    value_updated_by TEXT,          -- User who changed the value

    -- Core sync metadata
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When this status was added
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When value last changed or record soft deleted/undeleted
    created_by_user_id TEXT, -- User who added this status
    updated_by_user_id TEXT, -- User who last touched this record

    deleted_at TEXT DEFAULT NULL, -- If status types can be removed
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (value_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_status_types_value ON status_types(value);
CREATE INDEX IF NOT EXISTS idx_status_types_deleted_at ON status_types(deleted_at);

-- Initial seeding (still useful)
INSERT OR IGNORE INTO status_types (id, value, created_at, updated_at) VALUES
    (1, 'On Track', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    (2, 'At Risk', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    (3, 'Delayed', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    (4, 'Completed', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), strftime('%Y-%m-%dT%H:%M:%fZ', 'now'));
-- NOTE: Explicit IDs used here for potentially easier referencing if needed, but AUTOINCREMENT is fine too.

-- ----------- Strategic Goals (with Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS strategic_goals (
    id TEXT PRIMARY KEY, -- UUID

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

    -- sync_status: Can be removed or repurposed. Let's remove for now, relying on change_log.
    -- last_updated: Redundant, use updated_at.

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
    -- Add FKs for all _updated_by fields...
    FOREIGN KEY (objective_code_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (outcome_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (kpi_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (target_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (actual_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (responsible_team_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_strategic_goals_status ON strategic_goals(status_id);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_updated_at ON strategic_goals(updated_at);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_deleted_at ON strategic_goals(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_strategic_goals_created_by ON strategic_goals(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_updated_by ON strategic_goals(updated_by_user_id);


-- ----------- Projects Table (with Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY, -- UUID
    strategic_goal_id TEXT NOT NULL, -- This relationship itself might need update tracking if it can change

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

    -- Removed last_updated, sync_status

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,

    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (strategic_goal_id) REFERENCES strategic_goals(id) ON DELETE CASCADE,
    FOREIGN KEY (status_id) REFERENCES status_types(id) ON DELETE RESTRICT,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    -- Add FKs for all _updated_by fields...
    FOREIGN KEY (name_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (objective_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (outcome_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_id_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (timeline_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (responsible_team_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_projects_strategic_goal ON projects(strategic_goal_id);
CREATE INDEX IF NOT EXISTS idx_projects_status ON projects(status_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_at ON projects(updated_at);
CREATE INDEX IF NOT EXISTS idx_projects_deleted_at ON projects(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_projects_created_by ON projects(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_by ON projects(updated_by_user_id);


-- ----------- Activities Table (Needs Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS activities (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,

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

    -- Removed sync_status

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
    -- Add FKs for _updated_by fields...
    FOREIGN KEY (description_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (kpi_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (target_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (actual_value_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (status_id_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_activities_project ON activities(project_id);
CREATE INDEX IF NOT EXISTS idx_activities_status ON activities(status_id);
CREATE INDEX IF NOT EXISTS idx_activities_updated_at ON activities(updated_at);
CREATE INDEX IF NOT EXISTS idx_activities_deleted_at ON activities(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_activities_created_by ON activities(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_activities_updated_by ON activities(updated_by_user_id);


-- ----------- Participants Table (with Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS participants (
    id TEXT PRIMARY KEY, -- UUID

    name TEXT NOT NULL,
    name_updated_at TEXT,
    name_updated_by TEXT,

    gender TEXT,
    gender_updated_at TEXT,
    gender_updated_by TEXT,

    disability INTEGER DEFAULT 0, -- 0=false, 1=true
    disability_updated_at TEXT,
    disability_updated_by TEXT,

    disability_type TEXT DEFAULT NULL,
    disability_type_updated_at TEXT,
    disability_type_updated_by TEXT,

    age_group TEXT,
    age_group_updated_at TEXT,
    age_group_updated_by TEXT,

    location TEXT, -- Consider linking to locations table via location_id TEXT?
    location_updated_at TEXT,
    location_updated_by TEXT,

    -- Removed sync_status

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,

    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    -- Add FKs for _updated_by fields...
    FOREIGN KEY (name_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (gender_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (disability_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (disability_type_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (age_group_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (location_updated_by) REFERENCES users(id) ON DELETE SET NULL

);
CREATE INDEX IF NOT EXISTS idx_participants_location ON participants(location);
CREATE INDEX IF NOT EXISTS idx_participants_gender_age ON participants(gender, age_group);
CREATE INDEX IF NOT EXISTS idx_participants_updated_at ON participants(updated_at);
CREATE INDEX IF NOT EXISTS idx_participants_deleted_at ON participants(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_participants_created_by ON participants(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_participants_updated_by ON participants(updated_by_user_id);


-- ----------- Workshops Table (Needs Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS workshops (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,

    purpose TEXT,
    purpose_updated_at TEXT,
    purpose_updated_by TEXT,

    event_date TEXT,
    event_date_updated_at TEXT,
    event_date_updated_by TEXT,

    location TEXT, -- Link to locations table?
    location_updated_at TEXT,
    location_updated_by TEXT,

    budget REAL,
    budget_updated_at TEXT,
    budget_updated_by TEXT,

    actuals REAL,
    actuals_updated_at TEXT,
    actuals_updated_by TEXT,

    participant_count INTEGER DEFAULT 0, -- This might be derived or needs specific update logic
    participant_count_updated_at TEXT,
    participant_count_updated_by TEXT,

    local_partner TEXT, -- Link to partners table?
    local_partner_updated_at TEXT,
    local_partner_updated_by TEXT,

    partner_responsibility TEXT,
    partner_responsibility_updated_at TEXT,
    partner_responsibility_updated_by TEXT,

    -- Fields likely filled post-event, still need LWW tracking
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

    -- Removed sync_status

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,

    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (participant_count_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    -- Add FKs for all _updated_by fields...
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
CREATE INDEX IF NOT EXISTS idx_workshops_project ON workshops(project_id);
CREATE INDEX IF NOT EXISTS idx_workshops_event_date ON workshops(event_date);
CREATE INDEX IF NOT EXISTS idx_workshops_location ON workshops(location);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_at ON workshops(updated_at);
CREATE INDEX IF NOT EXISTS idx_workshops_deleted_at ON workshops(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_workshops_created_by ON workshops(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_by ON workshops(updated_by_user_id);


-- ----------- Workshop Participants (Junction Table) -----------
-- Junction tables typically track the existence of a relationship.
-- If the relationship itself is the unit of change (added/removed), LWW applies to the record.
CREATE TABLE IF NOT EXISTS workshop_participants (
    id TEXT PRIMARY KEY, -- UUID for the relationship instance
    workshop_id TEXT NOT NULL,
    participant_id TEXT NOT NULL,

    pre_evaluation TEXT,
    pre_evaluation_updated_at TEXT,
    pre_evaluation_updated_by TEXT,

    post_evaluation TEXT,
    post_evaluation_updated_at TEXT,
    post_evaluation_updated_by TEXT,

    -- Removed sync_status

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When relationship was added
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When evaluation fields were last updated
    created_by_user_id TEXT, -- User who added the participant
    updated_by_user_id TEXT, -- User who last updated evaluation

    deleted_at TEXT DEFAULT NULL, -- When relationship was removed (soft delete)
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (workshop_id) REFERENCES workshops(id) ON DELETE CASCADE,
    FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (pre_evaluation_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (post_evaluation_updated_by) REFERENCES users(id) ON DELETE SET NULL,

    UNIQUE(workshop_id, participant_id) -- Ensures a participant isn't added twice to the same workshop
);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_workshop ON workshop_participants(workshop_id);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_participant ON workshop_participants(participant_id);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_updated_at ON workshop_participants(updated_at);
CREATE INDEX IF NOT EXISTS idx_workshop_participants_deleted_at ON workshop_participants(deleted_at);


-- ----------- Livelihoods Table (Needs Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS livelihoods (
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

    -- Removed sync_status

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,

    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (participant_id) REFERENCES participants(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    -- Add FKs for _updated_by fields...
    FOREIGN KEY (grant_amount_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (purpose_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (progress1_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (progress2_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (outcome_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_livelihoods_participant ON livelihoods(participant_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_project ON livelihoods(project_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_at ON livelihoods(updated_at);
CREATE INDEX IF NOT EXISTS idx_livelihoods_deleted_at ON livelihoods(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_livelihoods_created_by ON livelihoods(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_by ON livelihoods(updated_by_user_id);


-- ----------- Subsequent Grants Table (Needs Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS subsequent_grants (
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

    -- Removed sync_status

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
    -- Add FKs for _updated_by fields...
    FOREIGN KEY (amount_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (purpose_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (grant_date_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_livelihood ON subsequent_grants(livelihood_id);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_date ON subsequent_grants(grant_date);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_updated_at ON subsequent_grants(updated_at);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_deleted_at ON subsequent_grants(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_created_by ON subsequent_grants(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_subsequent_grants_updated_by ON subsequent_grants(updated_by_user_id);


-- ----------- Donors Table (with Field Timestamps & Authors) -----------
CREATE TABLE IF NOT EXISTS donors (
    id TEXT PRIMARY KEY, -- UUID

    name TEXT NOT NULL,
    name_updated_at TEXT,
    name_updated_by TEXT,

    type TEXT, -- e.g., 'Individual', 'Foundation', 'Government', 'Corporate'
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

    country TEXT, -- Consider linking to locations table?
    country_updated_at TEXT,
    country_updated_by TEXT,

    first_donation_date TEXT, -- ISO date string
    first_donation_date_updated_at TEXT,
    first_donation_date_updated_by TEXT,

    notes TEXT,
    notes_updated_at TEXT,
    notes_updated_by TEXT,

    -- Core sync metadata
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT,
    updated_by_user_id TEXT,

    deleted_at TEXT DEFAULT NULL,
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    -- Add FKs for all _updated_by fields...
    FOREIGN KEY (name_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (type_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (contact_person_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (email_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (phone_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (country_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (first_donation_date_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (notes_updated_by) REFERENCES users(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_donors_name ON donors(name);
CREATE INDEX IF NOT EXISTS idx_donors_type ON donors(type);
CREATE INDEX IF NOT EXISTS idx_donors_country ON donors(country); -- Add if querying by country
CREATE INDEX IF NOT EXISTS idx_donors_updated_at ON donors(updated_at);
CREATE INDEX IF NOT EXISTS idx_donors_deleted_at ON donors(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_donors_created_by ON donors(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_donors_updated_by ON donors(updated_by_user_id);






-- ----------- Project Funding Table (with Field Timestamps & Authors) -----------
-- This table links Projects and Donors, tracking specific funding instances.
CREATE TABLE IF NOT EXISTS project_funding (
    id TEXT PRIMARY KEY, -- UUID for this specific funding record

    project_id TEXT NOT NULL,
    project_id_updated_at TEXT,     -- If funding can be re-assigned (less common)
    project_id_updated_by TEXT,

    donor_id TEXT NOT NULL,
    donor_id_updated_at TEXT,       -- If donor can be corrected
    donor_id_updated_by TEXT,

    grant_id TEXT,                  -- Reference number for the grant
    grant_id_updated_at TEXT,
    grant_id_updated_by TEXT,

    amount REAL,
    amount_updated_at TEXT,
    amount_updated_by TEXT,

    currency TEXT DEFAULT 'AUD',
    currency_updated_at TEXT,
    currency_updated_by TEXT,

    start_date TEXT,                -- ISO date string 'YYYY-MM-DD'
    start_date_updated_at TEXT,
    start_date_updated_by TEXT,

    end_date TEXT,                  -- ISO date string 'YYYY-MM-DD'
    end_date_updated_at TEXT,
    end_date_updated_by TEXT,

    status TEXT,                    -- e.g., 'Committed', 'Received', 'Pending', 'Completed'
    status_updated_at TEXT,
    status_updated_by TEXT,

    reporting_requirements TEXT,
    reporting_requirements_updated_at TEXT,
    reporting_requirements_updated_by TEXT,

    notes TEXT,
    notes_updated_at TEXT,
    notes_updated_by TEXT,

    -- Core sync metadata
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When this funding record was created
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When any field on this record was last updated
    created_by_user_id TEXT, -- User who entered this funding record
    updated_by_user_id TEXT, -- User who last modified this funding record

    deleted_at TEXT DEFAULT NULL, -- If funding records can be soft-deleted
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE, -- If project deleted, funding record might be irrelevant
    FOREIGN KEY (donor_id) REFERENCES donors(id) ON DELETE RESTRICT, -- Don't delete donor if they have funding attached
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    -- Add FKs for all _updated_by fields...
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

-- Indices for common query patterns
CREATE INDEX IF NOT EXISTS idx_project_funding_project ON project_funding(project_id);
CREATE INDEX IF NOT EXISTS idx_project_funding_donor ON project_funding(donor_id);
CREATE INDEX IF NOT EXISTS idx_project_funding_status ON project_funding(status); -- If filtering by status
CREATE INDEX IF NOT EXISTS idx_project_funding_updated_at ON project_funding(updated_at);
CREATE INDEX IF NOT EXISTS idx_project_funding_deleted_at ON project_funding(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_project_funding_created_by ON project_funding(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_project_funding_updated_by ON project_funding(updated_by_user_id);




-- ----------- Document Types Table (Syncable Configuration) -----------
-- Defines the types of documents allowed, their properties, and compression settings.
-- Made syncable to allow adding/editing types post-deployment.
CREATE TABLE IF NOT EXISTS document_types (
    id TEXT PRIMARY KEY, -- User-defined key, e.g., 'receipt', 'project_plan', 'photo_evidence'

    name TEXT NOT NULL,              -- Display name, e.g., "Receipt", "Project Plan"
    name_updated_at TEXT,
    name_updated_by TEXT,

    allowed_extensions TEXT NOT NULL, -- Comma-separated list: 'jpg,png,pdf'
    allowed_extensions_updated_at TEXT,
    allowed_extensions_updated_by TEXT,

    max_size INTEGER NOT NULL,       -- Maximum file size in bytes
    max_size_updated_at TEXT,
    max_size_updated_by TEXT,

    compression_level INTEGER NOT NULL DEFAULT 6, -- 0-9 (0=none, 9=max)
    compression_level_updated_at TEXT,
    compression_level_updated_by TEXT,

    compression_method TEXT DEFAULT 'default', -- 'default', 'lossless', 'lossy', 'none'
    compression_method_updated_at TEXT,
    compression_method_updated_by TEXT,

    min_size_for_compression INTEGER DEFAULT 10240, -- Don't compress if smaller (bytes)
    min_size_for_compression_updated_at TEXT,
    min_size_for_compression_updated_by TEXT,

    description TEXT,               -- Usage guidance for the type
    description_updated_at TEXT,
    description_updated_by TEXT,

    default_priority TEXT NOT NULL DEFAULT 'normal' CHECK(default_priority IN ('high', 'normal', 'low', 'never')), -- Sync priority hint
    default_priority_updated_at TEXT,
    default_priority_updated_by TEXT,

    icon TEXT,                      -- Icon identifier for UI (e.g., 'file-pdf', 'image')
    icon_updated_at TEXT,
    icon_updated_by TEXT,

    related_tables TEXT,            -- JSON array of table names where this type is typically used (for UI hints/filtering)
    related_tables_updated_at TEXT,
    related_tables_updated_by TEXT,

    -- Core sync metadata
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT, -- User who defined this type
    updated_by_user_id TEXT, -- User who last modified this type definition

    deleted_at TEXT DEFAULT NULL, -- If document types can be deactivated/hidden
    deleted_by_user_id TEXT DEFAULT NULL,

    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    -- Add FKs for all _updated_by fields...
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

-- Indices
CREATE UNIQUE INDEX IF NOT EXISTS idx_document_types_name ON document_types(name) WHERE deleted_at IS NULL; -- Unique active names
CREATE INDEX IF NOT EXISTS idx_document_types_updated_at ON document_types(updated_at);
CREATE INDEX IF NOT EXISTS idx_document_types_deleted_at ON document_types(deleted_at);

-- Add indexes for user tracking columns across all entity tables
CREATE INDEX IF NOT EXISTS idx_document_types_created_by ON document_types(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_document_types_updated_by ON document_types(updated_by_user_id);


-- ----------- Media/Documents Table (Needs Field Timestamps & Authors for mutable metadata) -----------
CREATE TABLE IF NOT EXISTS media_documents (
    id TEXT PRIMARY KEY NOT NULL,
    related_table TEXT NOT NULL,
    related_id TEXT NOT NULL,
    type_id TEXT NOT NULL,
    file_path TEXT NOT NULL,
    original_filename TEXT NOT NULL,
    title TEXT NULL,
    mime_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    field_identifier TEXT NULL,
    compression_status TEXT NOT NULL DEFAULT 'pending',
    compressed_file_path TEXT NULL,
    blob_sync_status TEXT NOT NULL DEFAULT 'pending',
    blob_storage_key TEXT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    created_by_user_id TEXT NULL,
    updated_by_user_id TEXT NULL,
    deleted_at TEXT NULL,
    deleted_by_user_id TEXT NULL,
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (updated_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (deleted_by_user_id) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (type_id) REFERENCES document_types(id) ON DELETE RESTRICT
);
CREATE INDEX IF NOT EXISTS idx_media_documents_related ON media_documents(related_table, related_id);
CREATE INDEX IF NOT EXISTS idx_media_documents_type ON media_documents(type_id);
CREATE INDEX IF NOT EXISTS idx_media_documents_user ON media_documents(created_by_user_id);
CREATE INDEX IF NOT EXISTS idx_media_documents_compression ON media_documents(compression_status);
CREATE INDEX IF NOT EXISTS idx_media_documents_blob_sync ON media_documents(blob_sync_status);
CREATE INDEX IF NOT EXISTS idx_media_documents_updated_at ON media_documents(updated_at);
CREATE INDEX IF NOT EXISTS idx_media_documents_deleted_at ON media_documents(deleted_at);


-- ----------- Document Versions Table (Local Log) -----------
-- Tracks the history of specific document files (original, compressed versions) locally.
-- This table itself is not typically synced between devices via LWW.
CREATE TABLE IF NOT EXISTS document_versions (
    id TEXT PRIMARY KEY,                -- UUID for this version entry
    document_id TEXT NOT NULL,          -- FK to media_documents.id
    version_number INTEGER NOT NULL,    -- Sequential version number (e.g., 1, 2, 3...)
    file_path TEXT NOT NULL,            -- Path to the file for this version (could be original or compressed)
    file_size INTEGER,                  -- Size of the file at this version
    is_compressed INTEGER DEFAULT 0,    -- 1 if this version represents a compressed file, 0 otherwise
    change_type TEXT NOT NULL CHECK(change_type IN ('original', 'compressed', 'modified', 'restored')), -- What action led to this version
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When this version record was created
    created_by_user_id TEXT,            -- User associated with the action creating this version

    FOREIGN KEY (document_id) REFERENCES media_documents(id) ON DELETE CASCADE, -- If document deleted, history goes too
    FOREIGN KEY (created_by_user_id) REFERENCES users(id) ON DELETE SET NULL
);

-- Indices
CREATE INDEX IF NOT EXISTS idx_document_versions_document ON document_versions(document_id);
CREATE INDEX IF NOT EXISTS idx_document_versions_created_at ON document_versions(created_at); -- For ordering history





-- ----------- Document Access Logs Table (Local Audit Log) -----------
-- Records access events (view, download, etc.) for documents on the local device.
-- Not synced between devices.
CREATE TABLE IF NOT EXISTS document_access_logs (
    id TEXT PRIMARY KEY,                -- UUID for the log entry
    document_id TEXT NOT NULL,          -- FK to media_documents.id
    user_id TEXT NOT NULL,              -- FK to users.id (who accessed)
    access_type TEXT NOT NULL CHECK(access_type IN ('view', 'download', 'edit_metadata', 'delete', 'print')), -- Type of access
    access_date TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- Timestamp of access
    details TEXT,                       -- Optional JSON for extra info (e.g., IP if relevant, success/fail)

    FOREIGN KEY (document_id) REFERENCES media_documents(id) ON DELETE CASCADE, -- If doc deleted, access log may remain or cascade? Cascade seems ok.
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE -- If user deleted, their access log remains (cascade ok)
);

-- Indices
CREATE INDEX IF NOT EXISTS idx_document_access_logs_document ON document_access_logs(document_id);
CREATE INDEX IF NOT EXISTS idx_document_access_logs_user ON document_access_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_document_access_logs_date ON document_access_logs(access_date);



-- ----------- Compression Queue Table (Local State) -----------
-- Manages the queue of documents needing background compression on this device.
-- Not synced.
CREATE TABLE IF NOT EXISTS compression_queue (
    id TEXT PRIMARY KEY,                -- UUID for the queue entry
    document_id TEXT NOT NULL UNIQUE,   -- FK to media_documents.id (Only one queue entry per doc)
    priority INTEGER DEFAULT 5,         -- Priority for processing (higher value = higher priority)
    attempts INTEGER DEFAULT 0,         -- Number of times processing was attempted
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'processing', 'completed', 'failed')), -- Current status
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When added to queue
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When status/attempts last changed
    error_message TEXT,                 -- Stores error if status is 'failed'

    FOREIGN KEY (document_id) REFERENCES media_documents(id) ON DELETE CASCADE -- If document deleted, remove from queue
);

-- Indices
-- Index for finding pending tasks, ordered by priority and age
CREATE INDEX IF NOT EXISTS idx_compression_queue_pending ON compression_queue(status, priority DESC, created_at ASC) WHERE status = 'pending';
CREATE INDEX IF NOT EXISTS idx_compression_queue_document ON compression_queue(document_id); -- Covered by UNIQUE constraint


-- ----------- Compression Stats Table (Local Aggregated State) -----------
-- Stores aggregated statistics about file compression performed on this device.
-- Not synced.
CREATE TABLE IF NOT EXISTS compression_stats (
    id TEXT PRIMARY KEY CHECK(id = 'global'), -- Singleton row
    total_original_size BIGINT DEFAULT 0,    -- Sum of original sizes of compressed files
    total_compressed_size BIGINT DEFAULT 0,  -- Sum of compressed sizes
    space_saved BIGINT DEFAULT 0,            -- Difference: original - compressed
    compression_ratio REAL DEFAULT 0,        -- Average ratio: (space_saved / total_original_size) * 100
    total_files_compressed INTEGER DEFAULT 0,-- Count of successfully compressed files
    total_files_pending INTEGER DEFAULT 0,   -- Current count of files in the compression queue (status='pending')
    total_files_failed INTEGER DEFAULT 0,    -- Count of files that failed compression
    last_compression_date TEXT,              -- Timestamp of the last successful compression
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) -- When stats were last updated
);

-- Insert the singleton row if it doesn't exist
INSERT OR IGNORE INTO compression_stats (id) VALUES ('global');


-- ----------- App Settings Table (Needs LWW if settings are synced) -----------
CREATE TABLE IF NOT EXISTS app_settings (
    id TEXT PRIMARY KEY CHECK(id = 'global'), -- Singleton

    compression_enabled INTEGER DEFAULT 1,
    compression_enabled_updated_at TEXT,
    compression_enabled_updated_by TEXT,

    default_compression_timing TEXT DEFAULT 'immediate' CHECK(default_compression_timing IN ('immediate', 'background', 'manual')),
    default_compression_timing_updated_at TEXT,
    default_compression_timing_updated_by TEXT,

    background_service_interval INTEGER DEFAULT 300,
    background_service_interval_updated_at TEXT,
    background_service_interval_updated_by TEXT,

    -- Other settings... add _updated_at / _updated_by if they need to be synced via LWW

    last_background_run TEXT, -- Local state, not synced

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- Should only happen once
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- Metadata record updated
    created_by_user_id TEXT, -- Who initially set up settings?
    updated_by_user_id TEXT, -- Who last changed a synced setting

    -- Add FKs for _updated_by fields...
    FOREIGN KEY (compression_enabled_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (default_compression_timing_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (background_service_interval_updated_by) REFERENCES users(id) ON DELETE SET NULL
);
INSERT OR IGNORE INTO app_settings (id) VALUES ('global');


-- ----------- Sync Settings Table (User-specific, needs LWW) -----------
CREATE TABLE IF NOT EXISTS sync_settings (
    user_id TEXT PRIMARY KEY,

    max_file_size INTEGER DEFAULT 10485760,
    max_file_size_updated_at TEXT,
    max_file_size_updated_by TEXT,

    compression_enabled INTEGER DEFAULT 1,
    compression_enabled_updated_at TEXT,
    compression_enabled_updated_by TEXT,

    compression_timing TEXT DEFAULT 'immediate' CHECK(compression_timing IN ('immediate', 'background', 'manual')),
    compression_timing_updated_at TEXT,
    compression_timing_updated_by TEXT,

    -- Other settings... add _updated_at / _updated_by

    -- Removed last_updated (use updated_at)

    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When prefs created for user
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')), -- When prefs last updated
    -- No created_by/updated_by needed, it's the user_id itself

    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
    -- Add FKs for _updated_by fields (referencing users.id)...
    FOREIGN KEY (max_file_size_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (compression_enabled_updated_by) REFERENCES users(id) ON DELETE SET NULL,
    FOREIGN KEY (compression_timing_updated_by) REFERENCES users(id) ON DELETE SET NULL
);


-- #############################################################
-- ##          NEW TABLES FOR SYNC AND LOCKING                ##
-- #############################################################

-- ----------- Change Log Table (Tracks specific changes for sync) -----------
CREATE TABLE IF NOT EXISTS change_log (
    operation_id TEXT PRIMARY KEY, -- Unique ID for this specific change event (UUID)
    entity_table TEXT NOT NULL,    -- e.g., 'users', 'projects'
    entity_id TEXT NOT NULL,       -- The ID of the record changed
    operation_type TEXT NOT NULL CHECK (operation_type IN (
        'create',       -- Record created
        'update',       -- Field updated
        'delete'        -- Record soft deleted (set deleted_at)
     )),
    field_name TEXT,               -- Field name for 'update'. NULL otherwise.
    old_value TEXT,                -- Optional: Previous value (JSON encoded) for auditing/debugging
    new_value TEXT,                -- New value (JSON encoded) for 'create', 'update'. NULL for delete.
    timestamp TEXT NOT NULL,       -- High precision ISO8601 UTC timestamp of the change
    user_id TEXT NOT NULL,         -- User performing the change
    device_id TEXT,                -- Optional: ID of the device making the change

    -- Sync processing state
    sync_batch_id TEXT,            -- ID of the sync batch this belongs to (upload or download)
    processed_at TEXT,             -- Timestamp when this incoming change was merged locally
    sync_error TEXT,               -- If processing failed

    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL, -- User might be deleted later
    FOREIGN KEY (sync_batch_id) REFERENCES sync_batches(batch_id) ON DELETE SET NULL -- Link to sync batch
);
CREATE INDEX IF NOT EXISTS idx_change_log_entity ON change_log(entity_table, entity_id);
CREATE INDEX IF NOT EXISTS idx_change_log_timestamp_user ON change_log(timestamp, user_id);
CREATE INDEX IF NOT EXISTS idx_change_log_unprocessed_for_upload ON change_log(sync_batch_id) WHERE sync_batch_id IS NULL AND processed_at IS NULL; -- Find changes to upload
CREATE INDEX IF NOT EXISTS idx_change_log_operation ON change_log(operation_type);
CREATE INDEX IF NOT EXISTS idx_change_log_batch ON change_log(sync_batch_id);

-- Fix the change_log partial index for finding upload candidates
DROP INDEX IF EXISTS idx_change_log_unprocessed_for_upload;
CREATE INDEX IF NOT EXISTS idx_change_log_unprocessed_for_upload 
ON change_log(entity_table, entity_id, timestamp) 
WHERE sync_batch_id IS NULL AND processed_at IS NULL;

/*
-- ----------- Locks Table (Tracks active locks) -----------
CREATE TABLE IF NOT EXISTS locks (
    id TEXT PRIMARY KEY,              -- Unique ID for the lock itself (UUID)
    entity_table TEXT NOT NULL,       -- e.g., 'users', 'projects'
    entity_id TEXT NOT NULL,          -- The ID of the record being locked
    field_name TEXT DEFAULT NULL,     -- Specific field locked (NULL means lock the whole record)

    lock_level TEXT NOT NULL CHECK (lock_level IN ('admin', 'field_tl')), -- Role level that *owns* this lock
    lock_reason TEXT,                 -- Optional reason for the lock

    -- LWW metadata for the lock itself
    locked_at TEXT NOT NULL,          -- High precision ISO8601 UTC timestamp when lock was applied/updated
    locked_by TEXT NOT NULL,          -- user_id who applied/updated the lock

    -- Unique constraint to prevent conflicting locks on the exact same target
    -- A record lock (field_name IS NULL) prevents field locks, and vice-versa implicitly by checks.
    UNIQUE (entity_table, entity_id, field_name)
);
CREATE INDEX IF NOT EXISTS idx_locks_target ON locks(entity_table, entity_id, field_name); -- Fast checking
CREATE INDEX IF NOT EXISTS idx_locks_user ON locks(locked_by);
CREATE INDEX IF NOT EXISTS idx_locks_timestamp ON locks(locked_at); -- For LWW resolution on the lock itself
*/

-- ----------- Sync Batches Table (Manages upload/download batches) -----------
-- Replaces the old complex sync_queue
CREATE TABLE IF NOT EXISTS sync_batches (
    batch_id TEXT PRIMARY KEY,          -- UUID for the batch
    direction TEXT NOT NULL CHECK (direction IN ('upload', 'download')),
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'processing', 'completed', 'failed', 'partially_failed')),
    item_count INTEGER DEFAULT 0,       -- Number of change_log entries included
    total_size INTEGER DEFAULT 0,       -- Estimated size for network awareness
    priority INTEGER DEFAULT 5,         -- Maybe influence order of processing
    attempts INTEGER DEFAULT 0,
    last_attempt_at TEXT,
    error_message TEXT,                 -- Summary error if batch failed
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    completed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_sync_batches_status_priority ON sync_batches(status, priority, created_at);
CREATE INDEX IF NOT EXISTS idx_sync_batches_direction ON sync_batches(direction, status);

-- Add FK constraint from change_log to sync_batches now that it's defined
-- Note: SQLite doesn't support ADD CONSTRAINT directly after table creation easily.
-- Usually handled by migration tools or recreating table. Assume it's handled.
-- ALTER TABLE change_log ADD CONSTRAINT fk_change_log_batch FOREIGN KEY (sync_batch_id) REFERENCES sync_batches(batch_id) ON DELETE SET NULL;



-- ----------- Audit Logs (Revised slightly) -----------
-- Kept for non-sync specific auditing. Actions expanded.
CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    action TEXT NOT NULL CHECK (action IN (
        'create', 'update', 'delete', -- Corresponds to change_log operations
        'login_success', 'login_fail', 'logout', -- Auth events
        'sync_upload_start', 'sync_upload_complete', 'sync_upload_fail', -- Sync events
        'sync_download_start', 'sync_download_complete', 'sync_download_fail',
        'merge_conflict_resolved', 'merge_conflict_detected', -- Sync details
        'permission_denied', 'data_export', 'data_import' -- Other actions
        )),
    entity_table TEXT, -- Table related to action (users, projects...)
    entity_id TEXT,    -- Record ID related to action
    field_name TEXT,   -- Field related to action (if applicable)
    details TEXT,      -- JSON blob for extra context (e.g., error msg, IP address, change diff summary)
    timestamp TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE -- Audit remains even if user deleted? Maybe SET NULL?
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_user_id ON audit_logs(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs(timestamp);
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_target ON audit_logs(entity_table, entity_id);




-- #############################################################
-- ##          REVISED TRIGGERS (Example - App Logic Recommended) ##
-- #############################################################

-- Note: Change logging and related operations are handled at the application level
-- in the Rust code for better atomicity, context access, and maintainability.
-- This is preferred over database triggers for this use case.

-- Example of how complex triggers would get (for reference only):
/*
CREATE TRIGGER after_user_update_generate_changelog
AFTER UPDATE ON users
WHEN OLD.updated_at <> NEW.updated_at -- Only log actual updates
BEGIN
    -- Log email change
    INSERT INTO change_log (operation_id, entity_table, entity_id, operation_type, field_name, old_value, new_value, timestamp, user_id, device_id)
    SELECT lower(hex(randomblob(16))), 'users', NEW.id, 'update', 'email', json_quote(OLD.email), json_quote(NEW.email), NEW.email_updated_at, NEW.email_updated_by, NULL -- device_id unavailable here
    WHERE OLD.email IS NOT NEW.email; -- Check null safety

    -- Log name change
    INSERT INTO change_log (operation_id, entity_table, entity_id, operation_type, field_name, old_value, new_value, timestamp, user_id, device_id)
    SELECT lower(hex(randomblob(16))), 'users', NEW.id, 'update', 'name', json_quote(OLD.name), json_quote(NEW.name), NEW.name_updated_at, NEW.name_updated_by, NULL
    WHERE OLD.name IS NOT NEW.name;

    -- Log role change
    INSERT INTO change_log (operation_id, entity_table, entity_id, operation_type, field_name, old_value, new_value, timestamp, user_id, device_id)
    SELECT lower(hex(randomblob(16))), 'users', NEW.id, 'update', 'role', json_quote(OLD.role), json_quote(NEW.role), NEW.role_updated_at, NEW.role_updated_by, NULL
    WHERE OLD.role IS NOT NEW.role;

    -- Log active change
    INSERT INTO change_log (operation_id, entity_table, entity_id, operation_type, field_name, old_value, new_value, timestamp, user_id, device_id)
    SELECT lower(hex(randomblob(16))), 'users', NEW.id, 'update', 'active', json_quote(OLD.active), json_quote(NEW.active), NEW.active_updated_at, NEW.active_updated_by, NULL
    WHERE OLD.active IS NOT NEW.active;

    -- Trigger for soft delete
    INSERT INTO change_log (operation_id, entity_table, entity_id, operation_type, field_name, old_value, new_value, timestamp, user_id, device_id)
    SELECT lower(hex(randomblob(16))), 'users', NEW.id, 'delete', NULL, NULL, NULL, NEW.deleted_at, NEW.deleted_by_user_id, NULL
    WHERE OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL;

    -- Add similar INSERTs for other fields and other tables... this gets HUGE!
END;
*/

-- Recommendation: Implement change_log population in Rust application/repository layer
-- within the same transaction as the main table modification for guaranteed atomicity
-- and easier access to context like device_id.

-- Index on strategic_goals objective_code
CREATE INDEX IF NOT EXISTS idx_strategic_goals_objective_code ON strategic_goals(objective_code);

-- Indexes on name fields and other key text fields for searching/sorting
CREATE INDEX IF NOT EXISTS idx_users_name ON users(name);
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);
CREATE INDEX IF NOT EXISTS idx_participants_name ON participants(name);
CREATE INDEX IF NOT EXISTS idx_donors_name ON donors(name);
CREATE INDEX IF NOT EXISTS idx_document_types_name ON document_types(name);

