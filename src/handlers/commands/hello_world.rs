use std::sync::Arc;

use tmi_rs::event::Event;

use async_trait::async_trait;

use crate::db::{CommandAttributes, InsertCommandAttributes};
use crate::handlers::CommandHandler;
use crate::state::BotContext;
use crate::Result;

pub struct HelloWorldCommand {
    ctx: BotContext,
}

#[async_trait]
impl CommandHandler for HelloWorldCommand {
    fn name(&self) -> &'static str {
        "hello-world"
    }

    async fn create(ctx: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        let attributes = InsertCommandAttributes {
            handler_name: "hello-world",
            description: Some("Test command"),
            enabled: true,
            default_active: true,
            cooldown: Some(5000),
            whisper_enabled: true,
        };
        CommandAttributes::initialize(ctx, attributes, &vec!["hello"]).await?;

        Ok(Box::new(HelloWorldCommand { ctx: ctx.clone() }) as Box<dyn CommandHandler>)
    }

    async fn run(&self, _event: &Arc<Event<String>>, _args: Option<&str>) -> Result<()> {
        info!("hello world");
        Ok(())
    }
}
