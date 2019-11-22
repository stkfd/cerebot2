#[macro_use]
extern crate log;

#[macro_use]
extern crate diesel;

use std::error::Error;

use crate::cerebot::Cerebot;
use crate::config::CerebotConfig;

mod cache;
mod cerebot;
mod config;
mod db;
mod dispatch;
mod error;
mod handlers;
mod schema;

#[tokio::main(multi_thread)]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut bot = Cerebot::create(CerebotConfig::load()?)?;
    bot.run().await
}
