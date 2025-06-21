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