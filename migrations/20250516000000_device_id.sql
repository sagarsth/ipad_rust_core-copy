-- Migration: Add Device ID for LWW Sync
-- Timestamp: 20250516080000

PRAGMA foreign_keys=OFF;

-- --------------------------------------------
-- Users Table
-- --------------------------------------------
ALTER TABLE users ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE users ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE users ADD COLUMN email_updated_by_device_id TEXT NULL;
ALTER TABLE users ADD COLUMN name_updated_by_device_id TEXT NULL;
ALTER TABLE users ADD COLUMN role_updated_by_device_id TEXT NULL;
ALTER TABLE users ADD COLUMN active_updated_by_device_id TEXT NULL;
ALTER TABLE users ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Status Types Table
-- --------------------------------------------
ALTER TABLE status_types ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE status_types ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE status_types ADD COLUMN value_updated_by_device_id TEXT NULL;
ALTER TABLE status_types ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Strategic Goals Table
-- --------------------------------------------
ALTER TABLE strategic_goals ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN objective_code_updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN outcome_updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN kpi_updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN target_value_updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN actual_value_updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN status_id_updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN responsible_team_updated_by_device_id TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Projects Table
-- --------------------------------------------
ALTER TABLE projects ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN name_updated_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN objective_updated_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN outcome_updated_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN status_id_updated_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN timeline_updated_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN responsible_team_updated_by_device_id TEXT NULL;
ALTER TABLE projects ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Activities Table
-- --------------------------------------------
ALTER TABLE activities ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE activities ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE activities ADD COLUMN description_updated_by_device_id TEXT NULL;
ALTER TABLE activities ADD COLUMN kpi_updated_by_device_id TEXT NULL;
ALTER TABLE activities ADD COLUMN target_value_updated_by_device_id TEXT NULL;
ALTER TABLE activities ADD COLUMN actual_value_updated_by_device_id TEXT NULL;
ALTER TABLE activities ADD COLUMN status_id_updated_by_device_id TEXT NULL;
ALTER TABLE activities ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Participants Table
-- --------------------------------------------
ALTER TABLE participants ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN name_updated_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN gender_updated_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN disability_updated_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN disability_type_updated_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN age_group_updated_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN location_updated_by_device_id TEXT NULL;
ALTER TABLE participants ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Workshops Table
-- --------------------------------------------
ALTER TABLE workshops ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN title_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN objective_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN rationale_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN methodology_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN facilitator_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN event_date_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN start_time_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN end_time_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN location_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN total_male_participants_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN total_female_participants_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN total_other_participants_updated_by_device_id TEXT NULL;
ALTER TABLE workshops ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Workshop Participants Table
-- --------------------------------------------
ALTER TABLE workshop_participants ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE workshop_participants ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE workshop_participants ADD COLUMN notes_updated_by_device_id TEXT NULL;
ALTER TABLE workshop_participants ADD COLUMN pre_evaluation_updated_by_device_id TEXT NULL;
ALTER TABLE workshop_participants ADD COLUMN post_evaluation_updated_by_device_id TEXT NULL;
ALTER TABLE workshop_participants ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Livelihoods Table
-- --------------------------------------------
ALTER TABLE livelihoods ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN type_updated_by_device_id TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN description_updated_by_device_id TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN status_id_updated_by_device_id TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN initial_grant_date_updated_by_device_id TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN initial_grant_amount_updated_by_device_id TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Subsequent Grants Table
-- --------------------------------------------
ALTER TABLE subsequent_grants ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE subsequent_grants ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE subsequent_grants ADD COLUMN amount_updated_by_device_id TEXT NULL;
ALTER TABLE subsequent_grants ADD COLUMN purpose_updated_by_device_id TEXT NULL;
ALTER TABLE subsequent_grants ADD COLUMN grant_date_updated_by_device_id TEXT NULL;
ALTER TABLE subsequent_grants ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Donors Table
-- --------------------------------------------
ALTER TABLE donors ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN name_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN type_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN contact_person_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN email_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN phone_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN country_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN first_donation_date_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN notes_updated_by_device_id TEXT NULL;
ALTER TABLE donors ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Project Funding Table
-- --------------------------------------------
ALTER TABLE project_funding ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN project_id_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN donor_id_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN grant_id_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN amount_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN currency_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN start_date_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN end_date_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN status_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN reporting_requirements_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN notes_updated_by_device_id TEXT NULL;
ALTER TABLE project_funding ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Document Types Table
-- --------------------------------------------
ALTER TABLE document_types ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN name_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN allowed_extensions_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN max_size_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN compression_level_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN compression_method_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN min_size_for_compression_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN description_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN default_priority_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN icon_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN related_tables_updated_by_device_id TEXT NULL;
ALTER TABLE document_types ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- Media/Documents Table
-- --------------------------------------------
ALTER TABLE media_documents ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE media_documents ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE media_documents ADD COLUMN deleted_by_device_id TEXT NULL;

-- Add field-level LWW for title in media_documents
ALTER TABLE media_documents ADD COLUMN title_updated_at TEXT NULL;
ALTER TABLE media_documents ADD COLUMN title_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE media_documents ADD COLUMN title_updated_by_device_id TEXT NULL;

-- Add field-level LWW for description in media_documents
ALTER TABLE media_documents ADD COLUMN description_updated_at TEXT NULL;
ALTER TABLE media_documents ADD COLUMN description_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE media_documents ADD COLUMN description_updated_by_device_id TEXT NULL;

-- --------------------------------------------
-- Document Versions Table
-- --------------------------------------------
ALTER TABLE document_versions ADD COLUMN created_by_device_id TEXT NULL;

-- --------------------------------------------
-- Document Access Logs Table
-- --------------------------------------------
ALTER TABLE document_access_logs ADD COLUMN device_id TEXT NULL;

-- --------------------------------------------
-- App Settings Table
-- --------------------------------------------
ALTER TABLE app_settings ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE app_settings ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE app_settings ADD COLUMN compression_enabled_updated_by_device_id TEXT NULL;
ALTER TABLE app_settings ADD COLUMN default_compression_timing_updated_by_device_id TEXT NULL;
ALTER TABLE app_settings ADD COLUMN background_service_interval_updated_by_device_id TEXT NULL;

-- --------------------------------------------
-- Sync Settings Table
-- --------------------------------------------
ALTER TABLE sync_settings ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE sync_settings ADD COLUMN updated_by_device_id TEXT NULL;
ALTER TABLE sync_settings ADD COLUMN max_file_size_updated_by_device_id TEXT NULL;
ALTER TABLE sync_settings ADD COLUMN compression_enabled_updated_by_device_id TEXT NULL;
ALTER TABLE sync_settings ADD COLUMN compression_timing_updated_by_device_id TEXT NULL;

-- --------------------------------------------
-- Audit Logs Table
-- --------------------------------------------
ALTER TABLE audit_logs ADD COLUMN device_id TEXT NULL;

-- --------------------------------------------
-- Tombstones Table
-- --------------------------------------------
ALTER TABLE tombstones ADD COLUMN deleted_by_device_id TEXT NULL;

-- --------------------------------------------
-- File Deletion Queue Table
-- --------------------------------------------
ALTER TABLE file_deletion_queue ADD COLUMN requested_by_device_id TEXT NULL;

-- --------------------------------------------
-- Sync Configs Table (from migration 20250502040000)
-- --------------------------------------------
ALTER TABLE sync_configs ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN sync_interval_minutes_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN sync_interval_minutes_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN sync_interval_minutes_updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN background_sync_enabled_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN background_sync_enabled_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN background_sync_enabled_updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN wifi_only_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN wifi_only_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN wifi_only_updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN charging_only_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN charging_only_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN charging_only_updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN sync_priority_threshold_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN sync_priority_threshold_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN sync_priority_threshold_updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN document_sync_enabled_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN document_sync_enabled_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN document_sync_enabled_updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN metadata_sync_enabled_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN metadata_sync_enabled_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN metadata_sync_enabled_updated_by_device_id TEXT NULL;

ALTER TABLE sync_configs ADD COLUMN server_token_updated_at TEXT NULL;
ALTER TABLE sync_configs ADD COLUMN server_token_updated_by_user_id TEXT NULL REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE sync_configs ADD COLUMN server_token_updated_by_device_id TEXT NULL;

-- --------------------------------------------
-- Sync Conflicts Table (from migration 20250502040000)
-- --------------------------------------------
ALTER TABLE sync_conflicts ADD COLUMN created_by_device_id TEXT NULL;
ALTER TABLE sync_conflicts ADD COLUMN resolved_by_device_id TEXT NULL;

-- =====================================================================================
-- CREATE INDEXES FOR DEVICE_ID COLUMNS
-- =====================================================================================

-- Main record level indexes
CREATE INDEX IF NOT EXISTS idx_users_created_by_device_id ON users(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_users_updated_by_device_id ON users(updated_by_device_id);

CREATE INDEX IF NOT EXISTS idx_strategic_goals_created_by_device_id ON strategic_goals(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_strategic_goals_updated_by_device_id ON strategic_goals(updated_by_device_id);

CREATE INDEX IF NOT EXISTS idx_projects_created_by_device_id ON projects(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_projects_updated_by_device_id ON projects(updated_by_device_id);

CREATE INDEX IF NOT EXISTS idx_activities_created_by_device_id ON activities(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_activities_updated_by_device_id ON activities(updated_by_device_id);

CREATE INDEX IF NOT EXISTS idx_participants_created_by_device_id ON participants(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_participants_updated_by_device_id ON participants(updated_by_device_id);

CREATE INDEX IF NOT EXISTS idx_workshops_created_by_device_id ON workshops(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_workshops_updated_by_device_id ON workshops(updated_by_device_id);

CREATE INDEX IF NOT EXISTS idx_livelihoods_created_by_device_id ON livelihoods(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_livelihoods_updated_by_device_id ON livelihoods(updated_by_device_id);

CREATE INDEX IF NOT EXISTS idx_donors_created_by_device_id ON donors(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_donors_updated_by_device_id ON donors(updated_by_device_id);

-- Media documents (high priority per analysis)
CREATE INDEX IF NOT EXISTS idx_media_documents_created_by_device_id ON media_documents(created_by_device_id);
CREATE INDEX IF NOT EXISTS idx_media_documents_updated_by_device_id ON media_documents(updated_by_device_id);

-- Deletion-related indexes
CREATE INDEX IF NOT EXISTS idx_users_deleted_by_device_id ON users(deleted_by_device_id);
CREATE INDEX IF NOT EXISTS idx_projects_deleted_by_device_id ON projects(deleted_by_device_id);
CREATE INDEX IF NOT EXISTS idx_media_documents_deleted_by_device_id ON media_documents(deleted_by_device_id);
CREATE INDEX IF NOT EXISTS idx_tombstones_deleted_by_device_id ON tombstones(deleted_by_device_id);

-- Access and activity tracking
CREATE INDEX IF NOT EXISTS idx_document_access_logs_device_id ON document_access_logs(device_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_device_id ON audit_logs(device_id);

-- Sync conflicts
CREATE INDEX IF NOT EXISTS idx_sync_conflicts_resolved_by_device_id ON sync_conflicts(resolved_by_device_id);


PRAGMA foreign_keys=ON;