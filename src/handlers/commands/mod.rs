use std::fmt::Debug;
use std::sync::Arc;

use fnv::FnvHashMap;
use futures::SinkExt;
use tmi_rs::{ChatSender, ClientMessage};
use tmi_rs::event::*;

use async_trait::async_trait;

use crate::{Error, Result};
use crate::db::{
    AddPermission, ChannelCommandConfig, CommandAlias, CommandAttributes, CommandPermission,
    create_permissions, NewPermissionAttributes, PermissionState, UserPermission,
};
use crate::dispatch::EventHandler;
use crate::event::CbEvent;
use crate::handlers::commands::error::CommandError;
use crate::handlers::commands::hello_world::HelloWorldCommand;
use crate::state::{BotContext, BotStateError, ChannelInfo};

mod hello_world;
pub mod error;

#[async_trait]
pub trait CommandHandler: Send + Sync + Debug {
    fn name(&self) -> &'static str;

    async fn create(bot: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized;

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()>;
}

#[derive(Debug)]
pub struct CommandRouter {
    ctx: BotContext,
    command_handlers: FnvHashMap<&'static str, Box<dyn CommandHandler>>,
    /// Map of command alias -> command_id pairs
    aliases: FnvHashMap<String, i32>,
    /// Map of command_id -> CommandAttributes to hold command configurations
    commands: FnvHashMap<i32, CommandAttributes>,
}

#[async_trait]
impl EventHandler<CbEvent> for CommandRouter {
    async fn create(ctx: &BotContext) -> Result<Self>
    where
        Self: Sized,
    {
        let handler_vec: Vec<&(dyn Sync + Fn(_) -> _)> = vec![&HelloWorldCommand::create];

        init_permissions(ctx).await?;

        let mut command_handlers = FnvHashMap::default();

        for creator in handler_vec {
            let handler = creator(ctx).await?;
            command_handlers.insert(handler.name(), handler);
        }

        let aliases = CommandAlias::all(&ctx.db_context)
            .await?
            .into_iter()
            .map(|alias| (alias.name, alias.command_id))
            .collect();

        let commands = CommandAttributes::all(&ctx.db_context)
            .await?
            .into_iter()
            .map(|attr| (attr.id, attr))
            .collect();

        Ok(CommandRouter {
            ctx: ctx.clone(),
            command_handlers,
            aliases,
            commands,
        })
    }

    async fn run(&self, event: &CbEvent) -> Result<()> {
        let args;
        let command_name;
        let channel_opt: Option<Arc<ChannelInfo>>;

        // first extract available data from the event, depending on if it's a
        // channel or whisper message
        match &**event {
            Event::PrivMsg(data) => {
                channel_opt = event.channel_info(&self.ctx).await?;
                let channel = channel_opt.as_ref().ok_or_else(|| {
                    Error::from(BotStateError::MissingChannel)
                })?;

                if channel.data.silent || channel.data.command_prefix.is_none() {
                    return Ok(());
                }

                let message = data.message().as_str();
                let prefix = channel.data.command_prefix.as_ref().unwrap();
                if prefix.is_empty() || !message.starts_with(prefix.as_str()) {
                    return Ok(());
                }

                let command_end_index = message.find(char::is_whitespace);
                command_name = if let Some(command_end_index) = command_end_index {
                    &message[prefix.len()..command_end_index]
                } else {
                    &message[prefix.len()..]
                };

                args = command_end_index
                    .map(|idx| &message[idx..])
                    .map(|s| s.to_owned());
            }
            Event::Whisper(data) => {
                channel_opt = None;

                let message = data.message().as_str();
                let command_end_index = message.find(char::is_whitespace);
                command_name = if let Some(command_end_index) = command_end_index {
                    &message[0..command_end_index]
                } else {
                    &message
                };

                args = command_end_index
                    .map(|idx| &message[idx..])
                    .map(|s| s.to_owned());
            }
            // abort on any non-message events
            _ => return Ok(()),
        }

        let attributes = self
            .aliases
            .get(command_name)
            .and_then(|command_id| self.commands.get(command_id));

        let handler = attributes
            .and_then(|attributes| self.command_handlers.get(attributes.handler_name.as_str()));

        if let (Some(attributes), Some(handler)) = (attributes, handler) {
            if !attributes.whisper_enabled && channel_opt.is_none() {
                return Ok(());
            }

            if let Some(ref channel) = channel_opt {
                if !attributes
                    .check_cooldown(&self.ctx.db_context, &channel.data.name)
                    .await?
                {
                    return Ok(());
                }
                attributes
                    .reset_cooldown(&self.ctx.db_context, &channel.data.name)
                    .await?;
            }

            self.run_command(
                attributes,
                &**handler,
                CommandContext {
                    args: args.as_ref().map(|s| s.as_str()),
                    event,
                    channel: channel_opt.as_ref(),
                    command_name
                },
            )
            .await
        } else {
            Ok(())
        }
    }
}

impl CommandRouter {
    async fn run_command(
        &self,
        attributes: &CommandAttributes,
        command_handler: &dyn CommandHandler,
        cmd_ctx: CommandContext<'_>,
    ) -> Result<()> {
        let ctx = &self.ctx;

        // load channel specific command config
        if let Some(channel) = &cmd_ctx.channel {
            let channel_config =
                ChannelCommandConfig::get(ctx, channel.data.id, attributes.id).await?;

            let active_in_channel = channel_config
                .and_then(|config| config.active)
                .unwrap_or(attributes.default_active);

            if !attributes.enabled || !active_in_channel {
                return Ok(());
            }
        }

        let user = cmd_ctx.event.user(ctx).await?;
        let permission_ids = if let Some(user) = user {
            UserPermission::get_by_user_id(&ctx.db_context, user.id).await?
        } else {
            vec![]
        };

        let permission_check = CommandPermission::get_by_command(&ctx, attributes.id)
            .await?
            .requirements()
            .check(&permission_ids);

        if !permission_check {
            return Ok(());
        }

        command_handler.run(&cmd_ctx).await
    }
}

pub struct CommandContext<'a> {
    args: Option<&'a str>,
    event: &'a CbEvent,
    channel: Option<&'a Arc<ChannelInfo>>,
    command_name: &'a str,
}

impl CommandContext<'_> {
    pub async fn reply(&self, message: &str, out: &mut ChatSender) -> Result<()> {
        match &**self.event {
            Event::PrivMsg(data) => {
                out.send(ClientMessage::message(data.channel().as_str(), message)).await?;
            }
            Event::Whisper(data) => {
                let sender = data
                    .sender()
                    .as_ref()
                    .ok_or_else::<Error, _>(|| CommandError::ReplyError("Whisper sender is missing from message").into())?
                    .as_str();
                out.send(ClientMessage::whisper(sender, message)).await?;
            },
            _ => return Err(CommandError::ReplyError("Can only reply to privmsg and whisper events").into())
        }
        Ok(())
    }
}

/// Initialize permissions required for the command router
async fn init_permissions(ctx: &BotContext) -> Result<()> {
    create_permissions(
        ctx,
        vec![AddPermission {
            attributes: NewPermissionAttributes {
                name: "cmd:bypass_cooldowns",
                description: Some("Bypass command cooldowns."),
                default_state: PermissionState::Deny,
            },
            implied_by: vec!["root"],
        }],
    )
    .await
}
