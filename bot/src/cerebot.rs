use std::pin::Pin;
use std::time::Duration;

use futures::channel::mpsc::UnboundedReceiver;
use futures::future::{join, ready};
use futures::{SinkExt, StreamExt};
use tmi_rs::stream::{ClientMessageStream, SendStreamExt};
use tmi_rs::{ClientMessage, TwitchChatConnection, TwitchClient, TwitchClientConfigBuilder};
use tokio::{task, time};

use persistence::channel::Channel;
use persistence::chat_event::persist_event_queue;
use persistence::permissions::create_default_permissions;
use persistence::DbContext;

use crate::config::CerebotConfig;
use crate::dispatch::matchers::{MatchAll, MatchMessages};
use crate::dispatch::{EventDispatch, EventHandler, HandlerBuilder, MatcherBuilder};
use crate::error::Error;
use crate::event::CbEvent;
use crate::handlers::{BotStateHandler, CommandRouter, LoggingHandler};
use crate::state::*;
use crate::Result;

pub struct Cerebot {
    chat_client: TwitchClient,
    config: CerebotConfig,
}

impl Cerebot {
    pub fn create(config: CerebotConfig) -> Result<Self> {
        Ok(Cerebot {
            chat_client: TwitchClientConfigBuilder::default()
                .username(config.username().to_string())
                .token(config.auth_token().to_string())
                //.send_middleware(Arc::new(send_middleware_setup))
                .build()
                .map_err(Error::TmiConfig)?
                .into(),
            config,
        })
    }

    pub async fn run(&mut self) -> Result<RunResult> {
        debug!("Creating database connection pool...");
        let db_context = DbContext::create(self.config.db(), self.config.redis()).await?;
        info!("Database connection pool created.");

        debug!("Connecting to Twitch chat...");
        let TwitchChatConnection {
            sender,
            receiver,
            error_receiver,
        } = self.chat_client.connect().await?;
        info!("Twitch chat connected.");

        let context: BotContext = BotContext::create(db_context, sender).await?;

        // log any connection errors
        let process_errors = error_receiver.for_each(|error| {
            async move {
                error!("Chat connection error: {}", error);
            }
        });

        let startup_channels = Channel::get_startup_channels(&context.db_context.db_pool)
            .await
            .expect("load startup channels");

        info!(
            "Joining startup channels: {:?}",
            startup_channels.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        // join a channel
        let mut sender = &context.sender;
        for channel in startup_channels {
            sender.send(ClientMessage::join(channel.name)).await?;
        }

        let heartbeat_ctx = context.db_context.clone();
        task::spawn(async move {
            let ctx = heartbeat_ctx;
            let mut interval = time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                persist_event_queue(&ctx).await.unwrap();
            }
        });

        if create_default_permissions(&context.db_context).await? > 0 {
            context.reload_permissions().await?;
        }

        let dispatch = EventDispatch::<CbEvent>::default();
        dispatch
            .match_events(MatchAll)
            .handle(Box::new(BotStateHandler::create(&context).await?))
            .handle(Box::new(LoggingHandler::create(&context).await?))
            .match_events(MatchMessages)
            .handle(Box::new(CommandRouter::create(&context).await?));
        info!("Initialized message handlers");

        // process messages and do stuff with the data
        let dispatch = &dispatch;
        let context = &context;
        let process_messages = receiver
            .take_while(|_| ready(!context.should_restart()))
            .map(|event| dispatch.dispatch(CbEvent::from(event)))
            .buffer_unordered(10)
            .for_each(|dispatch_result| {
                async move {
                    // run event handlers
                    if let Err(err) = dispatch_result {
                        error!("Event handler failed: {}", err)
                    }
                }
            });

        join(process_messages, process_errors).await;
        if context.should_restart() {
            Ok(RunResult::Restart)
        } else {
            Ok(RunResult::Ok)
        }
    }
}

fn send_middleware_setup(
    stream: UnboundedReceiver<ClientMessage<String>>,
) -> Pin<Box<dyn ClientMessageStream>> {
    let stream = stream.split_oversize(500).dedup();
    Box::pin(stream)
}

pub enum RunResult {
    Ok,
    Restart,
}
