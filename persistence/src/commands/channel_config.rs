use std::time::Duration;

use diesel::{ExpressionMethods, QueryDsl, Queryable};
use serde::{Deserialize, Serialize};
use tokio_diesel::{AsyncRunQueryDsl, OptionalExtension};

use crate::cache::Cacheable;
use crate::commands::attributes::DurationMillis;
use crate::impl_redis_bincode_int;
use crate::schema::*;
use crate::DbContext;
use crate::Result;

/// Channel specific command configuration
#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct ChannelCommandConfig {
    pub channel_id: i32,
    pub command_id: i32,
    /// disables or enables the command in this channel if set
    pub active: Option<bool>,
    /// per channel cooldown override
    pub cooldown: Option<DurationMillis>,
}

#[derive(Debug, Serialize, Deserialize, Queryable)]
pub struct ChannelCommandConfigNamed {
    pub channel_id: i32,
    pub channel_name: String,
    pub active: Option<bool>,
    pub cooldown: Option<DurationMillis>,
}

impl ChannelCommandConfig {
    pub async fn get(
        ctx: &DbContext,
        channel_id_value: i32,
        command_id_value: i32,
    ) -> Result<Option<Self>> {
        if let Some(cached_value) =
            Self::cache_get(&ctx.redis_pool, (channel_id_value, command_id_value)).await?
        {
            return Ok(Some(cached_value));
        }

        let config = channel_command_config::table
            .filter(channel_command_config::channel_id.eq(channel_id_value))
            .filter(channel_command_config::command_id.eq(command_id_value))
            .first_async::<ChannelCommandConfig>(&ctx.db_pool)
            .await
            .optional()?;

        if let Some(ref config) = config {
            config.cache_set(&ctx.redis_pool).await?;
        }
        Ok(config)
    }
}

impl_redis_bincode_int!(ChannelCommandConfig);

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
