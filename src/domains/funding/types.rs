use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use std::fmt;
use crate::domains::core::document_linking::{DocumentLinkable, EntityFieldMetadata, FieldType};
use std::collections::{HashSet, HashMap};
use crate::domains::document::types::MediaDocumentResponse;
use crate::domains::donor::types::DonorSummary;

/// Funding status enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingStatus {
    Committed,
    Received,
    Pending,
    Completed,
    Cancelled,
}

impl FundingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FundingStatus::Committed => "committed",
            FundingStatus::Received => "received",
            FundingStatus::Pending => "pending",
            FundingStatus::Completed => "completed",
            FundingStatus::Cancelled => "cancelled",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "committed" => Some(FundingStatus::Committed),
            "received" => Some(FundingStatus::Received),
            "pending" => Some(FundingStatus::Pending),
            "completed" => Some(FundingStatus::Completed),
            "cancelled" => Some(FundingStatus::Cancelled),
            _ => None,
        }
    }
}

impl fmt::Display for FundingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Project Funding entity - represents funding for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFunding {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_id_updated_at: Option<DateTime<Utc>>,
    pub project_id_updated_by: Option<Uuid>,
    pub project_id_updated_by_device_id: Option<Uuid>,
    pub donor_id: Uuid,
    pub donor_id_updated_at: Option<DateTime<Utc>>,
    pub donor_id_updated_by: Option<Uuid>,
    pub donor_id_updated_by_device_id: Option<Uuid>,
    pub grant_id: Option<String>,
    pub grant_id_updated_at: Option<DateTime<Utc>>,
    pub grant_id_updated_by: Option<Uuid>,
    pub grant_id_updated_by_device_id: Option<Uuid>,
    pub amount: Option<f64>,
    pub amount_updated_at: Option<DateTime<Utc>>,
    pub amount_updated_by: Option<Uuid>,
    pub amount_updated_by_device_id: Option<Uuid>,
    pub currency: String,
    pub currency_updated_at: Option<DateTime<Utc>>,
    pub currency_updated_by: Option<Uuid>,
    pub currency_updated_by_device_id: Option<Uuid>,
    pub start_date: Option<String>, // ISO date format YYYY-MM-DD
    pub start_date_updated_at: Option<DateTime<Utc>>,
    pub start_date_updated_by: Option<Uuid>,
    pub start_date_updated_by_device_id: Option<Uuid>,
    pub end_date: Option<String>, // ISO date format YYYY-MM-DD
    pub end_date_updated_at: Option<DateTime<Utc>>,
    pub end_date_updated_by: Option<Uuid>,
    pub end_date_updated_by_device_id: Option<Uuid>,
    pub status: Option<String>,
    pub status_updated_at: Option<DateTime<Utc>>,
    pub status_updated_by: Option<Uuid>,
    pub status_updated_by_device_id: Option<Uuid>,
    pub reporting_requirements: Option<String>,
    pub reporting_requirements_updated_at: Option<DateTime<Utc>>,
    pub reporting_requirements_updated_by: Option<Uuid>,
    pub reporting_requirements_updated_by_device_id: Option<Uuid>,
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

impl ProjectFunding {
    // Helper to check if funding is deleted
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
    
    // Helper to parse status
    pub fn parsed_status(&self) -> Option<FundingStatus> {
        self.status.as_ref().and_then(|s| FundingStatus::from_str(s))
    }
    
    // Helper to parse start date
    pub fn parsed_start_date(&self) -> Option<NaiveDate> {
        self.start_date.as_ref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }
    
    // Helper to parse end date
    pub fn parsed_end_date(&self) -> Option<NaiveDate> {
        self.end_date.as_ref().and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    }
    
    // Helper to check if funding is active
    pub fn is_active(&self) -> bool {
        if self.is_deleted() {
            return false;
        }
        
        if let Some(status) = self.parsed_status() {
            match status {
                FundingStatus::Completed | FundingStatus::Cancelled => return false,
                _ => {}
            }
        }
        
        // Check if current date is within funding period
        let today = chrono::Local::now().date_naive();
        
        let start_check = self.parsed_start_date()
            .map(|start| start <= today)
            .unwrap_or(true); // If no start date, assume started
            
        let end_check = self.parsed_end_date()
            .map(|end| end >= today)
            .unwrap_or(true); // If no end date, assume not ended
            
        start_check && end_check
    }
    
    // Helper to check if funding is upcoming
    pub fn is_upcoming(&self) -> bool {
        if self.is_deleted() {
            return false;
        }
        
        if let Some(status) = self.parsed_status() {
            if matches!(status, FundingStatus::Cancelled) {
                return false;
            }
        }
        
        // Check if start date is in the future
        let today = chrono::Local::now().date_naive();
        
        self.parsed_start_date()
            .map(|start| start > today)
            .unwrap_or(false)
    }
    
    // Helper to check if funding is overdue
    pub fn is_overdue(&self) -> bool {
        if self.is_deleted() {
            return false;
        }
        
        // Check if end date is in the past but status is not completed
        let today = chrono::Local::now().date_naive();
        
        let end_passed = self.parsed_end_date()
            .map(|end| end < today)
            .unwrap_or(false);
            
        let not_completed = self.parsed_status()
            .map(|status| !matches!(status, FundingStatus::Completed | FundingStatus::Cancelled))
            .unwrap_or(true);
            
        end_passed && not_completed
    }
}

// Implement DocumentLinkable for ProjectFunding
impl DocumentLinkable for ProjectFunding {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            // Existing fields - mark relevant ones as supports_documents: true
            EntityFieldMetadata { field_name: "grant_id", display_name: "Grant ID", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "amount", display_name: "Amount", supports_documents: true, field_type: FieldType::Number, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "currency", display_name: "Currency", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "start_date", display_name: "Start Date", supports_documents: false, field_type: FieldType::Date, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "end_date", display_name: "End Date", supports_documents: false, field_type: FieldType::Date, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "status", display_name: "Status", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false }, // maps to FundingStatus enum
            EntityFieldMetadata { field_name: "reporting_requirements", display_name: "Reporting Requirements", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "notes", display_name: "Notes", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            // Add specific document reference fields here if needed in the future
            // e.g., EntityFieldMetadata { field_name: "funding_agreement_doc_id", display_name: "Funding Agreement", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
        ]
    }
}

/// NewProjectFunding DTO - used when creating a new project funding
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewProjectFunding {
    pub project_id: Uuid,
    pub donor_id: Uuid,
    pub grant_id: Option<String>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: Option<String>,
    pub reporting_requirements: Option<String>,
    pub notes: Option<String>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewProjectFunding {
    fn validate(&self) -> DomainResult<()> {
        // Validate project_id
        ValidationBuilder::new("project_id", Some(self.project_id))
            .not_nil()
            .validate()?;
            
        // Validate donor_id
        ValidationBuilder::new("donor_id", Some(self.donor_id))
            .not_nil()
            .validate()?;
            
        // Validate amount if provided
        if let Some(amount) = self.amount {
            ValidationBuilder::new("amount", Some(amount))
                .min(0.0)
                .validate()?;
        }
        
        // Validate currency
        let currency = self.currency.clone().unwrap_or_else(|| "AUD".to_string());
        ValidationBuilder::new("currency", Some(currency))
            .one_of(&["AUD", "USD", "EUR", "GBP", "NZD", "CAD", "JPY", "CHF"], Some("Invalid currency"))
            .validate()?;
            
        // Validate status if provided
        if let Some(status) = &self.status {
            ValidationBuilder::new("status", Some(status.clone()))
                .one_of(&["committed", "received", "pending", "completed", "cancelled"], 
                       Some("Invalid funding status"))
                .validate()?;
        }
        
        // Validate start_date if provided
        if let Some(date) = &self.start_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "start_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        
        // Validate end_date if provided
        if let Some(date) = &self.end_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "end_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        
        // Validate that end_date is not before start_date if both provided
        if let (Some(start), Some(end)) = (&self.start_date, &self.end_date) {
            if let (Ok(start_date), Ok(end_date)) = (
                NaiveDate::parse_from_str(start, "%Y-%m-%d"),
                NaiveDate::parse_from_str(end, "%Y-%m-%d")
            ) {
                if end_date < start_date {
                    return Err(DomainError::Validation(
                        crate::errors::ValidationError::custom(
                            "End date cannot be before start date"
                        )
                    ));
                }
            }
        }
        
        Ok(())
    }
}

/// UpdateProjectFunding DTO - used when updating an existing project funding
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateProjectFunding {
    pub project_id: Option<Uuid>,
    pub donor_id: Option<Uuid>,
    pub grant_id: Option<String>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: Option<String>,
    pub reporting_requirements: Option<String>,
    pub notes: Option<String>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateProjectFunding {
    fn validate(&self) -> DomainResult<()> {
        // Validate amount if provided
        if let Some(amount) = self.amount {
            ValidationBuilder::new("amount", Some(amount))
                .min(0.0)
                .validate()?;
        }
        
        // Validate currency if provided
        if let Some(currency) = &self.currency {
            ValidationBuilder::new("currency", Some(currency.clone()))
                .one_of(&["AUD", "USD", "EUR", "GBP", "NZD", "CAD", "JPY", "CHF"], Some("Invalid currency"))
                .validate()?;
        }
        
        // Validate status if provided
        if let Some(status) = &self.status {
            ValidationBuilder::new("status", Some(status.clone()))
                .one_of(&["committed", "received", "pending", "completed", "cancelled"], 
                       Some("Invalid funding status"))
                .validate()?;
        }
        
        // Validate start_date if provided
        if let Some(date) = &self.start_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "start_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        
        // Validate end_date if provided
        if let Some(date) = &self.end_date {
            if NaiveDate::parse_from_str(date, "%Y-%m-%d").is_err() {
                return Err(DomainError::Validation(
                    crate::errors::ValidationError::format(
                        "end_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ));
            }
        }
        
        Ok(())
    }
}

/// ProjectFundingRow - SQLite row representation for mapping from database
#[derive(Debug, Clone, FromRow)]
pub struct ProjectFundingRow {
    pub id: String,
    pub project_id: String,
    pub project_id_updated_at: Option<String>,
    pub project_id_updated_by: Option<String>,
    pub project_id_updated_by_device_id: Option<String>,
    pub donor_id: String,
    pub donor_id_updated_at: Option<String>,
    pub donor_id_updated_by: Option<String>,
    pub donor_id_updated_by_device_id: Option<String>,
    pub grant_id: Option<String>,
    pub grant_id_updated_at: Option<String>,
    pub grant_id_updated_by: Option<String>,
    pub grant_id_updated_by_device_id: Option<String>,
    pub amount: Option<f64>,
    pub amount_updated_at: Option<String>,
    pub amount_updated_by: Option<String>,
    pub amount_updated_by_device_id: Option<String>,
    pub currency: String,
    pub currency_updated_at: Option<String>,
    pub currency_updated_by: Option<String>,
    pub currency_updated_by_device_id: Option<String>,
    pub start_date: Option<String>,
    pub start_date_updated_at: Option<String>,
    pub start_date_updated_by: Option<String>,
    pub start_date_updated_by_device_id: Option<String>,
    pub end_date: Option<String>,
    pub end_date_updated_at: Option<String>,
    pub end_date_updated_by: Option<String>,
    pub end_date_updated_by_device_id: Option<String>,
    pub status: Option<String>,
    pub status_updated_at: Option<String>,
    pub status_updated_by: Option<String>,
    pub status_updated_by_device_id: Option<String>,
    pub reporting_requirements: Option<String>,
    pub reporting_requirements_updated_at: Option<String>,
    pub reporting_requirements_updated_by: Option<String>,
    pub reporting_requirements_updated_by_device_id: Option<String>,
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

impl ProjectFundingRow {
    /// Convert database row to domain entity
    pub fn into_entity(self) -> DomainResult<ProjectFunding> {
        let parse_uuid = |s: &str, field_name: &str| Uuid::parse_str(s).map_err(|_| DomainError::Validation(ValidationError::format(field_name, &format!("Invalid UUID format: {}", s))));
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

        Ok(ProjectFunding {
            id: parse_uuid(&self.id, "id")?,
            project_id: parse_uuid(&self.project_id, "project_id")?,
            project_id_updated_at: parse_optional_datetime(&self.project_id_updated_at, "project_id_updated_at")?,
            project_id_updated_by: parse_optional_uuid(&self.project_id_updated_by, "project_id_updated_by")?,
            project_id_updated_by_device_id: parse_optional_uuid(&self.project_id_updated_by_device_id, "project_id_updated_by_device_id")?,
            donor_id: parse_uuid(&self.donor_id, "donor_id")?,
            donor_id_updated_at: parse_optional_datetime(&self.donor_id_updated_at, "donor_id_updated_at")?,
            donor_id_updated_by: parse_optional_uuid(&self.donor_id_updated_by, "donor_id_updated_by")?,
            donor_id_updated_by_device_id: parse_optional_uuid(&self.donor_id_updated_by_device_id, "donor_id_updated_by_device_id")?,
            grant_id: self.grant_id,
            grant_id_updated_at: parse_optional_datetime(&self.grant_id_updated_at, "grant_id_updated_at")?,
            grant_id_updated_by: parse_optional_uuid(&self.grant_id_updated_by, "grant_id_updated_by")?,
            grant_id_updated_by_device_id: parse_optional_uuid(&self.grant_id_updated_by_device_id, "grant_id_updated_by_device_id")?,
            amount: self.amount,
            amount_updated_at: parse_optional_datetime(&self.amount_updated_at, "amount_updated_at")?,
            amount_updated_by: parse_optional_uuid(&self.amount_updated_by, "amount_updated_by")?,
            amount_updated_by_device_id: parse_optional_uuid(&self.amount_updated_by_device_id, "amount_updated_by_device_id")?,
            currency: self.currency,
            currency_updated_at: parse_optional_datetime(&self.currency_updated_at, "currency_updated_at")?,
            currency_updated_by: parse_optional_uuid(&self.currency_updated_by, "currency_updated_by")?,
            currency_updated_by_device_id: parse_optional_uuid(&self.currency_updated_by_device_id, "currency_updated_by_device_id")?,
            start_date: self.start_date,
            start_date_updated_at: parse_optional_datetime(&self.start_date_updated_at, "start_date_updated_at")?,
            start_date_updated_by: parse_optional_uuid(&self.start_date_updated_by, "start_date_updated_by")?,
            start_date_updated_by_device_id: parse_optional_uuid(&self.start_date_updated_by_device_id, "start_date_updated_by_device_id")?,
            end_date: self.end_date,
            end_date_updated_at: parse_optional_datetime(&self.end_date_updated_at, "end_date_updated_at")?,
            end_date_updated_by: parse_optional_uuid(&self.end_date_updated_by, "end_date_updated_by")?,
            end_date_updated_by_device_id: parse_optional_uuid(&self.end_date_updated_by_device_id, "end_date_updated_by_device_id")?,
            status: self.status,
            status_updated_at: parse_optional_datetime(&self.status_updated_at, "status_updated_at")?,
            status_updated_by: parse_optional_uuid(&self.status_updated_by, "status_updated_by")?,
            status_updated_by_device_id: parse_optional_uuid(&self.status_updated_by_device_id, "status_updated_by_device_id")?,
            reporting_requirements: self.reporting_requirements,
            reporting_requirements_updated_at: parse_optional_datetime(&self.reporting_requirements_updated_at, "reporting_requirements_updated_at")?,
            reporting_requirements_updated_by: parse_optional_uuid(&self.reporting_requirements_updated_by, "reporting_requirements_updated_by")?,
            reporting_requirements_updated_by_device_id: parse_optional_uuid(&self.reporting_requirements_updated_by_device_id, "reporting_requirements_updated_by_device_id")?,
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

/// ProjectSummary for funding responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub id: Uuid,
    pub name: String,
}

/// ProjectFundingResponse DTO - used for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFundingResponse {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project: Option<ProjectSummary>,
    pub donor_id: Uuid,
    pub donor: Option<DonorSummary>,
    pub grant_id: Option<String>,
    pub amount: Option<f64>,
    pub currency: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub status: Option<String>,
    pub status_display: Option<String>,
    pub reporting_requirements: Option<String>,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub is_active: bool,
    pub is_upcoming: bool,
    pub is_overdue: bool,
}

impl From<ProjectFunding> for ProjectFundingResponse {
    fn from(funding: ProjectFunding) -> Self {
        // Clone fields that don't implement Copy to avoid partial move
        let status_display_val = funding.parsed_status().map(|s| s.to_string());
        let is_active_val = funding.is_active();
        let is_upcoming_val = funding.is_upcoming();
        let is_overdue_val = funding.is_overdue();

        Self {
            id: funding.id,
            project_id: funding.project_id,
            project: None,
            donor_id: funding.donor_id,
            donor: None,
            grant_id: funding.grant_id.clone(), // Clone Option<String>
            amount: funding.amount, // f64 is Copy
            currency: funding.currency.clone(), // Clone String
            start_date: funding.start_date.clone(), // Clone Option<String>
            end_date: funding.end_date.clone(), // Clone Option<String>
            status: funding.status.clone(), // Clone Option<String>
            status_display: status_display_val, // Use calculated value
            reporting_requirements: funding.reporting_requirements.clone(), // Clone Option<String>
            notes: funding.notes.clone(), // Clone Option<String>
            created_at: funding.created_at.to_rfc3339(),
            updated_at: funding.updated_at.to_rfc3339(),
            is_active: is_active_val, // Use calculated value
            is_upcoming: is_upcoming_val, // Use calculated value
            is_overdue: is_overdue_val, // Use calculated value
        }
    }
}

impl ProjectFundingResponse {
    /// Add project details
    pub fn with_project(mut self, project: ProjectSummary) -> Self {
        self.project = Some(project);
        self
    }
    
    /// Add donor details
    pub fn with_donor(mut self, donor: DonorSummary) -> Self {
        self.donor = Some(donor);
        self
    }
}

/// Enum for specifying included relations when fetching funding
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingInclude {
    Project,
    Donor,
    Documents,
    DocumentCounts,
    All,
}

/// Funding stats summary for dashboards and reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingStatsSummary {
    pub total_fundings: i64,
    pub active_fundings: i64,
    pub completed_fundings: i64,
    pub upcoming_fundings: i64,
    pub overdue_fundings: i64,
    pub total_funding_amount: f64,
    pub active_funding_amount: f64,
    pub average_funding_amount: f64,
    pub funding_by_currency: HashMap<String, f64>,
    pub funding_by_status: HashMap<String, i64>,
}

/// Project funding summary for use in donor responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectFundingSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub amount: Option<f64>,
    pub currency: String,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub is_active: bool,
}

/// Funding summary metrics for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorFundingMetrics {
    pub donor_id: Uuid,
    pub donor_name: String,
    pub total_funded_amount: f64,
    pub active_funded_amount: f64,
    pub project_count: i64,
    pub average_grant_size: f64,
    pub funding_by_currency: HashMap<String, f64>,
    pub funding_by_status: HashMap<String, i64>,
}

/// Detailed funding information for a donor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorWithFundingDetails {
    pub donor: DonorSummary,
    pub metrics: DonorFundingMetrics,
    pub recent_fundings: Vec<ProjectFundingSummary>,
}

/// Funding with document timeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingWithDocumentTimeline {
    pub funding: ProjectFundingResponse, // Assume ProjectFundingResponse exists
    pub documents_by_month: HashMap<String, Vec<MediaDocumentResponse>>,
    pub total_document_count: u64,
}