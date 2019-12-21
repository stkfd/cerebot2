use std::borrow::Cow;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::error::Error;
use crate::schema::channels;
use crate::state::{BotContext, ChannelInfo, DbContext};
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
        let ctx = ctx.clone();

        task::spawn_blocking(move || Self::get_blocking(&ctx, &channel_name)).await?
    }

    fn get_blocking(ctx: &DbContext, channel_name: &str) -> Result<Option<Channel>> {
        let pg = &*ctx.db_pool.get()?;
        let query_result = channels::table
            .filter(channels::name.eq(channel_name))
            .first::<Channel>(pg);

        match query_result {
            Ok(channel) => Ok(Some(channel)),
            Err(diesel::result::Error::NotFound) => Ok(None),
            Err(err) => Err(Error::Database(err)),
        }
    }

    /// Get a channel by the information received with the roomstate event or update the channel in
    /// the database. Inserts if not found, updates the Twitch room ID if not set in the database.
    pub async fn get_or_persist_roomstate(
        ctx: &DbContext,
        channel_values: UpdateChannelId<'static>,
    ) -> Result<Channel> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            if let Some(channel) = Self::get_blocking(&ctx, &channel_values.name)? {
                // update if twitch room id is missing in DB, for example on first join after creating
                // the channel in the database
                if channel.twitch_room_id.is_none() && channel_values.twitch_room_id.is_some() {
                    let pg = &*ctx.db_pool.get()?;

                    diesel::update(channels::table.filter(channels::name.eq(&channel_values.name)))
                        .set(channels::twitch_room_id.eq(channel_values.twitch_room_id.unwrap()))
                        .get_result(pg)
                        .map_err(Into::into)
                } else {
                    Ok(channel)
                }
            } else {
                // insert into DB if not found
                let pg_conn = &*ctx.db_pool.get()?;
                let inserted_channel = diesel::insert_into(channels::table)
                    .values(&channel_values)
                    .get_result::<Channel>(pg_conn)?;
                Ok(inserted_channel)
            }
        })
        .await?
    }

    /// Update a channel's settings
    pub async fn update_settings(
        ctx: &BotContext,
        channel_info: &Arc<ChannelInfo>,
        updated_settings: UpdateChannelSettings,
    ) -> Result<()> {
        let ctx_clone = ctx.clone();
        let ChannelInfo {
            state,
            data: channel_data,
        } = (&**channel_info).clone();

        let updated_channel = task::spawn_blocking(move || {
            let pg = &*ctx_clone.db_context.db_pool.get()?;

            // update the database
            let updated_channel =
                diesel::update(channels::table.filter(channels::name.eq(&channel_data.name)))
                    .set(updated_settings)
                    .get_result::<Channel>(pg)?;

            Result::Ok(updated_channel)
        })
        .await??;

        // update the bot's internal channel map
        ctx.update_channel(ChannelInfo {
            data: updated_channel,
            state,
        })
        .await;

        Ok(())
    }

    pub async fn create_channel(ctx: &BotContext, values: InsertChannel) -> Result<()> {
        let ctx_clone = ctx.clone();

        let inserted_channel = task::spawn_blocking(move || {
            let pg = &*ctx_clone.db_context.db_pool.get()?;

            let inserted_channel = diesel::insert_into(channels::table)
                .values(values)
                .get_result::<Channel>(pg)?;

            Result::Ok(inserted_channel)
        })
        .await??;

        // update the bot's internal channel map
        ctx.update_channel(ChannelInfo {
            data: inserted_channel,
            state: None,
        })
        .await;

        Ok(())
    }
}
