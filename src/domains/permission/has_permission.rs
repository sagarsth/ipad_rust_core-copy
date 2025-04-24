 // Adjust imports based on your actual error structure
use serde::{Deserialize, Serialize};
 // Required for AuditLogger example

// --- User Role Definition ---

/// UserRole enum for authorization in the application
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    FieldTeamLead,
    FieldOfficer,
}

// --- Permission Enum Definition (Combined) ---

/// Permission enum representing individual permissions in the system
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // User management
    ManageUsers,

    // Strategic Goal permissions (NEW)
    ViewStrategicGoals,
    EditStrategicGoals,
    CreateStrategicGoals,
    DeleteStrategicGoals,
    
    // Project permissions
    ViewProjects,
    EditProjects,
    CreateProjects,
    DeleteProjects,

    // Participant permissions
    ViewParticipants,
    EditParticipants,
    CreateParticipants,
    DeleteParticipants,

    // Workshop permissions
    ViewWorkshops,
    EditWorkshops,
    CreateWorkshops,
    DeleteWorkshops,

    // Activity permissions
    ViewActivities,
    EditActivities,
    CreateActivities,
    DeleteActivities,

    // Livelihood permissions
    ViewLivelihoods,
    EditLivelihoods,
    CreateLivelihoods,
    DeleteLivelihoods,

    // Document permissions
    ViewDocuments,
    EditDocuments,
    UploadDocuments,
    DeleteDocuments,

    // Donor permissions
    ViewDonors,
    EditDonors,
    CreateDonors,
    DeleteDonors,

    // System permissions
    ViewAuditLogs,
    ConfigureSystem,
    ExportData,
    ImportData,

    // Sync permissions
    SyncData,
    ConfigurePersonalSync,
    ViewSyncStatus,
    ManageGlobalSync,

    // Special permissions
    DeleteRecord,
    HardDeleteRecord,
    HardDeleteRecordWithDependencies,
}

// --- UserRole Implementation (Using Latest Logic) ---

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Admin => "admin",
            UserRole::FieldTeamLead => "field_tl",
            UserRole::FieldOfficer => "field",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "admin" => Some(UserRole::Admin),
            "field_tl" => Some(UserRole::FieldTeamLead),
            "field" => Some(UserRole::FieldOfficer),
            _ => None,
        }
    }

    /// Check if the user has a specific permission based on the updated logic
    pub fn has_permission(&self, permission: Permission) -> bool {
        match self {
            UserRole::Admin => true, // Admin has all permissions
            UserRole::FieldTeamLead => {
                match permission {
                    // Admin-only permissions - deny FieldTeamLead
                    Permission::ManageUsers
                    | Permission::ViewAuditLogs
                    | Permission::ConfigureSystem
                    | Permission::HardDeleteRecord
                    | Permission::HardDeleteRecordWithDependencies
                    | Permission::ManageGlobalSync => false,

                    // Personal sync config & basic sync allowed
                    Permission::SyncData
                    | Permission::ConfigurePersonalSync
                    | Permission::ViewSyncStatus => true,

                    // Everything else is assumed allowed for FieldTeamLead (including StrategicGoals)
                    _ => true,
                }
            }
            UserRole::FieldOfficer => {
                match permission {
                    // Personal sync config & basic sync allowed
                    Permission::SyncData
                    | Permission::ConfigurePersonalSync
                    | Permission::ViewSyncStatus => true,
                    
                    // Deny Strategic Goal modification for FieldOfficer by default?
                    Permission::EditStrategicGoals 
                    | Permission::CreateStrategicGoals 
                    | Permission::DeleteStrategicGoals => false, 

                    // Basic CRUD for most operational entities - allow FieldOfficer
                    Permission::ViewStrategicGoals // Allow viewing strategic goals
                    | Permission::ViewProjects | Permission::EditProjects
                    | Permission::CreateProjects | Permission::DeleteProjects
                    | Permission::ViewParticipants | Permission::EditParticipants
                    | Permission::CreateParticipants | Permission::DeleteParticipants
                    | Permission::ViewWorkshops | Permission::EditWorkshops
                    | Permission::CreateWorkshops | Permission::DeleteWorkshops
                    | Permission::ViewActivities | Permission::EditActivities
                    | Permission::CreateActivities | Permission::DeleteActivities
                    | Permission::ViewLivelihoods | Permission::EditLivelihoods
                    | Permission::CreateLivelihoods | Permission::DeleteLivelihoods
                    | Permission::ViewDocuments | Permission::EditDocuments
                    | Permission::UploadDocuments | Permission::DeleteDocuments
                    | Permission::DeleteRecord => true, // General soft delete

                    // Access to donor data and admin functions - deny FieldOfficer
                    Permission::ViewDonors | Permission::EditDonors
                    | Permission::CreateDonors | Permission::DeleteDonors
                    | Permission::ManageUsers
                    | Permission::ViewAuditLogs
                    | Permission::ConfigureSystem
                    | Permission::ExportData
                    | Permission::ImportData
                    | Permission::HardDeleteRecord
                    | Permission::HardDeleteRecordWithDependencies
                    | Permission::ManageGlobalSync => false,
                }
            }
        }
    }

    /// Check if the user has all of the specified permissions
    pub fn has_permissions(&self, permissions: &[Permission]) -> bool {
        permissions.iter().all(|p| self.has_permission(*p))
    }

    /// Check if this role can perform any form of hard delete
    pub fn can_hard_delete(&self) -> bool {
        self.has_permission(Permission::HardDeleteRecord) ||
        self.has_permission(Permission::HardDeleteRecordWithDependencies)
    }
}


// --- Permission Implementation (String Conversions & Listing) ---

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            // User management
            Permission::ManageUsers => "manage_users",
            // Strategic Goal permissions (NEW)
            Permission::ViewStrategicGoals => "view_strategic_goals",
            Permission::EditStrategicGoals => "edit_strategic_goals",
            Permission::CreateStrategicGoals => "create_strategic_goals",
            Permission::DeleteStrategicGoals => "delete_strategic_goals",
            // Project permissions
            Permission::ViewProjects => "view_projects",
            Permission::EditProjects => "edit_projects",
            Permission::CreateProjects => "create_projects",
            Permission::DeleteProjects => "delete_projects",
            // Participant permissions
            Permission::ViewParticipants => "view_participants",
            Permission::EditParticipants => "edit_participants",
            Permission::CreateParticipants => "create_participants",
            Permission::DeleteParticipants => "delete_participants",
            // Workshop permissions
            Permission::ViewWorkshops => "view_workshops",
            Permission::EditWorkshops => "edit_workshops",
            Permission::CreateWorkshops => "create_workshops",
            Permission::DeleteWorkshops => "delete_workshops",
            // Activity permissions
            Permission::ViewActivities => "view_activities",
            Permission::EditActivities => "edit_activities",
            Permission::CreateActivities => "create_activities",
            Permission::DeleteActivities => "delete_activities",
            // Livelihood permissions
            Permission::ViewLivelihoods => "view_livelihoods",
            Permission::EditLivelihoods => "edit_livelihoods",
            Permission::CreateLivelihoods => "create_livelihoods",
            Permission::DeleteLivelihoods => "delete_livelihoods",
            // Document permissions
            Permission::ViewDocuments => "view_documents",
            Permission::EditDocuments => "edit_documents",
            Permission::UploadDocuments => "upload_documents",
            Permission::DeleteDocuments => "delete_documents",
            // Donor permissions
            Permission::ViewDonors => "view_donors",
            Permission::EditDonors => "edit_donors",
            Permission::CreateDonors => "create_donors",
            Permission::DeleteDonors => "delete_donors",
            // System permissions
            Permission::ViewAuditLogs => "view_audit_logs",
            Permission::ConfigureSystem => "configure_system",
            Permission::ExportData => "export_data",
            Permission::ImportData => "import_data",
            // Sync permissions
            Permission::SyncData => "sync_data",
            Permission::ConfigurePersonalSync => "configure_personal_sync",
            Permission::ViewSyncStatus => "view_sync_status",
            Permission::ManageGlobalSync => "manage_global_sync",
            // Special permissions
            Permission::DeleteRecord => "delete_record",
            Permission::HardDeleteRecord => "hard_delete_record",
            Permission::HardDeleteRecordWithDependencies => "hard_delete_record_with_dependencies",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            // User management
            "manage_users" => Some(Permission::ManageUsers),
            // Strategic Goal permissions (NEW)
            "view_strategic_goals" => Some(Permission::ViewStrategicGoals),
            "edit_strategic_goals" => Some(Permission::EditStrategicGoals),
            "create_strategic_goals" => Some(Permission::CreateStrategicGoals),
            "delete_strategic_goals" => Some(Permission::DeleteStrategicGoals),
            // Project permissions
            "view_projects" => Some(Permission::ViewProjects),
            "edit_projects" => Some(Permission::EditProjects),
            "create_projects" => Some(Permission::CreateProjects),
            "delete_projects" => Some(Permission::DeleteProjects),
            // Participant permissions
            "view_participants" => Some(Permission::ViewParticipants),
            "edit_participants" => Some(Permission::EditParticipants),
            "create_participants" => Some(Permission::CreateParticipants),
            "delete_participants" => Some(Permission::DeleteParticipants),
            // Workshop permissions
            "view_workshops" => Some(Permission::ViewWorkshops),
            "edit_workshops" => Some(Permission::EditWorkshops),
            "create_workshops" => Some(Permission::CreateWorkshops),
            "delete_workshops" => Some(Permission::DeleteWorkshops),
            // Activity permissions
            "view_activities" => Some(Permission::ViewActivities),
            "edit_activities" => Some(Permission::EditActivities),
            "create_activities" => Some(Permission::CreateActivities),
            "delete_activities" => Some(Permission::DeleteActivities),
            // Livelihood permissions
            "view_livelihoods" => Some(Permission::ViewLivelihoods),
            "edit_livelihoods" => Some(Permission::EditLivelihoods),
            "create_livelihoods" => Some(Permission::CreateLivelihoods),
            "delete_livelihoods" => Some(Permission::DeleteLivelihoods),
            // Document permissions
            "view_documents" => Some(Permission::ViewDocuments),
            "edit_documents" => Some(Permission::EditDocuments),
            "upload_documents" => Some(Permission::UploadDocuments),
            "delete_documents" => Some(Permission::DeleteDocuments),
            // Donor permissions
            "view_donors" => Some(Permission::ViewDonors),
            "edit_donors" => Some(Permission::EditDonors),
            "create_donors" => Some(Permission::CreateDonors),
            "delete_donors" => Some(Permission::DeleteDonors),
            // System permissions
            "view_audit_logs" => Some(Permission::ViewAuditLogs),
            "configure_system" => Some(Permission::ConfigureSystem),
            "export_data" => Some(Permission::ExportData),
            "import_data" => Some(Permission::ImportData),
            // Sync permissions
            "sync_data" => Some(Permission::SyncData),
            "configure_personal_sync" => Some(Permission::ConfigurePersonalSync),
            "view_sync_status" => Some(Permission::ViewSyncStatus),
            "manage_global_sync" => Some(Permission::ManageGlobalSync),
            // Special permissions
            "delete_record" => Some(Permission::DeleteRecord),
            "hard_delete_record" => Some(Permission::HardDeleteRecord),
            "hard_delete_record_with_dependencies" => Some(Permission::HardDeleteRecordWithDependencies),
            // Default case
            _ => None,
        }
    }

    /// Get all permissions in the system (including new ones)
    pub fn all() -> Vec<Permission> {
        vec![
            // User management
            Permission::ManageUsers,
            // Strategic Goal permissions (NEW)
            Permission::ViewStrategicGoals, Permission::EditStrategicGoals, Permission::CreateStrategicGoals, Permission::DeleteStrategicGoals,
            // Project permissions
            Permission::ViewProjects, Permission::EditProjects, Permission::CreateProjects, Permission::DeleteProjects,
            // Participant permissions
            Permission::ViewParticipants, Permission::EditParticipants, Permission::CreateParticipants, Permission::DeleteParticipants,
            // Workshop permissions
            Permission::ViewWorkshops, Permission::EditWorkshops, Permission::CreateWorkshops, Permission::DeleteWorkshops,
            // Activity permissions
            Permission::ViewActivities, Permission::EditActivities, Permission::CreateActivities, Permission::DeleteActivities,
            // Livelihood permissions
            Permission::ViewLivelihoods, Permission::EditLivelihoods, Permission::CreateLivelihoods, Permission::DeleteLivelihoods,
            // Document permissions
            Permission::ViewDocuments, Permission::EditDocuments, Permission::UploadDocuments, Permission::DeleteDocuments,
            // Donor permissions
            Permission::ViewDonors, Permission::EditDonors, Permission::CreateDonors, Permission::DeleteDonors,
            // System permissions
            Permission::ViewAuditLogs, Permission::ConfigureSystem, Permission::ExportData, Permission::ImportData,
            // Sync permissions
            Permission::SyncData, Permission::ConfigurePersonalSync, Permission::ViewSyncStatus, Permission::ManageGlobalSync,
            // Special permissions
            Permission::DeleteRecord, Permission::HardDeleteRecord, Permission::HardDeleteRecordWithDependencies,
        ]
    }
}

