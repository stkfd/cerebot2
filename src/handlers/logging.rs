use std::sync::Arc;

use tmi_rs::event::*;

use async_trait::async_trait;

use crate::db::log_event;
use crate::dispatch::EventHandler;
use crate::Result;
use crate::state::BotContext;

pub struct LoggingHandler {
    ctx: BotContext,
}

#[async_trait]
impl EventHandler for LoggingHandler {
    async fn create(ctx: &BotContext) -> Result<Self>
    where
        Self: Sized,
    {
        Ok(LoggingHandler {
            ctx: (*ctx).clone(),
        })
    }

    async fn run(&self, event: &Arc<Event<String>>) -> Result<()> {
        let ctx = &self.ctx.db_context;
        log_event(&ctx, &event).await
    }
}
