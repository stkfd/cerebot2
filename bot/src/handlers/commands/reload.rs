use futures::future::join3;

use async_trait::async_trait;
use persistence::commands::attributes::InsertCommandAttributes;

use crate::handlers::{CommandContext, CommandHandler};
use crate::state::BotContext;
use crate::util::initialize_command;
use crate::Result;

#[derive(Debug)]
pub struct ReloadCommandHandler {
    ctx: BotContext,
}

const NAME: &str = "reload";

#[async_trait]
impl CommandHandler for ReloadCommandHandler {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        let (permissions, templates, commands) = join3(
            self.ctx.reload_permissions(),
            self.ctx.reload_templates(),
            self.ctx.reload_commands(),
        )
        .await;
        permissions?;
        templates?;
        commands?;
        cmd.reply("Reload done!", &self.ctx.sender).await?;
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
                description: Some("Reload command, permissions and templates".into()),
                enabled: true,
                default_active: true,
                cooldown: None,
                whisper_enabled: true,
            },
            vec!["root"],
            vec!["reload"],
        )
        .await?;

        Ok(Box::new(ReloadCommandHandler { ctx: bot.clone() }) as Box<dyn CommandHandler>)
    }
}
