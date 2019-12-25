use thiserror::Error;

use crate::event::LazyFetchError;
use crate::handlers::error::CommandError;
use crate::state::BotStateError;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    /// IO errors
    #[error("{0}")]
    Io(&'static str, std::io::Error),
    /// TOML deserialization errors
    #[error("{0}")]
    Toml(&'static str, #[source] toml::de::Error),
    /// Configuration errors (missing values etc)
    #[error("Bot configuration error: {0}")]
    Config(String),
    /// Client configuration error
    #[error("TMI config error: {0}")]
    TmiConfig(String),
    #[error("Chat connector error: {0}")]
    Tmi(#[from] tmi_rs::Error),
    #[error("Internal bot state error: {0}")]
    BotState(#[from] BotStateError),
    #[error("{0}")]
    LazyFetch(#[from] LazyFetchError),
    #[error("Command error: {0}")]
    Command(#[from] CommandError),
    #[error("Blocking task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    #[error("Template error: {0}")]
    TemplateError(#[from] tera::Error),
    #[error("{0}")]
    PersistenceError(#[from] persistence::Error),
}
