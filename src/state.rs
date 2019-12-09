use std::fmt;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use diesel::r2d2::ConnectionManager;
use diesel::PgConnection;
use fnv::FnvHashMap;
use r2d2::Pool;
use r2d2_redis::RedisConnectionManager;
use tmi_rs::ChatSender;

use crate::db::{Channel, PermissionStore};
use crate::sync::RwLock;
use crate::Result;

#[derive(Clone)]
pub struct BotContext(Arc<InnerBotContext>);

impl fmt::Debug for BotContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BotContext")
            .field("state", &self.state)
            .field("permissions", &self.permissions)
            .finish()
    }
}

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

#[derive(Debug)]
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

    pub async fn get_channel(&self, name: &str) -> Option<Arc<ChannelInfo>> {
        self.state.channels.read().await.get(name).cloned()
    }

    pub async fn update_channel(&self, channel_info: ChannelInfo) {
        self.state
            .channels
            .write()
            .await
            .insert(channel_info.data.name.to_owned(), Arc::new(channel_info));
    }
}

#[derive(Debug, Clone)]
pub struct ChannelInfo {
    /// persisted channel data from the database
    pub data: Channel,
    /// channel state - should be available after the ROOMSTATE event
    /// on join is received
    pub state: Option<ChannelState>,
}

#[derive(Debug, Clone)]
pub struct ChannelState {
    pub slow: Option<usize>,
    pub followers_only: Option<isize>,
    pub subs_only: bool,
    pub r9k: bool,
    pub emote_only: bool,
}

#[derive(Debug)]
pub enum BotStateError {
    MissingChannel,
    MissingCommandAttributes(String),
    PermissionNotFound(String),
}

impl std::error::Error for BotStateError {}

impl fmt::Display for BotStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BotStateError::MissingChannel => write!(f, "Channel data was unavailable"),
            BotStateError::MissingCommandAttributes(cmd) => write!(
                f,
                "Command attributes for {} are missing, check command boot function",
                cmd
            ),
            BotStateError::PermissionNotFound(permission) => {
                write!(f, "Tried to load non-existent permission: {}", permission)
            }
        }
    }
}
