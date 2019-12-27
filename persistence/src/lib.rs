#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;
#[macro_use]
extern crate diesel_migrations;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use r2d2::Pool;
use std::num::TryFromIntError;
use thiserror::Error;

pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub type RedisPool = darkredis::ConnectionPool;

embed_migrations!("../migrations");

#[derive(Clone)]
pub struct DbContext {
    pub db_pool: DbPool,
    pub redis_pool: RedisPool,
}

impl DbContext {
    pub async fn create(db_address: &str, redis_address: &str) -> Result<DbContext> {
        let manager = ConnectionManager::<PgConnection>::new(db_address);
        let db_pool = r2d2::Pool::builder()
            .build(manager)
            .map_err(Error::ConnectionPool)?;
        let redis_pool =
            darkredis::ConnectionPool::create(redis_address.to_string(), None, 3).await?;

        Ok(DbContext {
            db_pool,
            redis_pool,
        })
    }

    pub fn run_pending_migrations(&self) -> Result<()> {
        embedded_migrations::run(&*self.db_pool.get()?)?;
        Ok(())
    }
}

pub mod cache;
pub mod channel;
pub mod chat_event;
pub mod commands;
pub mod permissions;
pub mod schema;
pub mod user;

#[macro_use]
pub mod redis_values;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("Database error: {0}")]
    AsyncDiesel(#[from] tokio_diesel::AsyncError),
    #[error("Connection pool error: {0}")]
    ConnectionPool(#[from] r2d2::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] darkredis::Error),
    #[error("Redis serialization failed: {0}")]
    RedisSerialization(#[from] bincode::Error),
    #[error("Blocking task join error")]
    Join(#[from] tokio::task::JoinError),
    #[error("User with twitch ID {0} not found in database")]
    UserNotFound(i32),
    #[error("Expiry duration out of range ({0})")]
    InvalidRedisExpiry(#[source] TryFromIntError),
    #[error("Migration running failed: {0}")]
    MigrationError(#[from] diesel_migrations::RunMigrationsError),
}
type Result<T> = std::result::Result<T, Error>;

async fn with_db<O, F>(pool: &DbPool, func: F) -> Result<O>
where
    O: Send + 'static,
    F: FnOnce(&PgConnection) -> Result<O> + Send + 'static,
{
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = &mut *pool.get()?;
        func(conn)
    })
    .await?
}
