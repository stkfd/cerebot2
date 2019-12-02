use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use fnv::FnvHashMap;
use parking_lot::{MappedRwLockReadGuard, RwLock, RwLockReadGuard};
use r2d2::Pool;
use r2d2_redis::RedisConnectionManager;
use tmi_rs::ChatSender;

use crate::db::{Channel, PermissionStore};
use crate::Result;

#[derive(Clone)]
pub struct BotContext(Arc<InnerBotContext>);

impl Deref for BotContext {
    type Target = InnerBotContext;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct InnerBotContext {
    pub db_context: DbContext,
    pub sender: ChatSender,
    pub state: BotState,
    pub permissions: RwLock<PermissionStore>,
}

#[derive(Clone)]
pub struct DbContext {
    pub db_pool: Pool<ConnectionManager<PgConnection>>,
    pub redis_pool: Pool<RedisConnectionManager>,
}

pub struct BotState {
    channels: RwLock<FnvHashMap<String, Arc<ChannelInfo>>>,
    restart: AtomicBool,
}

impl Default for BotState {
    fn default() -> Self {
        BotState {
            channels: Default::default(),
            restart: AtomicBool::new(false),
        }
    }
}

impl BotContext {
    pub async fn create(
        db_pool: Pool<ConnectionManager<PgConnection>>,
        redis_pool: Pool<RedisConnectionManager>,
        sender: ChatSender,
    ) -> Result<Self> {
        let db_context = DbContext {
            db_pool,
            redis_pool,
        };
        let permissions = RwLock::new(PermissionStore::load(&db_context).await?);
        Ok(BotContext(Arc::new(InnerBotContext {
            db_context,
            sender,
            state: Default::default(),
            permissions,
        })))
    }

    /// Restarts the bot after handling the current message
    pub fn restart(&self) {
        self.state.restart.store(true, Ordering::SeqCst)
    }

    /// Check whether a restart is scheduled
    pub fn should_restart(&self) -> bool {
        self.state.restart.load(Ordering::SeqCst)
    }

    pub fn get_channel(&self, name: &str) -> Option<MappedRwLockReadGuard<'_, Arc<ChannelInfo>>> {
        let guard = self.state.channels.read();
        RwLockReadGuard::try_map(guard, |map| map.get(name)).ok()
    }

    pub fn update_channel(&self, channel_info: ChannelInfo) {
        self.state
            .channels
            .write()
            .insert(channel_info.data.name.to_owned(), Arc::new(channel_info));
    }
}

pub struct ChannelInfo {
    /// persisted channel data from the database
    pub data: Channel,
    /// channel state - should be available after the ROOMSTATE event
    /// on join is received
    pub state: Option<ChannelState>,
}

pub struct ChannelState {
    pub slow: Option<usize>,
    pub followers_only: Option<isize>,
    pub subs_only: bool,
    pub r9k: bool,
    pub emote_only: bool,
}
