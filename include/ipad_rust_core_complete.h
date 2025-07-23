#ifndef IPAD_RUST_CORE_H
#define IPAD_RUST_CORE_H

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

// ============================================================================
// AUTO-GENERATED C HEADER FOR IPAD RUST CORE FFI
// This file was generated automatically from Rust FFI functions
// DO NOT EDIT MANUALLY - regenerate using scripts/generate_header.py
// ============================================================================


// ============================================================================
// ACTIVITY FUNCTIONS (21 functions)
// ============================================================================

int32_t activity_create(const char*, char**);
int32_t activity_create_with_documents(const char*, char**);
int32_t activity_get(const char*, char**);
int32_t activity_list(const char*, char**);
int32_t activity_update(const char*, char**);
int32_t activity_delete(const char*, char**);
int32_t activity_upload_document(const char*, char**);
int32_t activity_bulk_upload_documents(const char*, char**);
int32_t activity_get_statistics(const char*, char**);
int32_t activity_get_status_breakdown(const char*, char**);
int32_t activity_get_metadata_counts(const char*, char**);
int32_t activity_find_by_status(const char*, char**);
int32_t activity_find_by_date_range(const char*, char**);
int32_t activity_search(const char*, char**);
int32_t activity_get_document_references(const char*, char**);
int32_t activity_get_filtered_ids(const char*, char**);
int32_t activity_bulk_update_status(const char*, char**);
int32_t activity_get_workload_by_project(const char*, char**);
int32_t activity_find_stale(const char*, char**);
int32_t activity_get_progress_analysis(const char*, char**);
void activity_free(char*);

// ============================================================================
// AUTH FUNCTIONS (23 functions)
// ============================================================================

int32_t auth_login(const char*, char**);
int32_t auth_verify_token(const char*, char**);
int32_t auth_refresh_token(const char*, char**);
int32_t auth_logout(const char*);
int32_t auth_hash_password(const char*, char**);
int32_t auth_create_user(const char*, const char*, char**);
int32_t auth_get_user(const char*, const char*, char**);
int32_t auth_get_all_users(const char*, char**);
int32_t auth_update_user(const char*, const char*, const char*, char**);
int32_t auth_hard_delete_user(const char*, const char*);
int32_t auth_get_current_user(const char*, char**);
int32_t auth_update_current_user(const char*, const char*, char**);
int32_t auth_change_password(const char*, const char*);
int32_t auth_is_email_unique(const char*, char**);
int32_t auth_initialize_default_accounts(const char*);
int32_t auth_initialize_test_data(const char*);
void auth_free(char*);
int32_t login(const char*, const char*, char**);
int32_t verify_token(const char*, char**);
int32_t refresh_token(const char*, char**);
int32_t logout(const char*, const char*);
int32_t hash_password(const char*, char**);
void free_string(char*);

// ============================================================================
// COMPRESSION FUNCTIONS (32 functions)
// ============================================================================

int32_t compression_compress_document(const char*, char**);
int32_t compression_get_queue_status(char**);
int32_t compression_queue_document(const char*);
int32_t compression_cancel(const char*, char**);
int32_t compression_get_stats(char**);
int32_t compression_get_document_status(const char*, char**);
int32_t compression_update_priority(const char*, char**);
int32_t compression_bulk_update_priority(const char*, char**);
int32_t compression_is_document_in_use(const char*, char**);
void compression_free(char*);
int32_t compression_get_queue_entries(const char*, char**);
int32_t compression_get_default_config(char**);
int32_t compression_validate_config(const char*, char**);
int32_t compression_retry_failed(const char*, char**);
int32_t compression_retry_all_failed(char**);
int32_t compression_process_queue_now(void);
int32_t compression_get_supported_methods(const char*, char**);
int32_t compression_get_document_history(const char*, char**);
int32_t compression_debug_info(char**);
int32_t compression_handle_memory_pressure(int32_t);
int32_t compression_update_ios_state(const char*);
int32_t compression_get_ios_status(char**);
int32_t compression_manual_trigger(const char*, char**);
int32_t compression_reset_stuck_comprehensive(const char*, char**);
int32_t compression_reset_stuck_jobs(const char*, char**);
int32_t compression_handle_background_task_extension(const char*);
int32_t compression_handle_content_visibility(const char*);
int32_t compression_handle_app_lifecycle_event(const char*);
int32_t compression_get_comprehensive_ios_status(char**);
int32_t compression_handle_enhanced_memory_warning(const char*);
int32_t compression_detect_ios_capabilities(char**);
int32_t compression_cleanup_stale_documents(char**);

// ============================================================================
// CORE FUNCTIONS (7 functions)
// ============================================================================

int32_t initialize_library(const char*, const char*, bool, const char*);
void set_offline_mode(bool);
int32_t get_device_id(char**);
bool is_offline_mode(void);
char* get_library_version(void);
char* get_last_error(void);
int32_t set_ios_storage_path(const char*);

// ============================================================================
// DOCUMENT FUNCTIONS (26 functions)
// ============================================================================

int32_t document_type_create(const char*, char**);
int32_t document_type_get(const char*, char**);
int32_t document_type_list(const char*, char**);
int32_t document_type_update(const char*, char**);
int32_t document_type_delete(const char*);
int32_t document_upload(const char*, char**);
int32_t document_bulk_upload(const char*, char**);
int32_t document_get(const char*, char**);
int32_t document_list_by_entity(const char*, char**);
int32_t document_download(const char*, char**);
int32_t document_open(const char*, char**);
int32_t document_is_available(const char*, char**);
int32_t document_delete(const char*);
int32_t document_calculate_summary(const char*, char**);
int32_t document_link_temp(const char*, char**);
int32_t document_register_in_use(const char*);
int32_t document_unregister_in_use(const char*);
int32_t document_type_find_by_name(const char*, char**);
int32_t document_find_by_date_range(const char*, char**);
int32_t document_get_counts_by_entities(const char*, char**);
int32_t document_bulk_update_sync_priority(const char*, char**);
int32_t document_get_versions(const char*, char**);
int32_t document_get_access_logs(const char*, char**);
int32_t document_upload_from_path(const char*, char**);
int32_t document_bulk_upload_from_paths(const char*, char**);
void document_free(char*);

// ============================================================================
// DONOR FUNCTIONS (19 functions)
// ============================================================================

int32_t donor_create(const char*, char**);
int32_t donor_create_with_documents(const char*, char**);
int32_t donor_get(const char*, char**);
int32_t donor_list(const char*, char**);
int32_t donor_update(const char*, char**);
int32_t donor_delete(const char*, char**);
int32_t donor_get_summary(const char*, char**);
int32_t donor_upload_document(const char*, char**);
int32_t donor_bulk_upload_documents(const char*, char**);
int32_t donor_get_statistics(const char*, char**);
int32_t donor_get_type_distribution(const char*, char**);
int32_t donor_get_country_distribution(const char*, char**);
int32_t donor_find_by_type(const char*, char**);
int32_t donor_find_by_country(const char*, char**);
int32_t donor_find_with_recent_donations(const char*, char**);
int32_t donor_find_by_date_range(const char*, char**);
int32_t donor_get_with_funding_details(const char*, char**);
int32_t donor_get_with_document_timeline(const char*, char**);
void donor_free(char*);

// ============================================================================
// EXPORT FUNCTIONS (29 functions)
// ============================================================================

int32_t export_create_export(const char*, const char*, char**);
int32_t export_get_status(const char*, char**);
int32_t export_strategic_goals_by_ids(const char*, const char*, char**);
int32_t export_strategic_goals_all(const char*, const char*, char**);
int32_t export_projects_by_ids(const char*, const char*, char**);
int32_t export_projects_all(const char*, const char*, char**);
int32_t export_participants_by_ids(const char*, const char*, char**);
int32_t export_participants_all(const char*, const char*, char**);
int32_t export_activities_all(const char*, const char*, char**);
int32_t export_donors_all(const char*, const char*, char**);
int32_t export_funding_all(const char*, const char*, char**);
int32_t export_livelihoods_all(const char*, const char*, char**);
int32_t export_workshops_all(const char*, const char*, char**);
int32_t export_unified_all_domains(const char*, const char*, char**);
int32_t export_strategic_goals_by_date_range(const char*, const char*, char**);
int32_t export_strategic_goals_by_filter(const char*, const char*, char**);
int32_t export_projects_by_date_range(const char*, const char*, char**);
int32_t export_activities_by_date_range(const char*, const char*, char**);
int32_t export_donors_by_date_range(const char*, const char*, char**);
int32_t export_funding_by_date_range(const char*, const char*, char**);
int32_t export_livelihoods_by_date_range(const char*, const char*, char**);
int32_t export_workshops_by_date_range(const char*, const char*, char**);
int32_t export_media_documents_by_date_range(const char*, const char*, char**);
int32_t export_unified_by_date_range(const char*, const char*, char**);
int32_t export_media_documents_by_entity(const char*, const char*, char**);
int32_t export_create_custom(const char*, const char*, char**);
int32_t export_validate_request(const char*, const char*, char**);
void export_free(char*);
int32_t export_create(const char*, char**);

// ============================================================================
// FUNDING FUNCTIONS (18 functions)
// ============================================================================

int32_t funding_create(const char*, char**);
int32_t funding_create_with_documents(const char*, char**);
int32_t funding_get(const char*, char**);
int32_t funding_list(const char*, char**);
int32_t funding_update(const char*, char**);
int32_t funding_delete(const char*, char**);
int32_t funding_find_by_donor(const char*, char**);
int32_t funding_find_by_project(const char*, char**);
int32_t funding_find_by_date_range(const char*, char**);
int32_t funding_create_project_funding(const char*, char**);
int32_t funding_update_project_funding(const char*, char**);
int32_t funding_get_analytics(const char*, char**);
int32_t funding_get_by_donor_summary(const char*, char**);
int32_t funding_get_by_project_summary(const char*, char**);
int32_t funding_get_timeline(const char*, char**);
int32_t funding_upload_document(const char*, char**);
int32_t funding_upload_documents_bulk(const char*, char**);
void funding_free(char*);

// ============================================================================
// LIVELIHOOD FUNCTIONS (23 functions)
// ============================================================================

int32_t livelihood_create(const char*, char**);
int32_t livelihood_create_with_documents(const char*, char**);
int32_t livelihood_get(const char*, char**);
int32_t livelihood_list(const char*, char**);
int32_t livelihood_update(const char*, char**);
int32_t livelihood_delete(const char*, char**);
int32_t livelihood_add_subsequent_grant(const char*, char**);
int32_t livelihood_update_subsequent_grant(const char*, char**);
int32_t livelihood_get_subsequent_grant(const char*, char**);
int32_t livelihood_delete_subsequent_grant(const char*);
int32_t livelihood_find_by_date_range(const char*, char**);
int32_t livelihood_find_with_outcome(const char*, char**);
int32_t livelihood_find_without_outcome(const char*, char**);
int32_t livelihood_find_with_multiple_grants(const char*, char**);
int32_t livelihood_get_statistics(const char*, char**);
int32_t livelihood_get_outcome_distribution(const char*, char**);
int32_t livelihood_get_participant_outcome_metrics(const char*, char**);
int32_t livelihood_get_dashboard_metrics(const char*, char**);
int32_t livelihood_get_with_participant_details(const char*, char**);
int32_t livelihood_get_with_document_timeline(const char*, char**);
int32_t livelihood_upload_document(const char*, char**);
int32_t livelihood_upload_documents_bulk(const char*, char**);
void livelihood_free(char*);

// ============================================================================
// PARTICIPANT FUNCTIONS (33 functions)
// ============================================================================

int32_t participant_create(const char*, char**);
int32_t participant_create_with_documents(const char*, char**);
int32_t participant_get(const char*, char**);
int32_t participant_list(const char*, char**);
int32_t participant_update(const char*, char**);
int32_t participant_delete(const char*, char**);
int32_t participant_find_ids_by_filter(const char*, char**);
int32_t participant_find_by_filter(const char*, char**);
int32_t participant_bulk_update_sync_priority_by_filter(const char*, char**);
int32_t participant_search_with_relationships(const char*, char**);
int32_t participant_get_with_enrichment(const char*, char**);
int32_t participant_get_comprehensive_statistics(const char*, char**);
int32_t participant_get_document_references(const char*, char**);
int32_t participant_bulk_update_streaming(const char*, char**);
int32_t participant_get_index_optimization_suggestions(const char*, char**);
int32_t participant_find_ids_by_filter_optimized(const char*, char**);
int32_t participant_upload_document(const char*, char**);
int32_t participant_bulk_upload_documents(const char*, char**);
int32_t participant_get_demographics(const char*, char**);
int32_t participant_get_gender_distribution(const char*, char**);
int32_t participant_get_age_group_distribution(const char*, char**);
int32_t participant_get_location_distribution(const char*, char**);
int32_t participant_get_disability_distribution(const char*, char**);
int32_t participant_find_by_gender(const char*, char**);
int32_t participant_find_by_age_group(const char*, char**);
int32_t participant_find_by_location(const char*, char**);
int32_t participant_find_by_disability(const char*, char**);
int32_t participant_get_workshop_participants(const char*, char**);
int32_t participant_get_with_workshops(const char*, char**);
int32_t participant_get_with_livelihoods(const char*, char**);
int32_t participant_get_with_document_timeline(const char*, char**);
int32_t participant_check_duplicates(const char*, char**);
void participant_free(char*);

// ============================================================================
// PROJECT FUNCTIONS (24 functions)
// ============================================================================

int32_t project_create(const char*, char**);
int32_t project_create_with_documents(const char*, char**);
int32_t project_get(const char*, char**);
int32_t project_list(const char*, char**);
int32_t project_update(const char*, char**);
int32_t project_delete(const char*, char**);
int32_t project_upload_document(const char*, char**);
int32_t project_bulk_upload_documents(const char*, char**);
int32_t project_get_statistics(const char*, char**);
int32_t project_get_status_breakdown(const char*, char**);
int32_t project_get_metadata_counts(const char*, char**);
int32_t project_find_by_status(const char*, char**);
int32_t project_find_by_responsible_team(const char*, char**);
int32_t project_find_by_date_range(const char*, char**);
int32_t project_search(const char*, char**);
int32_t project_get_with_document_timeline(const char*, char**);
int32_t project_get_document_references(const char*, char**);
int32_t project_get_filtered_ids(const char*, char**);
int32_t project_get_team_workload_distribution(const char*, char**);
int32_t project_get_strategic_goal_distribution(const char*, char**);
int32_t project_find_stale(const char*, char**);
int32_t project_get_document_coverage_analysis(const char*, char**);
int32_t project_get_activity_timeline(const char*, char**);
void project_free(char*);

// ============================================================================
// STRATEGIC_GOAL FUNCTIONS (21 functions)
// ============================================================================

int32_t strategic_goal_create(const char*, char**);
int32_t strategic_goal_create_with_documents(const char*, char**);
int32_t strategic_goal_get(const char*, char**);
int32_t strategic_goal_list(const char*, char**);
int32_t strategic_goal_update(const char*, char**);
int32_t strategic_goal_delete(const char*, char**);
int32_t strategic_goal_bulk_delete(const char*, char**);
int32_t strategic_goal_upload_document(const char*, char**);
int32_t strategic_goal_bulk_upload_documents(const char*, char**);
int32_t strategic_goal_upload_document_from_path(const char*, char**);
int32_t strategic_goal_bulk_upload_documents_from_paths(const char*, char**);
int32_t strategic_goal_find_by_status(const char*, char**);
int32_t strategic_goal_find_by_team(const char*, char**);
int32_t strategic_goal_find_by_user_role(const char*, char**);
int32_t strategic_goal_find_stale(const char*, char**);
int32_t strategic_goal_find_by_date_range(const char*, char**);
int32_t strategic_goal_get_status_distribution(const char*, char**);
int32_t strategic_goal_get_value_statistics(const char*, char**);
int32_t strategic_goal_get_filtered_ids(const char*, char**);
int32_t strategic_goal_list_summaries(const char*, char**);
void strategic_goal_free(char*);

// ============================================================================
// USER FUNCTIONS (9 functions)
// ============================================================================

int32_t user_create(const char*, char**);
int32_t user_get(const char*, char**);
int32_t user_get_all(const char*, char**);
int32_t user_update(const char*, char**);
int32_t user_hard_delete(const char*);
int32_t user_is_email_unique(const char*, char**);
int32_t user_change_password(const char*);
int32_t user_get_stats(const char*, char**);
void user_free(char*);

// ============================================================================
// WORKSHOP FUNCTIONS (25 functions)
// ============================================================================

int32_t workshop_create(const char*, char**);
int32_t workshop_create_with_documents(const char*, char**);
int32_t workshop_get(const char*, char**);
int32_t workshop_list(const char*, char**);
int32_t workshop_update(const char*, char**);
int32_t workshop_delete(const char*, char**);
int32_t workshop_add_participant(const char*, char**);
int32_t workshop_remove_participant(const char*);
int32_t workshop_batch_add_participants(const char*, char**);
int32_t workshop_update_participant_evaluation(const char*, char**);
int32_t workshop_find_by_date_range(const char*, char**);
int32_t workshop_find_past(const char*, char**);
int32_t workshop_find_upcoming(const char*, char**);
int32_t workshop_find_by_location(const char*, char**);
int32_t workshop_get_statistics(const char*, char**);
int32_t workshop_get_budget_statistics(const char*, char**);
int32_t workshop_get_project_metrics(const char*, char**);
int32_t workshop_get_participant_attendance(const char*, char**);
int32_t workshop_get_with_participants(const char*, char**);
int32_t workshop_get_with_document_timeline(const char*, char**);
int32_t workshop_get_budget_summaries_for_project(const char*, char**);
int32_t workshop_find_participants_with_missing_evaluations(const char*, char**);
int32_t workshop_upload_document(const char*, char**);
int32_t workshop_upload_documents_bulk(const char*, char**);
void workshop_free(char*);

#ifdef __cplusplus
}
#endif

#endif // IPAD_RUST_CORE_H
