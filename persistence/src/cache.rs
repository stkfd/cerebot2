use std::time::Duration;

use r2d2_redis::redis;

use async_trait::async_trait;

use crate::{with_redis, RedisPool, Result};

#[async_trait]
pub trait Cacheable<Id> {
    fn cache_key(&self) -> String;
    fn cache_key_from_id(id: Id) -> String;
    fn cache_life(&self) -> Duration;

    fn cache_set_blocking(&self, con: &mut dyn redis::ConnectionLike) -> Result<()>
    where
        for<'a> &'a Self: redis::ToRedisArgs,
    {
        redis::cmd("SETEX")
            .arg(self.cache_key())
            .arg(self.cache_life().as_secs())
            .arg(self)
            .query(con)?;
        Ok(())
    }

    async fn cache_set(&self, pool: &RedisPool) -> Result<()>
    where
        Id: 'async_trait,
        for<'a> &'a Self: redis::ToRedisArgs,
    {
        let mut cmd = redis::cmd("SETEX");
        cmd.arg(self.cache_key())
            .arg(self.cache_life().as_secs())
            .arg(self);
        with_redis(pool, move |conn| cmd.query(conn).map_err(Into::into)).await
    }

    fn cache_get_blocking(con: &mut dyn redis::ConnectionLike, id: Id) -> Result<Self>
    where
        Self: Sized + redis::FromRedisValue,
    {
        redis::cmd("GET")
            .arg(Self::cache_key_from_id(id))
            .query::<Self>(con)
            .map_err(Into::into)
    }

    async fn cache_get(pool: &RedisPool, id: Id) -> Result<Self>
    where
        Id: 'static + Send,
        Self: Sized + redis::FromRedisValue + Send + 'static,
    {
        with_redis(pool, move |conn| Self::cache_get_blocking(conn, id)).await
    }
}
