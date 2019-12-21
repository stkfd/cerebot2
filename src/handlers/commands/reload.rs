use std::sync::Arc;

use async_trait::async_trait;

use crate::db::commands::attributes::{CommandAttributes, InsertCommandAttributes};
use crate::handlers::{CommandContext, CommandHandler};
use crate::state::command_store::CommandStore;
use crate::state::permission_store::PermissionStore;
use crate::state::BotContext;
use crate::template_renderer::TemplateRenderer;
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
        self.ctx
            .permissions
            .store(Arc::new(PermissionStore::load(&self.ctx.db_context).await?));
        self.ctx.templates.store(Arc::new(
            TemplateRenderer::create(&self.ctx.db_context).await?,
        ));
        self.ctx
            .commands
            .store(Arc::new(CommandStore::load(&self.ctx.db_context).await?));
        cmd.reply("Reload done!", &self.ctx.sender).await?;
        Ok(())
    }

    async fn create(bot: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        CommandAttributes::initialize(
            bot,
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
