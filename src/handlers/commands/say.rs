use async_trait::async_trait;

use crate::db::commands::attributes::{CommandAttributes, InsertCommandAttributes};
use crate::handlers::commands::{CommandContext, CommandHandler};
use crate::state::BotContext;
use crate::Result;

#[derive(Debug)]
pub struct SayCommand {
    ctx: BotContext,
}

const NAME: &str = "say";

#[async_trait]
impl CommandHandler for SayCommand {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn create(ctx: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        CommandAttributes::initialize(
            ctx,
            InsertCommandAttributes {
                handler_name: NAME.into(),
                description: Some("Echo command".into()),
                enabled: true,
                default_active: true,
                cooldown: Some(5000),
                whisper_enabled: true,
            },
            vec!["root"],
            vec!["say"],
        )
        .await?;

        Ok(Box::new(SayCommand { ctx: ctx.clone() }) as Box<dyn CommandHandler>)
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        if let Some(idx) = cmd.args.find(char::is_whitespace) {
            cmd.reply(cmd.args.split_at(idx).1, &self.ctx.sender)
                .await?;
        }
        Ok(())
    }
}
