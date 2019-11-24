pub use channel::*;
pub use chat_event::*;
pub use permissions::*;
pub use user::*;

mod channel;
mod chat_event;
mod permissions;
mod user;

macro_rules! impl_redis_bincode {
    ($model: ident) => {
        impl redis::FromRedisValue for $model {
            fn from_redis_value(v: &redis::Value) -> Result<Self, redis::RedisError> {
                if let redis::Value::Data(data) = v {
                    Ok(bincode::deserialize(&data).map_err(|_| {
                        redis::RedisError::from((
                            redis::ErrorKind::TypeError,
                            "Deserialization failed",
                        ))
                    })?)
                } else {
                    Err(redis::RedisError::from((
                        redis::ErrorKind::TypeError,
                        "Unexpected value type returned from Redis",
                    )))
                }
            }
        }

        impl redis::ToRedisArgs for &$model {
            fn write_redis_args<W>(&self, out: &mut W)
            where
                W: ?Sized + redis::RedisWrite,
            {
                out.write_arg(&bincode::serialize(self).unwrap());
            }
        }
    };
}
