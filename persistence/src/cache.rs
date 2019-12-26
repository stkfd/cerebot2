use std::convert::TryInto;
use std::time::Duration;

use async_trait::async_trait;

use crate::redis_values::{FromRedisValue, ToRedisValue};
use crate::RedisPool;
use crate::{Error, Result};

#[async_trait]
pub trait Cacheable<Id> {
    fn cache_key(&self) -> String;
    fn cache_key_from_id(id: Id) -> String;
    fn cache_life(&self) -> Duration;

    async fn cache_set(&self, pool: &RedisPool) -> Result<()>
    where
        Id: 'async_trait,
        Self: ToRedisValue,
    {
        // TODO: add specific error for int conversion overflow?
        pool.get()
            .await
            .set_and_expire_seconds(
                self.cache_key(),
                self.to_redis()?,
                self.cache_life()
                    .as_secs()
                    .try_into()
                    .map_err(Error::InvalidRedisExpiry)?,
            )
            .await
            .map_err(Into::into)
    }

    async fn cache_get(pool: &RedisPool, id: Id) -> Result<Option<Self>>
    where
        Id: 'static + Send,
        Self: Sized + FromRedisValue + Send + 'static,
    {
        if let Some(cached_bin) = pool.get().await.get(Self::cache_key_from_id(id)).await? {
            Ok(Some(Self::from_redis(&cached_bin)?))
        } else {
            Ok(None)
        }
    }
}
