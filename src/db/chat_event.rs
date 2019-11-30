use std::io::Write;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

use chrono::{DateTime, FixedOffset, Utc};
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::serialize::{Output, ToSql};
use diesel::sql_types::Jsonb;
use diesel_derive_enum::DbEnum;
use fnv::FnvHashMap;
use r2d2_redis::redis;
use r2d2_redis::redis::PipelineCommands;
use serde::{Deserialize, Serialize};
use tmi_rs::event::tags::*;
use tmi_rs::event::*;
use tmi_rs::irc_constants::RPL_ENDOFMOTD;
use tokio::task;
use uuid::Uuid;

use crate::db::{get_channel, User};
use crate::error::Error;
use crate::schema::chat_events;
use crate::state::DbContext;

#[derive(DbEnum, Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChatEventType {
    Privmsg,
    Whisper,
    Notice,
    Usernotice,
    Host,
    Clearchat,
    Clearmsg,
    Roomstate,
    Connect,
}

#[derive(Queryable)]
pub struct ChatEvent {
    pub id: i64,
    pub event_type: ChatEventType,
    pub twitch_message_id: Option<uuid::Uuid>,
    pub message: Option<String>,
    pub channel_id: Option<i32>,
    pub sender_user_id: Option<i32>,
    pub tags: Option<Tags>,
    pub received_at: DateTime<Utc>,
}

#[derive(Insertable, Serialize, Deserialize, Debug, PartialEq)]
#[table_name = "chat_events"]
pub struct NewChatEvent {
    pub event_type: ChatEventType,
    pub twitch_message_id: Option<uuid::Uuid>,
    pub message: Option<String>,
    pub channel_id: Option<i32>,
    pub sender_user_id: Option<i32>,
    pub tags: Option<Tags>,
    pub received_at: DateTime<FixedOffset>,
}

impl redis::FromRedisValue for NewChatEvent {
    fn from_redis_value(v: &redis::Value) -> Result<Self, redis::RedisError> {
        if let redis::Value::Data(data) = v {
            Ok(
                bincode::deserialize::<'_, NewChatEvent>(&data).map_err(|_| {
                    redis::RedisError::from((redis::ErrorKind::TypeError, "Deserialization failed"))
                })?,
            )
        } else {
            Err(redis::RedisError::from((
                redis::ErrorKind::TypeError,
                "Unexpected value type returned from Redis",
            )))
        }
    }
}

impl redis::ToRedisArgs for &NewChatEvent {
    fn write_redis_args<W>(&self, out: &mut W)
    where
        W: ?Sized + redis::RedisWrite,
    {
        out.write_arg(&bincode::serialize(*self).unwrap());
    }
}

/// Convert any chat event into a db entry and save the db entry in the log queue, to
/// be persisted into the database at a later time
pub async fn log_event(ctx: &DbContext, event: &Arc<Event<String>>) -> Result<(), Error> {
    let ctx = ctx.clone();
    let user_id = User::get_or_insert(&ctx, event).await?.map(|u| u.id);

    let db_entry = match &**event {
        Event::PrivMsg(data) => Some(NewChatEvent {
            event_type: ChatEventType::Privmsg,
            twitch_message_id: Uuid::from_str(data.id()?).ok(),
            message: Some(data.message().clone()),
            channel_id: get_channel(&ctx, data.channel()).await?.map(|c| c.id),
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::Whisper(data) => Some(NewChatEvent {
            event_type: ChatEventType::Whisper,
            twitch_message_id: None,
            message: Some(data.message().clone()),
            channel_id: None,
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::Notice(data) => Some(NewChatEvent {
            event_type: ChatEventType::Notice,
            twitch_message_id: None,
            message: Some(data.message().clone()),
            channel_id: get_channel(&ctx, data.channel()).await?.map(|c| c.id),
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::UserNotice(data) => Some(NewChatEvent {
            event_type: ChatEventType::Usernotice,
            twitch_message_id: Uuid::from_str(data.id()?).ok(),
            message: Some(data.message().clone()),
            channel_id: get_channel(&ctx, data.channel()).await?.map(|c| c.id),
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::Host(data) => Some(NewChatEvent {
            event_type: ChatEventType::Host,
            twitch_message_id: None,
            message: None,
            channel_id: get_channel(&ctx, data.hosting_channel())
                .await?
                .map(|c| c.id),
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::ClearChat(data) => Some(NewChatEvent {
            event_type: ChatEventType::Clearchat,
            twitch_message_id: None,
            message: None,
            channel_id: get_channel(&ctx, data.channel()).await?.map(|c| c.id),
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::ClearMsg(data) => Some(NewChatEvent {
            event_type: ChatEventType::Clearmsg,
            twitch_message_id: None,
            message: Some(data.message().clone()),
            channel_id: get_channel(&ctx, data.channel()).await?.map(|c| c.id),
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::RoomState(data) => Some(NewChatEvent {
            event_type: ChatEventType::Roomstate,
            twitch_message_id: None,
            message: None,
            channel_id: get_channel(&ctx, data.channel()).await?.map(|c| c.id),
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        Event::ConnectMessage(data) if data.command() == RPL_ENDOFMOTD => Some(NewChatEvent {
            event_type: ChatEventType::Connect,
            twitch_message_id: None,
            message: None,
            channel_id: None,
            sender_user_id: user_id,
            tags: data.tags().clone().map(Into::into),
            received_at: chrono::Local::now().into(),
        }),
        _ => None,
    };

    if let Some(db_entry) = db_entry {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let conn = &mut *ctx.redis_pool.get()?;
            redis::pipe()
                .rpush("cb:persist_event_queue", &db_entry)
                .query(conn)
                .map_err(Into::into)
        })
        .await?
    } else {
        Ok(())
    }
}

/// Get all queued log events from redis and save them to the database in a batch
pub async fn persist_event_queue(ctx: &DbContext) -> Result<(), Error> {
    let ctx = ctx.clone();
    task::spawn_blocking(move || {
        let pg_conn = &*ctx.db_pool.get()?;
        let redis_conn = &mut *ctx.redis_pool.get()?;
        let (queued_events, _) = redis::pipe()
            .lrange("cb:persist_event_queue", 0, -1)
            .del("cb:persist_event_queue")
            .query::<(Vec<NewChatEvent>, redis::Value)>(redis_conn)?;

        diesel::insert_into(chat_events::table)
            .values(queued_events)
            .execute(pg_conn)?;
        Ok(())
    })
    .await?
}

#[derive(FromSqlRow, AsExpression, Debug, Serialize, Deserialize, PartialEq)]
#[sql_type = "Jsonb"]
pub struct Tags(FnvHashMap<String, String>);

impl From<FnvHashMap<String, String>> for Tags {
    fn from(map: FnvHashMap<String, String>) -> Self {
        Tags(map)
    }
}

impl Tags {
    pub fn into_map(self) -> FnvHashMap<String, String> {
        self.0
    }
}

impl Deref for Tags {
    type Target = FnvHashMap<String, String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromSql<Jsonb, Pg> for Tags {
    fn from_sql(bytes: Option<&[u8]>) -> diesel::deserialize::Result<Self> {
        let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
        Ok(Tags(serde_json::from_value::<FnvHashMap<String, String>>(
            value,
        )?))
    }
}

impl ToSql<Jsonb, Pg> for Tags {
    fn to_sql<W: Write>(&self, out: &mut Output<W, Pg>) -> diesel::serialize::Result {
        let value = serde_json::to_value(&self.0)?;
        <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&value, out)
    }
}
