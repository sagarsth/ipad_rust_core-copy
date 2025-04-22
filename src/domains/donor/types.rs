use crate::errors::{DomainError, DomainResult};
use crate::validation::{Validate, ValidationBuilder, Email};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use std::fmt;

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
    pub type_: Option<String>, // Using type_ to avoid reserved keyword
    pub type_updated_at: Option<DateTime<Utc>>,
    pub type_updated_by: Option<Uuid>,
    pub contact_person: Option<String>,
    pub contact_person_updated_at: Option<DateTime<Utc>>,
    pub contact_person_updated_by: Option<Uuid>,
    pub email: Option<String>,
    pub email_updated_at: Option<DateTime<Utc>>,
    pub email_updated_by: Option<Uuid>,
    pub phone: Option<String>,
    pub phone_updated_at: Option<DateTime<Utc>>,
    pub phone_updated_by: Option<Uuid>,
    pub country: Option<String>,
    pub country_updated_at: Option<DateTime<Utc>>,
    pub country_updated_by: Option<Uuid>,
    pub first_donation_date: Option<String>, // ISO date format YYYY-MM-DD
    pub first_donation_date_updated_at: Option<DateTime<Utc>>,
    pub first_donation_date_updated_by: Option<Uuid>,
    pub notes: Option<String>,
    pub notes_updated_at: Option<DateTime<Utc>>,
    pub notes_updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by_user_id: Option<Uuid>,
    pub updated_by_user_id: Option<Uuid>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub deleted_by_user_id: Option<Uuid>,
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
    pub type_: Option<String>,
    pub type_updated_at: Option<String>,
    pub type_updated_by: Option<String>,
    pub contact_person: Option<String>,
    pub contact_person_updated_at: Option<String>,
    pub contact_person_updated_by: Option<String>,
    pub email: Option<String>,
    pub email_updated_at: Option<String>,
    pub email_updated_by: Option<String>,
    pub phone: Option<String>,
    pub phone_updated_at: Option<String>,
    pub phone_updated_by: Option<String>,
    pub country: Option<String>,
    pub country_updated_at: Option<String>,
    pub country_updated_by: Option<String>,
    pub first_donation_date: Option<String>,
    pub first_donation_date_updated_at: Option<String>,
    pub first_donation_date_updated_by: Option<String>,
    pub notes: Option<String>,
    pub notes_updated_at: Option<String>,
    pub notes_updated_by: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub created_by_user_id: Option<String>,
    pub updated_by_user_id: Option<String>,
    pub deleted_at: Option<String>,
    pub deleted_by_user_id: Option<String>,
}

impl DonorRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<Donor> {
        let parse_uuid = |s: &Option<String>| -> Option<DomainResult<Uuid>> {
            s.as_ref().map(|id| {
                Uuid::parse_str(id).map_err(|_| DomainError::InvalidUuid(id.clone()))
            })
        };
        
        let parse_datetime = |s: &Option<String>| -> Option<DomainResult<DateTime<Utc>>> {
            s.as_ref().map(|dt| {
                DateTime::parse_from_rfc3339(dt)
                    .map(|dt| dt.with_timezone(&Utc))
                    .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", dt)))
            })
        };
        
        Ok(Donor {
            id: Uuid::parse_str(&self.id)
                .map_err(|_| DomainError::InvalidUuid(self.id))?,
            name: self.name,
            name_updated_at: parse_datetime(&self.name_updated_at)
                .transpose()?,
            name_updated_by: parse_uuid(&self.name_updated_by)
                .transpose()?,
            type_: self.type_,
            type_updated_at: parse_datetime(&self.type_updated_at)
                .transpose()?,
            type_updated_by: parse_uuid(&self.type_updated_by)
                .transpose()?,
            contact_person: self.contact_person,
            contact_person_updated_at: parse_datetime(&self.contact_person_updated_at)
                .transpose()?,
            contact_person_updated_by: parse_uuid(&self.contact_person_updated_by)
                .transpose()?,
            email: self.email,
            email_updated_at: parse_datetime(&self.email_updated_at)
                .transpose()?,
            email_updated_by: parse_uuid(&self.email_updated_by)
                .transpose()?,
            phone: self.phone,
            phone_updated_at: parse_datetime(&self.phone_updated_at)
                .transpose()?,
            phone_updated_by: parse_uuid(&self.phone_updated_by)
                .transpose()?,
            country: self.country,
            country_updated_at: parse_datetime(&self.country_updated_at)
                .transpose()?,
            country_updated_by: parse_uuid(&self.country_updated_by)
                .transpose()?,
            first_donation_date: self.first_donation_date,
            first_donation_date_updated_at: parse_datetime(&self.first_donation_date_updated_at)
                .transpose()?,
            first_donation_date_updated_by: parse_uuid(&self.first_donation_date_updated_by)
                .transpose()?,
            notes: self.notes,
            notes_updated_at: parse_datetime(&self.notes_updated_at)
                .transpose()?,
            notes_updated_by: parse_uuid(&self.notes_updated_by)
                .transpose()?,
            created_at: DateTime::parse_from_rfc3339(&self.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", self.created_at)))?,
            updated_at: DateTime::parse_from_rfc3339(&self.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| DomainError::Internal(format!("Invalid date format: {}", self.updated_at)))?,
            created_by_user_id: parse_uuid(&self.created_by_user_id)
                .transpose()?,
            updated_by_user_id: parse_uuid(&self.updated_by_user_id)
                .transpose()?,
            deleted_at: parse_datetime(&self.deleted_at)
                .transpose()?,
            deleted_by_user_id: parse_uuid(&self.deleted_by_user_id)
                .transpose()?,
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DonorInclude {
    FundingStats,
    All,
}