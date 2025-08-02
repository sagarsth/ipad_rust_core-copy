use crate::errors::{DomainError, DomainResult, ValidationError};
use crate::validation::{Validate, ValidationBuilder};
use uuid::Uuid;
use chrono::{DateTime, Utc, NaiveDate};
use serde::{Serialize, Deserialize};
use sqlx::FromRow;
use std::fmt;
use std::str::FromStr;
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

    pub fn all_variants() -> Vec<&'static str> {
        vec!["individual", "foundation", "government", "corporate", "other"]
    }
}

impl fmt::Display for DonorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Funding status enum for filtering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FundingStatus {
    Active,
    Completed,
    Pending,
    Cancelled,
}

impl FundingStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FundingStatus::Active => "active",
            FundingStatus::Completed => "completed",
            FundingStatus::Pending => "pending",
            FundingStatus::Cancelled => "cancelled",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "active" => Some(FundingStatus::Active),
            "completed" => Some(FundingStatus::Completed),
            "pending" => Some(FundingStatus::Pending),
            "cancelled" => Some(FundingStatus::Cancelled),
            _ => None,
        }
    }

    pub fn all_variants() -> Vec<&'static str> {
        vec!["active", "completed", "pending", "cancelled"]
    }
}

impl fmt::Display for FundingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Advanced donor filtering capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DonorFilter {
    /// Filter by donor types
    pub types: Option<Vec<DonorType>>,
    /// Filter by countries
    pub countries: Option<Vec<String>>,
    /// Filter by funding status
    pub funding_status: Option<Vec<FundingStatus>>,
    /// Search text across name, contact person, email
    pub search_text: Option<String>,
    /// Date range for first donation or created_at
    pub date_range: Option<(String, String)>,
    /// Filter by minimum funding amount
    pub min_funding_amount: Option<f64>,
    /// Filter by maximum funding amount  
    pub max_funding_amount: Option<f64>,
    /// Filter by users who created donors
    pub created_by_user_ids: Option<Vec<Uuid>>,
    /// Filter by document existence
    pub has_documents: Option<bool>,
    /// Filter by specific document-linked fields
    pub document_linked_fields: Option<Vec<String>>,
    /// Filter by agreement status
    pub has_agreements: Option<bool>,
    /// Filter by due diligence completion
    pub due_diligence_complete: Option<bool>,
    /// Exclude deleted donors (default: true)
    #[serde(default = "default_true")]
    pub exclude_deleted: bool,
    /// Filter by sync priority
    pub sync_priorities: Option<Vec<SyncPriority>>,
    /// Filter by tags
    pub tags: Option<Vec<String>>,
    /// Filter by donors funding specific projects
    pub project_ids: Option<Vec<Uuid>>,
}

fn default_true() -> bool {
    true
}

// Helper function to get SyncPriority variants as strings
fn sync_priority_variants() -> Vec<&'static str> {
    vec!["high", "normal", "low", "never"]
}

impl DonorFilter {
    /// Create a new empty filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add types filter.
    pub fn with_types(mut self, types: Vec<DonorType>) -> Self {
        self.types = Some(types);
        self
    }

    /// Add countries filter.
    pub fn with_countries(mut self, countries: Vec<String>) -> Self {
        self.countries = Some(countries);
        self
    }

    /// Add funding status filter.
    pub fn with_funding_status(mut self, statuses: Vec<FundingStatus>) -> Self {
        self.funding_status = Some(statuses);
        self
    }

    /// Add search text filter.
    pub fn with_search_text(mut self, text: String) -> Self {
        self.search_text = Some(text);
        self
    }

    /// Add date range filter.
    pub fn with_date_range(mut self, start_date: String, end_date: String) -> Self {
        self.date_range = Some((start_date, end_date));
        self
    }

    /// Add minimum funding amount filter.
    pub fn with_min_funding(mut self, min: f64) -> Self {
        self.min_funding_amount = Some(min);
        self
    }

    /// Add maximum funding amount filter.
    pub fn with_max_funding(mut self, max: f64) -> Self {
        self.max_funding_amount = Some(max);
        self
    }

    /// Add created by user filter.
    pub fn with_created_by_users(mut self, user_ids: Vec<Uuid>) -> Self {
        self.created_by_user_ids = Some(user_ids);
        self
    }

    /// Add has documents filter.
    pub fn with_has_documents(mut self, has_documents: bool) -> Self {
        self.has_documents = Some(has_documents);
        self
    }
    
    /// Add has agreements filter.
    pub fn with_has_agreements(mut self, has_agreements: bool) -> Self {
        self.has_agreements = Some(has_agreements);
        self
    }

    /// Add due diligence complete filter.
    pub fn with_due_diligence_complete(mut self, complete: bool) -> Self {
        self.due_diligence_complete = Some(complete);
        self
    }

    /// Add sync priorities filter.
    pub fn with_sync_priorities(mut self, priorities: Vec<SyncPriority>) -> Self {
        self.sync_priorities = Some(priorities);
        self
    }

    /// Add tags filter.
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Add project IDs filter.
    pub fn with_project_ids(mut self, project_ids: Vec<Uuid>) -> Self {
        self.project_ids = Some(project_ids);
        self
    }

    /// Include soft-deleted records.
    pub fn include_deleted(mut self) -> Self {
        self.exclude_deleted = false;
        self
    }

    /// Check if filter is empty (no filtering criteria).
    pub fn is_empty(&self) -> bool {
        self.types.is_none()
            && self.countries.is_none()
            && self.funding_status.is_none()
            && self.search_text.is_none()
            && self.date_range.is_none()
            && self.min_funding_amount.is_none()
            && self.max_funding_amount.is_none()
            && self.created_by_user_ids.is_none()
            && self.has_documents.is_none()
            && self.document_linked_fields.is_none()
            && self.has_agreements.is_none()
            && self.due_diligence_complete.is_none()
            && self.sync_priorities.is_none()
            && self.tags.is_none()
            && self.project_ids.is_none()
    }
}

impl Validate for DonorFilter {
    fn validate(&self) -> DomainResult<()> {
        // Validate date range if provided
        if let Some((start_date, end_date)) = &self.date_range {
            let start = NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
                .map_err(|_| DomainError::Validation(
                    ValidationError::format("date_range.start", "Invalid date format. Expected YYYY-MM-DD")
                ))?;
            let end = NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
                .map_err(|_| DomainError::Validation(
                    ValidationError::format("date_range.end", "Invalid date format. Expected YYYY-MM-DD")
                ))?;
            
            if start > end {
                return Err(DomainError::Validation(
                    ValidationError::custom("Start date cannot be after end date")
                ));
            }
        }

        // Validate funding amount range
        if let (Some(min), Some(max)) = (self.min_funding_amount, self.max_funding_amount) {
            if min < 0.0 {
                return Err(DomainError::Validation(
                    ValidationError::format("min_funding_amount", "Minimum funding amount cannot be negative")
                ));
            }
            if max < 0.0 {
                return Err(DomainError::Validation(
                    ValidationError::format("max_funding_amount", "Maximum funding amount cannot be negative")
                ));
            }
            if min > max {
                return Err(DomainError::Validation(
                    ValidationError::custom("Minimum funding amount cannot be greater than maximum")
                ));
            }
        }

        if let Some(fields) = &self.document_linked_fields {
            let valid_fields: HashSet<String> = Donor::field_metadata()
                .into_iter()
                .filter(|f| f.supports_documents)
                .map(|f| f.field_name.to_string())
                .collect();
                
            for field in fields {
                if !valid_fields.contains(field) {
                    return Err(DomainError::Validation(ValidationError::format(
                        "document_linked_fields",
                        &format!("Invalid field '{}'. Valid fields: {:?}", field, valid_fields)
                    )));
                }
            }
        }

        Ok(())
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
    /// Sync priority for external synchronization
    pub sync_priority: Option<String>,
    pub sync_priority_updated_at: Option<DateTime<Utc>>,
    pub sync_priority_updated_by: Option<Uuid>,
    pub sync_priority_updated_by_device_id: Option<Uuid>,
    /// Last successful sync timestamp
    pub last_sync_at: Option<DateTime<Utc>>,
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

    // Helper to parse sync priority
    pub fn parsed_sync_priority(&self) -> Option<SyncPriority> {
        self.sync_priority.as_ref().and_then(|s| SyncPriority::from_str(s).ok())
    }

    /// Calculate data completeness percentage
    pub fn data_completeness(&self) -> f64 {
        let total_fields = 8.0; // Core fields: name, type, contact_person, email, phone, country, first_donation_date, notes
        let mut completed_fields = 1.0; // name is always required

        if self.type_.is_some() { completed_fields += 1.0; }
        if self.contact_person.is_some() { completed_fields += 1.0; }
        if self.email.is_some() { completed_fields += 1.0; }
        if self.phone.is_some() { completed_fields += 1.0; }
        if self.country.is_some() { completed_fields += 1.0; }
        if self.first_donation_date.is_some() { completed_fields += 1.0; }
        if self.notes.is_some() { completed_fields += 1.0; }

        (completed_fields / total_fields) * 100.0
    }

    /// Check for suspicious patterns in data
    pub fn has_suspicious_patterns(&self) -> Vec<String> {
        let mut issues = Vec::new();

        // Check for numbers in name (usually suspicious)
        if self.name.chars().any(|c| c.is_ascii_digit()) {
            issues.push("Name contains numbers".to_string());
        }

        // Check for email without proper domain
        if let Some(email) = &self.email {
            if !email.contains('.') || email.split('@').count() != 2 {
                issues.push("Email format appears invalid".to_string());
            }
        }

        // Check for placeholder/test data
        let placeholder_patterns = ["test", "example", "temp", "placeholder", "dummy"];
        for pattern in placeholder_patterns {
            if self.name.to_lowercase().contains(pattern) {
                issues.push(format!("Name contains placeholder pattern: {}", pattern));
            }
        }

        issues
    }
}

impl DocumentLinkable for Donor {
    fn field_metadata() -> Vec<EntityFieldMetadata> {
        vec![
            EntityFieldMetadata { field_name: "name", display_name: "Donor Name", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "type_", display_name: "Type", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "contact_person", display_name: "Contact Person", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "email", display_name: "Email", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "phone", display_name: "Phone", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "country", display_name: "Country", supports_documents: false, field_type: FieldType::Text, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "first_donation_date", display_name: "First Donation Date", supports_documents: false, field_type: FieldType::Date, is_document_reference_only: false },
            EntityFieldMetadata { field_name: "notes", display_name: "Notes", supports_documents: true, field_type: FieldType::Text, is_document_reference_only: false },
            // Document Reference Fields
            EntityFieldMetadata { field_name: "donor_agreement", display_name: "Donor Agreement", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "due_diligence", display_name: "Due Diligence Docs", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "communication_log", display_name: "Communication Log", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "tax_information", display_name: "Tax Information", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "annual_report", display_name: "Annual Report (from Donor)", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "financial_statements", display_name: "Financial Statements", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
            EntityFieldMetadata { field_name: "legal_documents", display_name: "Legal Documents", supports_documents: true, field_type: FieldType::DocumentRef, is_document_reference_only: true },
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
    pub sync_priority: Option<String>,
    pub created_by_user_id: Option<Uuid>,
}

impl Validate for NewDonor {
    fn validate(&self) -> DomainResult<()> {
        // Validate name with enhanced rules
        ValidationBuilder::new("name", Some(self.name.clone()))
            .required()
            .min_length(2)
            .max_length(100)
            .validate()?;

        // Check for suspicious patterns in name
        if self.name.chars().any(|c| c.is_ascii_digit()) {
            return Err(DomainError::Validation(
                ValidationError::custom("Donor name should not contain numbers")
            ));
        }

        // Validate donor type if provided with better error message
        if let Some(type_) = &self.type_ {
            if DonorType::from_str(type_).is_none() {
                return Err(DomainError::Validation(
                    ValidationError::format(
                        "type", 
                        &format!("Invalid donor type '{}'. Valid options: {}", 
                            type_, 
                            DonorType::all_variants().join(", ")
                        )
                    )
                ));
            }
        }
        
        // Validate email if provided
        if let Some(email) = &self.email {
            ValidationBuilder::new("email", Some(email.clone()))
                .email()
                .validate()?;
        }
        
        // Validate first_donation_date if provided with business rules
        if let Some(date) = &self.first_donation_date {
            let parsed_date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| DomainError::Validation(
                    ValidationError::format(
                        "first_donation_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ))?;

            // Business rule: First donation date cannot be in the future
            if parsed_date > Utc::now().date_naive() {
                return Err(DomainError::Validation(
                    ValidationError::custom("First donation date cannot be in the future")
                ));
            }
        }

        // Validate sync priority if provided
        if let Some(priority) = &self.sync_priority {
            if SyncPriority::from_str(priority).is_err() {
                return Err(DomainError::Validation(
                    ValidationError::format(
                        "sync_priority", 
                        &format!("Invalid sync priority '{}'. Valid options: {}", 
                            priority, 
                            sync_priority_variants().join(", ")
                        )
                    )
                ));
            }
        }

        // Business rule validations based on donor type
        if let Some(type_) = &self.type_ {
            match DonorType::from_str(type_) {
                Some(DonorType::Corporate) => {
                    if self.contact_person.is_none() {
                        return Err(DomainError::Validation(
                            ValidationError::custom("Corporate donors must have a contact person")
                        ));
                    }
                }
                Some(DonorType::Government) => {
                    if self.country.is_none() {
                        return Err(DomainError::Validation(
                            ValidationError::custom("Government donors must have a country specified")
                        ));
                    }
                }
                Some(DonorType::Foundation) => {
                    if self.email.is_none() {
                        return Err(DomainError::Validation(
                            ValidationError::custom("Foundation donors should have an email address")
                        ));
                    }
                }
                _ => {}
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
    pub sync_priority: Option<String>,
    pub updated_by_user_id: Uuid,
}

impl Validate for UpdateDonor {
    fn validate(&self) -> DomainResult<()> {
        // Validate name if provided with enhanced rules
        if let Some(name) = &self.name {
            ValidationBuilder::new("name", Some(name.clone()))
                .min_length(2)
                .max_length(100)
                .validate()?;

            // Check for suspicious patterns in name
            if name.chars().any(|c| c.is_ascii_digit()) {
                return Err(DomainError::Validation(
                    ValidationError::custom("Donor name should not contain numbers")
                ));
            }
        }
        
        // Validate donor type if provided with better error message
        if let Some(type_) = &self.type_ {
            if DonorType::from_str(type_).is_none() {
                return Err(DomainError::Validation(
                    ValidationError::format(
                        "type", 
                        &format!("Invalid donor type '{}'. Valid options: {}", 
                            type_, 
                            DonorType::all_variants().join(", ")
                        )
                    )
                ));
            }
        }
        
        // Validate email if provided
        if let Some(email) = &self.email {
            ValidationBuilder::new("email", Some(email.clone()))
                .email()
                .validate()?;
        }
        
        // Validate first_donation_date if provided with business rules
        if let Some(date) = &self.first_donation_date {
            let parsed_date = NaiveDate::parse_from_str(date, "%Y-%m-%d")
                .map_err(|_| DomainError::Validation(
                    ValidationError::format(
                        "first_donation_date", 
                        "Invalid date format. Expected YYYY-MM-DD"
                    )
                ))?;

            // Business rule: First donation date cannot be in the future
            if parsed_date > Utc::now().date_naive() {
                return Err(DomainError::Validation(
                    ValidationError::custom("First donation date cannot be in the future")
                ));
            }
        }

        // Validate sync priority if provided
        if let Some(priority) = &self.sync_priority {
            if SyncPriority::from_str(priority).is_err() {
                return Err(DomainError::Validation(
                    ValidationError::format(
                        "sync_priority", 
                        &format!("Invalid sync priority '{}'. Valid options: {}", 
                            priority, 
                            sync_priority_variants().join(", ")
                        )
                    )
                ));
            }
        }

        Ok(())
    }
}

/// Bulk operation result for donor operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorBulkOperationResult {
    pub total_requested: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duplicates_found: usize,
    pub error_details: Vec<DonorOperationError>,
    pub success_ids: Vec<Uuid>,
    pub operation_duration_ms: u64,
    pub validation_errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorOperationError {
    pub donor_id: Option<Uuid>,
    pub donor_name: Option<String>,
    pub error_message: String,
    pub error_type: String, // "validation", "duplicate", "database", etc.
    pub field_errors: HashMap<String, String>,
}

/// Duplicate detection for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorDuplicateInfo {
    pub id: Uuid,
    pub name: String,
    pub type_: Option<DonorType>,
    pub contact_person: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub country: Option<String>,
    pub similarity_score: f64, // 0.0 to 1.0
    pub matching_fields: Vec<String>,
    pub confidence_level: DuplicateConfidence,
    pub document_similarity: Option<f64>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DuplicateConfidence {
    High,    // 90%+ similarity
    Medium,  // 70-89% similarity  
    Low,     // 50-69% similarity
}

/// Activity tracking for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorActivityTimeline {
    pub donor_id: Uuid,
    pub funding_activities: Vec<FundingActivity>,
    pub communication_activities: Vec<CommunicationActivity>,
    pub document_activities: Vec<DocumentActivity>,
    pub agreement_activities: Vec<AgreementActivity>,
    pub profile_changes: Vec<ProfileChangeActivity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingActivity {
    pub id: Uuid,
    pub project_id: Uuid,
    pub project_name: String,
    pub amount: f64,
    pub currency: String,
    pub date: NaiveDate,
    pub status: FundingStatus,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommunicationActivity {
    pub id: Uuid,
    pub communication_type: String, // email, phone, meeting, etc.
    pub subject: Option<String>,
    pub summary: Option<String>,
    pub date: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub document_references: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentActivity {
    pub document_id: Uuid,
    pub document_type: String,
    pub document_name: String,
    pub activity_type: String, // uploaded, updated, deleted, viewed
    pub date: DateTime<Utc>,
    pub user_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgreementActivity {
    pub agreement_id: Uuid,
    pub agreement_type: String,
    pub status: String, // draft, pending, signed, expired
    pub date: DateTime<Utc>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileChangeActivity {
    pub field_name: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub changed_by: Option<Uuid>,
    pub date: DateTime<Utc>,
}

/// Enhanced donor engagement metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorEngagementMetrics {
    pub donor_id: Uuid,
    pub engagement_score: f64, // 0.0 to 100.0
    pub funding_retention_rate: f64,
    pub avg_donation_frequency_months: f64,
    pub last_donation_date: Option<NaiveDate>,
    pub last_communication_date: Option<DateTime<Utc>>,
    pub communication_frequency_score: f64,
    pub project_success_correlation: f64,
    pub responsiveness_score: f64, // Based on response times
    pub relationship_strength: RelationshipStrength,
    pub risk_indicators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RelationshipStrength {
    Strong,    // High engagement, regular funding
    Moderate,  // Occasional engagement
    Weak,      // Infrequent contact, irregular funding
    AtRisk,    // Long periods without contact or funding
}

/// Trend analysis for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorTrendAnalysis {
    pub donor_id: Uuid,
    pub donation_patterns: HashMap<String, f64>, // Monthly/yearly patterns
    pub funding_amount_trend: Vec<TrendDataPoint>,
    pub communication_trend: Vec<TrendDataPoint>,
    pub engagement_trend: Vec<TrendDataPoint>,
    pub seasonal_patterns: HashMap<String, f64>,
    pub prediction_confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendDataPoint {
    pub period: String, // "2024-01" for monthly, "2024-Q1" for quarterly
    pub value: f64,
    pub date: NaiveDate,
}

/// Enhanced document handling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorDocumentTimeline {
    pub donor_id: Uuid,
    pub agreements_by_year: HashMap<i32, Vec<MediaDocumentResponse>>,
    pub communications_by_quarter: HashMap<String, Vec<MediaDocumentResponse>>,
    pub due_diligence_docs: Vec<MediaDocumentResponse>,
    pub tax_documents: Vec<MediaDocumentResponse>,
    pub legal_documents: Vec<MediaDocumentResponse>,
    pub total_document_count: usize,
    pub document_completeness_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorDocumentSummary {
    pub agreement_count: usize,
    pub due_diligence_count: usize,
    pub tax_document_count: usize,
    pub communication_count: usize,
    pub legal_document_count: usize,
    pub financial_statement_count: usize,
    pub total_size_mb: f64,
    pub last_document_upload: Option<DateTime<Utc>>,
    pub document_counts_by_type: HashMap<String, usize>,
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
    pub sync_priority: Option<String>,
    pub sync_priority_updated_at: Option<String>,
    pub sync_priority_updated_by: Option<String>,
    pub sync_priority_updated_by_device_id: Option<String>,
    pub last_sync_at: Option<String>,
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
            sync_priority: self.sync_priority,
            sync_priority_updated_at: parse_optional_datetime(&self.sync_priority_updated_at, "sync_priority_updated_at")?,
            sync_priority_updated_by: parse_optional_uuid(&self.sync_priority_updated_by, "sync_priority_updated_by")?,
            sync_priority_updated_by_device_id: parse_optional_uuid(&self.sync_priority_updated_by_device_id, "sync_priority_updated_by_device_id")?,
            last_sync_at: parse_optional_datetime(&self.last_sync_at, "last_sync_at")?,
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
    pub data_completeness: Option<f64>,
    pub engagement_score: Option<f64>,
}

impl From<Donor> for DonorSummary {
    fn from(donor: Donor) -> Self {
        let data_completeness = donor.data_completeness();
        Self {
            id: donor.id,
            name: donor.name,
            type_: donor.type_,
            country: donor.country,
            data_completeness: Some(data_completeness),
            engagement_score: None, // To be calculated separately
        }
    }
}

/// Enhanced DonorResponse DTO - used for API responses
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
    pub sync_priority: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_sync_at: Option<String>,
    // Enhanced statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_fundings_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_funding_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_fundings_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_funding_amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_funding_date: Option<String>,
    // Document information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_counts_by_type: Option<HashMap<String, usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_agreement: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_due_diligence: Option<bool>,
    // Engagement metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_completeness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engagement_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationship_strength: Option<RelationshipStrength>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_indicators: Option<Vec<String>>,
    // Activity summaries
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_communication_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub communication_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_activity_count: Option<usize>,
}

impl From<Donor> for DonorResponse {
    fn from(donor: Donor) -> Self {
        let data_completeness = donor.data_completeness();
        let risk_indicators = donor.has_suspicious_patterns();
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
            sync_priority: donor.sync_priority,
            created_at: donor.created_at.to_rfc3339(),
            updated_at: donor.updated_at.to_rfc3339(),
            last_sync_at: donor.last_sync_at.map(|dt| dt.to_rfc3339()),
            data_completeness: Some(data_completeness),
            risk_indicators: Some(risk_indicators),
            // Optional fields to be populated by service layer
            active_fundings_count: None,
            total_funding_amount: None,
            total_fundings_count: None,
            avg_funding_amount: None,
            last_funding_date: None,
            document_count: None,
            document_counts_by_type: None,
            has_agreement: None,
            has_due_diligence: None,
            engagement_score: None,
            relationship_strength: None,
            last_communication_date: None,
            communication_count: None,
            recent_activity_count: None,
        }
    }
}

impl DonorResponse {
    /// Add funding statistics
    pub fn with_funding_stats(mut self, active_count: i64, total_amount: f64, total_count: i64, avg_amount: f64, last_date: Option<String>) -> Self {
        self.active_fundings_count = Some(active_count);
        self.total_funding_amount = Some(total_amount);
        self.total_fundings_count = Some(total_count);
        self.avg_funding_amount = Some(avg_amount);
        self.last_funding_date = last_date;
        self
    }

    /// Add document statistics
    pub fn with_document_stats(mut self, total_count: usize, counts_by_type: HashMap<String, usize>, has_agreement: bool, has_due_diligence: bool) -> Self {
        self.document_count = Some(total_count);
        self.document_counts_by_type = Some(counts_by_type);
        self.has_agreement = Some(has_agreement);
        self.has_due_diligence = Some(has_due_diligence);
        self
    }

    /// Add engagement metrics
    pub fn with_engagement_metrics(mut self, score: f64, strength: RelationshipStrength) -> Self {
        self.engagement_score = Some(score);
        self.relationship_strength = Some(strength);
        self
    }

    /// Add activity information
    pub fn with_activity_stats(mut self, last_comm_date: Option<String>, comm_count: usize, recent_activity: usize) -> Self {
        self.last_communication_date = last_comm_date;
        self.communication_count = Some(comm_count);
        self.recent_activity_count = Some(recent_activity);
        self
    }
}

/// Enum for specifying included relations when fetching donors
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DonorInclude {
    FundingStats,
    Documents,
    FundingDetails,
    DocumentTimeline,
    ActivityTimeline,
    EngagementMetrics,
    TrendAnalysis,
    All,
}

/// Role a user can have in relation to a donor
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UserDonorRole {
    Created,    // User created the donor
    Updated,    // User last updated the donor
    Communicated, // User had recent communication
    Assigned,   // User is assigned to manage this donor
}

/// Enhanced summary of aggregate statistics for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorStatsSummary {
    pub total_donors: i64,
    pub active_donors: i64,
    pub inactive_donors: i64,
    pub at_risk_donors: i64,
    pub total_donation_amount: Option<f64>,
    pub avg_donation_amount: Option<f64>,
    pub median_donation_amount: Option<f64>,
    pub donor_count_by_type: HashMap<String, i64>,
    pub donor_count_by_country: HashMap<String, i64>,
    pub funding_trend: Vec<TrendDataPoint>,
    pub engagement_distribution: HashMap<String, i64>, // by RelationshipStrength
    pub data_completeness_avg: f64,
    pub document_compliance_rate: f64,
}

/// Enhanced funding statistics for a donor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorFundingStats {
    pub active_fundings_count: i64,
    pub total_fundings_count: i64,
    pub completed_fundings_count: i64,
    pub pending_fundings_count: i64,
    pub total_funding_amount: f64,
    pub active_funding_amount: f64,
    pub avg_funding_amount: f64,
    pub median_funding_amount: f64,
    pub largest_funding_amount: f64,
    pub smallest_funding_amount: f64,
    pub currency_distribution: HashMap<String, f64>,
    pub funding_frequency: f64, // fundings per year
    pub retention_rate: f64,
    pub funding_trend: Vec<TrendDataPoint>,
    pub project_success_rate: f64,
}

/// Donor with detailed funding and activity information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorWithFundingDetails {
    pub donor: DonorResponse,
    pub funding_stats: DonorFundingStats,
    pub recent_fundings: Vec<ProjectFundingSummary>,
    pub activity_timeline: DonorActivityTimeline,
    pub engagement_metrics: DonorEngagementMetrics,
}

/// Donor with comprehensive document information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorWithDocumentTimeline {
    pub donor: DonorResponse,
    pub document_timeline: DonorDocumentTimeline,
    pub document_summary: DonorDocumentSummary,
    pub compliance_status: DocumentComplianceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentComplianceStatus {
    pub has_required_documents: bool,
    pub missing_documents: Vec<String>,
    pub expired_documents: Vec<String>,
    pub compliance_score: f64,
}

/// Enhanced dashboard statistics for donors
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorDashboardStats {
    pub total_donors: i64,
    pub donors_by_type: HashMap<String, i64>,
    pub donors_by_country: HashMap<String, i64>,
    pub donors_by_engagement: HashMap<String, i64>,
    pub recent_donors_count: i64,
    pub at_risk_donors_count: i64,
    pub funding_summary: FundingSummaryStats,
    pub trend_analysis: DashboardTrendAnalysis,
    pub top_donors: Vec<DonorSummary>,
    pub alerts: Vec<DonorAlert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardTrendAnalysis {
    pub new_donors_trend: Vec<TrendDataPoint>,
    pub funding_amount_trend: Vec<TrendDataPoint>,
    pub engagement_trend: Vec<TrendDataPoint>,
    pub retention_trend: Vec<TrendDataPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorAlert {
    pub donor_id: Uuid,
    pub donor_name: String,
    pub alert_type: AlertType,
    pub message: String,
    pub severity: AlertSeverity,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertType {
    InactiveForLongTime,
    MissingDocuments,
    ExpiredAgreement,
    SuspiciousActivity,
    LowEngagement,
    FundingDelay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Low,
    Medium,
    High,
    Critical,
}

/// Enhanced funding summary statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundingSummaryStats {
    pub total_active_fundings: i64,
    pub total_funding_amount: f64,
    pub avg_funding_amount: f64,
    pub median_funding_amount: f64,
    pub funding_by_currency: HashMap<String, f64>,
    pub funding_by_status: HashMap<String, i64>,
    pub monthly_funding_trend: Vec<TrendDataPoint>,
    pub top_funding_countries: Vec<(String, f64)>,
    pub funding_concentration: f64, // Gini coefficient
}

/// Search index for optimized donor search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DonorSearchIndex {
    pub donor_id: Uuid,
    pub search_text: String, // Concatenated searchable fields
    pub type_: Option<String>,
    pub country: Option<String>,
    pub funding_amount_range: Option<String>, // "0-1000", "1000-10000", etc.
    pub engagement_level: Option<String>,
    pub has_documents: bool,
    pub last_activity: Option<DateTime<Utc>>,
    pub tags: Vec<String>, // Derived tags for categorization
}