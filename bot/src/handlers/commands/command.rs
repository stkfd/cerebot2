use structopt::StructOpt;

use async_trait::async_trait;
use persistence::commands::alias::CommandAlias;
use persistence::commands::attributes::InsertCommandAttributes;
use persistence::permissions::{
    create_permissions, AddPermission, NewPermissionAttributes, PermissionState,
};

use crate::handlers::commands::*;
use crate::state::BotContext;
use crate::util::initialize_command;
use crate::Result;

#[derive(Debug)]
pub struct CommandManagerCommand {
    ctx: BotContext,
}

const NAME: &str = "command";

#[async_trait]
impl CommandHandler for CommandManagerCommand {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        if let Some(channel) = cmd.channel {
            if cmd.command_name == "commands" {
                let commands =
                    CommandAlias::channel_commands(&self.ctx.db_context.db_pool, channel.data.id)
                        .await?
                        .into_iter()
                        .map(|alias| alias.name)
                        .collect::<Vec<String>>()
                        .join(", ");

                let msg = format!("Commands: {}", commands);
                cmd.reply(&msg, &self.ctx.sender).await?;
            }
        } else {
            cmd.reply(
                "This command is not supported for whispers yet, try again some other time :/",
                &self.ctx.sender,
            )
            .await?;
        }
        Ok(())
    }

    async fn create(ctx: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        create_permissions(
            &ctx.db_context.db_pool,
            Cow::Owned(vec![
                AddPermission {
                    attributes: NewPermissionAttributes {
                        name: "commands:manage",
                        description: Some("Manage the commands"),
                        default_state: PermissionState::Deny,
                    },
                    implied_by: vec!["root"],
                },
                AddPermission {
                    attributes: NewPermissionAttributes {
                        name: "commands:read",
                        description: Some("Get information about commands"),
                        default_state: PermissionState::Allow,
                    },
                    implied_by: vec!["root", "commands:manage"],
                },
            ]),
        )
        .await?;

        // register channel management command
        initialize_command(
            &ctx,
            InsertCommandAttributes {
                handler_name: NAME.into(),
                description: Some("Manage the bot commands".into()),
                enabled: true,
                default_active: true,
                cooldown: Some(20000),
                whisper_enabled: true,
            },
            Vec::<String>::new(), // permissions checked inside the handler
            vec!["command", "commands", "cmd"],
        )
        .await?;

        Ok(Box::new(CommandManagerCommand { ctx: ctx.clone() }) as Box<dyn CommandHandler>)
    }
}

#[derive(StructOpt, Debug)]
#[structopt(name = "command", template(SUBCOMMANDS_HELP_TEMPLATE))]
enum CommandsCommandArgs {
    #[structopt(template(OPTS_HELP_TEMPLATE))]
    List,
}
