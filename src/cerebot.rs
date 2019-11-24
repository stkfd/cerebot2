use std::error::Error as ErrorTrait;
use std::time::Duration;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use futures::future::{join, ready};
use futures::{SinkExt, StreamExt};
use r2d2::Pool;
use r2d2_redis::RedisConnectionManager;
use tmi_rs::event::Event;
use tmi_rs::rate_limits::RateLimiterConfig;
use tmi_rs::{
    ChatSender, ClientMessage, TwitchChatConnection, TwitchClient, TwitchClientConfigBuilder,
};
use tokio::timer::Interval;

use crate::config::CerebotConfig;
use crate::db::{create_permissions, persist_event_queue, Channel};
use crate::diesel::prelude::*;
use crate::dispatch::matchers::MatchAll;
use crate::dispatch::{EventDispatch, EventHandler, HandlerBuilder, MatcherBuilder};
use crate::error::Error;
use crate::handlers::LoggingHandler;
use crate::schema::channels;
use crate::state::BotState;
use std::ops::Deref;
use std::sync::Arc;

pub struct Cerebot {
    chat_client: TwitchClient,
    config: CerebotConfig,
}

impl Cerebot {
    pub fn create(config: CerebotConfig) -> Result<Self, Error> {
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

    pub async fn run(&mut self) -> Result<RunResult, Box<dyn ErrorTrait>> {
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

        let mut context = SharedBotContext(Arc::new(InnerBotContext {
            db_context: DbContext {
                db_pool,
                redis_pool,
            },
            sender,
            state: Default::default(),
        }));

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
        tokio::spawn(async move {
            let ctx = heartbeat_ctx;
            let mut interval = Interval::new_interval(Duration::from_secs(2));
            while let Some(_) = interval.next().await {
                persist_event_queue(&ctx).await.unwrap();
            }
        });

        create_permissions(&context.db_context).await?;

        let dispatch = EventDispatch::default();
        dispatch
            .match_events(MatchAll())
            .handle(Box::new(LoggingHandler::init(&context)));

        // process messages and do stuff with the data
        let process_messages = async {
            let dispatch = &dispatch;
            let context = &context;
            receiver
                .take_while(|event| {
                    if context.state.should_restart() {
                        return ready(false);
                    }
                    match &**event {
                        Event::Reconnect(_) => {
                            // mark for restart on next message
                            context.state.restart();
                        }
                        _ => {}
                    }
                    ready(true)
                })
                .for_each_concurrent(Some(10), |event| {
                    async move {
                        // run event handlers
                        if let Err(err) = dispatch.dispatch(&event, context).await {
                            error!("Event handler failed: {}", err)
                        }
                    }
                })
                .await;
        };

        join(process_messages, process_errors).await;
        if context.state.should_restart() {
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

#[derive(Clone)]
pub struct SharedBotContext(Arc<InnerBotContext>);

impl Deref for SharedBotContext {
    type Target = InnerBotContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct InnerBotContext {
    pub db_context: DbContext,
    pub sender: ChatSender,
    pub state: BotState,
}

#[derive(Clone)]
pub struct DbContext {
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
    pub redis_pool: Pool<RedisConnectionManager>,
}
