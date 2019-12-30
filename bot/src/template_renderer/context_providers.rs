use serde_json::{to_value, Value as JsonValue};

use async_trait::async_trait;

use crate::event::CbEvent;
use crate::state::BotContext;
use crate::util::split_args;
use crate::Result;

/// Trait for command context data providers. Implementing this trait is the main way to make
/// additional data available to command templates.
#[async_trait]
pub trait ContextProvider: Send + Sync {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>>;
}

/// Provides data about the user calling the command
pub struct UserProvider;

#[async_trait]
impl ContextProvider for UserProvider {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>> {
        if let JsonValue::Bool(true) = request["sender"] {
            Ok(Some((
                "sender".to_string(),
                to_value(event.user(bot).await?).unwrap(),
            )))
        } else {
            Ok(None)
        }
    }
}

/// Provides channel data and state for the channel where a command was called
pub struct ChannelInfoProvider;

#[async_trait]
impl ContextProvider for ChannelInfoProvider {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>> {
        if let JsonValue::Bool(true) = request["channel"] {
            Ok(Some((
                "channel".to_string(),
                to_value(event.channel_info(bot).await?.as_deref()).unwrap(),
            )))
        } else {
            Ok(None)
        }
    }
}

/// Provides command arguments given by the user
pub struct ArgsProvider;

#[async_trait]
impl ContextProvider for ArgsProvider {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>> {
        if let JsonValue::String(s) = &request["args"] {
            let message = event.message();
            let channel_info = event.channel_info(bot).await?;
            let prefix = channel_info
                .as_deref()
                .and_then(|channel_info| channel_info.data.command_prefix.as_ref());
            let args_str = message.map(|msg| {
                let msg_without_prefix = if let Some(prefix) = prefix {
                    msg.split_at(prefix.len()).1
                } else {
                    msg
                };
                // remove the command itself
                if let Some(index) = msg_without_prefix.find(char::is_whitespace) {
                    msg_without_prefix.split_at(index).1.trim()
                } else {
                    ""
                }
            });
            if s == "complete" {
                Ok(Some(("args".to_string(), to_value(args_str).unwrap())))
            } else if s == "array" {
                let value = to_value(args_str.map(|args| split_args(args).unwrap())).unwrap(); // get rid of unwrap
                Ok(Some(("args".to_string(), value)))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
