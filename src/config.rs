use std::path::PathBuf;
use std::{env, fs};

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::error::Error;
use crate::Result;

#[derive(Debug, Clone, Builder)]
#[builder(derive(Serialize, Deserialize))]
pub struct CerebotConfig {
    auth_token: String,
    username: String,
    db: String,
    redis: String,
}

impl CerebotConfig {
    pub fn auth_token(&self) -> &str {
        &self.auth_token
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn db(&self) -> &str {
        &self.db
    }

    pub fn redis(&self) -> &str {
        &self.redis
    }

    /// Load the bot's configuration. Attempts to load config files, by order of preference:
    ///
    /// - $HOME/.cerebot.toml
    /// - /etc/cerebot/config.toml
    ///
    /// After loading any found config files, values from the following environment variables are
    /// used to override the values from the config files:
    ///
    /// - CEREBOT_AUTH_TOKEN
    /// - CEREBOT_USERNAME
    /// - DATABASE_URL
    pub fn load() -> Result<Self> {
        let mut config_path = None;

        if let Some(mut home_dir) = dirs::home_dir() {
            home_dir.push(".cerebot.toml");
            if home_dir.exists() {
                config_path.replace(home_dir);
            }
        }

        if config_path.is_none() {
            let etc_path = PathBuf::from("/etc/cerebot/config.toml");
            if etc_path.exists() {
                config_path.replace(etc_path);
            }
        }

        let mut builder = if let Some(config_path) = config_path {
            debug!("Using config file: {}", config_path.to_string_lossy());
            let file_content = &fs::read_to_string(config_path)
                .map_err(|err| Error::Io("Error loading config file", err))?;
            toml::from_str::<CerebotConfigBuilder>(file_content)
                .map_err(|err| Error::Toml("Error while deserializing config file", err))?
        } else {
            CerebotConfigBuilder::default()
        };

        if let Ok(auth_token) = env::var("CEREBOT_AUTH_TOKEN") {
            builder.auth_token(auth_token);
        }

        if let Ok(username) = env::var("CEREBOT_USERNAME") {
            builder.username(username);
        }

        if let Ok(db) = env::var("DATABASE_URL") {
            builder.db(db);
        }

        if let Ok(redis) = env::var("REDIS_URL") {
            builder.redis(redis);
        }

        builder.build().map_err(Error::Config)
    }
}
