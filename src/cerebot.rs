use std::error::Error as ErrorTrait;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use futures::future::join;
use futures::{SinkExt, StreamExt};
use r2d2::Pool;
use r2d2_redis::RedisConnectionManager;
use tmi_rs::rate_limits::RateLimiterConfig;
use tmi_rs::{
    ChatSender, ClientMessage, TwitchChatConnection, TwitchClient, TwitchClientConfigBuilder,
};

use crate::config::CerebotConfig;
use crate::db::{Channel, persist_event_queue};
use crate::diesel::prelude::*;
use crate::dispatch::matchers::MatchAll;
use crate::dispatch::{EventDispatch, EventHandler, HandlerBuilder, MatcherBuilder};
use crate::error::Error;
use crate::handlers::LoggingHandler;
use crate::schema::channels;
use tokio::timer::Interval;
use std::time::Duration;

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

    pub async fn run(&mut self) -> Result<(), Box<dyn ErrorTrait>> {
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

        let mut context = BotContext {
            db_context: DbContext {
                db_pool,
                redis_pool,
            },
            sender,
        };

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
        for channel in startup_channels {
            context.sender.send(ClientMessage::join(channel.name)).await?;
        }

        let heartbeat_ctx = context.db_context.clone();
        tokio::spawn(async move {
            let ctx = heartbeat_ctx;
            let mut interval = Interval::new_interval(Duration::from_secs(2));
            while let Some(_) = interval.next().await {
                persist_event_queue(&ctx).await.unwrap();
            }
        });

        let dispatch = EventDispatch::default();
        dispatch
            .match_events(MatchAll())
            .handle(Box::new(LoggingHandler::init(&context)));

        // process messages and do stuff with the data
        let process_messages = async {
            let dispatch = &dispatch;
            let context = &context;
            receiver.for_each_concurrent(Some(100), |event| async move {
                if let Err(err) = dispatch.dispatch(&event, context).await {
                    error!("Event handler failed: {}", err)
                }
            }).await;
        };

        join(process_messages, process_errors).await;
        Ok(())
    }
}

#[derive(Clone)]
pub struct BotContext {
    pub db_context: DbContext,
    pub sender: ChatSender,
}

#[derive(Clone)]
pub struct DbContext {
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
    pub redis_pool: Pool<RedisConnectionManager>,
}
