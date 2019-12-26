use std::collections::BTreeMap;

use persistence::cache::Cacheable;
use persistence::commands::permission::{
    CommandPermissionSet, PermissionNode, PermissionRequirement,
};
use persistence::permissions::Permission;
use persistence::DbContext;

use crate::state::BotStateError;
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
        Ok(PermissionStore {
            permissions: Permission::all(&ctx.db_pool)
                .await?
                .into_iter()
                .map(|p| (p.name.clone(), p))
                .collect(),
            leaves: PermissionNode::all(&ctx.db_pool)
                .await?
                .into_iter()
                .map(|p| (p.permission_id, p))
                .collect(),
        })
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

    pub async fn get_by_command(
        &self,
        ctx: &DbContext,
        command_id: i32,
    ) -> Result<CommandPermissionSet> {
        if let Some(set) = CommandPermissionSet::cache_get(&ctx.redis_pool, command_id).await? {
            return Ok(set);
        }

        let load_result: Vec<i32> = Permission::get_by_command_id(&ctx.db_pool, command_id).await?;

        // resolve loaded permission IDs using the tree of permissions in
        // the bot context
        let resolved_requirement = self.get_requirement(load_result)?;

        let set = CommandPermissionSet::new(command_id, resolved_requirement);

        set.cache_set(&ctx.redis_pool).await?;
        Ok(set)
    }
}
