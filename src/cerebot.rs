use std::time::Duration;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use futures::future::{join, ready};
use futures::{SinkExt, StreamExt};
use tmi_rs::rate_limits::RateLimiterConfig;
use tmi_rs::{ClientMessage, TwitchChatConnection, TwitchClient, TwitchClientConfigBuilder};
use tokio::{task, time};

use crate::config::CerebotConfig;
use crate::db::{create_default_permissions, persist_event_queue, Channel};
use crate::diesel::prelude::*;
use crate::dispatch::matchers::{MatchAll, MatchMessages};
use crate::dispatch::{EventDispatch, EventHandler, HandlerBuilder, MatcherBuilder};
use crate::error::Error;
use crate::event::CbEvent;
use crate::handlers::{BotStateHandler, CommandRouter, LoggingHandler};
use crate::schema::channels;
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
                .rate_limiter(RateLimiterConfig::default())
                .build()
                .map_err(Error::TmiConfig)?
                .into(),
            config,
        })
    }

    pub async fn run(&mut self) -> Result<RunResult> {
        debug!("Creating database connection pool...");
        let manager = ConnectionManager::<PgConnection>::new(self.config.db());
        let db_pool = r2d2::Pool::builder()
            .build(manager)
            .map_err(Error::ConnectionPool)?;
        let redis_pool = r2d2::Pool::builder()
            .build(
                r2d2_redis::RedisConnectionManager::new(self.config.redis())
                    .map_err(Error::Redis)?,
            )
            .map_err(Error::ConnectionPool)?;
        info!("Database connection pool created.");

        debug!("Connecting to Twitch chat...");
        let TwitchChatConnection {
            sender,
            receiver,
            error_receiver,
        } = self.chat_client.connect().await?;
        info!("Twitch chat connected.");

        let context: BotContext = BotContext::create(db_pool, redis_pool, sender).await?;

        // log any connection errors
        let process_errors = error_receiver.for_each(|error| {
            async move {
                error!("Chat connection error: {}", error);
            }
        });

        let startup_channels = channels::table
            .filter(channels::join_on_start.eq(true))
            .load::<Channel>(&context.db_context.db_pool.get()?)
            .expect("load startup channels");

        info!(
            "Joining startup channels: {:?}",
            startup_channels.iter().map(|c| &c.name).collect::<Vec<_>>()
        );

        // join a channel
        let mut sender = context.sender.clone();
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

        create_default_permissions(&context).await?;

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

pub enum RunResult {
    Ok,
    Restart,
}
