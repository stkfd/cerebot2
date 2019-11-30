#![allow(dead_code)]

#[macro_use]
extern crate log;

#[macro_use]
extern crate diesel;

use std::error::Error;

use crate::cerebot::{Cerebot, RunResult};
use crate::config::CerebotConfig;

mod cache;
mod cerebot;
mod config;
mod db;
mod dispatch;
mod error;
mod handlers;
mod schema;
mod state;

#[tokio::main(threaded_scheduler)]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    let mut bot = Cerebot::create(CerebotConfig::load()?)?;
    while let RunResult::Restart = bot.run().await? {}
    Ok(())
}
