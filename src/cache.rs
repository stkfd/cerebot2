use std::time::Duration;

use r2d2_redis::redis;

use crate::Result;

pub trait Cacheable<Id> {
    fn cache_key(&self) -> String;
    fn cache_key_from_id(id: Id) -> String;
    fn cache_life(&self) -> Duration;

    fn cache_set(&self, con: &mut dyn redis::ConnectionLike) -> Result<()>
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

    fn cache_get(con: &mut dyn redis::ConnectionLike, id: Id) -> Result<Self>
    where
        Self: Sized + redis::FromRedisValue,
    {
        redis::cmd("GET")
            .arg(Self::cache_key_from_id(id))
            .query::<Self>(con)
            .map_err(Into::into)
    }
}
