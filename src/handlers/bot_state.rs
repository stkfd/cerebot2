use std::pin::Pin;
use std::sync::Arc;

use futures::future::ready;
use futures::Future;
use tmi_rs::event::tags::*;
use tmi_rs::event::*;

use crate::db::{get_or_save_channel, UpdateChannel};
use crate::dispatch::{ok_fut, EventHandler, Response};
use crate::error::Error;
use crate::state::{BotContext, ChannelInfo, ChannelState};

pub struct BotStateHandler {
    ctx: BotContext,
}

impl EventHandler for BotStateHandler {
    fn create(ctx: &BotContext) -> Pin<Box<dyn Future<Output = Self>>>
    where
        Self: Sized,
    {
        Box::pin(ready(BotStateHandler {
            ctx: (*ctx).clone(),
        }))
    }

    fn run(&self, event: &Arc<Event<String>>) -> Result<Response, Error> {
        let ctx = self.ctx.clone();
        let event = event.clone();
        ok_fut(async move {
            match &*event {
                Event::Reconnect(_) => {
                    // mark for restart on next message
                    ctx.restart();
                }
                Event::RoomState(data) => {
                    let channel = get_or_save_channel(
                        &ctx.db_context,
                        UpdateChannel {
                            twitch_room_id: Some(data.room_id()? as i32),
                            name: data.channel().clone().into(),
                        },
                    )
                    .await?;
                    ctx.update_channel(ChannelInfo {
                        data: channel,
                        state: Some(ChannelState {
                            slow: data.slow(),
                            followers_only: data.followers_only(),
                            subs_only: data.subs_only(),
                            r9k: data.r9k(),
                            emote_only: data.emote_only(),
                        }),
                    });
                }
                Event::PrivMsg(data) => {
                    info!("{}: {}", data.sender().as_ref().unwrap(), data.message());
                }
                _ => {}
            }
            Ok(())
        })
    }
}
