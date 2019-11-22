use chrono::{DateTime, FixedOffset, Local, Utc};
use diesel::prelude::*;
use r2d2_redis::redis;
use serde::{Deserialize, Serialize};
use tmi_rs::event::tags::*;
use tmi_rs::event::Event;

use crate::cache::Cacheable;
use crate::cerebot::DbContext;
use crate::error::Error;
use crate::schema::users;
use std::sync::Arc;
use std::time::Duration;
use tokio_executor::blocking;

#[derive(Queryable, Serialize, Deserialize)]
pub struct User {
    pub id: i32,
    pub twitch_user_id: i32,
    pub name: String,
    pub display_name: Option<String>,
    pub previous_names: Option<Vec<String>>,
    pub previous_display_names: Option<Vec<String>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl redis::FromRedisValue for User {
    fn from_redis_value(v: &redis::Value) -> Result<Self, redis::RedisError> {
        if let redis::Value::Data(data) = v {
            Ok(bincode::deserialize(&data).map_err(|_| {
                redis::RedisError::from((redis::ErrorKind::TypeError, "Deserialization failed"))
            })?)
        } else {
            Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Unexpected value type returned from Redis",
            )))
        }
    }
}

impl redis::ToRedisArgs for &User {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(&bincode::serialize(self).unwrap());
    }
}

#[derive(Insertable)]
#[table_name = "users"]
pub struct NewTwitchUser<'a> {
    pub twitch_user_id: i32,
    pub name: &'a str,
    pub display_name: Option<&'a str>,
    pub previous_names: Option<Vec<&'a str>>,
    pub previous_display_names: Option<Vec<&'a str>>,
    pub created_at: DateTime<FixedOffset>,
}

pub struct ChatUserInfo<'a> {
    pub twitch_user_id: i32,
    pub name: &'a str,
    pub display_name: Option<&'a str>,
}

impl Cacheable<i32> for User {
    fn cache_key(&self) -> String {
        format!("cb:user:{}", self.twitch_user_id)
    }

    fn cache_key_from_id(id: i32) -> String {
        format!("cb:user:{}", id)
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(60 * 60)
    }
}

fn event_user_id(event: &Event<String>) -> Result<Option<i32>, Error> {
    let id = match event {
        Event::GlobalUserState(data) => data.user_id(),
        Event::UserNotice(data) => data.user_id(),
        Event::PrivMsg(data) => data.user_id(),
        Event::Whisper(data) => data.user_id(),
        _ => {
            return Ok(None);
        }
    }?;
    Ok(Some(id as i32))
}

pub async fn get_or_insert_event_user(
    ctx: &DbContext,
    event: &Arc<Event<String>>,
) -> Result<Option<User>, Error> {
    if let Some(id) = event_user_id(&*event)? {
        let event = event.clone();
        let ctx = ctx.clone();

        blocking::run(move || {
            let redis = &mut *ctx.redis_pool.get()?;
            let pg = &*ctx.db_pool.get()?;

            if let Some(user) = get_user(pg, redis, id)? {
                Ok(Some(user))
            } else if let Some(ref user_info) = event_user_info(&*event)? {
                Ok(Some(insert_user(pg, user_info)?))
            } else {
                Ok(None)
            }
        })
        .await
    } else {
        Ok(None)
    }
}

fn get_user(
    pg: &PgConnection,
    redis: &mut dyn redis::ConnectionLike,
    twitch_id: i32,
) -> Result<Option<User>, Error> {
    if let Ok(cached) = User::cache_get(redis, twitch_id) {
        trace!("Cache hit for user {}", cached.name);
        Ok(Some(cached))
    } else {
        let query_result = users::table
            .filter(users::twitch_user_id.eq(twitch_id))
            .first::<User>(pg);

        match query_result {
            Ok(user) => {
                user.cache_set(redis)?;
                Ok(Some(user))
            }
            Err(diesel::result::Error::NotFound) => Ok(None),
            Err(err) => Err(Error::Database(err)),
        }
    }
}

fn update_user(conn: &PgConnection, user_info: &ChatUserInfo<'_>) -> Result<User, Error> {
    diesel::insert_into(users::table)
        .values(NewTwitchUser {
            twitch_user_id: user_info.twitch_user_id,
            name: user_info.name,
            display_name: user_info.display_name,
            previous_names: None,
            previous_display_names: None,
            created_at: Local::now().into(),
        })
        .get_result(conn)
        .map_err(Into::into)
}

fn insert_user(conn: &PgConnection, user_info: &ChatUserInfo<'_>) -> Result<User, Error> {
    diesel::insert_into(users::table)
        .values(NewTwitchUser {
            twitch_user_id: user_info.twitch_user_id,
            name: user_info.name,
            display_name: user_info.display_name,
            previous_names: None,
            previous_display_names: None,
            created_at: Local::now().into(),
        })
        .get_result(conn)
        .map_err(Into::into)
}

fn event_user_info(event: &Event<String>) -> Result<Option<ChatUserInfo>, Error> {
    Ok(Some(match event {
        Event::UserNotice(data) => ChatUserInfo {
            twitch_user_id: data.user_id()? as i32,
            name: data.login()?,
            display_name: data.display_name(),
        },
        Event::PrivMsg(data) => ChatUserInfo {
            twitch_user_id: data.user_id()? as i32,
            name: data.sender().as_ref().expect("privmsg without sender"),
            display_name: data.display_name(),
        },
        Event::Whisper(data) => ChatUserInfo {
            twitch_user_id: data.user_id()? as i32,
            name: data.sender().as_ref().expect("whisper without sender"),
            display_name: data.display_name(),
        },
        _ => {
            return Ok(None);
        }
    }))
}
