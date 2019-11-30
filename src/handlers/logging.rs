use std::pin::Pin;
use std::sync::Arc;

use futures::future::ready;
use futures::Future;
use tmi_rs::event::*;

use crate::db::log_event;
use crate::dispatch::{ok_fut, EventHandler, Response};
use crate::error::Error;
use crate::state::BotContext;

pub struct LoggingHandler {
    ctx: BotContext,
}

impl EventHandler for LoggingHandler {
    fn create(ctx: &BotContext) -> Pin<Box<dyn Future<Output = Self>>>
    where
        Self: Sized,
    {
        Box::pin(ready(LoggingHandler {
            ctx: (*ctx).clone(),
        }))
    }

    fn run(&self, event: &Arc<Event<String>>) -> Result<Response, Error> {
        let ctx = self.ctx.db_context.clone();
        let event = event.clone();
        ok_fut(async move { log_event(&ctx, &event).await })
    }
}
