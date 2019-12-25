use std::str::FromStr;

use tmi_rs::event::tags::*;
use tmi_rs::event::*;
use tmi_rs::irc_constants::RPL_ENDOFMOTD;
use uuid::Uuid;

use async_trait::async_trait;
use persistence::channel::Channel;
use persistence::chat_event::{log_event, ChatEventType, NewChatEvent};

use crate::dispatch::EventHandler;
use crate::event::CbEvent;
use crate::state::BotContext;
use crate::Result;

#[derive(Debug)]
pub struct LoggingHandler {
    ctx: BotContext,
}

#[async_trait]
impl EventHandler<CbEvent> for LoggingHandler {
    async fn create(ctx: &BotContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(LoggingHandler {
            ctx: (*ctx).clone(),
        })
    }

    async fn run(&self, event: &CbEvent) -> Result<()> {
        let db_entry = self.event_to_db_entry(event).await?;
        if let Some(db_entry) = db_entry {
            log_event(&self.ctx.db_context, db_entry).await?;
        }
        Ok(())
    }
}

impl LoggingHandler {
    async fn event_to_db_entry(&self, event: &CbEvent) -> Result<Option<NewChatEvent>> {
        let ctx = &self.ctx.db_context;
        let user_id = event.user(&self.ctx).await?.map(|u| u.id);
        let event = match &**event {
            Event::PrivMsg(data) => Some(NewChatEvent {
                event_type: ChatEventType::Privmsg,
                twitch_message_id: Uuid::from_str(data.id()?).ok(),
                message: Some(data.message().clone()),
                channel_id: Channel::get(ctx, data.channel()).await?.map(|c| c.id),
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
                channel_id: Channel::get(ctx, data.channel()).await?.map(|c| c.id),
                sender_user_id: user_id,
                tags: data.tags().clone().map(Into::into),
                received_at: chrono::Local::now().into(),
            }),
            Event::UserNotice(data) => Some(NewChatEvent {
                event_type: ChatEventType::Usernotice,
                twitch_message_id: Uuid::from_str(data.id()?).ok(),
                message: Some(data.message().clone()),
                channel_id: Channel::get(ctx, data.channel()).await?.map(|c| c.id),
                sender_user_id: user_id,
                tags: data.tags().clone().map(Into::into),
                received_at: chrono::Local::now().into(),
            }),
            Event::Host(data) => Some(NewChatEvent {
                event_type: ChatEventType::Host,
                twitch_message_id: None,
                message: None,
                channel_id: Channel::get(ctx, data.hosting_channel())
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
                channel_id: Channel::get(ctx, data.channel()).await?.map(|c| c.id),
                sender_user_id: user_id,
                tags: data.tags().clone().map(Into::into),
                received_at: chrono::Local::now().into(),
            }),
            Event::ClearMsg(data) => Some(NewChatEvent {
                event_type: ChatEventType::Clearmsg,
                twitch_message_id: None,
                message: Some(data.message().clone()),
                channel_id: Channel::get(ctx, data.channel()).await?.map(|c| c.id),
                sender_user_id: user_id,
                tags: data.tags().clone().map(Into::into),
                received_at: chrono::Local::now().into(),
            }),
            Event::RoomState(data) => Some(NewChatEvent {
                event_type: ChatEventType::Roomstate,
                twitch_message_id: None,
                message: None,
                channel_id: Channel::get(ctx, data.channel()).await?.map(|c| c.id),
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
        Ok(event)
    }
}
