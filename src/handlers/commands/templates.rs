use async_trait::async_trait;

use crate::handlers::{CommandContext, CommandHandler};
use crate::state::BotContext;
use crate::Result;

#[derive(Debug)]
pub struct TemplateCommandHandler {
    ctx: BotContext,
}

const NAME: &str = "template";

#[async_trait]
impl CommandHandler for TemplateCommandHandler {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        info!("template command");
        let render_output = self
            .ctx
            .templates
            .load()
            .render(cmd.attributes.id, cmd.event, &self.ctx)
            .await?;
        info!("{:?}", &render_output);
        let trimmed_output = render_output.trim();
        if !trimmed_output.is_empty() {
            cmd.reply(trimmed_output, &self.ctx.sender).await?;
        }
        Ok(())
    }

    async fn create(bot: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        let instance = TemplateCommandHandler { ctx: bot.clone() };
        Ok(Box::new(instance) as Box<dyn CommandHandler>)
    }
}
