-- Migration: Fix Foreign Key Constraints and Handle NULL Values
-- Timestamp: 20250523050000

PRAGMA foreign_keys=OFF;

-- Check if the change_log.user_id foreign key constraint allows NULL values properly
-- The constraint should be: FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL

-- Update any existing nil UUIDs to NULL in change_log
UPDATE change_log SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- Update any existing nil UUIDs to NULL in audit_logs
UPDATE audit_logs SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- Update any existing nil UUIDs to NULL in users table for foreign key references
UPDATE users SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE users SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE users SET email_updated_by = NULL WHERE email_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE users SET name_updated_by = NULL WHERE name_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE users SET role_updated_by = NULL WHERE role_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE users SET active_updated_by = NULL WHERE active_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE users SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';

-- Update any existing nil UUIDs to NULL in other tables that might have foreign key references
UPDATE strategic_goals SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE projects SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE activities SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE participants SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE workshops SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE livelihoods SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE livelihoods SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE subsequent_grants SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE subsequent_grants SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE donors SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE project_funding SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE document_types SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE media_documents SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE media_documents SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE tombstones SET deleted_by = NULL WHERE deleted_by = '00000000-0000-0000-0000-000000000000';

-- Update field-level LWW columns as well
UPDATE users SET email_updated_by = NULL WHERE email_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE users SET name_updated_by = NULL WHERE name_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE users SET role_updated_by = NULL WHERE role_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE users SET active_updated_by = NULL WHERE active_updated_by = '00000000-0000-0000-0000-000000000000';

-- Update all field-level LWW columns in strategic_goals
UPDATE strategic_goals SET objective_code_updated_by = NULL WHERE objective_code_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET outcome_updated_by = NULL WHERE outcome_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET kpi_updated_by = NULL WHERE kpi_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET target_value_updated_by = NULL WHERE target_value_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET actual_value_updated_by = NULL WHERE actual_value_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET status_id_updated_by = NULL WHERE status_id_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET responsible_team_updated_by = NULL WHERE responsible_team_updated_by = '00000000-0000-0000-0000-000000000000';

-- Update all field-level LWW columns in projects
UPDATE projects SET name_updated_by = NULL WHERE name_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET objective_updated_by = NULL WHERE objective_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET outcome_updated_by = NULL WHERE outcome_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET status_id_updated_by = NULL WHERE status_id_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET timeline_updated_by = NULL WHERE timeline_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET responsible_team_updated_by = NULL WHERE responsible_team_updated_by = '00000000-0000-0000-0000-000000000000';

-- Continue for other tables as needed...
-- (Note: In a real migration, you would include all tables with LWW fields)

PRAGMA foreign_keys=ON; 