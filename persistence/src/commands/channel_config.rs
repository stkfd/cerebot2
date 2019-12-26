use std::time::Duration;

use diesel::backend::Backend;
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use tokio_diesel::{AsyncRunQueryDsl, OptionalExtension};

use crate::cache::Cacheable;
use crate::impl_redis_bincode;
use crate::DbContext;
use crate::Result;

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
        ctx: &DbContext,
        channel_id_value: i32,
        command_id_value: i32,
    ) -> Result<Option<Self>> {
        use crate::schema::channel_command_config::dsl::*;

        if let Some(cached_value) =
            Self::cache_get(&ctx.redis_pool, (channel_id_value, command_id_value)).await?
        {
            return Ok(Some(cached_value));
        }

        let config = channel_command_config
            .filter(channel_id.eq(channel_id_value))
            .first_async::<ChannelCommandConfig>(&ctx.db_pool)
            .await
            .optional()?;

        if let Some(ref config) = config {
            config.cache_set(&ctx.redis_pool).await?;
        }
        Ok(config)
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
