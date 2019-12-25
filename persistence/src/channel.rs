use std::borrow::Cow;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use tokio_diesel::{AsyncRunQueryDsl, OptionalExtension};

use crate::schema::channels;
use crate::DbContext;
use crate::DbPool;
use crate::Result;

#[derive(Queryable, Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Channel {
    pub id: i32,
    pub twitch_room_id: Option<i32>,
    pub name: String,
    pub join_on_start: bool,
    pub command_prefix: Option<String>,
    pub updated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub silent: bool,
}

#[derive(Insertable, AsChangeset, Clone, Debug)]
#[table_name = "channels"]
pub struct UpdateChannelId<'a> {
    pub twitch_room_id: Option<i32>,
    pub name: Cow<'a, str>,
}

#[derive(AsChangeset, Debug, Clone)]
#[table_name = "channels"]
pub struct UpdateChannelSettings {
    pub join_on_start: Option<bool>,
    #[allow(clippy::option_option)]
    pub command_prefix: Option<Option<String>>,
    pub silent: Option<bool>,
}

#[derive(Insertable, Debug)]
#[table_name = "channels"]
pub struct InsertChannel {
    pub twitch_room_id: Option<i32>,
    pub name: String,
    pub join_on_start: Option<bool>,
    pub command_prefix: Option<String>,
    pub silent: Option<bool>,
}

impl Channel {
    /// Get a channel by its name
    pub async fn get(ctx: &DbContext, channel_name: &str) -> Result<Option<Channel>> {
        let channel_name = channel_name.to_owned();
        channels::table
            .filter(channels::name.eq(channel_name))
            .first_async::<Channel>(&ctx.db_pool)
            .await
            .optional()
            .map_err(Into::into)
    }

    /// Get a channel by the information received with the roomstate event or update the channel in
    /// the database. Inserts if not found, updates the Twitch room ID if not set in the database.
    pub async fn get_or_persist_roomstate(
        ctx: &DbContext,
        channel_values: UpdateChannelId<'static>,
    ) -> Result<Channel> {
        if let Some(channel) = Self::get(ctx, &channel_values.name).await? {
            // update if twitch room id is missing in DB, for example on first join after creating
            // the channel in the database
            if channel.twitch_room_id.is_none() && channel_values.twitch_room_id.is_some() {
                diesel::update(channels::table.filter(channels::name.eq(channel_values.name)))
                    .set(channels::twitch_room_id.eq(channel_values.twitch_room_id.unwrap()))
                    .get_result_async(&ctx.db_pool)
                    .await
                    .map_err(Into::into)
            } else {
                Ok(channel)
            }
        } else {
            // insert into DB if not found
            let inserted_channel = diesel::insert_into(channels::table)
                .values(channel_values)
                .get_result_async::<Channel>(&ctx.db_pool)
                .await?;
            Ok(inserted_channel)
        }
    }

    /// Update a channel's settings
    pub async fn update_settings(
        ctx: &DbContext,
        channel_name: impl Into<String>,
        updated_settings: UpdateChannelSettings,
    ) -> Result<Channel> {
        let channel_name = channel_name.into();

        // update the database
        let updated_channel =
            diesel::update(channels::table.filter(channels::name.eq(channel_name)))
                .set(updated_settings)
                .get_result_async::<Channel>(&ctx.db_pool)
                .await?;

        Ok(updated_channel)
    }

    pub async fn create_channel(ctx: &DbContext, values: InsertChannel) -> Result<Channel> {
        let inserted_channel = diesel::insert_into(channels::table)
            .values(values)
            .get_result_async::<Channel>(&ctx.db_pool)
            .await?;

        Ok(inserted_channel)
    }

    pub async fn get_startup_channels(pool: &DbPool) -> Result<Vec<Channel>> {
        channels::table
            .filter(channels::join_on_start.eq(true))
            .load_async::<Channel>(pool)
            .await
            .map_err(Into::into)
    }
}
