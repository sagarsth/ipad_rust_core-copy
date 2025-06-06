-- Migration: Fix Foreign Key Constraints, Handle Nil UUIDs, and Clean Orphaned Records
-- Timestamp: 20250523050000

PRAGMA foreign_keys=OFF;

-- Check if the change_log.user_id foreign key constraint allows NULL values properly
-- The constraint should be: FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE SET NULL

-- Update any existing nil UUIDs to NULL in change_log
UPDATE change_log SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- Update any existing nil UUIDs to NULL in audit_logs
UPDATE audit_logs SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- Users table
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
UPDATE strategic_goals SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET objective_code_updated_by = NULL WHERE objective_code_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET outcome_updated_by = NULL WHERE outcome_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET kpi_updated_by = NULL WHERE kpi_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET target_value_updated_by = NULL WHERE target_value_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET actual_value_updated_by = NULL WHERE actual_value_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET status_id_updated_by = NULL WHERE status_id_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE strategic_goals SET responsible_team_updated_by = NULL WHERE responsible_team_updated_by = '00000000-0000-0000-0000-000000000000';

-- Projects table
UPDATE projects SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET name_updated_by = NULL WHERE name_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET objective_updated_by = NULL WHERE objective_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET outcome_updated_by = NULL WHERE outcome_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET status_id_updated_by = NULL WHERE status_id_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET timeline_updated_by = NULL WHERE timeline_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE projects SET responsible_team_updated_by = NULL WHERE responsible_team_updated_by = '00000000-0000-0000-0000-000000000000';

-- Activities table
UPDATE activities SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET description_updated_by = NULL WHERE description_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET kpi_updated_by = NULL WHERE kpi_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET target_value_updated_by = NULL WHERE target_value_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET actual_value_updated_by = NULL WHERE actual_value_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE activities SET status_id_updated_by = NULL WHERE status_id_updated_by = '00000000-0000-0000-0000-000000000000';

-- Participants table
UPDATE participants SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET name_updated_by = NULL WHERE name_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET gender_updated_by = NULL WHERE gender_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET disability_updated_by = NULL WHERE disability_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET disability_type_updated_by = NULL WHERE disability_type_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET age_group_updated_by = NULL WHERE age_group_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE participants SET location_updated_by = NULL WHERE location_updated_by = '00000000-0000-0000-0000-000000000000';

-- Workshops table
UPDATE workshops SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET title_updated_by = NULL WHERE title_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET objective_updated_by = NULL WHERE objective_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET rationale_updated_by = NULL WHERE rationale_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET methodology_updated_by = NULL WHERE methodology_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET facilitator_updated_by = NULL WHERE facilitator_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET event_date_updated_by = NULL WHERE event_date_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET start_time_updated_by = NULL WHERE start_time_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET end_time_updated_by = NULL WHERE end_time_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET location_updated_by = NULL WHERE location_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET total_male_participants_updated_by = NULL WHERE total_male_participants_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET total_female_participants_updated_by = NULL WHERE total_female_participants_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshops SET total_other_participants_updated_by = NULL WHERE total_other_participants_updated_by = '00000000-0000-0000-0000-000000000000';

-- Workshop Participants table
UPDATE workshop_participants SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE workshop_participants SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE workshop_participants SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE workshop_participants SET notes_updated_by = NULL WHERE notes_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshop_participants SET pre_evaluation_updated_by = NULL WHERE pre_evaluation_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE workshop_participants SET post_evaluation_updated_by = NULL WHERE post_evaluation_updated_by = '00000000-0000-0000-0000-000000000000';

-- Livelihoods table
UPDATE livelihoods SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE livelihoods SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';

UPDATE subsequent_grants SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE subsequent_grants SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE subsequent_grants SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE subsequent_grants SET amount_updated_by = NULL WHERE amount_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE subsequent_grants SET purpose_updated_by = NULL WHERE purpose_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE subsequent_grants SET grant_date_updated_by = NULL WHERE grant_date_updated_by = '00000000-0000-0000-0000-000000000000';

-- Donors table
UPDATE donors SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET name_updated_by = NULL WHERE name_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET type_updated_by = NULL WHERE type_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET contact_person_updated_by = NULL WHERE contact_person_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET email_updated_by = NULL WHERE email_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET phone_updated_by = NULL WHERE phone_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET country_updated_by = NULL WHERE country_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET first_donation_date_updated_by = NULL WHERE first_donation_date_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE donors SET notes_updated_by = NULL WHERE notes_updated_by = '00000000-0000-0000-0000-000000000000';

-- Project Funding table
UPDATE project_funding SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET project_id_updated_by = NULL WHERE project_id_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET donor_id_updated_by = NULL WHERE donor_id_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET grant_id_updated_by = NULL WHERE grant_id_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET amount_updated_by = NULL WHERE amount_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET currency_updated_by = NULL WHERE currency_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET start_date_updated_by = NULL WHERE start_date_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET end_date_updated_by = NULL WHERE end_date_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET status_updated_by = NULL WHERE status_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET reporting_requirements_updated_by = NULL WHERE reporting_requirements_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE project_funding SET notes_updated_by = NULL WHERE notes_updated_by = '00000000-0000-0000-0000-000000000000';

-- Document Types table
UPDATE document_types SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET name_updated_by = NULL WHERE name_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET allowed_extensions_updated_by = NULL WHERE allowed_extensions_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET max_size_updated_by = NULL WHERE max_size_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET compression_level_updated_by = NULL WHERE compression_level_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET compression_method_updated_by = NULL WHERE compression_method_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET min_size_for_compression_updated_by = NULL WHERE min_size_for_compression_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET description_updated_by = NULL WHERE description_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET default_priority_updated_by = NULL WHERE default_priority_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET icon_updated_by = NULL WHERE icon_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE document_types SET related_tables_updated_by = NULL WHERE related_tables_updated_by = '00000000-0000-0000-0000-000000000000';

-- Media Documents table
UPDATE media_documents SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE media_documents SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE media_documents SET deleted_by_user_id = NULL WHERE deleted_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE media_documents SET title_updated_by_user_id = NULL WHERE title_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE media_documents SET description_updated_by_user_id = NULL WHERE description_updated_by_user_id = '00000000-0000-0000-0000-000000000000';

-- Document Versions table
UPDATE document_versions SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';

-- Document Access Logs table (now nullable)
UPDATE document_access_logs SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- Compression Queue table (no user fields)

-- Compression Stats table (no user fields)

-- App Settings table
UPDATE app_settings SET created_by_user_id = NULL WHERE created_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE app_settings SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE app_settings SET compression_enabled_updated_by = NULL WHERE compression_enabled_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE app_settings SET default_compression_timing_updated_by = NULL WHERE default_compression_timing_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE app_settings SET background_service_interval_updated_by = NULL WHERE background_service_interval_updated_by = '00000000-0000-0000-0000-000000000000';

-- Sync Settings table (user_id is primary key, so shouldn't update)
UPDATE sync_settings SET max_file_size_updated_by = NULL WHERE max_file_size_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE sync_settings SET compression_enabled_updated_by = NULL WHERE compression_enabled_updated_by = '00000000-0000-0000-0000-000000000000';
UPDATE sync_settings SET compression_timing_updated_by = NULL WHERE compression_timing_updated_by = '00000000-0000-0000-0000-000000000000';

-- Tombstones table
UPDATE tombstones SET deleted_by = NULL WHERE deleted_by = '00000000-0000-0000-0000-000000000000';

-- Device Sync State table (now nullable)
UPDATE device_sync_state SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- Active File Usage table (now nullable)
UPDATE active_file_usage SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- File Deletion Queue table
UPDATE file_deletion_queue SET requested_by = NULL WHERE requested_by = '00000000-0000-0000-0000-000000000000';

-- Sync Configs table
UPDATE sync_configs SET updated_by_user_id = NULL WHERE updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET sync_interval_minutes_updated_by_user_id = NULL WHERE sync_interval_minutes_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET background_sync_enabled_updated_by_user_id = NULL WHERE background_sync_enabled_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET wifi_only_updated_by_user_id = NULL WHERE wifi_only_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET charging_only_updated_by_user_id = NULL WHERE charging_only_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET sync_priority_threshold_updated_by_user_id = NULL WHERE sync_priority_threshold_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET document_sync_enabled_updated_by_user_id = NULL WHERE document_sync_enabled_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET metadata_sync_enabled_updated_by_user_id = NULL WHERE metadata_sync_enabled_updated_by_user_id = '00000000-0000-0000-0000-000000000000';
UPDATE sync_configs SET server_token_updated_by_user_id = NULL WHERE server_token_updated_by_user_id = '00000000-0000-0000-0000-000000000000';

-- Sync Conflicts table
UPDATE sync_conflicts SET resolved_by_user_id = NULL WHERE resolved_by_user_id = '00000000-0000-0000-0000-000000000000';

-- Sync Sessions table (now nullable)
UPDATE sync_sessions SET user_id = NULL WHERE user_id = '00000000-0000-0000-0000-000000000000';

-- =====================================================================================
-- Step 3: Update any other orphaned foreign key references to NULL
-- This handles cases where a user_id exists but is not in the users table
-- =====================================================================================

-- Change log
UPDATE change_log SET user_id = NULL 
WHERE user_id IS NOT NULL 
  AND user_id != '00000000-0000-0000-0000-000000000000' 
  AND user_id NOT IN (SELECT id FROM users);

-- Audit logs
UPDATE audit_logs SET user_id = NULL 
WHERE user_id IS NOT NULL 
  AND user_id != '00000000-0000-0000-0000-000000000000' 
  AND user_id NOT IN (SELECT id FROM users);

-- Users table - record level
UPDATE users SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE users SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE users SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Users table - field level
UPDATE users SET email_updated_by = NULL 
WHERE email_updated_by IS NOT NULL 
  AND email_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND email_updated_by NOT IN (SELECT id FROM users);

UPDATE users SET name_updated_by = NULL 
WHERE name_updated_by IS NOT NULL 
  AND name_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND name_updated_by NOT IN (SELECT id FROM users);

UPDATE users SET role_updated_by = NULL 
WHERE role_updated_by IS NOT NULL 
  AND role_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND role_updated_by NOT IN (SELECT id FROM users);

UPDATE users SET active_updated_by = NULL 
WHERE active_updated_by IS NOT NULL 
  AND active_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND active_updated_by NOT IN (SELECT id FROM users);

-- Strategic Goals table - record level
UPDATE strategic_goals SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Strategic Goals table - field level
UPDATE strategic_goals SET objective_code_updated_by = NULL 
WHERE objective_code_updated_by IS NOT NULL 
  AND objective_code_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND objective_code_updated_by NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET outcome_updated_by = NULL 
WHERE outcome_updated_by IS NOT NULL 
  AND outcome_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND outcome_updated_by NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET kpi_updated_by = NULL 
WHERE kpi_updated_by IS NOT NULL 
  AND kpi_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND kpi_updated_by NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET target_value_updated_by = NULL 
WHERE target_value_updated_by IS NOT NULL 
  AND target_value_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND target_value_updated_by NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET actual_value_updated_by = NULL 
WHERE actual_value_updated_by IS NOT NULL 
  AND actual_value_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND actual_value_updated_by NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET status_id_updated_by = NULL 
WHERE status_id_updated_by IS NOT NULL 
  AND status_id_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND status_id_updated_by NOT IN (SELECT id FROM users);

UPDATE strategic_goals SET responsible_team_updated_by = NULL 
WHERE responsible_team_updated_by IS NOT NULL 
  AND responsible_team_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND responsible_team_updated_by NOT IN (SELECT id FROM users);

-- Status Types table - record level
UPDATE status_types SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE status_types SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE status_types SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Status Types table - field level
UPDATE status_types SET value_updated_by = NULL 
WHERE value_updated_by IS NOT NULL 
  AND value_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND value_updated_by NOT IN (SELECT id FROM users);

-- Projects table - record level
UPDATE projects SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE projects SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE projects SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Projects table - field level
UPDATE projects SET name_updated_by = NULL 
WHERE name_updated_by IS NOT NULL 
  AND name_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND name_updated_by NOT IN (SELECT id FROM users);

UPDATE projects SET objective_updated_by = NULL 
WHERE objective_updated_by IS NOT NULL 
  AND objective_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND objective_updated_by NOT IN (SELECT id FROM users);

UPDATE projects SET outcome_updated_by = NULL 
WHERE outcome_updated_by IS NOT NULL 
  AND outcome_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND outcome_updated_by NOT IN (SELECT id FROM users);

UPDATE projects SET status_id_updated_by = NULL 
WHERE status_id_updated_by IS NOT NULL 
  AND status_id_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND status_id_updated_by NOT IN (SELECT id FROM users);

UPDATE projects SET timeline_updated_by = NULL 
WHERE timeline_updated_by IS NOT NULL 
  AND timeline_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND timeline_updated_by NOT IN (SELECT id FROM users);

UPDATE projects SET responsible_team_updated_by = NULL 
WHERE responsible_team_updated_by IS NOT NULL 
  AND responsible_team_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND responsible_team_updated_by NOT IN (SELECT id FROM users);

-- Activities table - record level
UPDATE activities SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE activities SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE activities SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Activities table - field level
UPDATE activities SET description_updated_by = NULL 
WHERE description_updated_by IS NOT NULL 
  AND description_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND description_updated_by NOT IN (SELECT id FROM users);

UPDATE activities SET kpi_updated_by = NULL 
WHERE kpi_updated_by IS NOT NULL 
  AND kpi_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND kpi_updated_by NOT IN (SELECT id FROM users);

UPDATE activities SET target_value_updated_by = NULL 
WHERE target_value_updated_by IS NOT NULL 
  AND target_value_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND target_value_updated_by NOT IN (SELECT id FROM users);

UPDATE activities SET actual_value_updated_by = NULL 
WHERE actual_value_updated_by IS NOT NULL 
  AND actual_value_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND actual_value_updated_by NOT IN (SELECT id FROM users);

UPDATE activities SET status_id_updated_by = NULL 
WHERE status_id_updated_by IS NOT NULL 
  AND status_id_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND status_id_updated_by NOT IN (SELECT id FROM users);

-- Participants table - record level
UPDATE participants SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE participants SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE participants SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Participants table - field level
UPDATE participants SET name_updated_by = NULL 
WHERE name_updated_by IS NOT NULL 
  AND name_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND name_updated_by NOT IN (SELECT id FROM users);

UPDATE participants SET gender_updated_by = NULL 
WHERE gender_updated_by IS NOT NULL 
  AND gender_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND gender_updated_by NOT IN (SELECT id FROM users);

UPDATE participants SET disability_updated_by = NULL 
WHERE disability_updated_by IS NOT NULL 
  AND disability_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND disability_updated_by NOT IN (SELECT id FROM users);

UPDATE participants SET disability_type_updated_by = NULL 
WHERE disability_type_updated_by IS NOT NULL 
  AND disability_type_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND disability_type_updated_by NOT IN (SELECT id FROM users);

UPDATE participants SET age_group_updated_by = NULL 
WHERE age_group_updated_by IS NOT NULL 
  AND age_group_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND age_group_updated_by NOT IN (SELECT id FROM users);

UPDATE participants SET location_updated_by = NULL 
WHERE location_updated_by IS NOT NULL 
  AND location_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND location_updated_by NOT IN (SELECT id FROM users);

-- Workshops table - record level
UPDATE workshops SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE workshops SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE workshops SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Workshops table - field level
UPDATE workshops SET title_updated_by = NULL 
WHERE title_updated_by IS NOT NULL 
  AND title_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND title_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET objective_updated_by = NULL 
WHERE objective_updated_by IS NOT NULL 
  AND objective_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND objective_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET rationale_updated_by = NULL 
WHERE rationale_updated_by IS NOT NULL 
  AND rationale_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND rationale_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET methodology_updated_by = NULL 
WHERE methodology_updated_by IS NOT NULL 
  AND methodology_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND methodology_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET facilitator_updated_by = NULL 
WHERE facilitator_updated_by IS NOT NULL 
  AND facilitator_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND facilitator_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET event_date_updated_by = NULL 
WHERE event_date_updated_by IS NOT NULL 
  AND event_date_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND event_date_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET start_time_updated_by = NULL 
WHERE start_time_updated_by IS NOT NULL 
  AND start_time_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND start_time_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET end_time_updated_by = NULL 
WHERE end_time_updated_by IS NOT NULL 
  AND end_time_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND end_time_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET location_updated_by = NULL 
WHERE location_updated_by IS NOT NULL 
  AND location_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND location_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET total_male_participants_updated_by = NULL 
WHERE total_male_participants_updated_by IS NOT NULL 
  AND total_male_participants_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND total_male_participants_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET total_female_participants_updated_by = NULL 
WHERE total_female_participants_updated_by IS NOT NULL 
  AND total_female_participants_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND total_female_participants_updated_by NOT IN (SELECT id FROM users);

UPDATE workshops SET total_other_participants_updated_by = NULL 
WHERE total_other_participants_updated_by IS NOT NULL 
  AND total_other_participants_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND total_other_participants_updated_by NOT IN (SELECT id FROM users);

-- Workshop Participants table - record level
UPDATE workshop_participants SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE workshop_participants SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE workshop_participants SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Workshop Participants table - field level
UPDATE workshop_participants SET notes_updated_by = NULL 
WHERE notes_updated_by IS NOT NULL 
  AND notes_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND notes_updated_by NOT IN (SELECT id FROM users);

UPDATE workshop_participants SET pre_evaluation_updated_by = NULL 
WHERE pre_evaluation_updated_by IS NOT NULL 
  AND pre_evaluation_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND pre_evaluation_updated_by NOT IN (SELECT id FROM users);

UPDATE workshop_participants SET post_evaluation_updated_by = NULL 
WHERE post_evaluation_updated_by IS NOT NULL 
  AND post_evaluation_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND post_evaluation_updated_by NOT IN (SELECT id FROM users);

-- Livelihoods table - record level
UPDATE livelihoods SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE livelihoods SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE livelihoods SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Livelihoods table - field level
UPDATE livelihoods SET type_updated_by = NULL 
WHERE type_updated_by IS NOT NULL 
  AND type_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND type_updated_by NOT IN (SELECT id FROM users);

UPDATE livelihoods SET description_updated_by = NULL 
WHERE description_updated_by IS NOT NULL 
  AND description_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND description_updated_by NOT IN (SELECT id FROM users);

UPDATE livelihoods SET status_id_updated_by = NULL 
WHERE status_id_updated_by IS NOT NULL 
  AND status_id_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND status_id_updated_by NOT IN (SELECT id FROM users);

UPDATE livelihoods SET initial_grant_date_updated_by = NULL 
WHERE initial_grant_date_updated_by IS NOT NULL 
  AND initial_grant_date_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND initial_grant_date_updated_by NOT IN (SELECT id FROM users);

UPDATE livelihoods SET initial_grant_amount_updated_by = NULL 
WHERE initial_grant_amount_updated_by IS NOT NULL 
  AND initial_grant_amount_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND initial_grant_amount_updated_by NOT IN (SELECT id FROM users);

-- Subsequent Grants table - record level
UPDATE subsequent_grants SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE subsequent_grants SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE subsequent_grants SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Subsequent Grants table - field level
UPDATE subsequent_grants SET amount_updated_by = NULL 
WHERE amount_updated_by IS NOT NULL 
  AND amount_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND amount_updated_by NOT IN (SELECT id FROM users);

UPDATE subsequent_grants SET purpose_updated_by = NULL 
WHERE purpose_updated_by IS NOT NULL 
  AND purpose_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND purpose_updated_by NOT IN (SELECT id FROM users);

UPDATE subsequent_grants SET grant_date_updated_by = NULL 
WHERE grant_date_updated_by IS NOT NULL 
  AND grant_date_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND grant_date_updated_by NOT IN (SELECT id FROM users);

-- Donors table - record level
UPDATE donors SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE donors SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE donors SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Donors table - field level
UPDATE donors SET name_updated_by = NULL 
WHERE name_updated_by IS NOT NULL 
  AND name_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND name_updated_by NOT IN (SELECT id FROM users);

UPDATE donors SET type_updated_by = NULL 
WHERE type_updated_by IS NOT NULL 
  AND type_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND type_updated_by NOT IN (SELECT id FROM users);

UPDATE donors SET contact_person_updated_by = NULL 
WHERE contact_person_updated_by IS NOT NULL 
  AND contact_person_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND contact_person_updated_by NOT IN (SELECT id FROM users);

UPDATE donors SET email_updated_by = NULL 
WHERE email_updated_by IS NOT NULL 
  AND email_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND email_updated_by NOT IN (SELECT id FROM users);

UPDATE donors SET phone_updated_by = NULL 
WHERE phone_updated_by IS NOT NULL 
  AND phone_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND phone_updated_by NOT IN (SELECT id FROM users);

UPDATE donors SET country_updated_by = NULL 
WHERE country_updated_by IS NOT NULL 
  AND country_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND country_updated_by NOT IN (SELECT id FROM users);

UPDATE donors SET first_donation_date_updated_by = NULL 
WHERE first_donation_date_updated_by IS NOT NULL 
  AND first_donation_date_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND first_donation_date_updated_by NOT IN (SELECT id FROM users);

UPDATE donors SET notes_updated_by = NULL 
WHERE notes_updated_by IS NOT NULL 
  AND notes_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND notes_updated_by NOT IN (SELECT id FROM users);

-- Project Funding table - record level
UPDATE project_funding SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE project_funding SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE project_funding SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Project Funding table - field level
UPDATE project_funding SET project_id_updated_by = NULL 
WHERE project_id_updated_by IS NOT NULL 
  AND project_id_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND project_id_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET donor_id_updated_by = NULL 
WHERE donor_id_updated_by IS NOT NULL 
  AND donor_id_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND donor_id_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET grant_id_updated_by = NULL 
WHERE grant_id_updated_by IS NOT NULL 
  AND grant_id_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND grant_id_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET amount_updated_by = NULL 
WHERE amount_updated_by IS NOT NULL 
  AND amount_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND amount_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET currency_updated_by = NULL 
WHERE currency_updated_by IS NOT NULL 
  AND currency_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND currency_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET start_date_updated_by = NULL 
WHERE start_date_updated_by IS NOT NULL 
  AND start_date_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND start_date_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET end_date_updated_by = NULL 
WHERE end_date_updated_by IS NOT NULL 
  AND end_date_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND end_date_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET status_updated_by = NULL 
WHERE status_updated_by IS NOT NULL 
  AND status_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND status_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET reporting_requirements_updated_by = NULL 
WHERE reporting_requirements_updated_by IS NOT NULL 
  AND reporting_requirements_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND reporting_requirements_updated_by NOT IN (SELECT id FROM users);

UPDATE project_funding SET notes_updated_by = NULL 
WHERE notes_updated_by IS NOT NULL 
  AND notes_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND notes_updated_by NOT IN (SELECT id FROM users);

-- Document Types table - record level
UPDATE document_types SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE document_types SET updated_by_user_id = NULL 
WHERE updated_by_user_id IS NOT NULL 
  AND updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND updated_by_user_id NOT IN (SELECT id FROM users);

UPDATE document_types SET deleted_by_user_id = NULL 
WHERE deleted_by_user_id IS NOT NULL 
  AND deleted_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by_user_id NOT IN (SELECT id FROM users);

-- Document Types table - field level
UPDATE document_types SET name_updated_by = NULL 
WHERE name_updated_by IS NOT NULL 
  AND name_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND name_updated_by NOT IN (SELECT id FROM users);

UPDATE document_types SET allowed_extensions_updated_by = NULL 
WHERE allowed_extensions_updated_by IS NOT NULL 
  AND allowed_extensions_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND allowed_extensions_updated_by NOT IN (SELECT id FROM users);

UPDATE document_types SET max_size_updated_by = NULL 
WHERE max_size_updated_by IS NOT NULL 
  AND max_size_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND max_size_updated_by NOT IN (SELECT id FROM users);

UPDATE document_types SET compression_level_updated_by = NULL 
WHERE compression_level_updated_by IS NOT NULL 
  AND compression_level_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND compression_level_updated_by NOT IN (SELECT id FROM users);

UPDATE document_types SET compression_method_updated_by = NULL 
WHERE compression_method_updated_by IS NOT NULL 
  AND compression_method_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND compression_method_updated_by NOT IN (SELECT id FROM users);

UPDATE document_types SET min_size_for_compression_updated_by = NULL 
WHERE min_size_for_compression_updated_by IS NOT NULL 
  AND min_size_for_compression_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND min_size_for_compression_updated_by NOT IN (SELECT id FROM users);

UPDATE document_types SET description_updated_by = NULL 
WHERE description_updated_by IS NOT NULL 
  AND description_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND description_updated_by NOT IN (SELECT id FROM users);

UPDATE document_types SET default_priority_updated_by = NULL 
WHERE default_priority_updated_by IS NOT NULL 
  AND default_priority_updated_by != '00000000-0000-0000-0000-000000000000' 
  AND default_priority_updated_by NOT IN (SELECT id FROM users);

-- Media Documents table
UPDATE media_documents SET created_by_user_id = NULL 
WHERE created_by_user_id IS NOT NULL 
  AND created_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND created_by_user_id NOT IN (SELECT id FROM users);

UPDATE media_documents SET title_updated_by_user_id = NULL 
WHERE title_updated_by_user_id IS NOT NULL 
  AND title_updated_by_user_id != '00000000-0000-0000-0000-000000000000' 
  AND title_updated_by_user_id NOT IN (SELECT id FROM users);

-- Tombstones table
UPDATE tombstones SET deleted_by = NULL 
WHERE deleted_by IS NOT NULL 
  AND deleted_by != '00000000-0000-0000-0000-000000000000' 
  AND deleted_by NOT IN (SELECT id FROM users);

-- Document Access Logs
UPDATE document_access_logs SET user_id = NULL 
WHERE user_id IS NOT NULL 
  AND user_id != '00000000-0000-0000-0000-000000000000' 
  AND user_id NOT IN (SELECT id FROM users);

-- Device Sync State
UPDATE device_sync_state SET user_id = NULL 
WHERE user_id IS NOT NULL 
  AND user_id != '00000000-0000-0000-0000-000000000000' 
  AND user_id NOT IN (SELECT id FROM users);

-- Active File Usage
UPDATE active_file_usage SET user_id = NULL 
WHERE user_id IS NOT NULL 
  AND user_id != '00000000-0000-0000-0000-000000000000' 
  AND user_id NOT IN (SELECT id FROM users);

-- Sync Sessions
UPDATE sync_sessions SET user_id = NULL 
WHERE user_id IS NOT NULL 
  AND user_id != '00000000-0000-0000-0000-000000000000' 
  AND user_id NOT IN (SELECT id FROM users);

-- =====================================================================================
-- Final Step: Re-enable foreign key constraints
-- =====================================================================================
PRAGMA foreign_keys=ON;

-- =====================================================================================
-- Post-migration verification (run these manually after migration)
-- =====================================================================================
-- PRAGMA foreign_key_check;
-- PRAGMA integrity_check;