use arrow::datatypes::{DataType, Field, Schema, TimeUnit};
use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::Lazy;

/// Cache for domain schemas to avoid recreation
static SCHEMA_CACHE: Lazy<HashMap<&'static str, Arc<Schema>>> = Lazy::new(|| {
    let mut cache = HashMap::new();
    
    cache.insert("strategic_goals", Arc::new(strategic_goals_schema()));
    cache.insert("projects", Arc::new(projects_schema()));
    cache.insert("workshops", Arc::new(workshops_schema()));
    cache.insert("media_documents", Arc::new(media_documents_schema()));
    
    // Add missing domain schemas
    cache.insert("activities", Arc::new(activities_schema()));
    cache.insert("donors", Arc::new(donors_schema()));
    cache.insert("funding", Arc::new(funding_schema()));
    cache.insert("livelihoods", Arc::new(livelihoods_schema()));
    cache.insert("workshop_participants", Arc::new(workshop_participants_schema()));
    cache.insert("participants", Arc::new(participants_schema()));

    cache
});

/// Get cached schema for a domain
pub fn get_cached_schema(domain: &str) -> Option<Arc<Schema>> {
    SCHEMA_CACHE.get(domain).cloned()
}

/// iOS-optimized schema trait
pub trait IOSParquetSchema {
    /// Core schema definition with mobile-optimized data types
    fn raw_schema() -> Schema;
    
    /// Memory-optimized schema for background exports
    fn memory_optimized_schema() -> Schema {
        let mut schema = Self::raw_schema();
        let mut metadata = HashMap::new();
        metadata.insert("ios.optimization".to_string(), "mobile".to_string());
        metadata.insert("compression".to_string(), "snappy".to_string());
        schema = schema.with_metadata(metadata);
        schema
    }
    
    /// Full fidelity schema for foreground exports
    fn full_fidelity_schema() -> Schema {
        let mut schema = Self::raw_schema();
        let mut metadata = HashMap::new();
        metadata.insert("ios.optimization".to_string(), "full_fidelity".to_string());
        metadata.insert("compression".to_string(), "uncompressed".to_string());
        schema = schema.with_metadata(metadata);
        schema
    }
}

// Strategic Goals Schema
pub fn strategic_goals_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("title", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("status", DataType::Utf8, false),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("parent_id", DataType::Utf8, true),
        Field::new("position", DataType::Int32, false),
        Field::new("metadata", DataType::Utf8, true), // JSON string
    ])
}

// Projects Schema
pub fn projects_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("status", DataType::Utf8, false),
        Field::new("start_date", DataType::Date32, true),
        Field::new("end_date", DataType::Date32, true),
        Field::new("budget", DataType::Float64, true),
        Field::new("strategic_goal_id", DataType::Utf8, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
    ])
}

// Workshops Schema with nested participants
pub fn workshops_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("title", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("start_time", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("duration_minutes", DataType::Int32, false),
        Field::new("location", DataType::Utf8, true),
        Field::new("max_participants", DataType::Int32, true),
        Field::new_list(
            "participants",
            Field::new("item", DataType::Struct(vec![
                Field::new("user_id", DataType::Utf8, false),
                Field::new("name", DataType::Utf8, false),
                Field::new("attended", DataType::Boolean, false),
                Field::new("certificate_issued", DataType::Boolean, false),
            ].into()), true),
            true
        ),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
    ])
}

// Media Documents Schema
pub fn media_documents_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("file_name", DataType::Utf8, false),
        Field::new("mime_type", DataType::Utf8, false),
        Field::new("size_bytes", DataType::Int64, false),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("related_entity_type", DataType::Utf8, true),
        Field::new("related_entity_id", DataType::Utf8, true),
        Field::new("blob_path", DataType::Utf8, true), // External reference
        Field::new("checksum", DataType::Utf8, true),
    ])
}

// Activities Schema
pub fn activities_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("project_id", DataType::Utf8, true),
        Field::new("name", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("activity_type", DataType::Utf8, true),
        Field::new("status", DataType::Utf8, false),
        Field::new("start_date", DataType::Date32, true),
        Field::new("end_date", DataType::Date32, true),
        Field::new("budget_allocated", DataType::Float64, true),
        Field::new("budget_spent", DataType::Float64, true),
        Field::new("responsible_team", DataType::Utf8, true),
        Field::new("location", DataType::Utf8, true),
        Field::new("participants_count", DataType::Int32, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("created_by_user_id", DataType::Utf8, true),
        Field::new("updated_by_user_id", DataType::Utf8, true),
        Field::new("deleted_at", DataType::Timestamp(TimeUnit::Millisecond, None), true),
    ])
}

// Donors Schema
pub fn donors_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("type", DataType::Utf8, false), // Individual, Organization, Government, etc.
        Field::new("contact_email", DataType::Utf8, true),
        Field::new("contact_phone", DataType::Utf8, true),
        Field::new("address", DataType::Utf8, true),
        Field::new("country", DataType::Utf8, true),
        Field::new("total_donated", DataType::Float64, true),
        Field::new("first_donation_date", DataType::Date32, true),
        Field::new("last_donation_date", DataType::Date32, true),
        Field::new("donation_frequency", DataType::Utf8, true),
        Field::new("preferred_communication", DataType::Utf8, true),
        Field::new("tax_exempt_status", DataType::Boolean, true),
        Field::new("notes", DataType::Utf8, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("created_by_user_id", DataType::Utf8, true),
        Field::new("updated_by_user_id", DataType::Utf8, true),
        Field::new("deleted_at", DataType::Timestamp(TimeUnit::Millisecond, None), true),
    ])
}

// Funding Schema  
pub fn funding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("donor_id", DataType::Utf8, false),
        Field::new("project_id", DataType::Utf8, true),
        Field::new("strategic_goal_id", DataType::Utf8, true),
        Field::new("amount", DataType::Float64, false),
        Field::new("currency", DataType::Utf8, false),
        Field::new("funding_type", DataType::Utf8, false), // Grant, Donation, Government, etc.
        Field::new("funding_status", DataType::Utf8, false), // Pending, Approved, Disbursed, etc.
        Field::new("application_date", DataType::Date32, true),
        Field::new("approval_date", DataType::Date32, true),
        Field::new("disbursement_date", DataType::Date32, true),
        Field::new("funding_period_start", DataType::Date32, true),
        Field::new("funding_period_end", DataType::Date32, true),
        Field::new("purpose", DataType::Utf8, true),
        Field::new("restrictions", DataType::Utf8, true),
        Field::new("reporting_requirements", DataType::Utf8, true),
        Field::new("contract_reference", DataType::Utf8, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("created_by_user_id", DataType::Utf8, true),
        Field::new("updated_by_user_id", DataType::Utf8, true),
        Field::new("deleted_at", DataType::Timestamp(TimeUnit::Millisecond, None), true),
    ])
}

// Livelihoods Schema
pub fn livelihoods_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("participant_id", DataType::Utf8, false),
        Field::new("project_id", DataType::Utf8, true),
        Field::new("livelihood_type", DataType::Utf8, false), // Agriculture, Trade, Services, etc.
        Field::new("skill_area", DataType::Utf8, true),
        Field::new("training_received", DataType::Boolean, true),
        Field::new("training_completion_date", DataType::Date32, true),
        Field::new("assets_provided", DataType::Utf8, true),
        Field::new("asset_value", DataType::Float64, true),
        Field::new("monthly_income_before", DataType::Float64, true),
        Field::new("monthly_income_after", DataType::Float64, true),
        Field::new("employment_status", DataType::Utf8, true),
        Field::new("business_established", DataType::Boolean, true),
        Field::new("business_registration_date", DataType::Date32, true),
        Field::new("employees_hired", DataType::Int32, true),
        Field::new("sustainability_score", DataType::Float64, true),
        Field::new("follow_up_date", DataType::Date32, true),
        Field::new("success_indicators", DataType::Utf8, true),
        Field::new("challenges", DataType::Utf8, true),
        Field::new("notes", DataType::Utf8, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("created_by_user_id", DataType::Utf8, true),
        Field::new("updated_by_user_id", DataType::Utf8, true),
        Field::new("deleted_at", DataType::Timestamp(TimeUnit::Millisecond, None), true),
    ])
}

// Workshop Participants Schema (separate from workshops)
pub fn workshop_participants_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("workshop_id", DataType::Utf8, false),
        Field::new("participant_id", DataType::Utf8, false),
        Field::new("user_id", DataType::Utf8, true),
        Field::new("registration_date", DataType::Date32, false),
        Field::new("attendance_status", DataType::Utf8, false), // Registered, Attended, NoShow, Cancelled
        Field::new("attended", DataType::Boolean, false),
        Field::new("attendance_percentage", DataType::Float64, true),
        Field::new("completion_status", DataType::Utf8, true), // Completed, Partial, Dropped
        Field::new("certificate_issued", DataType::Boolean, false),
        Field::new("certificate_date", DataType::Date32, true),
        Field::new("pre_assessment_score", DataType::Float64, true),
        Field::new("post_assessment_score", DataType::Float64, true),
        Field::new("feedback_rating", DataType::Int32, true), // 1-5 scale
        Field::new("feedback_comments", DataType::Utf8, true),
        Field::new("special_requirements", DataType::Utf8, true),
        Field::new("payment_status", DataType::Utf8, true),
        Field::new("payment_amount", DataType::Float64, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("created_by_user_id", DataType::Utf8, true),
        Field::new("updated_by_user_id", DataType::Utf8, true),
        Field::new("deleted_at", DataType::Timestamp(TimeUnit::Millisecond, None), true),
    ])
}

// Participants Schema (standalone demographic participant entities)
pub fn participants_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("name", DataType::Utf8, false),
        Field::new("gender", DataType::Utf8, true),
        Field::new("disability", DataType::Boolean, false),
        Field::new("disability_type", DataType::Utf8, true),
        Field::new("age_group", DataType::Utf8, true), // child, youth, adult, elderly
        Field::new("location", DataType::Utf8, true),
        Field::new("sync_priority", DataType::Utf8, true),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("created_by_user_id", DataType::Utf8, true),
        Field::new("created_by_device_id", DataType::Utf8, true),
        Field::new("updated_by_user_id", DataType::Utf8, true),
        Field::new("updated_by_device_id", DataType::Utf8, true),
        Field::new("deleted_at", DataType::Timestamp(TimeUnit::Millisecond, None), true),
        Field::new("deleted_by_user_id", DataType::Utf8, true),
        Field::new("deleted_by_device_id", DataType::Utf8, true),
    ])
}

/// Schema builder for dynamic schemas
pub struct SchemaBuilder {
    fields: Vec<Field>,
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self { fields: Vec::new() }
    }
    
    pub fn add_field(mut self, name: &str, data_type: DataType, nullable: bool) -> Self {
        self.fields.push(Field::new(name, data_type, nullable));
        self
    }
    
    pub fn add_id_field(self) -> Self {
        self.add_field("id", DataType::Utf8, false)
    }
    
    pub fn add_timestamps(self) -> Self {
        self.add_field("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false)
            .add_field("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false)
    }
    
    pub fn build(self) -> Schema {
        Schema::new(self.fields)
    }
}

/// Helper functions for common schema patterns
pub fn create_minimal_schema(entity_name: &str) -> Schema {
    SchemaBuilder::new()
        .add_id_field()
        .add_field("name", DataType::Utf8, false)
        .add_timestamps()
        .build()
}

pub fn create_entity_schema_with_metadata(entity_name: &str, fields: Vec<Field>) -> Schema {
    let mut all_fields = vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("created_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
        Field::new("updated_at", DataType::Timestamp(TimeUnit::Millisecond, None), false),
    ];
    
    all_fields.extend(fields);
    
    let mut metadata = HashMap::new();
    metadata.insert("entity_type".to_string(), entity_name.to_string());
    metadata.insert("schema_version".to_string(), "1.0".to_string());
    
    Schema::new(all_fields).with_metadata(metadata)
} 