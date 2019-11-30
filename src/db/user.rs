use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local, Utc};
use diesel::prelude::*;
use r2d2_redis::redis;
use serde::{Deserialize, Serialize};
use tmi_rs::event::tags::*;
use tmi_rs::event::Event;
use tokio::task;

use crate::cache::Cacheable;
use crate::error::Error;
use crate::schema::users;
use crate::state::DbContext;

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

impl_redis_bincode!(User);

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

#[derive(AsChangeset)]
#[table_name = "users"]
pub struct UpdateTwitchUser<'a> {
    pub twitch_user_id: i32,
    pub name: &'a str,
    pub display_name: Option<&'a str>,
    pub previous_names: Option<Vec<&'a str>>,
    pub previous_display_names: Option<Vec<&'a str>>,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ChatUserInfo<'a> {
    pub twitch_user_id: i32,
    pub name: &'a str,
    pub display_name: Option<&'a str>,
}

impl ChatUserInfo<'_> {
    pub fn data_matches(&self, user: &User) -> bool {
        self.name == user.name
            && self.display_name == user.display_name.as_ref().map(|s| s.as_str())
    }
}

impl Cacheable<i32> for User {
    fn cache_key(&self) -> String {
        format!("cb:user:{}", self.twitch_user_id)
    }

    fn cache_key_from_id(id: i32) -> String {
        format!("cb:user:{}", id)
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(60 * 10)
    }
}

impl User {
    pub async fn get_or_insert(
        ctx: &DbContext,
        event: &Arc<Event<String>>,
    ) -> Result<Option<User>, Error> {
        if event_has_user_info(&**event) {
            let event = event.clone();
            let ctx = ctx.clone();

            task::spawn_blocking(move || {
                let pg = &*ctx.db_pool.get()?;
                let redis = &mut *ctx.redis_pool.get()?;
                let user_info = event_user_info(&*event)?.unwrap();

                if let Some(user) = Self::get_blocking(pg, redis, user_info.twitch_user_id)? {
                    if !user_info.data_matches(&user) {
                        Ok(Some(Self::update(pg, redis, &user_info)?))
                    } else {
                        Ok(Some(user))
                    }
                } else {
                    Ok(Some(Self::insert(pg, &user_info)?))
                }
            })
                .await?
        } else {
            Ok(None)
        }
    }

    fn get_blocking(
        pg: &PgConnection,
        redis: &mut dyn redis::ConnectionLike,
        twitch_id: i32,
    ) -> Result<Option<User>, Error> {
        if let Ok(cached) = User::cache_get(redis, twitch_id) {
            trace!("Cache hit for user {}", cached.name);
            Ok(Some(cached))
        } else {
            let query_result = Self::get_no_cache(pg, twitch_id);
            if let Ok(Some(user)) = &query_result {
                user.cache_set(redis)?;
            }

            query_result
        }
    }

    fn get_no_cache(pg: &PgConnection, twitch_id: i32) -> Result<Option<User>, Error> {
        let query_result = users::table
            .filter(users::twitch_user_id.eq(twitch_id))
            .first::<User>(pg);

        match query_result {
            Ok(user) => Ok(Some(user)),
            Err(diesel::result::Error::NotFound) => Ok(None),
            Err(err) => Err(Error::Database(err)),
        }
    }

    fn update(
        conn: &PgConnection,
        redis: &mut dyn redis::ConnectionLike,
        user_info: &ChatUserInfo<'_>,
    ) -> Result<User, Error> {
        let user = Self::get_no_cache(conn, user_info.twitch_user_id)?
            .ok_or_else(|| Error::UserNotFound(user_info.twitch_user_id))?;

        let mut previous_names: Vec<&str> = user
            .previous_names
            .as_ref()
            .map(|names| names.iter().map(AsRef::as_ref).collect())
            .unwrap_or_else(|| vec![]);
        if user_info.name != user.name {
            previous_names.push(&user.name);
        }

        let mut previous_display_names: Vec<&str> = user
            .previous_display_names
            .as_ref()
            .map(|names| names.iter().map(AsRef::as_ref).collect())
            .unwrap_or_else(|| vec![]);
        if user.display_name.as_ref().map(AsRef::as_ref) != user_info.display_name {
            previous_display_names.push(&user.display_name.as_ref().unwrap());
        }

        let updated_user =
            diesel::update(users::table.filter(users::twitch_user_id.eq(user_info.twitch_user_id)))
                .set(UpdateTwitchUser {
                    twitch_user_id: user_info.twitch_user_id,
                    name: user_info.name,
                    display_name: user_info.display_name,
                    previous_names: Some(previous_names),
                    previous_display_names: Some(previous_display_names),
                })
                .get_result::<User>(conn)?;

        updated_user.cache_set(redis)?;
        Ok(updated_user)
    }

    fn insert(conn: &PgConnection, user_info: &ChatUserInfo<'_>) -> Result<User, Error> {
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
}

fn event_has_user_info(event: &Event<String>) -> bool {
    match event {
        Event::UserNotice(_) | Event::PrivMsg(_) | Event::Whisper(_) => true,
        _ => false,
    }
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
