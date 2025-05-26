use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use std::fmt;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};
use crate::domains::funding::types::ProjectFundingSummary;
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::sync::types::SyncPriority;

/// Donor type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DonorType {
    Individual,
    Foundation,
    Government,
    Corporate,
    Other,
}

impl DonorType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DonorType::Individual => "individual",
            DonorType::Foundation => "foundation",
            DonorType::Government => "government",
            DonorType::Corporate => "corporate",
            DonorType::Other => "other",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "individual" => Some(DonorType::Individual),
            "foundation" => Some(DonorType::Foundation),
            "government" => Some(DonorType::Government),
            "corporate" => Some(DonorType::Corporate),
            "other" => Some(DonorType::Other),
            _ => None,
        }
    }
}

impl fmt::Display for DonorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Donor entity - represents a donor in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Donor {
    pub id: Uuid,
    pub name: String,
    pub name_updated_at: Option<DateTime<Utc>>,
    pub name_updated_by: Option<Uuid>,
    pub name_updated_by_device_id: Option<Uuid>,
    pub type_: Option<String>, // Using type_ to avoid reserved keyword
    pub type_updated_at: Option<DateTime<Utc>>,
    pub type_updated_by: Option<Uuid>,
    pub type_updated_by_device_id: Option<Uuid>,
    pub contact_person: Option<String>,
    pub contact_person_updated_at: Option<DateTime<Utc>>,
    pub contact_person_updated_by: Option<Uuid>,
    pub contact_person_updated_by_device_id: Option<Uuid>,
    pub email: Option<String>,
    pub email_updated_at: Option<DateTime<Utc>>,
    pub email_updated_by: Option<Uuid>,
    pub email_updated_by_device_id: Option<Uuid>,
    pub phone: Option<String>,
    pub phone_updated_at: Option<DateTime<Utc>>,
    pub phone_updated_by: Option<Uuid>,
    pub phone_updated_by_device_id: Option<Uuid>,
    pub country: Option<String>,
    pub country_updated_at: Option<DateTime<Utc>>,
    pub country_updated_by: Option<Uuid>,
    pub country_updated_by_device_id: Option<Uuid>,
    pub first_donation_date: Option<String>, // ISO date format YYYY-MM-DD
    pub first_donation_date_updated_at: Option<DateTime<Utc>>,
    pub first_donation_date_updated_by: Option<Uuid>,
    pub first_donation_date_updated_by_device_id: Option<Uuid>,
    pub notes: Option<String>,
    pub notes_updated_at: Option<DateTime<Utc>>,
    pub notes_updated_by: Option<Uuid>,
    pub notes_updated_by_device_id: Option<Uuid>,
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

impl Donor {
    // Helper to check if donor is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to parse donor type
    pub fn parsed_type(&self) -> Option<DonorType> {
        self.type_.as_ref().and_then(|t| DonorType::from_str(t))
    }
    
    // Helper to parse first donation date
    pub fn parsed_first_donation_date(&self) -> Option<NaiveDate> {
        self.first_donation_date.as_ref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }
}

impl DocumentLinkable for Donor {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "name", display_name: "Donor Name", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "type_", display_name: "Type", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false }, // maps to DonorType enum
            EntityFieldMetadata { field_name: "contact_person", display_name: "Contact Person", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "email", display_name: "Email", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "phone", display_name: "Phone", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "country", display_name: "Country", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "first_donation_date", display_name: "First Donation Date", supports_documents: false, field_type: FieldType::Date, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "notes", display_name: "Notes", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            // Document Reference Fields from Migration
            EntityFieldMetadata { field_name: "donor_agreement", display_name: "Donor Agreement", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "due_diligence", display_name: "Due Diligence Docs", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "communication_log", display_name: "Communication Log", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "tax_information", display_name: "Tax Information", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "annual_report", display_name: "Annual Report (from Donor)", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewDonor DTO - used when creating a new donor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewDonor {
    pub name: String,
    pub type_: Option<String>,
    pub contact_person: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub first_donation_date: Option<String>,
    pub notes: Option<String>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewDonor {
    fn validate(&self) -> DomainResult<()> {
        // Validate name
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(100)
            .validate()?;
            
        // Validate donor type if provided
        if let Some(type_) = &self.type_ {
            ValidationBuilder::new("type", Some(type_.clone()))
                .one_of(&["individual", "foundation", "government", "corporate", "other"], 
                       Some("Invalid donor type"))
                .validate()?;
        }
        
        // Validate email if provided
        if let Some(email) = &self.email {
            ValidationBuilder::new("email", Some(email.clone()))
                .email()
                .validate()?;
        }
        
        // Validate first_donation_date if provided
        if let Some(date) = &self.first_donation_date {
            // Check if date is in valid format (YYYY-MM-DD)
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "first_donation_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        
        Ok(())
    }
}

/// UpdateDonor DTO - used when updating an existing donor
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateDonor {
    pub name: Option<String>,
    pub type_: Option<String>,
    pub contact_person: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub first_donation_date: Option<String>,
    pub notes: Option<String>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateDonor {
    fn validate(&self) -> DomainResult<()> {
        // Validate name if provided
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .min_length(2)
                .max_length(100)
                .validate()?;
        }
        
        // Validate donor type if provided
        if let Some(type_) = &self.type_ {
            ValidationBuilder::new("type", Some(type_.clone()))
                .one_of(&["individual", "foundation", "government", "corporate", "other"], 
                       Some("Invalid donor type"))
                .validate()?;
        }
        
        // Validate email if provided
        if let Some(email) = &self.email {
            ValidationBuilder::new("email", Some(email.clone()))
                .email()
                .validate()?;
        }
        
        // Validate first_donation_date if provided
        if let Some(date) = &self.first_donation_date {
            // Check if date is in valid format (YYYY-MM-DD)
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "first_donation_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        
        Ok(())
    }
}

/// DonorRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct DonorRow {
    pub id: String,
    pub name: String,
    pub name_updated_at: Option<String>,
    pub name_updated_by: Option<String>,
    pub name_updated_by_device_id: Option<String>,
    pub type_: Option<String>,
    pub type_updated_at: Option<String>,
    pub type_updated_by: Option<String>,
    pub type_updated_by_device_id: Option<String>,
    pub contact_person: Option<String>,
    pub contact_person_updated_at: Option<String>,
    pub contact_person_updated_by: Option<String>,
    pub contact_person_updated_by_device_id: Option<String>,
    pub email: Option<String>,
    pub email_updated_at: Option<String>,
    pub email_updated_by: Option<String>,
    pub email_updated_by_device_id: Option<String>,
    pub phone: Option<String>,
    pub phone_updated_at: Option<String>,
    pub phone_updated_by: Option<String>,
    pub phone_updated_by_device_id: Option<String>,
    pub country: Option<String>,
    pub country_updated_at: Option<String>,
    pub country_updated_by: Option<String>,
    pub country_updated_by_device_id: Option<String>,
    pub first_donation_date: Option<String>,
    pub first_donation_date_updated_at: Option<String>,
    pub first_donation_date_updated_by: Option<String>,
    pub first_donation_date_updated_by_device_id: Option<String>,
    pub notes: Option<String>,
    pub notes_updated_at: Option<String>,
    pub notes_updated_by: Option<String>,
    pub notes_updated_by_device_id: Option<String>,
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

impl DonorRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Donor> {
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

        Ok(Donor {
            id: Uuid::parse_str(&self.id).map_err(|_| DomainError::Validation(ValidationError::format("id", &format!("Invalid UUID format: {}", self.id))))?,
            name: self.name,
            name_updated_at: parse_optional_datetime(&self.name_updated_at, "name_updated_at")?,
            name_updated_by: parse_optional_uuid(&self.name_updated_by, "name_updated_by")?,
            name_updated_by_device_id: parse_optional_uuid(&self.name_updated_by_device_id, "name_updated_by_device_id")?,
            type_: self.type_,
            type_updated_at: parse_optional_datetime(&self.type_updated_at, "type_updated_at")?,
            type_updated_by: parse_optional_uuid(&self.type_updated_by, "type_updated_by")?,
            type_updated_by_device_id: parse_optional_uuid(&self.type_updated_by_device_id, "type_updated_by_device_id")?,
            contact_person: self.contact_person,
            contact_person_updated_at: parse_optional_datetime(&self.contact_person_updated_at, "contact_person_updated_at")?,
            contact_person_updated_by: parse_optional_uuid(&self.contact_person_updated_by, "contact_person_updated_by")?,
            contact_person_updated_by_device_id: parse_optional_uuid(&self.contact_person_updated_by_device_id, "contact_person_updated_by_device_id")?,
            email: self.email,
            email_updated_at: parse_optional_datetime(&self.email_updated_at, "email_updated_at")?,
            email_updated_by: parse_optional_uuid(&self.email_updated_by, "email_updated_by")?,
            email_updated_by_device_id: parse_optional_uuid(&self.email_updated_by_device_id, "email_updated_by_device_id")?,
            phone: self.phone,
            phone_updated_at: parse_optional_datetime(&self.phone_updated_at, "phone_updated_at")?,
            phone_updated_by: parse_optional_uuid(&self.phone_updated_by, "phone_updated_by")?,
            phone_updated_by_device_id: parse_optional_uuid(&self.phone_updated_by_device_id, "phone_updated_by_device_id")?,
            country: self.country,
            country_updated_at: parse_optional_datetime(&self.country_updated_at, "country_updated_at")?,
            country_updated_by: parse_optional_uuid(&self.country_updated_by, "country_updated_by")?,
            country_updated_by_device_id: parse_optional_uuid(&self.country_updated_by_device_id, "country_updated_by_device_id")?,
            first_donation_date: self.first_donation_date,
            first_donation_date_updated_at: parse_optional_datetime(&self.first_donation_date_updated_at, "first_donation_date_updated_at")?,
            first_donation_date_updated_by: parse_optional_uuid(&self.first_donation_date_updated_by, "first_donation_date_updated_by")?,
            first_donation_date_updated_by_device_id: parse_optional_uuid(&self.first_donation_date_updated_by_device_id, "first_donation_date_updated_by_device_id")?,
            notes: self.notes,
            notes_updated_at: parse_optional_datetime(&self.notes_updated_at, "notes_updated_at")?,
            notes_updated_by: parse_optional_uuid(&self.notes_updated_by, "notes_updated_by")?,
            notes_updated_by_device_id: parse_optional_uuid(&self.notes_updated_by_device_id, "notes_updated_by_device_id")?,
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

/// DonorSummary - for use in nested responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorSummary {
    pub id: Uuid,
    pub name: String,
    pub type_: Option<String>,
    pub country: Option<String>,
}

impl From<Donor> for DonorSummary {
    fn from(donor: Donor) -> Self {
        Self {
            id: donor.id,
            name: donor.name,
            type_: donor.type_,
            country: donor.country,
        }
    }
}

/// DonorResponse DTO - used for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorResponse {
    pub id: Uuid,
    pub name: String,
    pub type_: Option<String>,
    pub contact_person: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub first_donation_date: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub active_fundings_count: Option<i64>,
    pub total_funding_amount: Option<f64>,
}

impl From<Donor> for DonorResponse {
    fn from(donor: Donor) -> Self {
        Self {
            id: donor.id,
            name: donor.name,
            type_: donor.type_,
            contact_person: donor.contact_person,
            email: donor.email,
            phone: donor.phone,
            country: donor.country,
            first_donation_date: donor.first_donation_date,
            notes: donor.notes,
            created_at: donor.created_at.to_rfc3339(),
            updated_at: donor.updated_at.to_rfc3339(),
            active_fundings_count: None,
            total_funding_amount: None,
        }
    }
}

impl DonorResponse {
    /// Add funding statistics
    pub fn with_funding_stats(mut self, active_count: i64, total_amount: f64) -> Self {
        self.active_fundings_count = Some(active_count);
        self.total_funding_amount = Some(total_amount);
        self
    }
}

/// Enum for specifying included relations when fetching donors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DonorInclude {
    FundingStats,
    Documents,
    FundingDetails,
    All,
}

/// Role a user can have in relation to a donor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserDonorRole {
    Created,    // User created the donor
    Updated,    // User last updated the donor
}

/// Summary of aggregate statistics for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorStatsSummary {
    pub total_donors: i64,
    pub active_donors: i64,
    pub total_donation_amount: Option<f64>,
    pub avg_donation_amount: Option<f64>,
    pub donor_count_by_type: HashMap<String, i64>,
    pub donor_count_by_country: HashMap<String, i64>,
}

/// Funding statistics for a donor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorFundingStats {
    pub active_fundings_count: i64,
    pub total_fundings_count: i64,
    pub total_funding_amount: f64,
    pub active_funding_amount: f64,
    pub avg_funding_amount: f64,
    pub largest_funding_amount: f64,
    pub currency_distribution: HashMap<String, f64>,
}

/// Donor with detailed funding information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorWithFundingDetails {
    pub donor: DonorResponse,
    pub funding_stats: DonorFundingStats,
    pub recent_fundings: Vec<ProjectFundingSummary>,
}

/// Donor with document timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorWithDocumentTimeline {
    pub donor: DonorResponse,
    pub documents_by_month: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}

/// Dashboard statistics for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorDashboardStats {
    pub total_donors: i64,
    pub donors_by_type: HashMap<String, i64>,
    pub donors_by_country: HashMap<String, i64>,
    pub recent_donors_count: i64,
    pub funding_summary: FundingSummaryStats,
}

/// Funding summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingSummaryStats {
    pub total_active_fundings: i64,
    pub total_funding_amount: f64,
    pub avg_funding_amount: f64,
    pub funding_by_currency: HashMap<String, f64>,
}