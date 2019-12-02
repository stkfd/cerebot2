#![allow(dead_code)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use std::error::Error as StdError;
use std::pin::Pin;
use std::result::Result as StdResult;

use futures::Future;

use crate::cerebot::{Cerebot, RunResult};
use crate::config::CerebotConfig;
use crate::error::Error;

mod cache;
mod cerebot;
mod config;
mod db;
mod dispatch;
mod error;
mod event;
mod handlers;
mod schema;
mod state;

type Result<T> = StdResult<T, Error>;
type AsyncResult<'asn, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'asn>>;

#[tokio::main(threaded_scheduler)]
async fn main() -> StdResult<(), Box<dyn StdError>> {
    env_logger::init();
    let mut bot = Cerebot::create(CerebotConfig::load()?)?;
    while let RunResult::Restart = bot.run().await? {}
    Ok(())
}
