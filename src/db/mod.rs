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

pub mod channel;
pub mod chat_event;
pub mod commands;
pub mod permissions;
pub mod user;
