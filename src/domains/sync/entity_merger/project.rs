// sync/entity_merger/project.rs

use super::{DomainEntityMerger, BaseDomainMerger};
use async_trait::async_trait;
use crate::auth::AuthContext;
use crate::errors::{DomainResult, DomainError};
use crate::domains::sync::types::{ChangeLogEntry, Tombstone};
use crate::domains::project::repository::ProjectRepository; // Assuming this trait exists
use crate::domains::project::types::{NewProject, UpdateProject}; // Assuming these types exist
use std::sync::Arc;
use uuid::Uuid;

pub struct ProjectEntityMerger {
    project_repo: Arc<dyn ProjectRepository>,
}

impl ProjectEntityMerger {
    pub fn new(project_repo: Arc<dyn ProjectRepository>) -> Self {
        Self { project_repo }
    }
}

#[async_trait]
impl DomainEntityMerger for ProjectEntityMerger {
    fn entity_table(&self) -> &'static str {
        "projects"
    }
    
    async fn apply_create(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        // Skip if change is from local device (already applied)
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local create for project: {}", change.entity_id);
            return Ok(());
        }
        
        // Parse the new value as NewProject
        let new_project: NewProject = BaseDomainMerger::parse_json_value(
            &change.new_value,
            "new_project"
        )?.ok_or_else(|| DomainError::Internal("Missing new_value for project create".to_string()))?;
        
        // Apply the creation
        log::info!("Applying remote create for project: {}", change.entity_id);
        self.project_repo.create(&new_project, auth).await?; // Ensure ProjectRepository has `create`
        Ok(())
    }
    
    async fn apply_update(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        // Skip if change is from local device
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local update for project: {}", change.entity_id);
            return Ok(());
        }
        
        let entity_id = BaseDomainMerger::get_entity_id(change)?;
        log::info!("Applying remote update for project: {} (field: {:?})", entity_id, change.field_name);

        // For updates, we might need to handle field-level changes
        if let Some(field_name) = &change.field_name {
            // This is a field-level update
            // Ensure your UpdateProject allows for partial updates or construct it carefully.
            let mut update_payload = UpdateProject::default(); // Assuming UpdateProject implements Default and represents partial updates

            match field_name.as_str() {
                "name" => {
                    let new_name: String = BaseDomainMerger::parse_json_value(
                        &change.new_value,
                        "name"
                    )?.ok_or_else(|| DomainError::Internal("Missing name value for project update".to_string()))?;
                    update_payload.name = Some(new_name);
                }
                "status_id" => {
                    // Assuming status_id in DB is INTEGER, but your NewProject/UpdateProject might use a specific type (e.g., i64, Uuid, or an enum)
                    // The schema shows status_id as INTEGER. Here we parse as i64.
                    let new_status: i64 = BaseDomainMerger::parse_json_value(
                        &change.new_value,
                        "status_id"
                    )?.ok_or_else(|| DomainError::Internal("Missing status_id value for project update".to_string()))?;
                    update_payload.status_id = Some(new_status); 
                }
                // Add other fields as needed from your `projects` table schema
                // e.g., "objective", "outcome", "timeline", "responsible_team", "strategic_goal_id"
                "objective" => {
                    update_payload.objective = BaseDomainMerger::parse_json_value(&change.new_value, "objective")?;
                }
                "outcome" => {
                    update_payload.outcome = BaseDomainMerger::parse_json_value(&change.new_value, "outcome")?;
                }
                "timeline" => {
                    update_payload.timeline = BaseDomainMerger::parse_json_value(&change.new_value, "timeline")?;
                }
                "responsible_team" => {
                    update_payload.responsible_team = BaseDomainMerger::parse_json_value(&change.new_value, "responsible_team")?;
                }
                "strategic_goal_id" => {
                    let sg_id_str: Option<String> = BaseDomainMerger::parse_json_value(&change.new_value, "strategic_goal_id")?;
                    // If UpdateProject.strategic_goal_id is Option<Option<Uuid>> to distinguish NotSet vs SetToNull:
                    // The `transpose()?` part yields Option<Uuid>. Wrap in Some() for Option<Option<Uuid>>.
                    update_payload.strategic_goal_id = Some(sg_id_str.map(|s| Uuid::parse_str(&s).map_err(|_| DomainError::Internal("Invalid UUID for strategic_goal_id".to_string()))).transpose()?);
                }
                _ => {
                    log::warn!("Unhandled field update: \"{}\" for projects table. Change ID: {:?}", field_name, change.operation_id);
                    return Ok(()); // Or return an error if unhandled fields are critical
                }
            }
            self.project_repo.update(entity_id, &update_payload, auth).await?; // Ensure ProjectRepository has `update` taking UpdateProject

        } else {
            // This is a full entity update (all fields submitted)
            let update_data: UpdateProject = BaseDomainMerger::parse_json_value(
                &change.new_value,
                "update_project"
            )?.ok_or_else(|| DomainError::Internal("Missing new_value for full project update".to_string()))?;
            
            self.project_repo.update(entity_id, &update_data, auth).await?;
        }
        
        Ok(())
    }
    
    async fn apply_soft_delete(&self, change: &ChangeLogEntry, auth: &AuthContext) -> DomainResult<()> {
        // Skip if change is from local device
        if BaseDomainMerger::is_local_change(change, auth) {
            log::debug!("Skipping local soft delete for project: {}", change.entity_id);
            return Ok(());
        }
        
        let entity_id = BaseDomainMerger::get_entity_id(change)?;
        log::info!("Applying remote soft delete for project: {}", entity_id);
        self.project_repo.soft_delete(entity_id, auth).await // Ensure ProjectRepository has `soft_delete`
    }
    
    async fn apply_hard_delete(&self, tombstone: &Tombstone, auth: &AuthContext) -> DomainResult<()> {
        // Hard deletes are always applied (tombstones are global)
        // No local check needed for hard delete via tombstone
        log::info!("Applying remote hard delete for project: {}", tombstone.entity_id);
        self.project_repo.hard_delete(tombstone.entity_id, auth).await // Ensure ProjectRepository has `hard_delete`
    }
} 