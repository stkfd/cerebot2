use std::fmt::Debug;
use std::sync::Arc;

use fnv::FnvHashMap;
use futures::SinkExt;
use once_cell::sync::Lazy;
use regex::Regex;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use tmi_rs::event::*;
use tmi_rs::{ChatSender, ClientMessage};

use async_trait::async_trait;
use persistence::commands::attributes::CommandAttributes;
use persistence::commands::channel_config::ChannelCommandConfig;
use persistence::commands::permission::PermissionRequirement;
use persistence::permissions::{
    create_permissions, AddPermission, NewPermissionAttributes, PermissionState, UserPermission,
};

use crate::dispatch::EventHandler;
use crate::event::CbEvent;
use crate::handlers::commands::error::CommandError;
use crate::state::{BotContext, BotStateError, ChannelInfo};
use crate::util::split_args;
use crate::{Error, Result};
use std::borrow::Cow;

mod channel;
mod command;
pub mod error;
mod reload;
mod restart;
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
}

#[async_trait]
impl EventHandler<CbEvent> for CommandRouter {
    async fn create(ctx: &BotContext) -> Result<Self>
    where
        Self: Sized,
    {
        let handler_vec: Vec<&(dyn Sync + Fn(_) -> _)> = vec![
            &say::SayCommand::create,
            &command::CommandManagerCommand::create,
            &channel::ChannelManagerCommand::create,
            &templates::TemplateCommandHandler::create,
            &reload::ReloadCommandHandler::create,
            &restart::RestartCommandHandler::create,
        ];

        init_command_router_permissions(ctx).await?;

        let mut command_handlers = FnvHashMap::default();

        for creator in handler_vec {
            let handler = creator(ctx).await?;
            command_handlers.insert(handler.name(), handler);
        }

        Ok(CommandRouter {
            ctx: ctx.clone(),
            command_handlers,
        })
    }

    async fn run(&self, event: &CbEvent) -> Result<()> {
        // will contain everything but the command prefix
        let args;
        // will contain only the name of the command alias (without prefix)
        let command_name;
        // channel where the command is called, if applicable
        let channel_opt: Option<Arc<ChannelInfo>>;

        // first extract available data from the event, depending on if it's a
        // channel or whisper message
        match &**event {
            Event::PrivMsg(data) => {
                channel_opt = event.channel_info(&self.ctx).await?;
                let channel = channel_opt
                    .as_ref()
                    .ok_or_else(|| BotStateError::MissingChannel)?;

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
                let command_end_index = message.split_at(prefix.len()).1.find(char::is_whitespace);
                command_name = if let Some(command_end_index) = command_end_index {
                    &message[prefix.len()..(command_end_index + prefix.len())]
                } else {
                    &message[prefix.len()..]
                };

                debug!("{}", command_name);

                args = &message[prefix.len()..];
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

        let command_store = self.ctx.commands.load();
        let attributes = command_store.get_by_alias(command_name);

        let handler = attributes
            .and_then(|attributes| self.command_handlers.get(attributes.handler_name.as_str()));

        if let (Some(attributes), Some(handler)) = (attributes, handler) {
            debug!("Preparing command handler {}", handler.name());
            if !attributes.whisper_enabled && channel_opt.is_none() {
                debug!("Command can't be used in whispers, ignoring");
                return Ok(());
            }

            self.run_command(
                &**handler,
                CommandContext {
                    args,
                    event,
                    channel: channel_opt.as_ref(),
                    command_name,
                    attributes,
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
        command_handler: &dyn CommandHandler,
        cmd_ctx: CommandContext<'_>,
    ) -> Result<()> {
        let ctx = &self.ctx;

        // load channel specific command config
        if let Some(channel) = &cmd_ctx.channel {
            let channel_config =
                ChannelCommandConfig::get(&ctx.db_context, channel.data.id, cmd_ctx.attributes.id)
                    .await?;

            let active_in_channel = channel_config
                .as_ref()
                .and_then(|config| config.active)
                .unwrap_or(cmd_ctx.attributes.default_active);

            if !cmd_ctx.attributes.enabled || !active_in_channel {
                return Ok(());
            }

            let channel_cooldown = channel_config
                .as_ref()
                .and_then(|config| config.cooldown.as_deref().copied());

            if !cmd_ctx
                .attributes
                .check_cooldown(
                    &self.ctx.db_context.redis_pool,
                    &channel.data.name,
                    channel_cooldown,
                )
                .await?
            {
                let permission_check = cmd_ctx
                    .check_permissions(&self.ctx, &["cmd:bypass_cooldowns"], false)
                    .await;
                if let Err(Error::Command(CommandError::PermissionRequired(_))) = permission_check {
                    debug!("Cooldown for {} still active", cmd_ctx.command_name);
                    return Ok(());
                }
                // if other errors than missing permission occurred
                permission_check?;
            }
            cmd_ctx
                .attributes
                .reset_cooldown(
                    &self.ctx.db_context.redis_pool,
                    &channel.data.name,
                    channel_cooldown,
                )
                .await?;
        }

        let command_permissions = ctx
            .permissions
            .load()
            .get_by_command(&ctx.db_context, cmd_ctx.attributes.id)
            .await?;

        cmd_ctx
            .check_permission_requirement(ctx, command_permissions.requirements(), true)
            .await?;

        debug!("Running {} command handler", command_handler.name());
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
    attributes: &'a CommandAttributes,
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

        let permission_store = ctx.permissions.load();
        let permissions = permission_store.get_permissions(names.iter().copied())?;
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
        let args = split_args(self.args)?;
        debug!("{:?}", args);

        let result = T::clap()
            .global_settings(&[
                AppSettings::DisableVersion,
                AppSettings::DisableHelpSubcommand,
                AppSettings::ColorNever,
            ])
            .get_matches_from_safe(args);
        match result {
            Ok(matches) => Ok(Some(T::from_clap(&matches))),
            // display help or errors if required
            Err(structopt::clap::Error { message, .. }) => {
                let inline_help_message_rx = Lazy::new(|| Regex::new("\n\\W*").unwrap());

                self.reply(
                    &(&*inline_help_message_rx).replace_all(&message, " | "),
                    &bot.sender,
                )
                .await?;

                Ok(None)
            }
        }
    }
}

/// Initialize permissions required for the command router
async fn init_command_router_permissions(ctx: &BotContext) -> Result<()> {
    init_permissions(
        &ctx,
        Cow::Owned(vec![AddPermission {
            attributes: NewPermissionAttributes {
                name: "cmd:bypass_cooldowns",
                description: Some("Bypass command cooldowns."),
                default_state: PermissionState::Deny,
            },
            implied_by: vec!["root"],
        }]),
    )
    .await?;
    Ok(())
}

/// help template to show only options and usage
const OPTS_HELP_TEMPLATE: &str = "{usage} options: {unified}";

/// help template to show subcommands and info
const SUBCOMMANDS_HELP_TEMPLATE: &str = "{about} - {usage} {subcommands}";

async fn init_permissions(
    ctx: &BotContext,
    permissions: Cow<'static, Vec<AddPermission<'_>>>,
) -> Result<()> {
    if create_permissions(&ctx.db_context.db_pool, permissions).await? > 0 {
        ctx.reload_permissions().await?;
    }
    Ok(())
}
