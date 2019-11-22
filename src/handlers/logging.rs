use std::sync::Arc;

use tmi_rs::event::tags::*;
use tmi_rs::event::*;

use crate::cerebot::BotContext;
use crate::db::{get_or_save_channel, log_event, NewChannel};
use crate::dispatch::{ok_fut, EventHandler, Response};
use crate::error::Error;

pub struct LoggingHandler {
    ctx: BotContext,
}

impl EventHandler for LoggingHandler {
    fn init(ctx: &BotContext) -> Self
    where
        Self: Sized,
    {
        LoggingHandler { ctx: ctx.clone() }
    }

    fn run(&self, event: &Arc<Event<String>>) -> Result<Response, Error> {
        let ctx = self.ctx.db_context.clone();
        let event = event.clone();
        ok_fut(async move {
            match &*event {
                Event::RoomState(data) => {
                    let channel_data = NewChannel {
                        twitch_room_id: Some(data.room_id()? as i32),
                        name: data.channel().clone().into(),
                        join_on_start: false,
                        command_prefix: None,
                        created_at: chrono::Local::now().into(),
                    };
                    get_or_save_channel(&ctx, channel_data).await?;
                }
                Event::PrivMsg(data) => {
                    info!("{}: {}", data.sender().as_ref().unwrap(), data.message());
                }
                _ => {}
            }
            log_event(&ctx, &event).await?;
            Ok(())
        })
    }
}
