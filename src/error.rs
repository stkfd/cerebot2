use std::error::Error as ErrorTrait;
use std::fmt;

use r2d2_redis::redis;
use serde::export::Formatter;

use crate::event::LazyFetchError;
use crate::handlers::error::CommandError;
use crate::state::BotStateError;

#[derive(Debug)]
pub enum Error {
    /// IO errors
    Io(&'static str, std::io::Error),
    /// TOML deserialization errors
    Toml(&'static str, toml::de::Error),
    /// Configuration errors (missing values etc)
    Config(String),
    /// Client configuration error
    TmiConfig(String),
    Database(diesel::result::Error),
    ConnectionPool(r2d2::Error),
    Redis(redis::RedisError),
    Tmi(tmi_rs::Error),
    UserNotFound(i32),
    BotState(BotStateError),
    LazyFetch(LazyFetchError),
    Command(CommandError),
    Join(tokio::task::JoinError),
}

impl From<tmi_rs::Error> for Error {
    fn from(err: tmi_rs::Error) -> Self {
        Error::Tmi(err)
    }
}

impl From<diesel::result::Error> for Error {
    fn from(err: diesel::result::Error) -> Self {
        Error::Database(err)
    }
}

impl From<r2d2::Error> for Error {
    fn from(err: r2d2::Error) -> Self {
        Error::ConnectionPool(err)
    }
}

impl From<redis::RedisError> for Error {
    fn from(err: redis::RedisError) -> Self {
        Error::Redis(err)
    }
}

impl From<BotStateError> for Error {
    fn from(err: BotStateError) -> Self {
        Error::BotState(err)
    }
}

impl From<CommandError> for Error {
    fn from(err: CommandError) -> Self {
        Error::Command(err)
    }
}

impl From<LazyFetchError> for Error {
    fn from(err: LazyFetchError) -> Self {
        Error::LazyFetch(err)
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Error::Join(err)
    }
}

impl ErrorTrait for Error {
    fn source(&self) -> Option<&(dyn ErrorTrait + 'static)> {
        match self {
            Error::Io(_, inner) => Some(inner),
            Error::Toml(_, inner) => Some(inner),
            Error::Tmi(inner) => Some(inner),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(context, _) => write!(f, "{}", context),
            Error::Toml(context, _) => write!(f, "{}", context),
            Error::Config(details) => write!(f, "configuration error: {}", details),
            Error::TmiConfig(details) => write!(f, "tmi-rs configuration error: {}", details),
            Error::ConnectionPool(source) => write!(f, "connection pool error: {:?}", source),
            Error::Tmi(source) => write!(f, "tmi-rs error: {}", source),
            Error::Redis(source) => write!(f, "Redis error: {}", source),
            Error::Database(source) => write!(f, "Database error: {}", source),
            Error::UserNotFound(twitch_id) => {
                write!(f, "User with twitch ID {} not found", twitch_id)
            }
            Error::BotState(err) => write!(f, "Bot state error: {}", err),
            Error::LazyFetch(err) => write!(f, "Lazy fetch error caused by: {}", err.source()),
            Error::Command(err) => err.fmt(f),
            Error::Join(err) => write!(f, "Task join error: {}", err),
        }
    }
}
