use async_trait::async_trait;

use crate::db::{CommandAttributes, InsertCommandAttributes};
use crate::handlers::commands::{CommandContext, CommandHandler};
use crate::state::BotContext;
use crate::Result;

#[derive(Debug)]
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
        CommandAttributes::initialize(
            ctx,
            InsertCommandAttributes {
                handler_name: "hello-world".into(),
                description: Some("Test command".into()),
                enabled: true,
                default_active: true,
                cooldown: Some(5000),
                whisper_enabled: true,
            },
            Vec::<String>::new(),
            vec!["hello"],
        )
        .await?;

        Ok(Box::new(HelloWorldCommand { ctx: ctx.clone() }) as Box<dyn CommandHandler>)
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        info!("hello world");
        cmd.reply("Hello!", &mut self.ctx.sender.clone()).await?;
        Ok(())
    }
}
