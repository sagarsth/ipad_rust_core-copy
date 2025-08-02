use serde::Serialize;
use crate::domains::strategic_goal::types::{StrategicGoal, StrategicGoalResponse};

/// iOS-specific string sanitization
pub fn sanitize_for_ios(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\u{2028}' | '\u{2029}' => None, // Remove line/paragraph separators
            '\u{00A0}' => Some(' '), // Replace non-breaking space
            ';' => Some(','), // iOS apps often confuse ; and ,
            c => Some(c),
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Trait for types that can be exported to CSV
pub trait CsvRecord: Serialize {
    /// Get CSV headers for this type
    fn headers() -> Vec<&'static str>;
    
    /// Convert to CSV row
    fn to_csv(&self) -> Vec<String>;
}

// Macro to implement CsvRecord for common patterns
#[macro_export]
macro_rules! impl_csv_record {
    ($type:ty, $headers:expr, $($field:ident),+) => {
        impl CsvRecord for $type {
            fn headers() -> Vec<&'static str> {
                $headers.to_vec()
            }
            
            fn to_csv(&self) -> Vec<String> {
                vec![
                    $(
                        self.$field.to_string()
                    ),+
                ]
            }
        }
    };
}

// Helper for converting values to CSV-safe strings
pub fn csv_value_to_string<T: std::fmt::Display>(value: &T) -> String {
    sanitize_for_ios(&value.to_string())
}

// Helper for optional values
pub fn csv_optional_to_string<T: std::fmt::Display>(value: &Option<T>) -> String {
    value.as_ref()
        .map(|v| csv_value_to_string(v))
        .unwrap_or_default()
}

// Helper for optional UUID values
pub fn csv_optional_uuid_to_string(value: &Option<uuid::Uuid>) -> String {
    value.as_ref()
        .map(|v| v.to_string())
        .unwrap_or_default()
}

// Helper for datetime formatting
pub fn csv_datetime_to_string(dt: &chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339()
}

// Implementation for StrategicGoal
impl CsvRecord for StrategicGoal {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "objective_code",
            "outcome",
            "kpi",
            "target_value",
            "actual_value",
            "progress_percentage",
            "status_id",
            "responsible_team",
            "sync_priority",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "updated_by_user_id",
            "deleted_at",
            "last_synced_at"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            csv_value_to_string(&self.objective_code),
            csv_optional_to_string(&self.outcome),
            csv_optional_to_string(&self.kpi),
            csv_optional_to_string(&self.target_value),
            csv_optional_to_string(&self.actual_value),
            csv_optional_to_string(&self.progress_percentage()),
            csv_optional_to_string(&self.status_id),
            csv_optional_to_string(&self.responsible_team),
            self.sync_priority.to_string(),
            csv_datetime_to_string(&self.created_at),
            csv_datetime_to_string(&self.updated_at),
            csv_optional_to_string(&self.created_by_user_id),
            csv_optional_to_string(&self.updated_by_user_id),
            String::new(), // deleted_at - not available in StrategicGoal
            String::new(), // last_synced_at - not available in StrategicGoal
        ]
    }
}

// Implementation for ProjectExport 
impl CsvRecord for crate::domains::export::repository_v2::ProjectExport {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "strategic_goal_id", 
            "name",
            "objective",
            "outcome", 
            "status_id",
            "timeline",
            "responsible_team",
            "sync_priority",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "updated_by_user_id",
            "deleted_at"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            csv_optional_uuid_to_string(&self.strategic_goal_id),
            self.name.clone(), // Required field - use directly
            csv_optional_to_string(&self.objective),
            csv_optional_to_string(&self.outcome),
            csv_optional_to_string(&self.status_id),
            csv_optional_to_string(&self.timeline),
            csv_optional_to_string(&self.responsible_team),
            csv_optional_to_string(&self.sync_priority),
            csv_datetime_to_string(&self.created_at),
            csv_datetime_to_string(&self.updated_at),
            csv_optional_uuid_to_string(&self.created_by_user_id),
            csv_optional_uuid_to_string(&self.updated_by_user_id),
            self.deleted_at.as_ref().map(|dt| csv_datetime_to_string(dt)).unwrap_or_default()
        ]
    }
}

impl CsvRecord for crate::domains::export::repository_v2::ParticipantExport {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "name",
            "gender",
            "disability",
            "disability_type",
            "age_group",
            "location",
            "sync_priority",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "created_by_device_id",
            "updated_by_user_id",
            "updated_by_device_id",
            "deleted_at",
            "deleted_by_user_id",
            "deleted_by_device_id",
            "workshop_count",
            "completed_workshop_count",
            "upcoming_workshop_count",
            "livelihood_count",
            "active_livelihood_count",
            "document_count",
            "created_by_username",
            "updated_by_username"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            self.name.clone(), // Required field - use directly
            csv_optional_to_string(&self.gender),
            self.disability.to_string(), // Required field - use directly
            csv_optional_to_string(&self.disability_type),
            csv_optional_to_string(&self.age_group),
            csv_optional_to_string(&self.location),
            csv_optional_to_string(&self.sync_priority),
            csv_datetime_to_string(&self.created_at),
            csv_datetime_to_string(&self.updated_at),
            csv_optional_uuid_to_string(&self.created_by_user_id),
            csv_optional_uuid_to_string(&self.created_by_device_id),
            csv_optional_uuid_to_string(&self.updated_by_user_id),
            csv_optional_uuid_to_string(&self.updated_by_device_id),
            self.deleted_at.as_ref().map(|dt| csv_datetime_to_string(dt)).unwrap_or_default(),
            csv_optional_uuid_to_string(&self.deleted_by_user_id),
            csv_optional_uuid_to_string(&self.deleted_by_device_id),
            self.workshop_count.map(|c| c.to_string()).unwrap_or_default(),
            self.completed_workshop_count.map(|c| c.to_string()).unwrap_or_default(),
            self.upcoming_workshop_count.map(|c| c.to_string()).unwrap_or_default(),
            self.livelihood_count.map(|c| c.to_string()).unwrap_or_default(),
            self.active_livelihood_count.map(|c| c.to_string()).unwrap_or_default(),
            self.document_count.map(|c| c.to_string()).unwrap_or_default(),
            self.created_by_username.clone().unwrap_or_default(),
            self.updated_by_username.clone().unwrap_or_default()
        ]
    }
}

impl CsvRecord for crate::domains::export::repository_v2::ActivityExport {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "project_id", 
            "description",
            "kpi",
            "target_value",
            "actual_value", 
            "status_id",
            "sync_priority",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "created_by_device_id",
            "updated_by_user_id",
            "updated_by_device_id",
            "deleted_at",
            "deleted_by_user_id",
            "deleted_by_device_id",
            "project_name",
            "status_name",
            "progress_percentage",
            "document_count",
            "created_by_username",
            "updated_by_username"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            csv_optional_uuid_to_string(&self.project_id),
            self.description.clone().unwrap_or_default(),
            self.kpi.clone().unwrap_or_default(),
            self.target_value.map(|v| v.to_string()).unwrap_or_default(),
            self.actual_value.map(|v| v.to_string()).unwrap_or_default(),
            self.status_id.map(|v| v.to_string()).unwrap_or_default(),
            self.sync_priority.clone().unwrap_or_default(),
            csv_datetime_to_string(&self.created_at),
            csv_datetime_to_string(&self.updated_at),
            csv_optional_uuid_to_string(&self.created_by_user_id),
            csv_optional_uuid_to_string(&self.created_by_device_id),
            csv_optional_uuid_to_string(&self.updated_by_user_id),
            csv_optional_uuid_to_string(&self.updated_by_device_id),
            self.deleted_at.as_ref().map(|d| csv_datetime_to_string(d)).unwrap_or_default(),
            csv_optional_uuid_to_string(&self.deleted_by_user_id),
            csv_optional_uuid_to_string(&self.deleted_by_device_id),
            self.project_name.clone().unwrap_or_default(),
            self.status_name.clone().unwrap_or_default(),
            self.progress_percentage.map(|v| format!("{:.2}", v)).unwrap_or_default(),
            self.document_count.map(|c| c.to_string()).unwrap_or_default(),
            self.created_by_username.clone().unwrap_or_default(),
            self.updated_by_username.clone().unwrap_or_default()
        ]
    }
}

impl CsvRecord for crate::domains::export::repository_v2::DonorExport {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "name",
            "type_",
            "contact_person",
            "email",
            "phone",
            "country",
            "first_donation_date",
            "notes",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "created_by_device_id",
            "updated_by_user_id",
            "updated_by_device_id",
            "deleted_at",
            "deleted_by_user_id",
            "deleted_by_device_id"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            self.name.clone(),
            self.type_.clone().unwrap_or_default(),
            self.contact_person.clone().unwrap_or_default(),
            self.email.clone().unwrap_or_default(),
            self.phone.clone().unwrap_or_default(),
            self.country.clone().unwrap_or_default(),
            self.first_donation_date.clone().unwrap_or_default(),
            self.notes.clone().unwrap_or_default(),
            self.created_at.to_rfc3339(),
            self.updated_at.to_rfc3339(),
            self.created_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.created_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
            self.updated_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.updated_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
            self.deleted_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            self.deleted_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.deleted_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
        ]
    }
}

impl CsvRecord for crate::domains::export::repository_v2::FundingExport {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "project_id",
            "donor_id",
            "grant_id",
            "amount",
            "currency",
            "start_date",
            "end_date",
            "status",
            "reporting_requirements",
            "notes",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "created_by_device_id",
            "updated_by_user_id",
            "updated_by_device_id",
            "deleted_at",
            "deleted_by_user_id",
            "deleted_by_device_id"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            self.project_id.to_string(),
            self.donor_id.to_string(),
            self.grant_id.clone().unwrap_or_default(),
            self.amount.map(|v| v.to_string()).unwrap_or_default(),
            self.currency.clone(),
            self.start_date.clone().unwrap_or_default(),
            self.end_date.clone().unwrap_or_default(),
            self.status.clone().unwrap_or_default(),
            self.reporting_requirements.clone().unwrap_or_default(),
            self.notes.clone().unwrap_or_default(),
            self.created_at.to_rfc3339(),
            self.updated_at.to_rfc3339(),
            self.created_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.created_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
            self.updated_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.updated_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
            self.deleted_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            self.deleted_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.deleted_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
        ]
    }
}

impl CsvRecord for crate::domains::export::repository_v2::LivelihoodExport {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "participant_id",
            "project_id",
            "type_",
            "description",
            "status_id",
            "initial_grant_date",
            "initial_grant_amount",
            "sync_priority",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "created_by_device_id",
            "updated_by_user_id",
            "updated_by_device_id",
            "deleted_at",
            "deleted_by_user_id",
            "deleted_by_device_id"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            self.participant_id.map(|u| u.to_string()).unwrap_or_default(),
            self.project_id.map(|u| u.to_string()).unwrap_or_default(),
            self.type_.clone(),
            self.description.clone().unwrap_or_default(),
            self.status_id.map(|v| v.to_string()).unwrap_or_default(),
            self.initial_grant_date.clone().unwrap_or_default(),
            self.initial_grant_amount.map(|v| v.to_string()).unwrap_or_default(),
            self.sync_priority.clone(),
            self.created_at.to_rfc3339(),
            self.updated_at.to_rfc3339(),
            self.created_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.created_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
            self.updated_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.updated_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
            self.deleted_at.map(|d| d.to_rfc3339()).unwrap_or_default(),
            self.deleted_by_user_id.map(|u| u.to_string()).unwrap_or_default(),
            self.deleted_by_device_id.map(|u| u.to_string()).unwrap_or_default(),
        ]
    }
}

// Helper functions for CSV formatting

// Implementation for StrategicGoalResponse (more commonly used for exports)
impl CsvRecord for StrategicGoalResponse {
    fn headers() -> Vec<&'static str> {
        vec![
            "id",
            "objective_code",
            "outcome",
            "kpi",
            "target_value",
            "actual_value",
            "progress_percentage",
            "status_id",
            "responsible_team",
            "sync_priority",
            "created_at",
            "updated_at",
            "created_by_user_id",
            "updated_by_user_id",
            "deleted_at",
            "last_synced_at"
        ]
    }
    
    fn to_csv(&self) -> Vec<String> {
        vec![
            self.id.to_string(),
            csv_value_to_string(&self.objective_code),
            csv_optional_to_string(&self.outcome),
            csv_optional_to_string(&self.kpi),
            csv_optional_to_string(&self.target_value),
            csv_optional_to_string(&self.actual_value),
            csv_optional_to_string(&self.progress_percentage),
            csv_optional_to_string(&self.status_id),
            csv_optional_to_string(&self.responsible_team),
            self.sync_priority.to_string(),
            csv_value_to_string(&self.created_at),
            csv_value_to_string(&self.updated_at),
            csv_optional_to_string(&self.created_by_user_id),
            csv_optional_to_string(&self.updated_by_user_id),
            String::new(), // deleted_at - not available in StrategicGoalResponse
            csv_optional_to_string(&self.last_synced_at),
        ]
    }
} 