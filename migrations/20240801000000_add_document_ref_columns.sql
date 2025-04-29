-- Migration: Add Document Reference Columns to Entities
-- Created: 2024-08-01
-- Description: Adds nullable TEXT columns to store MediaDocument IDs for conceptual document links.

-- --- Participants ---
ALTER TABLE participants ADD COLUMN profile_photo_ref TEXT NULL;
ALTER TABLE participants ADD COLUMN identification_ref TEXT NULL;
ALTER TABLE participants ADD COLUMN consent_form_ref TEXT NULL;
ALTER TABLE participants ADD COLUMN needs_assessment_ref TEXT NULL;
-- Optional FKs:
-- ALTER TABLE participants ADD CONSTRAINT fk_participant_profile_photo FOREIGN KEY (profile_photo_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE participants ADD CONSTRAINT fk_participant_identification FOREIGN KEY (identification_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE participants ADD CONSTRAINT fk_participant_consent_form FOREIGN KEY (consent_form_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE participants ADD CONSTRAINT fk_participant_needs_assessment FOREIGN KEY (needs_assessment_ref) REFERENCES media_documents(id) ON DELETE SET NULL;

-- --- Projects ---
ALTER TABLE projects ADD COLUMN proposal_document_ref TEXT NULL;
ALTER TABLE projects ADD COLUMN budget_document_ref TEXT NULL;
ALTER TABLE projects ADD COLUMN logical_framework_ref TEXT NULL;
ALTER TABLE projects ADD COLUMN final_report_ref TEXT NULL;
ALTER TABLE projects ADD COLUMN monitoring_plan_ref TEXT NULL;
-- Optional FKs:
-- ALTER TABLE projects ADD CONSTRAINT fk_project_proposal FOREIGN KEY (proposal_document_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE projects ADD CONSTRAINT fk_project_budget FOREIGN KEY (budget_document_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE projects ADD CONSTRAINT fk_project_logframe FOREIGN KEY (logical_framework_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE projects ADD CONSTRAINT fk_project_final_report FOREIGN KEY (final_report_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE projects ADD CONSTRAINT fk_project_monitoring_plan FOREIGN KEY (monitoring_plan_ref) REFERENCES media_documents(id) ON DELETE SET NULL;

-- --- Strategic Goals ---
ALTER TABLE strategic_goals ADD COLUMN supporting_documentation_ref TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN impact_assessment_ref TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN theory_of_change_ref TEXT NULL;
ALTER TABLE strategic_goals ADD COLUMN baseline_data_ref TEXT NULL;
-- Optional FKs:
-- ALTER TABLE strategic_goals ADD CONSTRAINT fk_sg_support_docs FOREIGN KEY (supporting_documentation_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE strategic_goals ADD CONSTRAINT fk_sg_impact_assessment FOREIGN KEY (impact_assessment_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE strategic_goals ADD CONSTRAINT fk_sg_toc FOREIGN KEY (theory_of_change_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE strategic_goals ADD CONSTRAINT fk_sg_baseline FOREIGN KEY (baseline_data_ref) REFERENCES media_documents(id) ON DELETE SET NULL;

-- --- Workshops ---
ALTER TABLE workshops ADD COLUMN agenda_ref TEXT NULL;
ALTER TABLE workshops ADD COLUMN materials_ref TEXT NULL;
ALTER TABLE workshops ADD COLUMN attendance_sheet_ref TEXT NULL;
ALTER TABLE workshops ADD COLUMN evaluation_summary_ref TEXT NULL;
ALTER TABLE workshops ADD COLUMN photos_ref TEXT NULL; -- Could link to a 'gallery' document type or multiple docs via linked_field
-- Optional FKs:
-- ALTER TABLE workshops ADD CONSTRAINT fk_workshop_agenda FOREIGN KEY (agenda_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE workshops ADD CONSTRAINT fk_workshop_materials FOREIGN KEY (materials_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE workshops ADD CONSTRAINT fk_workshop_attendance FOREIGN KEY (attendance_sheet_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE workshops ADD CONSTRAINT fk_workshop_eval_summary FOREIGN KEY (evaluation_summary_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE workshops ADD CONSTRAINT fk_workshop_photos FOREIGN KEY (photos_ref) REFERENCES media_documents(id) ON DELETE SET NULL;

-- --- Livelihoods ---
ALTER TABLE livelihoods ADD COLUMN business_plan_ref TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN grant_agreement_ref TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN receipts_ref TEXT NULL; -- Could link multiple via field or use a specific receipt type
ALTER TABLE livelihoods ADD COLUMN progress_photos_ref TEXT NULL;
ALTER TABLE livelihoods ADD COLUMN case_study_ref TEXT NULL;
-- Optional FKs:
-- ALTER TABLE livelihoods ADD CONSTRAINT fk_live_biz_plan FOREIGN KEY (business_plan_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE livelihoods ADD CONSTRAINT fk_live_grant_agree FOREIGN KEY (grant_agreement_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE livelihoods ADD CONSTRAINT fk_live_receipts FOREIGN KEY (receipts_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE livelihoods ADD CONSTRAINT fk_live_prog_photos FOREIGN KEY (progress_photos_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE livelihoods ADD CONSTRAINT fk_live_case_study FOREIGN KEY (case_study_ref) REFERENCES media_documents(id) ON DELETE SET NULL;

-- --- Donors ---
ALTER TABLE donors ADD COLUMN donor_agreement_ref TEXT NULL;
ALTER TABLE donors ADD COLUMN due_diligence_ref TEXT NULL;
ALTER TABLE donors ADD COLUMN communication_log_ref TEXT NULL;
ALTER TABLE donors ADD COLUMN tax_information_ref TEXT NULL;
ALTER TABLE donors ADD COLUMN annual_report_ref TEXT NULL; -- Report *from* the donor?
-- Optional FKs:
-- ALTER TABLE donors ADD CONSTRAINT fk_donor_agreement FOREIGN KEY (donor_agreement_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE donors ADD CONSTRAINT fk_donor_due_diligence FOREIGN KEY (due_diligence_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE donors ADD CONSTRAINT fk_donor_comm_log FOREIGN KEY (communication_log_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE donors ADD CONSTRAINT fk_donor_tax_info FOREIGN KEY (tax_information_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE donors ADD CONSTRAINT fk_donor_annual_report FOREIGN KEY (annual_report_ref) REFERENCES media_documents(id) ON DELETE SET NULL;

-- --- Subsequent Grants ---
ALTER TABLE subsequent_grants ADD COLUMN grant_application_ref TEXT NULL;
ALTER TABLE subsequent_grants ADD COLUMN grant_report_ref TEXT NULL;
ALTER TABLE subsequent_grants ADD COLUMN receipts_ref TEXT NULL;
-- Optional FKs:
-- ALTER TABLE subsequent_grants ADD CONSTRAINT fk_sg_application FOREIGN KEY (grant_application_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE subsequent_grants ADD CONSTRAINT fk_sg_report FOREIGN KEY (grant_report_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE subsequent_grants ADD CONSTRAINT fk_sg_receipts FOREIGN KEY (receipts_ref) REFERENCES media_documents(id) ON DELETE SET NULL;

-- --- Activities ---
ALTER TABLE activities ADD COLUMN photo_evidence_ref TEXT NULL;
ALTER TABLE activities ADD COLUMN receipts_ref TEXT NULL;
ALTER TABLE activities ADD COLUMN signed_report_ref TEXT NULL;
ALTER TABLE activities ADD COLUMN monitoring_data_ref TEXT NULL;
ALTER TABLE activities ADD COLUMN output_verification_ref TEXT NULL;
-- Optional FKs:
-- ALTER TABLE activities ADD CONSTRAINT fk_activity_photo FOREIGN KEY (photo_evidence_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE activities ADD CONSTRAINT fk_activity_receipts FOREIGN KEY (receipts_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE activities ADD CONSTRAINT fk_activity_signed_report FOREIGN KEY (signed_report_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE activities ADD CONSTRAINT fk_activity_monitoring FOREIGN KEY (monitoring_data_ref) REFERENCES media_documents(id) ON DELETE SET NULL;
-- ALTER TABLE activities ADD CONSTRAINT fk_activity_output_verif FOREIGN KEY (output_verification_ref) REFERENCES media_documents(id) ON DELETE SET NULL; 