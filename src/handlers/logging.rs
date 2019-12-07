use async_trait::async_trait;

use crate::db::log_event;
use crate::dispatch::EventHandler;
use crate::event::CbEvent;
use crate::Result;
use crate::state::BotContext;

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
        log_event(&self.ctx, event).await?;
        Ok(())
    }
}
