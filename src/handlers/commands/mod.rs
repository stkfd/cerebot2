use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use fnv::FnvHashMap;
use futures::future::ready;
use futures::Future;
use tmi_rs::event::*;

use crate::db::{ChannelCommandConfig, CommandAttributes, CommandPermission, UserPermission, User};
use crate::dispatch::{ok_fut, EventHandler, Response};
use crate::error::Error;
use crate::state::{BotContext, ChannelInfo};

pub trait CommandHandler: Send + Sync {
    fn boot(ctx: &BotContext) -> (Vec<String>, Self)
    where
        Self: Sized;

    fn run(
        &self,
        ctx: &BotContext,
        event: &Arc<Event<String>>,
        args: Option<&str>,
    ) -> Pin<Box<dyn Future<Output = Result<Response, Error>> + Send + 'static>>;
}

pub struct CommandRouter {
    ctx: BotContext,
    commands: FnvHashMap<String, Arc<Box<dyn CommandHandler>>>,
}

impl EventHandler for CommandRouter {
    fn create(ctx: &BotContext) -> Pin<Box<dyn Future<Output = Self>>>
    where
        Self: Sized,
    {
        Box::pin(ready(CommandRouter {
            ctx: ctx.clone(),
            commands: Default::default(),
        }))
    }

    fn run(&self, event: &Arc<Event<String>>) -> Result<Response, Error> {
        let args;
        let command_name;
        let channel_opt;

        match &**event {
            Event::PrivMsg(data) => {
                let channel_lock = self
                    .ctx
                    .get_channel(data.channel())
                    .ok_or_else(|| CommandHandlerError::MissingChannel)?;
                channel_opt = Some((&*channel_lock).clone());
                let channel = &*channel_lock;

                if channel.data.silent || channel.data.command_prefix.is_none() {
                    return Ok(Response::OkNow);
                }

                let message = data.message().as_str();

                let prefix = channel.data.command_prefix.as_ref().unwrap();
                if prefix.is_empty() || !message.starts_with(prefix.as_str()) {
                    return Ok(Response::OkNow);
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
            _ => return Ok(Response::OkNow),
        }

        let event = event.clone();
        let ctx = self.ctx.clone();
        let command_name = command_name.to_string();
        if let Some(command_handler) = self.commands.get(&command_name).cloned() {
            ok_fut(async move {
                run_command(
                    &ctx,
                    &event,
                    channel_opt,
                    &command_name,
                    args.as_ref().map(|s| s.as_str()),
                    &**command_handler,
                )
                .await
            })
        } else {
            return Ok(Response::OkNow);
        }
    }
}

struct CommandContext<'a> {
    args: Option<&'a str>,
    event: &'a Arc<Event<String>>,
    channel: Option<Arc<ChannelInfo>>,
    command_name: &'a str,
    ctx: &'a BotContext,
}

async fn run_command(
    ctx: &BotContext,
    event: &Arc<Event<String>>,
    channel: Option<Arc<ChannelInfo>>,
    command_name: &str,
    args: Option<&str>,
    command_handler: &dyn CommandHandler,
) -> Result<(), Error> {
    let command_attributes = get_command(ctx, command_name).await?;

    // load channel specific command config
    if let Some(channel) = channel {
        let channel_config =
            ChannelCommandConfig::get(ctx, channel.data.id, command_attributes.id).await?;

        let active_in_channel = channel_config
            .and_then(|config| config.active)
            .unwrap_or(command_attributes.default_active);

        if !command_attributes.enabled || !active_in_channel {
            return Ok(());
        }
    }

    let user = User::get_or_insert(&ctx.db_context, event).await?;
    let permission_ids = if let Some(user) = user {
        UserPermission::get_by_user_id(&ctx.db_context, user.id).await?
    } else {
        vec![]
    };

    let permission_check = CommandPermission::get_by_command(&ctx, command_attributes.id)
        .await?
        .requirements()
        .check(&permission_ids);

    if !permission_check { return Ok(()); }

    command_handler.run(&ctx, &event, args).await?;
    Ok(())
}

/// load command settings or insert default values into db
async fn get_command(ctx: &BotContext, command_name: &str) -> Result<CommandAttributes, Error> {
    if let Some(result) = CommandAttributes::get(&ctx, command_name).await? {
        Ok(result)
    } else {
        Err(Error::from(CommandHandlerError::MissingCommandAttributes(
            command_name.to_string(),
        )))
    }
}

#[derive(Debug)]
pub enum CommandHandlerError {
    MissingChannel,
    MissingCommandAttributes(String),
}

impl std::error::Error for CommandHandlerError {}

impl fmt::Display for CommandHandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            CommandHandlerError::MissingChannel => write!(f, "Channel data was unavailable"),
            CommandHandlerError::MissingCommandAttributes(cmd) => write!(
                f,
                "Command attributes for {} are missing, check command boot function",
                cmd
            ),
        }
    }
}
