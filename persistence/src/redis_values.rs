use crate::Result;

pub trait ToRedisValue {
    fn to_redis(&self) -> Result<Vec<u8>>;
}

pub trait FromRedisValue {
    fn from_redis(value: &[u8]) -> Result<Self>
    where
        Self: Sized;
}

#[macro_export]
macro_rules! impl_redis_bincode {
    ($model: ty) => {
        impl persistence::redis_values::ToRedisValue for $model {
            fn to_redis(&self) -> std::result::Result<Vec<u8>, persistence::Error> {
                bincode::serialize(self).map_err(Into::into)
            }
        }

        impl persistence::redis_values::FromRedisValue for $model {
            fn from_redis(value: &[u8]) -> std::result::Result<Self, persistence::Error> {
                bincode::deserialize(value).map_err(Into::into)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_redis_bincode_int {
    ($model: ty) => {
        impl crate::redis_values::ToRedisValue for $model {
            fn to_redis(&self) -> crate::Result<Vec<u8>> {
                bincode::serialize(self).map_err(Into::into)
            }
        }

        impl crate::redis_values::FromRedisValue for $model {
            fn from_redis(value: &[u8]) -> crate::Result<Self> {
                bincode::deserialize(value).map_err(Into::into)
            }
        }
    };
}
