use async_trait::async_trait;

use crate::error::Error;
use crate::handlers::{CommandContext, CommandHandler};
use crate::state::BotContext;

#[derive(Debug)]
pub struct TemplateCommandHandler {}

const NAME: &str = "template";

#[async_trait]
impl CommandHandler for TemplateCommandHandler {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<(), Error> {
        unimplemented!()
    }

    async fn create(bot: &BotContext) -> Result<Box<dyn CommandHandler>, Error>
    where
        Self: Sized,
    {
        unimplemented!()
    }
}
