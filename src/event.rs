use std::ops::Deref;
use std::sync::Arc;

use double_checked_cell::DoubleCheckedCell;
use futures::TryFutureExt;
use tmi_rs::event::*;

use crate::db::User;
use crate::error::Error;
use crate::state::{BotContext, BotStateError, ChannelInfo};

#[derive(Debug, Clone)]
pub struct CbEvent {
    data: Arc<InnerEventData>,
}

#[derive(Debug)]
struct InnerEventData {
    event: Arc<Event<String>>,
    user: DoubleCheckedCell<Result<Option<User>, LazyFetchError>>,
}

impl CbEvent {
    pub fn inner(&self) -> &Arc<Event<String>> {
        &self.data.event
    }

    pub async fn user(&self, ctx: &BotContext) -> Result<Option<&User>, Error> {
        self.data
            .user
            .get_or_init(async {
                User::get_or_insert(&ctx.db_context, &self.data.event)
                    .map_err(|e| LazyFetchError::new(e))
                    .await
            })
            .await
            .as_ref()
            .map(|opt| opt.as_ref())
            .map_err(|e| e.clone().into())
    }

    pub async fn channel_info<'a>(
        &self,
        ctx: &'a BotContext,
    ) -> Result<Option<Arc<ChannelInfo>>, Error> {
        let channel = match &*self.data.event {
            Event::PrivMsg(e) => Some(e.channel()),
            Event::Join(e) => Some(e.channel()),
            Event::Mode(e) => Some(e.channel()),
            Event::Part(e) => Some(e.channel()),
            Event::ClearChat(e) => Some(e.channel()),
            Event::ClearMsg(e) => Some(e.channel()),
            Event::Host(e) => Some(e.hosting_channel()),
            Event::Notice(e) => Some(e.channel()),
            Event::RoomState(e) => Some(e.channel()),
            Event::UserNotice(e) => Some(e.channel()),
            Event::UserState(e) => Some(e.channel()),
            _ => None,
        };

        if let Some(channel) = channel {
            let channel_info = ctx
                .get_channel(channel)
                .await
                .ok_or_else::<Error, _>(|| BotStateError::MissingChannel.into())?;
            Ok(Some(channel_info))
        } else {
            Ok(None)
        }
    }
}

#[derive(Clone, Debug)]
pub struct LazyFetchError {
    source: Arc<Error>,
}

impl LazyFetchError {
    pub fn new(source: Error) -> Self {
        LazyFetchError {
            source: Arc::new(source),
        }
    }

    pub fn source(&self) -> &Error {
        &*self.source
    }
}

impl From<Arc<Event<String>>> for CbEvent {
    fn from(evt: Arc<Event<String>>) -> Self {
        CbEvent {
            data: Arc::new(InnerEventData {
                event: evt,
                user: DoubleCheckedCell::new(),
            }),
        }
    }
}

impl Deref for CbEvent {
    type Target = Event<String>;

    fn deref(&self) -> &Self::Target {
        &*self.data.event
    }
}
