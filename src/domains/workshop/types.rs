use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use std::str::FromStr;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

// Added imports
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};

/// Workshop entity - represents a workshop in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workshop {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub purpose_updated_at: Option<DateTime<Utc>>,
    pub purpose_updated_by: Option<Uuid>,
    pub purpose_updated_by_device_id: Option<Uuid>,
    pub event_date: Option<String>, // ISO date format YYYY-MM-DD
    pub event_date_updated_at: Option<DateTime<Utc>>,
    pub event_date_updated_by: Option<Uuid>,
    pub event_date_updated_by_device_id: Option<Uuid>,
    pub location: Option<String>,
    pub location_updated_at: Option<DateTime<Utc>>,
    pub location_updated_by: Option<Uuid>,
    pub location_updated_by_device_id: Option<Uuid>,
    pub budget: Option<Decimal>,
    pub budget_updated_at: Option<DateTime<Utc>>,
    pub budget_updated_by: Option<Uuid>,
    pub budget_updated_by_device_id: Option<Uuid>,
    pub actuals: Option<Decimal>,
    pub actuals_updated_at: Option<DateTime<Utc>>,
    pub actuals_updated_by: Option<Uuid>,
    pub actuals_updated_by_device_id: Option<Uuid>,
    pub participant_count: i64,
    pub participant_count_updated_at: Option<DateTime<Utc>>,
    pub participant_count_updated_by: Option<Uuid>,
    pub participant_count_updated_by_device_id: Option<Uuid>,
    pub local_partner: Option<String>,
    pub local_partner_updated_at: Option<DateTime<Utc>>,
    pub local_partner_updated_by: Option<Uuid>,
    pub local_partner_updated_by_device_id: Option<Uuid>,
    pub partner_responsibility: Option<String>,
    pub partner_responsibility_updated_at: Option<DateTime<Utc>>,
    pub partner_responsibility_updated_by: Option<Uuid>,
    pub partner_responsibility_updated_by_device_id: Option<Uuid>,
    pub partnership_success: Option<String>,
    pub partnership_success_updated_at: Option<DateTime<Utc>>,
    pub partnership_success_updated_by: Option<Uuid>,
    pub partnership_success_updated_by_device_id: Option<Uuid>,
    pub capacity_challenges: Option<String>,
    pub capacity_challenges_updated_at: Option<DateTime<Utc>>,
    pub capacity_challenges_updated_by: Option<Uuid>,
    pub capacity_challenges_updated_by_device_id: Option<Uuid>,
    pub strengths: Option<String>,
    pub strengths_updated_at: Option<DateTime<Utc>>,
    pub strengths_updated_by: Option<Uuid>,
    pub strengths_updated_by_device_id: Option<Uuid>,
    pub outcomes: Option<String>,
    pub outcomes_updated_at: Option<DateTime<Utc>>,
    pub outcomes_updated_by: Option<Uuid>,
    pub outcomes_updated_by_device_id: Option<Uuid>,
    pub recommendations: Option<String>,
    pub recommendations_updated_at: Option<DateTime<Utc>>,
    pub recommendations_updated_by: Option<Uuid>,
    pub recommendations_updated_by_device_id: Option<Uuid>,
    pub challenge_resolution: Option<String>,
    pub challenge_resolution_updated_at: Option<DateTime<Utc>>,
    pub challenge_resolution_updated_by: Option<Uuid>,
    pub challenge_resolution_updated_by_device_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,
    pub sync_priority: SyncPriority,
}

impl Workshop {
    // Helper to check if workshop is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to parse event date
    pub fn parsed_event_date(&self) -> Option<NaiveDate> {
        self.event_date.as_ref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }
    
    // Helper to calculate variance between budget and actuals
    pub fn budget_variance(&self) -> Option<Decimal> {
        match (self.budget, self.actuals) {
            (Some(budget), Some(actuals)) => Some(actuals - budget),
            _ => None,
        }
    }
    
    // Helper to calculate variance percentage
    pub fn budget_variance_percentage(&self) -> Option<Decimal> {
        match (self.budget, self.actuals) {
            (Some(budget), Some(actuals)) if !budget.is_zero() => {
                Some(((actuals - budget) / budget) * dec!(100.0))
            },
            _ => None,
        }
    }
    
    // Helper to check if workshop is in the past
    pub fn is_past(&self) -> bool {
        if let Some(date) = self.parsed_event_date() {
            let today = chrono::Local::now().date_naive();
            date < today
        } else {
            false
        }
    }
    
    // Helper to check if workshop is in the future
    pub fn is_future(&self) -> bool {
        if let Some(date) = self.parsed_event_date() {
            let today = chrono::Local::now().date_naive();
            date > today
        } else {
            false
        }
    }
}

impl DocumentLinkable for Workshop {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "purpose", display_name: "Purpose", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "event_date", display_name: "Event Date", supports_documents: false, field_type: FieldType::Date, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "location", display_name: "Location", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "budget", display_name: "Budget", supports_documents: true, field_type: FieldType::Decimal, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "actuals", display_name: "Actuals", supports_documents: true, field_type: FieldType::Decimal, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "local_partner", display_name: "Local Partner", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "partner_responsibility", display_name: "Partner Responsibility", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "partnership_success", display_name: "Partnership Success", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "capacity_challenges", display_name: "Capacity Challenges", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "strengths", display_name: "Strengths", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "outcomes", display_name: "Outcomes", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "recommendations", display_name: "Recommendations", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "challenge_resolution", display_name: "Challenge Resolution", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "project_id", display_name: "Project", supports_documents: false, field_type: FieldType::Uuid, is_document_reference_only: false },
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "agenda", display_name: "Agenda", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "materials", display_name: "Materials", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "attendance_sheet", display_name: "Attendance Sheet", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "evaluation_summary", display_name: "Evaluation Summary", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "photos", display_name: "Photos", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewWorkshop DTO - used when creating a new workshop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWorkshop {
    pub project_id: Option<Uuid>,
    pub purpose: Option<String>,
    pub event_date: Option<String>,
    pub location: Option<String>,
    pub budget: Option<Decimal>,
    pub actuals: Option<Decimal>,
    pub participant_count: Option<i64>,
    pub local_partner: Option<String>,
    pub partner_responsibility: Option<String>,
    pub created_by_user_id: Option<Uuid>,
    pub sync_priority: SyncPriority,
}

impl Validate for NewWorkshop {
    fn validate(&self) -> DomainResult<()> {
        // Validate project_id if provided
        if let Some(p_id) = self.project_id {
            ValidationBuilder::new("project_id", Some(p_id))
                .not_nil()
                .validate()?;
        }
        
        if let Some(date) = &self.event_date {
            crate::validation::common::validate_date_format(date, "event_date")?;
        }
        
        // Validate budget if provided (must be non-negative)
        if let Some(budget) = self.budget {
            if budget.is_sign_negative() {
                return Err(DomainError::Validation(ValidationError::invalid_value(
                    "budget", "must be non-negative"
                )));
            }
        }
        
        // Validate actuals if provided (must be non-negative)
        if let Some(actuals) = self.actuals {
            if actuals.is_sign_negative() {
                return Err(DomainError::Validation(ValidationError::invalid_value(
                    "actuals", "must be non-negative"
                )));
            }
        }
        
        Ok(())
    }
}

// Custom serde module for proper double-optional handling in workshop updates
mod double_option_workshop {
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
                println!("🔧 [WORKSHOP_SERDE] visit_none - field missing");
                Ok(None)
            }
            
            fn visit_unit<E>(self) -> Result<Self::Value, E>
            where
                E: Error,
            {
                println!("🔧 [WORKSHOP_SERDE] visit_unit - field is null");
                Ok(Some(None))
            }
            
            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: Error,
            {
                println!("🔧 [WORKSHOP_SERDE] visit_str - field has UUID string");
                let uuid = Uuid::parse_str(value)
                    .map_err(|e| E::custom(format!("Invalid UUID format: {}", e)))?;
                Ok(Some(Some(uuid)))
            }
        }
        
        deserializer.deserialize_any(DoubleOptionVisitor)
    }
}

/// UpdateWorkshop DTO - used when updating an existing workshop
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateWorkshop {
    #[serde(skip_serializing_if = "Option::is_none", deserialize_with = "double_option_workshop::deserialize")]
    pub project_id: Option<Option<Uuid>>,
    pub purpose: Option<String>,
    pub event_date: Option<String>,
    pub location: Option<String>,
    pub budget: Option<Decimal>,
    pub actuals: Option<Decimal>,
    pub participant_count: Option<i64>,
    pub local_partner: Option<String>,
    pub partner_responsibility: Option<String>,
    pub partnership_success: Option<String>,
    pub capacity_challenges: Option<String>,
    pub strengths: Option<String>,
    pub outcomes: Option<String>,
    pub recommendations: Option<String>,
    pub challenge_resolution: Option<String>,
    pub updated_by_user_id: Option<Uuid>,
    pub sync_priority: Option<SyncPriority>,
}

impl Validate for UpdateWorkshop {
    fn validate(&self) -> DomainResult<()> {
        // Validate project_id if explicitly provided
        if let Some(opt_p_id) = self.project_id {
            if let Some(p_id) = opt_p_id {
                ValidationBuilder::new("project_id", Some(p_id)).not_nil().validate()?;
            }
        }
        
        if let Some(date) = &self.event_date {
            crate::validation::common::validate_date_format(date, "event_date")?;
        }
        
        // Validate budget if provided (must be non-negative)
        if let Some(budget) = self.budget {
            if budget.is_sign_negative() {
                return Err(DomainError::Validation(ValidationError::invalid_value(
                    "budget", "must be non-negative"
                )));
            }
        }
        
        // Validate actuals if provided (must be non-negative)
        if let Some(actuals) = self.actuals {
            if actuals.is_sign_negative() {
                return Err(DomainError::Validation(ValidationError::invalid_value(
                    "actuals", "must be non-negative"
                )));
            }
        }
        
        Ok(())
    }
}

/// WorkshopRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct WorkshopRow {
    pub id: String,
    pub project_id: Option<String>,
    pub purpose: Option<String>,
    pub purpose_updated_at: Option<String>,
    pub purpose_updated_by: Option<String>,
    pub purpose_updated_by_device_id: Option<String>,
    pub event_date: Option<String>,
    pub event_date_updated_at: Option<String>,
    pub event_date_updated_by: Option<String>,
    pub event_date_updated_by_device_id: Option<String>,
    pub location: Option<String>,
    pub location_updated_at: Option<String>,
    pub location_updated_by: Option<String>,
    pub location_updated_by_device_id: Option<String>,
    pub budget: Option<String>,
    pub budget_updated_at: Option<String>,
    pub budget_updated_by: Option<String>,
    pub budget_updated_by_device_id: Option<String>,
    pub actuals: Option<String>,
    pub actuals_updated_at: Option<String>,
    pub actuals_updated_by: Option<String>,
    pub actuals_updated_by_device_id: Option<String>,
    pub participant_count: i64,
    pub participant_count_updated_at: Option<String>,
    pub participant_count_updated_by: Option<String>,
    pub participant_count_updated_by_device_id: Option<String>,
    pub local_partner: Option<String>,
    pub local_partner_updated_at: Option<String>,
    pub local_partner_updated_by: Option<String>,
    pub local_partner_updated_by_device_id: Option<String>,
    pub partner_responsibility: Option<String>,
    pub partner_responsibility_updated_at: Option<String>,
    pub partner_responsibility_updated_by: Option<String>,
    pub partner_responsibility_updated_by_device_id: Option<String>,
    pub partnership_success: Option<String>,
    pub partnership_success_updated_at: Option<String>,
    pub partnership_success_updated_by: Option<String>,
    pub partnership_success_updated_by_device_id: Option<String>,
    pub capacity_challenges: Option<String>,
    pub capacity_challenges_updated_at: Option<String>,
    pub capacity_challenges_updated_by: Option<String>,
    pub capacity_challenges_updated_by_device_id: Option<String>,
    pub strengths: Option<String>,
    pub strengths_updated_at: Option<String>,
    pub strengths_updated_by: Option<String>,
    pub strengths_updated_by_device_id: Option<String>,
    pub outcomes: Option<String>,
    pub outcomes_updated_at: Option<String>,
    pub outcomes_updated_by: Option<String>,
    pub outcomes_updated_by_device_id: Option<String>,
    pub recommendations: Option<String>,
    pub recommendations_updated_at: Option<String>,
    pub recommendations_updated_by: Option<String>,
    pub recommendations_updated_by_device_id: Option<String>,
    pub challenge_resolution: Option<String>,
    pub challenge_resolution_updated_at: Option<String>,
    pub challenge_resolution_updated_by: Option<String>,
    pub challenge_resolution_updated_by_device_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub created_by_device_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub updated_by_device_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub deleted_by_device_id: Option<String>,
    pub sync_priority: String,
}

impl WorkshopRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Workshop> {
        let parse_optional_uuid = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", id_str)))),
                None => Ok(None),
            }
        };
        let parse_datetime = |s: &str, field_name: &str| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", s))));
        let parse_optional_datetime = |s: &Option<String>, field_name: &str| -> DomainResult<Option<DateTime<Utc>>> {
            match s {
                Some(dt_str) => DateTime::parse_from_rfc3339(dt_str)
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", dt_str)))),
                None => Ok(None),
            }
        };
        let parse_optional_decimal = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Decimal>> {
            match s {
                Some(val_str) => Decimal::from_str(val_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid Decimal format: {}", val_str)))),
                None => Ok(None),
            }
        };
        
        Ok(Workshop {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::Validation(ValidationError::format("id", &format!("Invalid UUID format: {}", self.id))))?,
            project_id: parse_optional_uuid(&self.project_id, "project_id")?,
            purpose: self.purpose,
            purpose_updated_at: parse_optional_datetime(&self.purpose_updated_at, "purpose_updated_at")?,
            purpose_updated_by: parse_optional_uuid(&self.purpose_updated_by, "purpose_updated_by")?,
            purpose_updated_by_device_id: parse_optional_uuid(&self.purpose_updated_by_device_id, "purpose_updated_by_device_id")?,
            event_date: self.event_date,
            event_date_updated_at: parse_optional_datetime(&self.event_date_updated_at, "event_date_updated_at")?,
            event_date_updated_by: parse_optional_uuid(&self.event_date_updated_by, "event_date_updated_by")?,
            event_date_updated_by_device_id: parse_optional_uuid(&self.event_date_updated_by_device_id, "event_date_updated_by_device_id")?,
            location: self.location,
            location_updated_at: parse_optional_datetime(&self.location_updated_at, "location_updated_at")?,
            location_updated_by: parse_optional_uuid(&self.location_updated_by, "location_updated_by")?,
            location_updated_by_device_id: parse_optional_uuid(&self.location_updated_by_device_id, "location_updated_by_device_id")?,
            budget: parse_optional_decimal(&self.budget, "budget")?,
            budget_updated_at: parse_optional_datetime(&self.budget_updated_at, "budget_updated_at")?,
            budget_updated_by: parse_optional_uuid(&self.budget_updated_by, "budget_updated_by")?,
            budget_updated_by_device_id: parse_optional_uuid(&self.budget_updated_by_device_id, "budget_updated_by_device_id")?,
            actuals: parse_optional_decimal(&self.actuals, "actuals")?,
            actuals_updated_at: parse_optional_datetime(&self.actuals_updated_at, "actuals_updated_at")?,
            actuals_updated_by: parse_optional_uuid(&self.actuals_updated_by, "actuals_updated_by")?,
            actuals_updated_by_device_id: parse_optional_uuid(&self.actuals_updated_by_device_id, "actuals_updated_by_device_id")?,
            participant_count: self.participant_count,
            participant_count_updated_at: parse_optional_datetime(&self.participant_count_updated_at, "participant_count_updated_at")?,
            participant_count_updated_by: parse_optional_uuid(&self.participant_count_updated_by, "participant_count_updated_by")?,
            participant_count_updated_by_device_id: parse_optional_uuid(&self.participant_count_updated_by_device_id, "participant_count_updated_by_device_id")?,
            local_partner: self.local_partner,
            local_partner_updated_at: parse_optional_datetime(&self.local_partner_updated_at, "local_partner_updated_at")?,
            local_partner_updated_by: parse_optional_uuid(&self.local_partner_updated_by, "local_partner_updated_by")?,
            local_partner_updated_by_device_id: parse_optional_uuid(&self.local_partner_updated_by_device_id, "local_partner_updated_by_device_id")?,
            partner_responsibility: self.partner_responsibility,
            partner_responsibility_updated_at: parse_optional_datetime(&self.partner_responsibility_updated_at, "partner_responsibility_updated_at")?,
            partner_responsibility_updated_by: parse_optional_uuid(&self.partner_responsibility_updated_by, "partner_responsibility_updated_by")?,
            partner_responsibility_updated_by_device_id: parse_optional_uuid(&self.partner_responsibility_updated_by_device_id, "partner_responsibility_updated_by_device_id")?,
            partnership_success: self.partnership_success,
            partnership_success_updated_at: parse_optional_datetime(&self.partnership_success_updated_at, "partnership_success_updated_at")?,
            partnership_success_updated_by: parse_optional_uuid(&self.partnership_success_updated_by, "partnership_success_updated_by")?,
            partnership_success_updated_by_device_id: parse_optional_uuid(&self.partnership_success_updated_by_device_id, "partnership_success_updated_by_device_id")?,
            capacity_challenges: self.capacity_challenges,
            capacity_challenges_updated_at: parse_optional_datetime(&self.capacity_challenges_updated_at, "capacity_challenges_updated_at")?,
            capacity_challenges_updated_by: parse_optional_uuid(&self.capacity_challenges_updated_by, "capacity_challenges_updated_by")?,
            capacity_challenges_updated_by_device_id: parse_optional_uuid(&self.capacity_challenges_updated_by_device_id, "capacity_challenges_updated_by_device_id")?,
            strengths: self.strengths,
            strengths_updated_at: parse_optional_datetime(&self.strengths_updated_at, "strengths_updated_at")?,
            strengths_updated_by: parse_optional_uuid(&self.strengths_updated_by, "strengths_updated_by")?,
            strengths_updated_by_device_id: parse_optional_uuid(&self.strengths_updated_by_device_id, "strengths_updated_by_device_id")?,
            outcomes: self.outcomes,
            outcomes_updated_at: parse_optional_datetime(&self.outcomes_updated_at, "outcomes_updated_at")?,
            outcomes_updated_by: parse_optional_uuid(&self.outcomes_updated_by, "outcomes_updated_by")?,
            outcomes_updated_by_device_id: parse_optional_uuid(&self.outcomes_updated_by_device_id, "outcomes_updated_by_device_id")?,
            recommendations: self.recommendations,
            recommendations_updated_at: parse_optional_datetime(&self.recommendations_updated_at, "recommendations_updated_at")?,
            recommendations_updated_by: parse_optional_uuid(&self.recommendations_updated_by, "recommendations_updated_by")?,
            recommendations_updated_by_device_id: parse_optional_uuid(&self.recommendations_updated_by_device_id, "recommendations_updated_by_device_id")?,
            challenge_resolution: self.challenge_resolution,
            challenge_resolution_updated_at: parse_optional_datetime(&self.challenge_resolution_updated_at, "challenge_resolution_updated_at")?,
            challenge_resolution_updated_by: parse_optional_uuid(&self.challenge_resolution_updated_by, "challenge_resolution_updated_by")?,
            challenge_resolution_updated_by_device_id: parse_optional_uuid(&self.challenge_resolution_updated_by_device_id, "challenge_resolution_updated_by_device_id")?,
            created_at: parse_datetime(&self.created_at, "created_at")?,
            updated_at: parse_datetime(&self.updated_at, "updated_at")?,
            created_by_user_id: parse_optional_uuid(&self.created_by_user_id, "created_by_user_id")?,
            created_by_device_id: parse_optional_uuid(&self.created_by_device_id, "created_by_device_id")?,
            updated_by_user_id: parse_optional_uuid(&self.updated_by_user_id, "updated_by_user_id")?,
            updated_by_device_id: parse_optional_uuid(&self.updated_by_device_id, "updated_by_device_id")?,
            deleted_at: parse_optional_datetime(&self.deleted_at, "deleted_at")?,
            deleted_by_user_id: parse_optional_uuid(&self.deleted_by_user_id, "deleted_by_user_id")?,
            deleted_by_device_id: parse_optional_uuid(&self.deleted_by_device_id, "deleted_by_device_id")?,
            sync_priority: SyncPriority::from_str(&self.sync_priority).unwrap_or_default(),
        })
    }
}

/// Basic workshop summary for nested responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopSummary {
    pub id: Uuid,
    pub purpose: Option<String>,
    pub event_date: Option<String>,
    pub location: Option<String>,
    pub participant_count: i64,
}

impl From<Workshop> for WorkshopSummary {
    fn from(workshop: Workshop) -> Self {
        Self {
            id: workshop.id,
            purpose: workshop.purpose,
            event_date: workshop.event_date,
            location: workshop.location,
            participant_count: workshop.participant_count,
        }
    }
}

/// Project summary for workshop responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
}

/// WorkshopResponse DTO - used for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopResponse {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub project: Option<ProjectSummary>,
    pub purpose: Option<String>,
    pub event_date: Option<String>,
    pub location: Option<String>,
    pub budget: Option<Decimal>,
    pub actuals: Option<Decimal>,
    pub variance: Option<Decimal>,
    pub variance_percentage: Option<Decimal>,
    pub participant_count: i64,
    pub local_partner: Option<String>,
    pub partner_responsibility: Option<String>,
    pub partnership_success: Option<String>,
    pub capacity_challenges: Option<String>,
    pub strengths: Option<String>,
    pub outcomes: Option<String>,
    pub recommendations: Option<String>,
    pub challenge_resolution: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub is_past: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participants: Option<Vec<ParticipantSummary>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documents: Option<Vec<MediaDocumentResponse>>,
}

/// Participant summary for workshop responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantSummary {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub disability: bool,
    pub pre_evaluation: Option<String>,
    pub post_evaluation: Option<String>,
}

impl WorkshopResponse {
    /// Create a basic workshop response without related data
    pub fn from_workshop(workshop: Workshop) -> Self {
        // Pre-calculate values from methods before potential moves
        let variance = workshop.budget_variance();
        let variance_percentage = workshop.budget_variance_percentage();
        let is_past = workshop.is_past();
        
        Self {
            id: workshop.id, // Uuid is Copy
            project_id: workshop.project_id, // Uuid is Copy
            project: None, // Initialized later if needed
            purpose: workshop.purpose.clone(), // Clone Option<String>
            event_date: workshop.event_date.clone(), // Clone Option<String>
            location: workshop.location.clone(), // Clone Option<String>
            budget: workshop.budget, // Option<Decimal> is Copy
            actuals: workshop.actuals, // Option<Decimal> is Copy
            variance, // Use pre-calculated value
            variance_percentage, // Use pre-calculated value
            participant_count: workshop.participant_count, // i64 is Copy
            local_partner: workshop.local_partner.clone(), // Clone Option<String>
            partner_responsibility: workshop.partner_responsibility.clone(), // Clone Option<String>
            partnership_success: workshop.partnership_success.clone(), // Clone Option<String>
            capacity_challenges: workshop.capacity_challenges.clone(), // Clone Option<String>
            strengths: workshop.strengths.clone(), // Clone Option<String>
            outcomes: workshop.outcomes.clone(), // Clone Option<String>
            recommendations: workshop.recommendations.clone(), // Clone Option<String>
            challenge_resolution: workshop.challenge_resolution.clone(), // Clone Option<String>
            created_at: workshop.created_at.to_rfc3339(),
            updated_at: workshop.updated_at.to_rfc3339(),
            is_past, // Use pre-calculated value
            participants: None, // Initialized later if needed
            documents: None, // Needs enrichment
        }
    }
    
    /// Add project information
    pub fn with_project(mut self, project: ProjectSummary) -> Self {
        self.project = Some(project);
        self
    }
    
    /// Add participants
    pub fn with_participants(mut self, participants: Vec<ParticipantSummary>) -> Self {
        self.participants = Some(participants);
        self
    }
}

/// Enum for specifying included relations when fetching workshops
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkshopInclude {
    Project,
    Participants,
    Documents,
    ParticipantDetails,
    DocumentCounts,
    BudgetDetails,
    All,
}

// --- Workshop Participant Junction Types ---

/// WorkshopParticipant entity - represents the link between a workshop and a participant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopParticipant {
    pub id: Uuid, // UUID for the relationship instance itself
    pub workshop_id: Uuid,
    pub participant_id: Uuid,
    pub pre_evaluation: Option<String>,
    pub pre_evaluation_updated_at: Option<DateTime<Utc>>,
    pub pre_evaluation_updated_by: Option<Uuid>,
    pub pre_evaluation_updated_by_device_id: Option<Uuid>,
    pub post_evaluation: Option<String>,
    pub post_evaluation_updated_at: Option<DateTime<Utc>>,
    pub post_evaluation_updated_by: Option<Uuid>,
    pub post_evaluation_updated_by_device_id: Option<Uuid>,
    // Core fields
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub created_by_device_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub updated_by_device_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
    pub deleted_by_device_id: Option<Uuid>,
}

impl WorkshopParticipant {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

/// NewWorkshopParticipant DTO - used when adding a participant to a workshop
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewWorkshopParticipant {
    pub workshop_id: Uuid,
    pub participant_id: Uuid,
    pub pre_evaluation: Option<String>,
    pub post_evaluation: Option<String>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewWorkshopParticipant {
    fn validate(&self) -> DomainResult<()> {
        ValidationBuilder::new("workshop_id", Some(self.workshop_id))
            .not_nil()
            .validate()?;
        ValidationBuilder::new("participant_id", Some(self.participant_id))
            .not_nil()
            .validate()?;
            
        // Basic length validation for evaluations if provided
        if let Some(eval) = &self.pre_evaluation {
             ValidationBuilder::new("pre_evaluation", Some(eval.clone()))
                 .max_length(1000) // Example max length
                 .validate()?;
        }
        if let Some(eval) = &self.post_evaluation {
             ValidationBuilder::new("post_evaluation", Some(eval.clone()))
                 .max_length(1000) // Example max length
                 .validate()?;
        }
            
        Ok(())
    }
}

/// UpdateWorkshopParticipant DTO - used when updating evaluation fields
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateWorkshopParticipant {
    pub pre_evaluation: Option<String>,
    pub post_evaluation: Option<String>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateWorkshopParticipant {
    fn validate(&self) -> DomainResult<()> {
         // Basic length validation for evaluations if provided
         if let Some(eval) = &self.pre_evaluation {
             ValidationBuilder::new("pre_evaluation", Some(eval.clone()))
                 .max_length(1000) // Example max length
                 .validate()?;
        }
        if let Some(eval) = &self.post_evaluation {
             ValidationBuilder::new("post_evaluation", Some(eval.clone()))
                 .max_length(1000) // Example max length
                 .validate()?;
        }
        Ok(())
    }
}

/// WorkshopParticipantRow - SQLite row representation
#[derive(Debug, Clone, FromRow)]
pub struct WorkshopParticipantRow {
    pub id: String,
    pub workshop_id: String,
    pub participant_id: String,
    pub pre_evaluation: Option<String>,
    pub pre_evaluation_updated_at: Option<String>,
    pub pre_evaluation_updated_by: Option<String>,
    pub pre_evaluation_updated_by_device_id: Option<String>,
    pub post_evaluation: Option<String>,
    pub post_evaluation_updated_at: Option<String>,
    pub post_evaluation_updated_by: Option<String>,
    pub post_evaluation_updated_by_device_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub created_by_device_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub updated_by_device_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
    pub deleted_by_device_id: Option<String>,
}

impl WorkshopParticipantRow {
    pub fn into_entity(self) -> DomainResult<WorkshopParticipant> {
        let parse_optional_uuid = |s: &Option<String>, field_name: &str| -> DomainResult<Option<Uuid>> {
            match s {
                Some(id_str) => Uuid::parse_str(id_str)
                    .map(Some)
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", id_str)))),
                None => Ok(None),
            }
        };
        let parse_datetime = |s: &str, field_name: &str| DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)).map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", s))));
        let parse_optional_datetime = |s: &Option<String>, field_name: &str| -> DomainResult<Option<DateTime<Utc>>> {
            match s {
                Some(dt_str) => DateTime::parse_from_rfc3339(dt_str)
                    .map(|dt| Some(dt.with_timezone(&Utc)))
                    .map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid RFC3339 format: {}", dt_str)))),
                None => Ok(None),
            }
        };

        Ok(WorkshopParticipant {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::Validation(ValidationError::format("id", &format!("Invalid UUID format: {}", self.id))))?,
            workshop_id: Uuid::parse_str(&self.workshop_id).map_err(|_| DomainError::Validation(ValidationError::format("workshop_id", &format!("Invalid UUID format: {}", self.workshop_id))))?,
            participant_id: Uuid::parse_str(&self.participant_id).map_err(|_| DomainError::Validation(ValidationError::format("participant_id", &format!("Invalid UUID format: {}", self.participant_id))))?,
            pre_evaluation: self.pre_evaluation,
            pre_evaluation_updated_at: parse_optional_datetime(&self.pre_evaluation_updated_at, "pre_evaluation_updated_at")?,
            pre_evaluation_updated_by: parse_optional_uuid(&self.pre_evaluation_updated_by, "pre_evaluation_updated_by")?,
            pre_evaluation_updated_by_device_id: parse_optional_uuid(&self.pre_evaluation_updated_by_device_id, "pre_evaluation_updated_by_device_id")?,
            post_evaluation: self.post_evaluation,
            post_evaluation_updated_at: parse_optional_datetime(&self.post_evaluation_updated_at, "post_evaluation_updated_at")?,
            post_evaluation_updated_by: parse_optional_uuid(&self.post_evaluation_updated_by, "post_evaluation_updated_by")?,
            post_evaluation_updated_by_device_id: parse_optional_uuid(&self.post_evaluation_updated_by_device_id, "post_evaluation_updated_by_device_id")?,
            created_at: parse_datetime(&self.created_at, "created_at")?,
            updated_at: parse_datetime(&self.updated_at, "updated_at")?,
            created_by_user_id: parse_optional_uuid(&self.created_by_user_id, "created_by_user_id")?,
            created_by_device_id: parse_optional_uuid(&self.created_by_device_id, "created_by_device_id")?,
            updated_by_user_id: parse_optional_uuid(&self.updated_by_user_id, "updated_by_user_id")?,
            updated_by_device_id: parse_optional_uuid(&self.updated_by_device_id, "updated_by_device_id")?,
            deleted_at: parse_optional_datetime(&self.deleted_at, "deleted_at")?,
            deleted_by_user_id: parse_optional_uuid(&self.deleted_by_user_id, "deleted_by_user_id")?,
            deleted_by_device_id: parse_optional_uuid(&self.deleted_by_device_id, "deleted_by_device_id")?,
        })
    }
}

// --- New Types Added ---

/// Workshop statistics for dashboard views
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopStatistics {
    pub total_workshops: i64,
    pub past_workshops: i64,
    pub upcoming_workshops: i64,
    pub total_participants: i64,
    pub avg_participants_per_workshop: f64,
    pub total_budget: Decimal,
    pub total_actuals: Decimal,
    pub avg_budget_variance: Decimal,
    pub by_location: HashMap<String, i64>,
    pub by_month: HashMap<String, i64>,
    pub by_project: HashMap<Uuid, i64>,
}

/// Workshop with full participant details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopWithParticipants {
    pub workshop: WorkshopResponse,
    pub participants: Vec<ParticipantDetail>,
    pub total_participants: i64,
    pub evaluation_completion_rate: f64,
}

/// Detailed participant information for workshops
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantDetail {
    pub id: Uuid,
    pub name: String,
    pub gender: Option<String>,
    pub age_group: Option<String>,
    pub disability: bool,
    pub disability_type: Option<String>,
    pub pre_evaluation: Option<String>,
    pub post_evaluation: Option<String>,
    pub evaluation_complete: bool,
}

/// Participant attendance record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantAttendance {
    pub participant_id: Uuid,
    pub participant_name: String,
    pub workshops_attended: i64,
    pub workshops_upcoming: i64,
    pub evaluation_completion_rate: f64,
    pub recent_workshops: Vec<WorkshopSummary>,
}

/// Workshop with document timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopWithDocumentTimeline {
    pub workshop: WorkshopResponse,
    pub documents_by_category: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}

/// Workshop budget summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopBudgetSummary {
    pub workshop_id: Uuid,
    pub purpose: Option<String>,
    pub event_date: Option<String>,
    pub budget: Option<Decimal>,
    pub actuals: Option<Decimal>,
    pub variance: Option<Decimal>,
    pub variance_percentage: Option<Decimal>,
}

/// Project with workshop metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWorkshopMetrics {
    pub project_id: Uuid,
    pub project_name: String,
    pub total_workshops: i64,
    pub completed_workshops: i64,
    pub upcoming_workshops: i64,
    pub total_participants: i64,
    pub total_budget: Decimal,
    pub total_actuals: Decimal,
    pub budget_variance: Decimal,
    pub workshops_by_month: HashMap<String, i64>,
}