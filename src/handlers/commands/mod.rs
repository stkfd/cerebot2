use std::fmt::Debug;
use std::sync::Arc;

use fnv::FnvHashMap;
use futures::SinkExt;
use once_cell::sync::Lazy;
use regex::Regex;
use structopt::StructOpt;
use tmi_rs::event::*;
use tmi_rs::{ChatSender, ClientMessage};

use async_trait::async_trait;

use crate::db::commands::alias::CommandAlias;
use crate::db::commands::attributes::CommandAttributes;
use crate::db::commands::channel_config::ChannelCommandConfig;
use crate::db::commands::permission::CommandPermission;
use crate::db::permissions::{
    create_permissions, AddPermission, NewPermissionAttributes, PermissionRequirement,
    PermissionState, UserPermission,
};
use crate::dispatch::EventHandler;
use crate::event::CbEvent;
use crate::handlers::commands::error::CommandError;
use crate::state::{BotContext, BotStateError, ChannelInfo};
use crate::util::disallowed_input_chars;
use crate::{Error, Result};

mod channel;
pub mod error;
mod say;
mod templates;

#[async_trait]
pub trait CommandHandler: Send + Sync + Debug {
    fn name(&self) -> &'static str;

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()>;

    async fn create(bot: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized;
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
        let handler_vec: Vec<&(dyn Sync + Fn(_) -> _)> = vec![
            &say::SayCommand::create,
            &channel::ChannelManagerCommand::create,
        ];

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
                let channel = channel_opt
                    .as_ref()
                    .ok_or_else(|| Error::from(BotStateError::MissingChannel))?;

                // abort if the channel has no prefix or is set to silent
                if channel.data.silent || channel.data.command_prefix.is_none() {
                    return Ok(());
                }

                let message = data.message().as_str();

                // match channel command prefix, abort on empty prefix or no match
                let prefix = channel.data.command_prefix.as_ref().unwrap();
                if prefix.is_empty() || !message.starts_with(prefix.as_str()) {
                    return Ok(());
                }

                // extract name of the command
                let command_end_index = message.find(char::is_whitespace);
                command_name = if let Some(command_end_index) = command_end_index {
                    &message[prefix.len()..command_end_index]
                } else {
                    &message[prefix.len()..]
                };

                args = message;
            }
            Event::Whisper(data) => {
                channel_opt = None;

                let message = data.message().as_str();

                // extract name of the command
                let command_end_index = message.find(char::is_whitespace);
                command_name = if let Some(command_end_index) = command_end_index {
                    &message[0..command_end_index]
                } else {
                    &message
                };

                args = message;
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
                    args,
                    event,
                    channel: channel_opt.as_ref(),
                    command_name,
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

        let command_permissions = CommandPermission::get_by_command(&ctx, attributes.id).await?;

        cmd_ctx
            .check_permission_requirement(ctx, command_permissions.requirements(), true)
            .await?;

        command_handler.run(&cmd_ctx).await
    }
}

pub struct CommandContext<'a> {
    /// command argument list (includes the command name itself)
    args: &'a str,
    /// event that triggered the command
    event: &'a CbEvent,
    /// channel information, unless used as a whisper
    channel: Option<&'a Arc<ChannelInfo>>,
    /// name of the command
    command_name: &'a str,
}

impl CommandContext<'_> {
    /// Reply to the current message. Sends a message to the channel this event originated from or a whisper reply
    /// if this event is a whisper message. Fails on all other event types.
    pub async fn reply(&self, message: &str, mut out: &ChatSender) -> Result<()> {
        match &**self.event {
            Event::PrivMsg(data) => {
                out.send(ClientMessage::message(data.channel().as_str(), message))
                    .await?;
            }
            Event::Whisper(data) => {
                let sender = data
                    .sender()
                    .as_ref()
                    .ok_or_else::<Error, _>(|| {
                        CommandError::ReplyError("Whisper sender is missing from message").into()
                    })?
                    .as_str();
                out.send(ClientMessage::whisper(sender, message)).await?;
            }
            _ => {
                return Err(CommandError::ReplyError(
                    "Can only reply to privmsg and whisper events",
                )
                .into())
            }
        }
        Ok(())
    }

    /// Check whether the current user's permissions fulfill a given `PermissionRequirement`
    pub async fn check_permission_requirement(
        &self,
        ctx: &BotContext,
        req: &PermissionRequirement,
        reply_on_error: bool,
    ) -> Result<()> {
        let user = self.event.user(ctx).await?;
        let user_permission_ids = if let Some(user) = user {
            UserPermission::get_by_user_id(&ctx.db_context, user.id).await?
        } else {
            vec![]
        };

        if !req.check(&user_permission_ids) {
            if reply_on_error {
                self.reply(
                    "You don't have the permissions needed to use this command.",
                    &ctx.sender,
                )
                .await?;
            }
            Err(CommandError::PermissionRequired(req.clone()).into())
        } else {
            Ok(())
        }
    }

    /// Check whether the current user has the permissions with the given names
    pub async fn check_permissions(
        &self,
        ctx: &BotContext,
        names: &[&str],
        reply_on_error: bool,
    ) -> Result<()> {
        let user = self.event.user(ctx).await?;
        let user_permission_ids = if let Some(user) = user {
            UserPermission::get_by_user_id(&ctx.db_context, user.id).await?
        } else {
            vec![]
        };

        let permission_store = ctx.permissions.read().await;
        let permissions = permission_store.get_permissions(names.iter().map(|s| *s))?;
        let req = permission_store.get_requirement(permissions.iter().map(|p| p.id))?;

        if !req.check(&user_permission_ids) {
            if reply_on_error {
                self.reply(
                    "You don't have the permissions needed to use this command.",
                    &ctx.sender,
                )
                .await?;
            }
            Err(CommandError::PermissionRequired(req).into())
        } else {
            Ok(())
        }
    }

    pub async fn parse_args<T: Debug + StructOpt>(&self, bot: &BotContext) -> Result<Option<T>> {
        info!("{}", self.args.replace(disallowed_input_chars, ""));
        let result = T::from_iter_safe(
            self.args
                .replace(disallowed_input_chars, "")
                .split_whitespace(),
        );
        match result {
            Ok(matches) => Ok(Some(matches)),
            // display help or errors if required
            Err(structopt::clap::Error { message, .. }) => {
                let inline_help_message_rx = Lazy::new(|| Regex::new("\n\\W*").unwrap());

                self.reply(
                    &(&*inline_help_message_rx).replace_all(&message, " | "),
                    &mut bot.sender.clone(),
                )
                .await?;

                Ok(None)
            }
        }
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
