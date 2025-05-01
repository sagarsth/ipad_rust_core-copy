use crate::auth::AuthContext;
use crate::domains::permission::Permission;

/// Map our funding service permissions to the existing permission system
pub trait FundingPermissionAdapter {
    fn has_funding_view_permission(&self) -> bool;
    fn has_funding_manage_permission(&self) -> bool;
}

impl FundingPermissionAdapter for AuthContext {
    fn has_funding_view_permission(&self) -> bool {
        // Since funding is closely tied to donors, we use the same permissions
        // as donor viewing, which should already be admin-restricted
        self.has_permission(Permission::ViewDonors)
    }

    fn has_funding_manage_permission(&self) -> bool {
        // Need both edit and create donor permissions for full funding management
        self.has_permission(Permission::EditDonors) &&
        self.has_permission(Permission::CreateDonors)
    }
}