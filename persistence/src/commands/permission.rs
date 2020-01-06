use std::time::Duration;

use diesel::sql_query;
use diesel::sql_types::*;
use serde::{Deserialize, Serialize};
use tokio_diesel::AsyncRunQueryDsl;

use crate::cache::Cacheable;
use crate::impl_redis_bincode_int;
use crate::{DbPool, Result};

/// Required permissions for a command
#[derive(Queryable)]
pub struct CommandPermission {
    pub command_id: i32,
    pub permission_id: i32,
}

#[derive(Serialize, Deserialize)]
pub struct CommandPermissionSet {
    command_id: i32,
    req: PermissionRequirement,
}

impl CommandPermissionSet {
    pub fn new(command_id: i32, req: PermissionRequirement) -> Self {
        CommandPermissionSet { command_id, req }
    }
    /// Get the command ID this set applies to
    pub fn command_id(&self) -> i32 {
        self.command_id
    }
    /// Get slice of (id, name) tuples of the contained permissions
    pub fn requirements(&self) -> &PermissionRequirement {
        &self.req
    }
}

impl_redis_bincode_int!(CommandPermissionSet);

impl Cacheable<i32> for CommandPermissionSet {
    fn cache_key(&self) -> String {
        format!("cb:command_permissions:{}", self.command_id)
    }

    fn cache_key_from_id(id: i32) -> String {
        format!("cb:command_permissions:{}", id)
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(5 * 60)
    }
}

/// Fully flattened permission requirement with a method to check whether a set of permissions is
/// sufficient to satisfy it
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PermissionRequirement {
    pub required: Vec<Vec<i32>>,
}

impl PermissionRequirement {
    /// Check whether the given set of permissions (by IDs) is sufficient to satisfy this permission
    /// requirement
    pub fn check(&self, available_permissions: &[i32]) -> bool {
        let result = self.required.iter().all(|any_required| {
            any_required
                .iter()
                .any(|id| available_permissions.contains(id))
        });
        if !result {
            debug!(
                "Permission check failed! Required: {:?} Actual: {:?}",
                self.required, available_permissions
            );
        }
        result
    }
}

/// Contains a permission ID and all other permissions that imply this permission is present.
#[derive(QueryableByName, Debug)]
pub struct PermissionNode {
    #[sql_type = "Int4"]
    pub permission_id: i32,
    #[sql_type = "Array<Int4>"]
    pub implied_by: Vec<i32>,
}

impl PermissionNode {
    /// Get all permission nodes grouped by permission id
    pub async fn all(pool: &DbPool) -> Result<Vec<PermissionNode>> {
        sql_query(
            "select permission_id, array_agg(implied_by_id) as implied_by \
             from implied_permissions \
             group by permission_id;",
        )
        .load_async::<PermissionNode>(pool)
        .await
        .map_err(Into::into)
    }
}
