// src/domains/activity/types.rs

use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};
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
    pub description_updated_by_device_id: Option<Uuid>,
    // Added KPI fields
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<DateTime<Utc>>,
    pub kpi_updated_by: Option<Uuid>,
    pub kpi_updated_by_device_id: Option<Uuid>,
    // Added Target Value fields
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<DateTime<Utc>>,
    pub target_value_updated_by: Option<Uuid>,
    pub target_value_updated_by_device_id: Option<Uuid>,
    // Added Actual Value fields
    pub actual_value: Option<f64>, // Changed to Option<f64>
    pub actual_value_updated_at: Option<DateTime<Utc>>,
    pub actual_value_updated_by: Option<Uuid>,
    pub actual_value_updated_by_device_id: Option<Uuid>,
    // Removed start/end date fields
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<DateTime<Utc>>,
    pub status_id_updated_by: Option<Uuid>,
    pub status_id_updated_by_device_id: Option<Uuid>,
    pub sync_priority: SyncPriorityFromSyncDomain,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Uuid, // Changed to Uuid
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Uuid, // Changed to Uuid
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,

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

// Custom serde module for proper double-optional handling in activity updates
mod double_option_activity {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use uuid::Uuid;

    pub fn serialize<S>(value: &Option<Option<Uuid>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(Some(uuid)) => uuid.serialize(serializer),
            Some(None) => serializer.serialize_none(),
            None => serializer.serialize_none(), // Field not present in update
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};
        use std::fmt;
        
        struct DoubleOptionVisitor;
        
        impl<'de> Visitor<'de> for DoubleOptionVisitor {
            type Value = Option<Option<Uuid>>;
            
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a UUID string, null, or missing field")
            }
            
            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                println!("🔧 [ACTIVITY_SERDE] visit_none - field missing");
                Ok(None)
            }
            
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                println!("🔧 [ACTIVITY_SERDE] visit_unit - field is null");
                Ok(Some(None))
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                println!("🔧 [ACTIVITY_SERDE] visit_str - field has UUID string");
                let uuid = Uuid::parse_str(value)
                    .map_err(|e| E::custom(format!("Invalid UUID format: {}", e)))?;
                Ok(Some(Some(uuid)))
            }
        }
        
        deserializer.deserialize_any(DoubleOptionVisitor)
    }
}

/// UpdateActivity DTO - used when updating an existing activity
/// Aligned with v1_schema.sql
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateActivity {
    // Use Option<Option<Uuid>> to allow setting project_id to NULL
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "double_option_activity::deserialize")]
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
    pub description_updated_by_device_id: Option<String>,
    // Added KPI fields
    pub kpi: Option<String>,
    pub kpi_updated_at: Option<String>,
    pub kpi_updated_by: Option<String>,
    pub kpi_updated_by_device_id: Option<String>,
    // Added Target Value fields
    pub target_value: Option<f64>,
    pub target_value_updated_at: Option<String>,
    pub target_value_updated_by: Option<String>,
    pub target_value_updated_by_device_id: Option<String>,
    // Added Actual Value fields
    pub actual_value: Option<f64>, // Changed to Option<f64>
    pub actual_value_updated_at: Option<String>,
    pub actual_value_updated_by: Option<String>,
    pub actual_value_updated_by_device_id: Option<String>,
    // Removed start/end date fields
    pub status_id: Option<i64>,
    pub status_id_updated_at: Option<String>,
    pub status_id_updated_by: Option<String>,
    pub status_id_updated_by_device_id: Option<String>,
    pub sync_priority: String,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: String, // Keep as String for FromRow
    pub created_by_device_id: Option<String>,
    pub updated_by_user_id: String, // Keep as String for FromRow
    pub updated_by_device_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub deleted_by_device_id: Option<String>,

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
        let parse_uuid = |s: String, field_name: &str| Uuid::from_str(&s).map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", s))));
        let parse_optional_uuid = |s: Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(&id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", id_str)))),
                None => Ok(None),
            }
        };
        let parse_datetime = |s: String, field_name: &str| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", s))));
        let parse_optional_datetime = |s: Option<String>, field_name: &str| -> DomainResult<Option<DateTime<Utc>>> {
             match s {
                Some(dt_str) => DateTime::parse_from_rfc3339(&dt_str)
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", dt_str)))),
                None => Ok(None),
            }
        };
        let parse_sync_priority = |s: String| SyncPriorityFromSyncDomain::from_str(&s).map_err(|e| DomainError::Validation(ValidationError::format("sync_priority", &format!("Invalid sync priority: {} - Error: {}",s, e))));

        Ok(Activity {
            id: parse_uuid(self.id, "id")?,
            project_id: parse_optional_uuid(self.project_id, "project_id")?,
            description: self.description,
            description_updated_at: parse_optional_datetime(self.description_updated_at, "description_updated_at")?,
            description_updated_by: parse_optional_uuid(self.description_updated_by, "description_updated_by")?,
            description_updated_by_device_id: parse_optional_uuid(self.description_updated_by_device_id, "description_updated_by_device_id")?,
            kpi: self.kpi,
            kpi_updated_at: parse_optional_datetime(self.kpi_updated_at, "kpi_updated_at")?,
            kpi_updated_by: parse_optional_uuid(self.kpi_updated_by, "kpi_updated_by")?,
            kpi_updated_by_device_id: parse_optional_uuid(self.kpi_updated_by_device_id, "kpi_updated_by_device_id")?,
            target_value: self.target_value,
            target_value_updated_at: parse_optional_datetime(self.target_value_updated_at, "target_value_updated_at")?,
            target_value_updated_by: parse_optional_uuid(self.target_value_updated_by, "target_value_updated_by")?,
            target_value_updated_by_device_id: parse_optional_uuid(self.target_value_updated_by_device_id, "target_value_updated_by_device_id")?,
            actual_value: self.actual_value,
            actual_value_updated_at: parse_optional_datetime(self.actual_value_updated_at, "actual_value_updated_at")?,
            actual_value_updated_by: parse_optional_uuid(self.actual_value_updated_by, "actual_value_updated_by")?,
            actual_value_updated_by_device_id: parse_optional_uuid(self.actual_value_updated_by_device_id, "actual_value_updated_by_device_id")?,
            status_id: self.status_id,
            status_id_updated_at: parse_optional_datetime(self.status_id_updated_at, "status_id_updated_at")?,
            status_id_updated_by: parse_optional_uuid(self.status_id_updated_by, "status_id_updated_by")?,
            status_id_updated_by_device_id: parse_optional_uuid(self.status_id_updated_by_device_id, "status_id_updated_by_device_id")?,
            sync_priority: parse_sync_priority(self.sync_priority.clone())?,
            created_at: parse_datetime(self.created_at, "created_at")?,
            updated_at: parse_datetime(self.updated_at, "updated_at")?,
            created_by_user_id: parse_uuid(self.created_by_user_id, "created_by_user_id")?,
            created_by_device_id: parse_optional_uuid(self.created_by_device_id, "created_by_device_id")?,
            updated_by_user_id: parse_uuid(self.updated_by_user_id, "updated_by_user_id")?,
            updated_by_device_id: parse_optional_uuid(self.updated_by_device_id, "updated_by_device_id")?,
            deleted_at: parse_optional_datetime(self.deleted_at, "deleted_at")?,
            deleted_by_user_id: parse_optional_uuid(self.deleted_by_user_id, "deleted_by_user_id")?,
            deleted_by_device_id: parse_optional_uuid(self.deleted_by_device_id, "deleted_by_device_id")?,

            // Parse document reference UUIDs
            photo_evidence_ref: parse_optional_uuid(self.photo_evidence_ref, "photo_evidence_ref")?,
            receipts_ref: parse_optional_uuid(self.receipts_ref, "receipts_ref")?,
            signed_report_ref: parse_optional_uuid(self.signed_report_ref, "signed_report_ref")?,
            monitoring_data_ref: parse_optional_uuid(self.monitoring_data_ref, "monitoring_data_ref")?,
            output_verification_ref: parse_optional_uuid(self.output_verification_ref, "output_verification_ref")?,
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
/// Aligned with v1_schema.sql and enhanced with enrichment fields
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<StatusInfo>,      // Populated when details are fetched
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectSummary>, // Populated when details are fetched
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Uuid,
    pub updated_by_user_id: Uuid,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,

    // Enhanced enrichment fields following Project domain patterns
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_by_username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_count: Option<i64>,
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
            // Initialize enrichment fields to None
            created_by_username: None,
            updated_by_username: None,
            project_name: None,
            status_name: None,
            document_count: None,
            documents: None,
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

    /// Set created by username
    pub fn with_created_by_username(mut self, username: String) -> Self {
        self.created_by_username = Some(username);
        self
    }

    /// Set updated by username
    pub fn with_updated_by_username(mut self, username: String) -> Self {
        self.updated_by_username = Some(username);
        self
    }

    /// Set project name
    pub fn with_project_name(mut self, name: String) -> Self {
        self.project_name = Some(name);
        self
    }

    /// Set status name
    pub fn with_status_name(mut self, name: String) -> Self {
        self.status_name = Some(name);
        self
    }

    /// Set document count
    pub fn with_document_count(mut self, count: i64) -> Self {
        self.document_count = Some(count);
        self
    }

    /// Add documents to the response
    pub fn with_documents(mut self, documents: Vec<MediaDocumentResponse>) -> Self {
        self.documents = Some(documents);
        self
    }
}

/// Document reference summary for an activity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityDocumentReference {
    pub field_name: String,
    pub display_name: String,
    pub document_id: Option<Uuid>,
    pub filename: Option<String>,
    pub upload_date: Option<DateTime<Utc>>,
    pub file_size: Option<u64>,
}

/// Comprehensive filter for searching activities with multiple criteria,
/// allowing for intuitive, multi-faceted filtering from the UI.
/// Follows the same pattern as ProjectFilter for consistency.
#[derive(Debug, serde::Deserialize, serde::Serialize, Clone, Default)]
pub struct ActivityFilter {
    /// Filter by one or more statuses (e.g., "In Progress", "Completed").
    /// Uses OR logic internally: status_id = 1 OR status_id = 2.
    pub status_ids: Option<Vec<i64>>,
    
    /// Filter by one or more parent projects.
    /// Uses OR logic internally.
    pub project_ids: Option<Vec<Uuid>>,
    
    /// A free-text search term to apply to description, KPI, etc.
    pub search_text: Option<String>,
    
    /// Filter for activities updated within a specific date range.
    pub date_range: Option<(String, String)>, // (start_rfc3339, end_rfc3339)
    
    /// Filter by target value range (min, max)
    pub target_value_range: Option<(f64, f64)>,
    
    /// Filter by actual value range (min, max)
    pub actual_value_range: Option<(f64, f64)>,
    
    /// Whether to exclude soft-deleted records. Defaults to true.
    pub exclude_deleted: Option<bool>,
}

/// Activity statistics for dashboard views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityStatistics {
    pub total_activities: i64,
    pub by_status: HashMap<String, i64>,
    pub by_project: HashMap<String, i64>,
    pub completion_rate: f64,
    pub average_progress: f64,
    pub document_count: i64,
}

/// Activity status breakdown for reporting
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityStatusBreakdown {
    pub status_id: i64,
    pub status_name: String,
    pub count: i64,
    pub percentage: f64,
}

/// Activity metadata counts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityMetadataCounts {
    pub activities_by_project: HashMap<String, i64>,
    pub activities_by_status: HashMap<String, i64>,
    pub activities_with_targets: i64,
    pub activities_with_actuals: i64,
    pub activities_with_documents: i64,
}

/// Activity progress analysis for dashboard widgets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivityProgressAnalysis {
    pub activities_on_track: i64,        // Progress >= 80%
    pub activities_behind: i64,          // Progress < 50%
    pub activities_at_risk: i64,         // Progress 50-79%
    pub activities_no_progress: i64,     // Progress = 0%
    pub average_progress_percentage: f64,
    pub completion_rate: f64,
    pub activities_with_targets: i64,
    pub activities_without_targets: i64,
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