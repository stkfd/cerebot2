use std::time::Duration;

use chrono::{DateTime, FixedOffset, Local, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use tokio_diesel::AsyncRunQueryDsl;

use crate::cache::Cacheable;
use crate::impl_redis_bincode_int;
use crate::schema::users;
use crate::DbContext;
use crate::Result;
use crate::{DbPool, Error};

#[derive(Queryable, Serialize, Deserialize, Debug)]
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

impl_redis_bincode_int!(User);

#[derive(Insertable, Debug)]
#[table_name = "users"]
pub struct NewTwitchUser {
    pub twitch_user_id: i32,
    pub name: String,
    pub display_name: Option<String>,
    pub previous_names: Option<Vec<String>>,
    pub previous_display_names: Option<Vec<String>>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(AsChangeset, Debug)]
#[table_name = "users"]
pub struct UpdateTwitchUser {
    pub twitch_user_id: i32,
    pub name: String,
    pub display_name: Option<String>,
    pub previous_names: Option<Vec<String>>,
    pub previous_display_names: Option<Vec<String>>,
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

pub struct OwnedChatUserInfo {
    pub twitch_user_id: i32,
    pub name: String,
    pub display_name: Option<String>,
}

impl From<&ChatUserInfo<'_>> for OwnedChatUserInfo {
    fn from(source: &ChatUserInfo<'_>) -> Self {
        OwnedChatUserInfo {
            twitch_user_id: source.twitch_user_id,
            name: source.name.to_string(),
            display_name: source.display_name.map(|s| s.to_string()),
        }
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
    pub async fn get_or_insert(ctx: &DbContext, user_info: ChatUserInfo<'_>) -> Result<User> {
        let user = Self::get(ctx, user_info.twitch_user_id).await;
        if let Ok(user) = user {
            if !user_info.data_matches(&user) {
                let updated = Self::update(ctx, &user_info).await?;
                Ok(updated)
            } else {
                Ok(user)
            }
        } else if let Err(Error::NotFound) = user {
            Ok(Self::insert(ctx, &user_info).await?)
        } else {
            user
        }
    }

    pub async fn get(ctx: &DbContext, twitch_id: i32) -> Result<User> {
        if let Some(cached) = User::cache_get(&ctx.redis_pool, twitch_id).await? {
            trace!("Cache hit for user {}", cached.name);
            Ok(cached)
        } else {
            let query_result = Self::get_no_cache(&ctx.db_pool, twitch_id).await;
            if let Ok(user) = &query_result {
                user.cache_set(&ctx.redis_pool).await?;
            }

            query_result
        }
    }

    async fn get_no_cache(pool: &DbPool, twitch_id: i32) -> Result<User> {
        users::table
            .filter(users::twitch_user_id.eq(twitch_id))
            .first_async::<User>(pool)
            .await
            .map_err(Into::into)
    }

    async fn update(ctx: &DbContext, user_info: &ChatUserInfo<'_>) -> Result<User> {
        let User {
            name,
            display_name,
            mut previous_names,
            mut previous_display_names,
            ..
        } = Self::get_no_cache(&ctx.db_pool, user_info.twitch_user_id).await?;

        if user_info.name != name {
            previous_names
                .get_or_insert_with(|| vec![])
                .push(name.clone());
        }

        if display_name.as_ref().map(String::as_str) != user_info.display_name {
            if let Some(display_name) = display_name {
                previous_display_names
                    .get_or_insert_with(|| vec![])
                    .push(display_name);
            }
        }

        let user_info: OwnedChatUserInfo = user_info.into();

        let updated_user =
            diesel::update(users::table.filter(users::twitch_user_id.eq(user_info.twitch_user_id)))
                .set(UpdateTwitchUser {
                    twitch_user_id: user_info.twitch_user_id,
                    name: user_info.name,
                    display_name: user_info.display_name,
                    previous_names,
                    previous_display_names,
                })
                .get_result_async::<User>(&ctx.db_pool)
                .await?;

        updated_user.cache_set(&ctx.redis_pool).await?;
        Ok(updated_user)
    }

    async fn insert(ctx: &DbContext, user_info: &ChatUserInfo<'_>) -> Result<User> {
        let user_info: OwnedChatUserInfo = user_info.into();
        diesel::insert_into(users::table)
            .values(NewTwitchUser {
                twitch_user_id: user_info.twitch_user_id,
                name: user_info.name,
                display_name: user_info.display_name,
                previous_names: None,
                previous_display_names: None,
                created_at: Local::now().into(),
            })
            .get_result_async(&ctx.db_pool)
            .await
            .map_err(Into::into)
    }
}
