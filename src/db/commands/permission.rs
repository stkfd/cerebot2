use std::time::Duration;

use diesel::prelude::*;
use futures::executor::block_on;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::cache::Cacheable;
use crate::db::permissions::PermissionRequirement;
use crate::schema::*;
use crate::state::BotContext;
use crate::Result;

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
    /// Get the command ID this set applies to
    pub fn command_id(&self) -> i32 {
        self.command_id
    }
    /// Get slice of (id, name) tuples of the contained permissions
    pub fn requirements(&self) -> &PermissionRequirement {
        &self.req
    }
}

impl_redis_bincode!(CommandPermissionSet);

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

impl CommandPermission {
    pub async fn get_by_command(ctx: &BotContext, command_id: i32) -> Result<CommandPermissionSet> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let rd = &mut *ctx.db_context.redis_pool.get()?;
            let pg = &*ctx.db_context.db_pool.get()?;
            CommandPermissionSet::cache_get(rd, command_id).or_else(|_| {
                let load_result: Vec<i32> = permissions::table
                    .select(permissions::id)
                    .filter(command_permissions::command_id.eq(command_id))
                    .left_outer_join(command_permissions::table)
                    .load::<i32>(pg)?;

                // resolve loaded permission IDs using the tree of permissions in
                // the bot context
                let resolved_requirement =
                    block_on(ctx.permissions.read()).get_requirement(load_result)?;

                let set = CommandPermissionSet {
                    command_id,
                    req: resolved_requirement,
                };

                set.cache_set(rd)?;
                Ok(set)
            })
        })
        .await?
    }
}
