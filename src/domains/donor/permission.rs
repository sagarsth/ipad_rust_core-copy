use crate::auth::AuthContext;
use crate::domains::permission::Permission;

/// Map our donor service permissions to the existing permission system
pub trait DonorPermissionAdapter {
    fn has_donor_view_permission(&self) -> bool;
    fn has_donor_manage_permission(&self) -> bool;
}

impl DonorPermissionAdapter for AuthContext {
    fn has_donor_view_permission(&self) -> bool {
        // Map to existing ViewDonors permission using the method on AuthContext
        self.has_permission(Permission::ViewDonors)
    }

    fn has_donor_manage_permission(&self) -> bool {
        // Need both edit and create permissions for full management
        self.has_permission(Permission::EditDonors) &&
        self.has_permission(Permission::CreateDonors)
    }
}