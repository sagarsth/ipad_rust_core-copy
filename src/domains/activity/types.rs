// src/domains/activity/types.rs

use crate::errors::{DomainError, DomainResult};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::HashSet;
use rust_decimal::Decimal;
use std::str::FromStr;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority as SyncPriorityFromSyncDomain;

/// Activity entity - represents a specific activity within a project
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Activity {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    // Removed name fields
    pub description: Option<String>,
    pub description_updated_at: Option<DateTime<Utc>>,
    pub description_updated_by: Option<Uuid>,
    // Added KPI fields
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<DateTime<Utc>>,
    pub kpi_updated_by: Option<Uuid>,
    // Added Target Value fields
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<DateTime<Utc>>,
    pub target_value_updated_by: Option<Uuid>,
    // Added Actual Value fields
    pub actual_value: Option<f64>, // Changed to Option<f64>
    pub actual_value_updated_at: Option<DateTime<Utc>>,
    pub actual_value_updated_by: Option<Uuid>,
    // Removed start/end date fields
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<DateTime<Utc>>,
    pub status_id_updated_by: Option<Uuid>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Uuid, // Changed to Uuid
    pub updated_by_user_id: Uuid, // Changed to Uuid
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,

    // Document reference fields
    pub photo_evidence_ref: Option<Uuid>,
    pub receipts_ref: Option<Uuid>,
    pub signed_report_ref: Option<Uuid>,
    pub monitoring_data_ref: Option<Uuid>,
    pub output_verification_ref: Option<Uuid>,
}

impl Activity {
    /// Helper to check if activity is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    // Removed date-related helpers: parsed_start_date, parsed_end_date, is_active

     // Helper to calculate progress percentage against target, if applicable
    pub fn progress_percentage(&self) -> Option<f64> {
        if let Some(target) = self.target_value {
            if target > 0.0 {
                // Ensure we don't divide by zero
                return Some((self.actual_value.unwrap_or(0.0) / target) * 100.0);
            }
        }
        None // No target or target is zero
    }

    /// Example calculated field (can be added if needed)
    pub fn calculate_progress_percentage(&self) -> Option<f64> {
        if let (Some(actual), Some(target)) = (self.actual_value, self.target_value) {
            if target > 0.0 {
                return Some((actual / target) * 100.0);
            }
        }
        None
    }
}

impl DocumentLinkable for Activity {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "description", display_name: "Description", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "kpi", display_name: "KPI", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "target_value", display_name: "Target Value", supports_documents: false, field_type: FieldType::Number, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "actual_value", display_name: "Actual Value", supports_documents: true, field_type: FieldType::Number, is_document_reference_only: false }, // e.g., proof of actuals
            EntityFieldMetadata { field_name: "project_id", display_name: "Project", supports_documents: false, field_type: FieldType::Uuid, is_document_reference_only: false },
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "photo_evidence", display_name: "Photo Evidence", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "receipts", display_name: "Receipts", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true }, // May represent multiple docs
            EntityFieldMetadata { field_name: "signed_report", display_name: "Signed Report", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "monitoring_data", display_name: "Monitoring Data", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "output_verification", display_name: "Output Verification", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewActivity DTO - used when creating a new activity
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewActivity {
    pub project_id: Option<Uuid>,
    // Removed name
    pub description: Option<String>, // Made optional, adjust if description is required
    // Added KPI, Target, Actual
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>, // Optional on create, defaults to 0 in DB
    // Removed start/end date
    pub status_id: Option<i64>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewActivity {
    fn validate(&self) -> DomainResult<()> {
        // Validate project_id IF provided
        if let Some(p_id) = self.project_id {
            ValidationBuilder::new("project_id", Some(p_id))
                .not_nil()
                .validate()?;
        }

        // Validate description if it's considered required
        // If optional, this validation can be removed or adjusted.
        // ValidationBuilder::new("description", self.description.as_deref())
        //     .required()
        //     .min_length(2)
        //     .max_length(500) // Example max length
        //     .validate()?;

        // Validate target_value if provided (must be non-negative)
        if let Some(target) = self.target_value {
            ValidationBuilder::new("target_value", Some(target))
                .min(0.0)
                .validate()?;
        }

        // Validate actual_value if provided (must be non-negative)
        if let Some(actual) = self.actual_value {
            ValidationBuilder::new("actual_value", Some(actual))
                .min(0.0)
                .validate()?;
        }

        Ok(())
    }
}

/// UpdateActivity DTO - used when updating an existing activity
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateActivity {
    // Use Option<Option<Uuid>> to allow setting project_id to NULL
    pub project_id: Option<Option<Uuid>>, 
    // Removed name
    pub description: Option<String>,
    // Added KPI, Target, Actual
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    // Removed start/end date
    pub status_id: Option<i64>,
    pub sync_priority: Option<SyncPriorityFromSyncDomain>,
    pub updated_by_user_id: Uuid, // Required for tracking who made the update
}

impl Validate for UpdateActivity {
     fn validate(&self) -> DomainResult<()> {
        // Validate project_id if explicitly provided (even if setting to None)
        if let Some(opt_p_id) = self.project_id {
            if let Some(p_id) = opt_p_id {
                // If an ID is actually provided, ensure it's not nil
                 ValidationBuilder::new("project_id", Some(p_id))
                    .not_nil()
                    .validate()?;
            }
            // Allow Some(None) - represents setting the field to null
        }

        // Validate description if provided
        // if let Some(desc) = &self.description {
        //     ValidationBuilder::new("description", Some(desc.clone()))
        //         .min_length(2)
        //         .max_length(500)
        //         .validate()?;
        // }

         // Validate target_value if provided (must be non-negative)
        if let Some(target) = self.target_value {
            ValidationBuilder::new("target_value", Some(target))
                .min(0.0)
                .validate()?;
        }

        // Validate actual_value if provided (must be non-negative)
        if let Some(actual) = self.actual_value {
            ValidationBuilder::new("actual_value", Some(actual))
                .min(0.0)
                .validate()?;
        }

        Ok(())
    }
}

/// ActivityRow - SQLite row representation for mapping from database
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, FromRow)]
pub struct ActivityRow {
    pub id: String,
    pub project_id: Option<String>,
    // Removed name fields
    pub description: Option<String>,
    pub description_updated_at: Option<String>,
    pub description_updated_by: Option<String>,
    // Added KPI fields
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<String>,
    pub kpi_updated_by: Option<String>,
    // Added Target Value fields
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<String>,
    pub target_value_updated_by: Option<String>,
    // Added Actual Value fields
    pub actual_value: Option<f64>, // Changed to Option<f64>
    pub actual_value_updated_at: Option<String>,
    pub actual_value_updated_by: Option<String>,
    // Removed start/end date fields
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<String>,
    pub status_id_updated_by: Option<String>,
    pub sync_priority: String,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: String, // Keep as String for FromRow
    pub updated_by_user_id: String, // Keep as String for FromRow
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,

    // Document reference columns (TEXT NULL in DB)
    pub photo_evidence_ref: Option<String>,
    pub receipts_ref: Option<String>,
    pub signed_report_ref: Option<String>,
    pub monitoring_data_ref: Option<String>,
    pub output_verification_ref: Option<String>,
}

impl ActivityRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Activity> {
        let parse_uuid = |s: String| Uuid::from_str(&s).map_err(|_| DomainError::InvalidUuid(s));
        let parse_uuid_opt = |s: Option<String>| s.map(parse_uuid).transpose();
        let parse_datetime = |s: String| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Internal(format!("Invalid date format: {}", s)));
        let parse_datetime_opt = |s: Option<String>| s.map(parse_datetime).transpose();
        let parse_sync_priority = |s: String| SyncPriorityFromSyncDomain::from_str(&s).map_err(|_| DomainError::Internal(format!("Invalid sync priority format: {}", s)));

        Ok(Activity {
            id: parse_uuid(self.id)?,
            project_id: parse_uuid_opt(self.project_id)?,
            description: self.description,
            description_updated_at: parse_datetime_opt(self.description_updated_at)?,
            description_updated_by: parse_uuid_opt(self.description_updated_by)?,
            kpi: self.kpi,
            kpi_updated_at: parse_datetime_opt(self.kpi_updated_at)?,
            kpi_updated_by: parse_uuid_opt(self.kpi_updated_by)?,
            target_value: self.target_value,
            target_value_updated_at: parse_datetime_opt(self.target_value_updated_at)?,
            target_value_updated_by: parse_uuid_opt(self.target_value_updated_by)?,
            actual_value: self.actual_value,
            actual_value_updated_at: parse_datetime_opt(self.actual_value_updated_at)?,
            actual_value_updated_by: parse_uuid_opt(self.actual_value_updated_by)?,
            status_id: self.status_id,
            status_id_updated_at: parse_datetime_opt(self.status_id_updated_at)?,
            status_id_updated_by: parse_uuid_opt(self.status_id_updated_by)?,
            sync_priority: parse_sync_priority(self.sync_priority.clone())?,
            created_at: parse_datetime(self.created_at)?,
            updated_at: parse_datetime(self.updated_at)?,
            created_by_user_id: parse_uuid(self.created_by_user_id)?, // Parse from String
            updated_by_user_id: parse_uuid(self.updated_by_user_id)?, // Parse from String
            deleted_at: parse_datetime_opt(self.deleted_at)?,
            deleted_by_user_id: parse_uuid_opt(self.deleted_by_user_id)?,

            // Parse document reference UUIDs
            photo_evidence_ref: parse_uuid_opt(self.photo_evidence_ref)?,
            receipts_ref: parse_uuid_opt(self.receipts_ref)?,
            signed_report_ref: parse_uuid_opt(self.signed_report_ref)?,
            monitoring_data_ref: parse_uuid_opt(self.monitoring_data_ref)?,
            output_verification_ref: parse_uuid_opt(self.output_verification_ref)?,
        })
    }
}

// --- Response DTOs ---

/// Basic project summary for nested responses (consider moving to shared location)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
}

/// Status information (consider moving to shared location)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusInfo {
    pub id: i64,
    pub value: String,
}

/// ActivityResponse DTO - used for API responses
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityResponse {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    // Removed name
    pub description: Option<String>,
    // Added KPI, Target, Actual
    pub kpi: Option<String>,
    pub target_value: Option<f64>,
    pub actual_value: Option<f64>,
    pub progress_percentage: Option<f64>, // Calculated field
    // Removed start/end date, is_active
    pub status_id: Option<i64>,
    pub status: Option<StatusInfo>,      // Populated when details are fetched
    pub project: Option<ProjectSummary>, // Populated when details are fetched
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Uuid,
    pub updated_by_user_id: Uuid,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,

    // Added for enrichment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<MediaDocumentResponse>>,
}

impl From<Activity> for ActivityResponse {
    fn from(activity: Activity) -> Self {
        let progress = activity.progress_percentage(); // Calculate before move
        Self {
            id: activity.id,
            project_id: activity.project_id,
            sync_priority: activity.sync_priority,
            // name removed
            description: activity.description,
            // kpi, target, actual added
            kpi: activity.kpi,
            target_value: activity.target_value,
            actual_value: activity.actual_value,
            progress_percentage: progress,
            // start/end date, is_active removed
            status_id: activity.status_id,
            status: None, // Needs to be populated separately
            project: None, // Needs to be populated separately
            created_at: activity.created_at.to_rfc3339(),
            updated_at: activity.updated_at.to_rfc3339(),
            created_by_user_id: activity.created_by_user_id,
            updated_by_user_id: activity.updated_by_user_id,
            deleted_at: activity.deleted_at,
            deleted_by_user_id: activity.deleted_by_user_id,
            documents: None, // Default to None, enrichment happens in service
        }
    }
}

impl ActivityResponse {
     /// Add project information
    pub fn with_project(mut self, project: ProjectSummary) -> Self {
        self.project = Some(project);
        self
    }

    /// Add status information
    pub fn with_status(mut self, status: StatusInfo) -> Self {
        self.status = Some(status);
        self
    }
}

/// Enum to specify which related entities to include in the response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivityInclude {
    Project,    // Include a summary of the related project
    Status,     // Include details about the status ID
    Documents,  // Include linked media documents
    CreatedBy,  // Include summary of the user who created it
    UpdatedBy,  // Include summary of the user who last updated it
    All,        // Include all of the above
}