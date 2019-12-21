use std::collections::BTreeMap;

use diesel::{sql_query, sql_types::*, RunQueryDsl};
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::db::permissions::Permission;
use crate::schema::permissions;
use crate::state::{BotStateError, DbContext};
use crate::Result;

/// Permission information loaded from the database. Provides methods to resolve permission
/// requirements for commands
#[derive(Debug)]
pub struct PermissionStore {
    permissions: BTreeMap<String, Permission>,
    leaves: BTreeMap<i32, PermissionNode>,
}

impl PermissionStore {
    /// Loads all permissions from the database and saves them in a sort of tree structure in memory
    /// which can be used to resolve the requirements of individual commands
    pub async fn load(ctx: &DbContext) -> Result<Self> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &ctx.db_pool.get()?;
            Ok(PermissionStore {
                permissions: permissions::table
                    .load::<Permission>(pg)?
                    .into_iter()
                    .map(|p| (p.name.clone(), p))
                    .collect(),
                leaves: sql_query(
                    "select permission_id, array_agg(implied_by_id) as implied_by \
                     from implied_permissions \
                     group by permission_id;",
                )
                .load::<PermissionNode>(pg)?
                .into_iter()
                .map(|p| (p.permission_id, p))
                .collect(),
            })
        })
        .await?
    }

    /// use the permission store to create a `PermissionRequirement` that can be used to check whether
    /// a user has the needed permissions to fulfill it. This resolves a set of permission IDs, taking
    /// into account which permissions are implied by other permissions
    pub fn get_requirement(
        &self,
        permission_ids: impl IntoIterator<Item = i32>,
    ) -> Result<PermissionRequirement> {
        let mut requirements_vec: Vec<Vec<i32>> = vec![];
        for id in permission_ids.into_iter() {
            let mut v = vec![id];
            if let Some(node) = self.leaves.get(&id) {
                v.extend(&node.implied_by)
            }
            requirements_vec.push(v);
        }

        Ok(PermissionRequirement {
            required: requirements_vec,
        })
    }

    pub fn get_permissions<'a>(
        &self,
        names: impl IntoIterator<Item = &'a str>,
    ) -> Result<Vec<&Permission>> {
        names
            .into_iter()
            .map(|name| {
                self.permissions
                    .get(name)
                    .ok_or_else(|| BotStateError::PermissionNotFound(name.to_string()).into())
            })
            .collect::<Result<Vec<_>>>()
    }

    pub fn get_permission(&self, name: &str) -> Result<&Permission> {
        self.permissions
            .get(name)
            .ok_or_else(|| BotStateError::PermissionNotFound(name.to_string()).into())
    }
}

/// Fully flattened permission requirement with a method to check whether a set of permissions is
/// sufficient to satisfy it
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PermissionRequirement {
    required: Vec<Vec<i32>>,
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
struct PermissionNode {
    #[sql_type = "Int4"]
    permission_id: i32,
    #[sql_type = "Array<Int4>"]
    implied_by: Vec<i32>,
}
