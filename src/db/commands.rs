use std::borrow::Cow;
use std::ops::Deref;
use std::time::Duration;

use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::prelude::*;
use diesel::sql_types::Integer;
use futures::executor::block_on;
use r2d2_redis::redis;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::cache::Cacheable;
use crate::db::PermissionRequirement;
use crate::schema::*;
use crate::state::{BotContext, DbContext};
use crate::Result;

/// DB persisted command attributes
#[derive(Serialize, Deserialize, Debug, Clone, Queryable)]
pub struct CommandAttributes {
    pub id: i32,
    /// name of the command handler. Used to identify the right handler in the bot.
    pub handler_name: String,
    /// User facing description
    pub description: Option<String>,
    /// global switch to enable/disable a command
    pub enabled: bool,
    /// whether the command is active by default in all channels
    pub default_active: bool,
    /// minimum time between command uses
    pub cooldown: Option<DurationMillis>,
    /// whether the command can be used in whispers
    pub whisper_enabled: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, FromSqlRow)]
pub struct DurationMillis(Duration);

impl Deref for DurationMillis {
    type Target = Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<DB> FromSql<Integer, DB> for DurationMillis
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: Option<&DB::RawValue>) -> diesel::deserialize::Result<Self> {
        let millis = i32::from_sql(bytes)?;
        if millis >= 0 {
            Ok(DurationMillis(Duration::from_millis(millis as u64)))
        } else {
            Err("Cooldown duration can't be negative".into())
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Queryable)]
pub struct CommandAlias {
    pub name: String,
    pub command_id: i32,
}

impl CommandAlias {
    pub async fn all(ctx: &DbContext) -> Result<Vec<CommandAlias>> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &*ctx.db_pool.get()?;
            command_aliases::table.load(pg).map_err(Into::into)
        })
        .await?
    }
}

#[derive(Insertable)]
#[table_name = "command_attributes"]
pub struct InsertCommandAttributes<'a> {
    pub handler_name: Cow<'a, str>,
    /// User facing description
    pub description: Option<Cow<'a, str>>,
    /// global switch to enable/disable a command
    pub enabled: bool,
    /// whether the command is active by default in all channels
    pub default_active: bool,
    /// minimum time between command uses in milliseconds
    pub cooldown: Option<i32>,
    /// whether the command can be used in whispers
    pub whisper_enabled: bool,
}

fn cooldown_key(handler_name: &str, scope: &str) -> String {
    format!("cb:cooldowns:{}:{}", handler_name, scope)
}

impl CommandAttributes {
    pub async fn reset_cooldown(&self, ctx: &DbContext, scope: &str) -> Result<()> {
        if let Some(cooldown) = &self.cooldown {
            let key = cooldown_key(&self.handler_name, scope);
            let ctx = ctx.clone();
            let cooldown = cooldown.clone();

            task::spawn_blocking(move || {
                let rd = &mut *ctx.redis_pool.get()?;
                redis::cmd("PSETEX")
                    .arg(key)
                    .arg(cooldown.as_millis() as u64)
                    .arg(true)
                    .query(rd)?;
                Ok(())
            })
            .await?
        } else {
            Ok(())
        }
    }

    pub async fn check_cooldown(&self, ctx: &DbContext, scope: &str) -> Result<bool> {
        if self.cooldown.is_some() {
            let key = cooldown_key(&self.handler_name, scope);
            let ctx = ctx.clone();
            task::spawn_blocking(move || {
                let rd = &mut *ctx.redis_pool.get()?;
                let exists = redis::cmd("EXISTS").arg(key).query::<i64>(rd)? > 0;
                Ok(!exists)
            })
            .await?
        } else {
            Ok(true)
        }
    }

    pub async fn all(ctx: &DbContext) -> Result<Vec<CommandAttributes>> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &*ctx.db_pool.get()?;
            command_attributes::table.load(pg).map_err(Into::into)
        })
        .await?
    }

    fn insert_blocking(
        pg: &PgConnection,
        data: InsertCommandAttributes<'_>,
    ) -> Result<CommandAttributes> {
        use crate::schema::command_attributes::dsl::*;
        diesel::insert_into(command_attributes)
            .values(data)
            .get_result(pg)
            .map_err(Into::into)
    }

    pub async fn insert(
        ctx: &BotContext,
        data: InsertCommandAttributes<'static>,
    ) -> Result<CommandAttributes> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &*ctx.db_context.db_pool.get()?;
            Self::insert_blocking(pg, data)
        })
        .await?
    }

    pub async fn initialize(
        ctx: &BotContext,
        data: InsertCommandAttributes<'static>,
        required_permissions: Vec<impl AsRef<str> + Send + 'static>,
        aliases: Vec<impl AsRef<str> + Send + 'static>,
    ) -> Result<()> {
        use diesel::dsl::*;

        //let aliases = aliases.iter().map(|a| a.to_string()).collect::<Vec<_>>();
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &*ctx.db_context.db_pool.get()?;
            let command_exists: bool = select(exists(
                command_attributes::table
                    .filter(command_attributes::handler_name.eq(&data.handler_name)),
            ))
            .get_result(pg)?;
            if !command_exists {
                info!(
                    "Setting up new command \"{}\", handler name: {}",
                    aliases.get(0).map(|a| a.as_ref()).unwrap_or_else(|| ""),
                    &data.handler_name
                );
                let attributes = Self::insert_blocking(pg, data)?;
                diesel::insert_into(command_aliases::table)
                    .values(
                        aliases
                            .iter()
                            .map(|alias| {
                                (
                                    command_aliases::command_id.eq(attributes.id),
                                    command_aliases::name.eq(alias.as_ref()),
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                    .execute(pg)?;
            }
            Ok(())
        })
        .await?
    }
}

impl_redis_bincode!(CommandAttributes);

impl Cacheable<&str> for CommandAttributes {
    fn cache_key(&self) -> String {
        format!("cb:cmd:{}", &self.handler_name)
    }

    fn cache_key_from_id(id: &str) -> String {
        format!("cb:cmd:{}", id)
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(600)
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
    ) -> Result<Option<Self>> {
        use crate::schema::channel_command_config::dsl::*;

        let ctx = ctx.clone();

        task::spawn_blocking(move || {
            let rd = &mut *ctx.db_context.redis_pool.get()?;
            let pg = &*ctx.db_context.db_pool.get()?;

            if let Ok(cached_value) = Self::cache_get(rd, (channel_id_value, command_id_value)) {
                return Ok(Some(cached_value));
            }

            let config = channel_command_config
                .filter(channel_id.eq(channel_id_value))
                .first::<ChannelCommandConfig>(pg)
                .optional()?;

            if let Some(ref config) = config {
                config.cache_set(rd)?;
            }
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

#[allow(clippy::type_complexity)]
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
