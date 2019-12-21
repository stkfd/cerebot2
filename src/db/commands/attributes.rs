use std::borrow::Cow;
use std::ops::Deref;
use std::time::Duration;

use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::prelude::*;
use diesel::sql_types::Integer;
use r2d2_redis::redis;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::cache::Cacheable;
use crate::schema::*;
use crate::state::{BotContext, DbContext};
use crate::Result;

/// DB persisted command attributes
#[derive(Serialize, Deserialize, Debug, Clone, Queryable)]
pub struct CommandAttributes {
    pub id: i32,
    /// User facing description
    pub description: Option<String>,
    /// name of the command handler. Used to identify the right handler in the bot.
    pub handler_name: String,
    /// global switch to enable/disable a command
    pub enabled: bool,
    /// whether the command is active by default in all channels
    pub default_active: bool,
    /// minimum time between command uses
    pub cooldown: Option<DurationMillis>,
    /// whether the command can be used in whispers
    pub whisper_enabled: bool,
}

pub type DefaultColumns = (
    command_attributes::id,
    command_attributes::description,
    command_attributes::handler_name,
    command_attributes::enabled,
    command_attributes::default_active,
    command_attributes::cooldown,
    command_attributes::whisper_enabled,
);

impl CommandAttributes {
    pub const COLUMNS: DefaultColumns = (
        command_attributes::id,
        command_attributes::description,
        command_attributes::handler_name,
        command_attributes::enabled,
        command_attributes::default_active,
        command_attributes::cooldown,
        command_attributes::whisper_enabled,
    );
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

fn cooldown_cache_key(handler_name: &str, scope: &str) -> String {
    format!("cb:cooldowns:{}:{}", handler_name, scope)
}

impl CommandAttributes {
    pub async fn reset_cooldown(&self, ctx: &DbContext, scope: &str) -> Result<()> {
        if let Some(cooldown) = &self.cooldown {
            let key = cooldown_cache_key(&self.handler_name, scope);
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
            let key = cooldown_cache_key(&self.handler_name, scope);
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
            command_attributes::table
                .select(CommandAttributes::COLUMNS)
                .load(pg)
                .map_err(Into::into)
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
            .returning(CommandAttributes::COLUMNS)
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
        required_permission_names: Vec<impl AsRef<str> + Send + 'static>,
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

                // insert attributes and aliases
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

                // insert default permissions
                let required_permission_values: Vec<_> = ctx
                    .permissions
                    .load()
                    .get_permissions(required_permission_names.iter().map(|s| s.as_ref()))?
                    .iter()
                    .map(|permission| {
                        (
                            command_permissions::permission_id.eq(permission.id),
                            command_permissions::command_id.eq(attributes.id),
                        )
                    })
                    .collect();
                diesel::insert_into(command_permissions::table)
                    .values(required_permission_values)
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
