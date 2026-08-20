//! PostgreSQL role names used after `SET LOCAL ROLE` in MCP transactions.

use crate::models::user::AccountTypeSlug;

pub const ROLE_ADMIN: &str = "elidune_mcp_admin";
pub const ROLE_STAFF: &str = "elidune_mcp_staff";
pub const ROLE_PATRON: &str = "elidune_mcp_patron";
pub const ROLE_CATALOG: &str = "elidune_mcp_catalog";

/// Map an Elidune account type to the MCP PostgreSQL role.
pub fn pg_role(account_type: &AccountTypeSlug) -> &'static str {
    match account_type {
        AccountTypeSlug::Admin => ROLE_ADMIN,
        AccountTypeSlug::Librarian => ROLE_STAFF,
        AccountTypeSlug::Reader | AccountTypeSlug::Group => ROLE_PATRON,
        AccountTypeSlug::Guest => ROLE_CATALOG,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_account_types() {
        assert_eq!(pg_role(&AccountTypeSlug::Admin), ROLE_ADMIN);
        assert_eq!(pg_role(&AccountTypeSlug::Librarian), ROLE_STAFF);
        assert_eq!(pg_role(&AccountTypeSlug::Reader), ROLE_PATRON);
        assert_eq!(pg_role(&AccountTypeSlug::Group), ROLE_PATRON);
        assert_eq!(pg_role(&AccountTypeSlug::Guest), ROLE_CATALOG);
    }
}
