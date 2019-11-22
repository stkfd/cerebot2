use std::borrow::Cow;

use chrono::{DateTime, FixedOffset, Utc};
use diesel::prelude::*;
use r2d2_redis::redis;
use r2d2_redis::redis::RedisError;
use serde::{Deserialize, Serialize};
use tokio_executor::blocking;

use crate::cache::Cacheable;
use crate::cerebot::DbContext;
use crate::error::Error;
use crate::schema::channels;
use std::time::Duration;

#[derive(Queryable, Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Channel {
    pub id: i32,
    pub twitch_room_id: Option<i32>,
    pub name: String,
    pub join_on_start: bool,
    pub command_prefix: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl Cacheable<&str> for Channel {
    fn cache_key(&self) -> String {
        format!("cb:channel:{}", self.name)
    }

    /// Channel name is used as cache ID here
    fn cache_key_from_id(name: &str) -> String {
        format!("cb:channel:{}", name)
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }
}

#[derive(Insertable, AsChangeset, Clone)]
#[table_name = "channels"]
pub struct NewChannel {
    pub twitch_room_id: Option<i32>,
    pub name: Cow<'static, str>,
    pub join_on_start: bool,
    pub command_prefix: Option<Cow<'static, str>>,
    pub created_at: DateTime<FixedOffset>,
}

impl redis::FromRedisValue for Channel {
    fn from_redis_value(v: &redis::Value) -> Result<Self, RedisError> {
        if let redis::Value::Data(data) = v {
            Ok(bincode::deserialize(&data).map_err(|_| {
                RedisError::from((redis::ErrorKind::TypeError, "Deserialization failed"))
            })?)
        } else {
            Err(RedisError::from((
                redis::ErrorKind::TypeError,
                "Unexpected value type returned from Redis",
            )))
        }
    }
}

impl redis::ToRedisArgs for &Channel {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(&bincode::serialize(self).unwrap());
    }
}

pub async fn get_channel(ctx: &DbContext, channel_name: &str) -> Result<Option<Channel>, Error> {
    let channel_name = channel_name.to_owned();
    let ctx = ctx.clone();

    blocking::run(move || {
        let pg = &*ctx.db_pool.get()?;
        let redis = &mut *ctx.redis_pool.get()?;

        let cached = Channel::cache_get(redis, &channel_name);
        if let Ok(cached) = cached {
            trace!("Cache hit for channel {}", channel_name);
            Ok(Some(cached))
        } else {
            let query_result = channels::table
                .filter(channels::name.eq(channel_name))
                .first::<Channel>(pg);

            match query_result {
                Ok(channel) => {
                    channel.cache_set(redis)?;
                    Ok(Some(channel))
                }
                Err(diesel::result::Error::NotFound) => Ok(None),
                Err(err) => Err(Error::Database(err)),
            }
        }
    })
    .await
}

pub async fn get_or_save_channel(
    ctx: &DbContext,
    channel_values: NewChannel,
) -> Result<Channel, Error> {
    if let Some(channel) = get_channel(&ctx, &channel_values.name).await? {
        Ok(channel)
    } else {
        let ctx = ctx.clone();
        blocking::run(move || {
            let pg_conn = ctx.db_pool.get()?;
            let mut redis_conn = ctx.redis_pool.get()?;
            let inserted_channel = diesel::insert_into(channels::table)
                .values(&channel_values)
                .get_result::<Channel>(&pg_conn)?;
            inserted_channel.cache_set(&mut *redis_conn)?;

            Ok(inserted_channel)
        })
        .await
    }
}
