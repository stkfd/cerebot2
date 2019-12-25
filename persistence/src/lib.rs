#[macro_use]
extern crate diesel;
#[macro_use]
extern crate log;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use r2d2::Pool;
use r2d2_redis::redis::ConnectionLike;
use r2d2_redis::{redis, RedisConnectionManager};
use thiserror::Error;

pub type DbPool = Pool<ConnectionManager<PgConnection>>;
pub type RedisPool = Pool<RedisConnectionManager>;

#[derive(Clone)]
pub struct DbContext {
    pub db_pool: DbPool,
    pub redis_pool: RedisPool,
}

impl DbContext {
    pub fn create(db_connection: &str, redis_connection: &str) -> Result<DbContext> {
        let manager = ConnectionManager::<PgConnection>::new(db_connection);
        let db_pool = r2d2::Pool::builder()
            .build(manager)
            .map_err(Error::ConnectionPool)?;
        let redis_pool = r2d2::Pool::builder()
            .build(r2d2_redis::RedisConnectionManager::new(redis_connection).map_err(Error::Redis)?)
            .map_err(Error::ConnectionPool)?;

        Ok(DbContext {
            db_pool,
            redis_pool,
        })
    }
}

#[macro_export]
macro_rules! impl_redis_bincode {
    ($model: ty) => {
        impl r2d2_redis::redis::FromRedisValue for $model {
            fn from_redis_value(
                v: &r2d2_redis::redis::Value,
            ) -> std::result::Result<Self, r2d2_redis::redis::RedisError> {
                if let r2d2_redis::redis::Value::Data(data) = v {
                    Ok(bincode::deserialize(&data).map_err(|_| {
                        r2d2_redis::redis::RedisError::from((
                            r2d2_redis::redis::ErrorKind::TypeError,
                            "Deserialization failed",
                        ))
                    })?)
                } else {
                    Err(r2d2_redis::redis::RedisError::from((
                        r2d2_redis::redis::ErrorKind::TypeError,
                        "Unexpected value type returned from Redis",
                    )))
                }
            }
        }

        impl r2d2_redis::redis::ToRedisArgs for &$model {
            fn write_redis_args<W>(&self, out: &mut W)
            where
                W: ?Sized + r2d2_redis::redis::RedisWrite,
            {
                out.write_arg(&bincode::serialize(self).unwrap());
            }
        }
    };
}

pub mod cache;
pub mod channel;
pub mod chat_event;
pub mod commands;
pub mod permissions;
pub mod schema;
pub mod user;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),
    #[error("Database error: {0}")]
    AsyncDiesel(#[from] tokio_diesel::AsyncError),
    #[error("Connection pool error: {0}")]
    ConnectionPool(#[from] r2d2::Error),
    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("Blocking task join error")]
    Join(#[from] tokio::task::JoinError),
    #[error("User with twitch ID {0} not found in database")]
    UserNotFound(i32),
}
type Result<T> = std::result::Result<T, Error>;

async fn with_redis<O, F>(pool: &RedisPool, func: F) -> Result<O>
where
    O: Send + 'static,
    F: FnOnce(&mut dyn ConnectionLike) -> Result<O> + Send + 'static,
{
    let pool = pool.clone();
    tokio::task::spawn_blocking(move || {
        let conn = &mut *pool.get()?;
        func(conn)
    })
    .await?
}

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
