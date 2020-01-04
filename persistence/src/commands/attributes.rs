use std::borrow::Cow;
use std::convert::TryInto;
use std::ops::Deref;
use std::time::Duration;

use diesel::backend::Backend;
use diesel::deserialize::FromSql;
use diesel::prelude::*;
use diesel::sql_types::{Array, Integer, Text};
use serde::{Deserialize, Serialize};
use tokio_diesel::AsyncRunQueryDsl;

use crate::cache::Cacheable;
use crate::commands::channel_config::ChannelCommandConfigNamed;
use crate::commands::templates::CommandTemplate;
use crate::schema::*;
use crate::Result;
use crate::{impl_redis_bincode, OffsetParameters};
use crate::{DbPool, Error, RedisPool};
use diesel::dsl::count;
use diesel::sql_query;

/// DB persisted command attributes
#[derive(Serialize, Deserialize, Debug, Clone, Queryable, QueryableByName)]
#[table_name = "command_attributes"]
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

fn cooldown_cache_key(command_id: i32, scope: &str) -> String {
    format!("cb:cooldowns:cmd:{}:{}", command_id, scope)
}

#[derive(Debug, QueryableByName)]
pub struct CommandWithAliases {
    #[diesel(embed)]
    pub attributes: CommandAttributes,
    #[sql_type = "Array<Text>"]
    pub aliases: Vec<String>,
}

#[derive(Debug, QueryableByName)]
pub struct CommandDetails {
    #[diesel(embed)]
    pub attributes: CommandAttributes,
    #[sql_type = "Array<Text>"]
    pub aliases: Vec<String>,
    #[diesel(embed)]
    pub template: CommandTemplate,
}

impl CommandAttributes {
    pub async fn reset_cooldown(
        &self,
        pool: &RedisPool,
        scope: &str,
        cooldown_override: Option<Duration>,
    ) -> Result<()> {
        let cooldown = cooldown_override
            .as_ref()
            .or_else(|| self.cooldown.as_deref());
        if let Some(&cooldown) = cooldown {
            let key = cooldown_cache_key(self.id, scope);
            pool.get()
                .await
                .set_and_expire_ms(
                    key,
                    b"1",
                    cooldown
                        .as_millis()
                        .try_into()
                        .map_err(Error::InvalidRedisExpiry)?,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn check_cooldown(
        &self,
        pool: &RedisPool,
        scope: &str,
        cooldown_override: Option<Duration>,
    ) -> Result<bool> {
        let cooldown = cooldown_override
            .as_ref()
            .or_else(|| self.cooldown.as_deref());
        if cooldown.is_some() {
            let key = cooldown_cache_key(self.id, scope);
            Ok(!pool.get().await.exists(key).await?)
        } else {
            Ok(true)
        }
    }

    pub async fn list_all(pool: &DbPool) -> Result<Vec<CommandAttributes>> {
        command_attributes::table
            .select(CommandAttributes::COLUMNS)
            .load_async(pool)
            .await
            .map_err(Into::into)
    }

    pub async fn list_with_aliases(
        pool: &DbPool,
        slice: OffsetParameters,
    ) -> Result<(u64, Vec<CommandWithAliases>)> {
        let items = sql_query(
            r#"select
a.id, a.description, a.enabled, a.default_active, a.cooldown,
a.whisper_enabled, a.handler_name,
array_agg(ca.name order by length(ca.name)) aliases
from command_attributes a
left join command_aliases ca on a.id = ca.command_id
where ca.name is not null
group by a.id
order by aliases
offset $1 limit $2"#,
        )
        .bind::<Integer, _>(slice.offset() as i32)
        .bind::<Integer, _>(slice.limit() as i32)
        .load_async::<CommandWithAliases>(pool)
        .await?;

        let total: i64 = command_attributes::table
            .select(count(command_attributes::id))
            .first_async(pool)
            .await?;

        Ok((total as u64, items))
    }

    pub async fn get_detailed(
        pool: &DbPool,
        command_id: i32,
    ) -> Result<(CommandDetails, Vec<ChannelCommandConfigNamed>)> {
        let command = sql_query(
            r#"select
a.id, a.description, a.enabled, a.default_active, a.cooldown,
a.whisper_enabled, a.handler_name, a.template, a.template_context,
array_agg(ca.name order by length(ca.name)) aliases
from command_attributes a
left join command_aliases ca on a.id = ca.command_id
where a.id = $1
group by a.id"#,
        )
        .bind::<Integer, _>(command_id)
        .get_result_async::<CommandDetails>(pool)
        .await?;

        let channel_configs = channel_command_config::table
            .inner_join(channels::table.on(channels::id.eq(channel_command_config::channel_id)))
            .select((
                channels::id,
                channels::name,
                channel_command_config::active,
                channel_command_config::cooldown,
            ))
            .filter(channel_command_config::command_id.eq(command_id))
            .load_async::<ChannelCommandConfigNamed>(pool)
            .await?;
        Ok((command, channel_configs))
    }

    pub async fn insert(
        pool: &DbPool,
        data: InsertCommandAttributes<'static>,
    ) -> Result<CommandAttributes> {
        diesel::insert_into(command_attributes::table)
            .values(data)
            .returning(CommandAttributes::COLUMNS)
            .get_result_async(pool)
            .await
            .map_err(Into::into)
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
