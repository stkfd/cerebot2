use std::env;

use once_cell::sync::OnceCell;

pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub app_url: String,
    pub twitch_client_id: String,
    pub twitch_client_secret: String,
}

static CONFIG: OnceCell<Config> = OnceCell::new();

impl Config {
    pub fn get() -> &'static Config {
        CONFIG.get().expect("configuration is not initialized")
    }

    pub fn init() -> &'static Config {
        CONFIG.get_or_init(|| Config {
            database_url: env::var("DATABASE_URL").expect("database address"),
            redis_url: env::var("REDIS_URL").expect("redis address"),
            app_url: env::var("APP_URL").expect("redis address"),
            twitch_client_id: env::var("TWITCH_CLIENT_ID").expect("twitch client id"),
            twitch_client_secret: env::var("TWITCH_CLIENT_SECRET").expect("twitch client secret"),
        })
    }
}
