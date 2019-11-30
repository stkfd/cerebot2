use std::time::Duration;

use diesel::backend::Backend;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;

use crate::cache::Cacheable;
use crate::error::Error;
use crate::schema::*;
use crate::state::BotContext;
use crate::db::PermissionRequirement;

/// DB persisted command attributes
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CommandAttributes {
    pub id: i32,
    /// name of the command (used to match calls)
    pub name: String,
    /// User facing description
    pub description: Option<String>,
    /// global switch to enable/disable a command
    pub enabled: bool,
    /// whether the command is active by default in all channels
    pub default_active: bool,
    /// minimum time between command uses
    pub cooldown: Option<Duration>,
}

#[derive(Insertable)]
#[table_name = "command_attributes"]
pub struct NewCommandAttributes {
    pub name: String,
    /// User facing description
    pub description: Option<String>,
    /// global switch to enable/disable a command
    pub enabled: bool,
    /// whether the command is active by default in all channels
    pub default_active: bool,
    /// minimum time between command uses in milliseconds
    pub cooldown: Option<i32>,
}

impl CommandAttributes {
    pub async fn get(ctx: &BotContext, command_name: &str) -> Result<Option<Self>, Error> {
        use crate::schema::command_attributes::dsl::*;

        let ctx = ctx.db_context.clone();
        let command_name = command_name.to_string();
        spawn_blocking(move || {
            let rd = &mut *ctx.redis_pool.get()?;
            let pg = &*ctx.db_pool.get()?;

            if let Ok(cached_value) = Self::cache_get(rd, &command_name) {
                return Ok(Some(cached_value));
            }

            command_attributes
                .filter(name.eq(command_name))
                .first(pg)
                .optional()
                .map_err(Into::into)
        })
        .await?
    }

    pub async fn insert(ctx: &BotContext, data: NewCommandAttributes) -> Result<Self, Error> {
        use crate::schema::command_attributes::dsl::*;

        let ctx = ctx.db_context.clone();
        spawn_blocking(move || {
            let pg = &*ctx.db_pool.get()?;

            diesel::insert_into(command_attributes)
                .values(data)
                .get_result(pg)
                .map_err(Into::into)
        })
        .await?
    }
}

impl_redis_bincode!(CommandAttributes);

impl Cacheable<&str> for CommandAttributes {
    fn cache_key(&self) -> String {
        format!("cb:cmd:{}", &self.name)
    }

    fn cache_key_from_id(id: &str) -> String {
        format!("cb:cmd:{}", id)
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(600)
    }
}

impl<Db: Backend, St> Queryable<St, Db> for CommandAttributes
where
    (i32, String, Option<String>, bool, bool, Option<i32>): Queryable<St, Db>,
{
    type Row = <(i32, String, Option<String>, bool, bool, Option<i32>) as Queryable<St, Db>>::Row;
    fn build(row: Self::Row) -> Self {
        let row: (i32, String, Option<String>, bool, bool, Option<i32>) = Queryable::build(row);
        Self {
            id: row.0,
            name: row.1,
            description: row.2,
            enabled: row.3,
            default_active: row.4,
            cooldown: row.5.map(|millis| Duration::from_millis(millis as u64)),
        }
    }
}

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
        Duration::from_secs(3600)
    }
}

impl CommandPermission {
    pub async fn get_by_command(
        ctx: &BotContext,
        command_id: i32,
    ) -> Result<CommandPermissionSet, Error> {
        let ctx = ctx.clone();

        spawn_blocking(move || {
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
                let resolved_requirement = ctx.permissions
                    .read()
                    .get_requirement(load_result)?;

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

/// Channel specific command configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct ChannelCommandConfig {
    pub channel_id: i32,
    pub command_id: i32,
    /// disables or enables the command in this channel if set
    pub active: Option<bool>,
    /// per channel cooldown override
    pub cooldown: Option<Duration>,
}

impl ChannelCommandConfig {
    pub async fn get(
        ctx: &BotContext,
        channel_id_value: i32,
        command_id_value: i32,
    ) -> Result<Option<Self>, Error> {
        use crate::schema::channel_command_config::dsl::*;

        let ctx = ctx.db_context.clone();
        spawn_blocking(move || {
            let rd = &mut *ctx.redis_pool.get()?;
            let pg = &*ctx.db_pool.get()?;

            if let Ok(cached_value) = Self::cache_get(rd, (channel_id_value, command_id_value)) {
                return Ok(Some(cached_value));
            }

            let config = channel_command_config
                .filter(channel_id.eq(channel_id_value))
                .first::<ChannelCommandConfig>(pg)
                .optional()?;

            if let Some(ref config) = config { config.cache_set(rd)?; }
            Ok(config)
        })
        .await?
    }
}

impl_redis_bincode!(ChannelCommandConfig);

impl Cacheable<(i32, i32)> for ChannelCommandConfig {
    fn cache_key(&self) -> String {
        format!("cb:cmd_cfg:{}:{}", self.channel_id, self.command_id)
    }

    fn cache_key_from_id(id: (i32, i32)) -> String {
        format!("cb:cmd_cfg:{}:{}", id.0, id.1)
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(600)
    }
}

impl<Db: Backend, St> Queryable<St, Db> for ChannelCommandConfig
where
    (i32, i32, Option<bool>, Option<i32>): Queryable<St, Db>,
{
    type Row = <(i32, i32, Option<bool>, Option<i32>) as Queryable<St, Db>>::Row;
    fn build(row: Self::Row) -> Self {
        let row: (i32, i32, Option<bool>, Option<i32>) = Queryable::build(row);
        Self {
            channel_id: row.0,
            command_id: row.1,
            active: row.2,
            cooldown: row.3.map(|millis| Duration::from_millis(millis as u64)),
        }
    }
}
