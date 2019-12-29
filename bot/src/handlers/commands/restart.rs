use async_trait::async_trait;
use persistence::commands::attributes::InsertCommandAttributes;

use crate::handlers::{CommandContext, CommandHandler};
use crate::state::BotContext;
use crate::util::initialize_command;
use crate::Result;

#[derive(Debug)]
pub struct RestartCommandHandler {
    ctx: BotContext,
}

const NAME: &str = "restart";

#[async_trait]
impl CommandHandler for RestartCommandHandler {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        cmd.reply("Reconnecting MrDestructoid", &self.ctx.sender)
            .await?;
        self.ctx.restart().await?;
        Ok(())
    }

    async fn create(bot: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        initialize_command(
            &bot,
            InsertCommandAttributes {
                handler_name: NAME.into(),
                description: Some("Restarts the bot".into()),
                enabled: true,
                default_active: false,
                cooldown: None,
                whisper_enabled: true,
            },
            vec!["root"],
            vec!["restart"],
        )
        .await?;

        Ok(Box::new(RestartCommandHandler { ctx: bot.clone() }) as Box<dyn CommandHandler>)
    }
}
