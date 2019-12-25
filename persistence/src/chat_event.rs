use std::io::Write;
use std::ops::Deref;

use chrono::{DateTime, FixedOffset, Utc};
use diesel::deserialize::FromSql;
use diesel::pg::Pg;
use diesel::serialize::{Output, ToSql};
use diesel::sql_types::Jsonb;
use diesel_derive_enum::DbEnum;
use fnv::FnvHashMap;
use r2d2_redis::redis;
use r2d2_redis::redis::PipelineCommands;
use serde::{Deserialize, Serialize};
use tokio_diesel::AsyncRunQueryDsl;

use crate::schema::chat_events;
use crate::Result;
use crate::{with_redis, DbContext};

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

impl_redis_bincode!(NewChatEvent);

/// Convert any chat event into a db entry and save the db entry in the log queue, to
/// be persisted into the database at a later time
pub async fn log_event(ctx: &DbContext, event: NewChatEvent) -> Result<()> {
    with_redis(&ctx.redis_pool, move |conn| {
        redis::pipe()
            .rpush("cb:persist_event_queue", &event)
            .query(conn)
            .map_err(Into::into)
    })
    .await
}

/// Get all queued log events from redis and save them to the database in a batch
pub async fn persist_event_queue(ctx: &DbContext) -> Result<()> {
    let (queued_events, _) = with_redis(&ctx.redis_pool, move |rd| {
        redis::pipe()
            .lrange("cb:persist_event_queue", 0, -1)
            .del("cb:persist_event_queue")
            .query::<(Vec<NewChatEvent>, redis::Value)>(rd)
            .map_err(Into::into)
    })
    .await?;

    diesel::insert_into(chat_events::table)
        .values(queued_events)
        .execute_async(&ctx.db_pool)
        .await?;

    Ok(())
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
