use tmi_rs::event::tags::*;
use tmi_rs::event::*;

use async_trait::async_trait;

use crate::db::{Channel, UpdateChannelId};
use crate::dispatch::EventHandler;
use crate::event::CbEvent;
use crate::state::{BotContext, ChannelInfo, ChannelState};
use crate::Result;

#[derive(Debug)]
pub struct BotStateHandler {
    ctx: BotContext,
}

#[async_trait]
impl EventHandler<CbEvent> for BotStateHandler {
    async fn create(ctx: &BotContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(BotStateHandler {
            ctx: (*ctx).clone(),
        })
    }

    async fn run(&self, event: &CbEvent) -> Result<()> {
        let ctx = &self.ctx;
        match &**event {
            Event::Reconnect(_) => {
                // mark for restart on next message
                ctx.restart();
            }
            Event::RoomState(data) => {
                let channel = Channel::get_or_persist_roomstate(
                    &ctx.db_context,
                    UpdateChannelId {
                        twitch_room_id: Some(data.room_id()? as i32),
                        name: data.channel().to_owned().into(),
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
                })
                .await;
            }
            Event::PrivMsg(data) => {
                info!("{}: {}", data.sender().as_ref().unwrap(), data.message());
            }
            _ => {}
        }
        Ok(())
    }
}
