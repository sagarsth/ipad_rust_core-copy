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