#![allow(dead_code)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use std::pin::Pin;
use std::result::Result as StdResult;

use dotenv::dotenv;
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
mod sync;

type Result<T> = StdResult<T, Error>;
type AsyncResult<'asn, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'asn>>;

fn main() {
    dotenv().ok();
    env_logger::init();
    let mut runtime = tokio::runtime::Builder::new()
        .threaded_scheduler()
        .enable_all()
        .num_threads(8)
        .build()
        .unwrap();

    runtime.block_on(async move {
        let config = CerebotConfig::load().unwrap();
        let mut bot = Cerebot::create(config).unwrap();
        while let RunResult::Restart = bot.run().await.unwrap() {}
    });
}
