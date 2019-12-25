use std::fmt;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use arc_swap::ArcSwap;
use fnv::FnvHashMap;
use futures::future::join3;
use serde::Serialize;
use tmi_rs::ChatSender;

use persistence::channel::Channel;
use persistence::DbContext;
use util::sync::RwLock;

use crate::state::command_store::CommandStore;
use crate::state::permission_store::PermissionStore;
use crate::template_renderer::TemplateRenderer;
use crate::Result;

pub mod command_store;
pub mod permission_store;

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
    pub permissions: ArcSwap<PermissionStore>,
    pub templates: ArcSwap<TemplateRenderer>,
    pub commands: ArcSwap<CommandStore>,
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
    pub async fn create(db_context: DbContext, sender: ChatSender) -> Result<Self> {
        let (permissions, commands, templates) = join3(
            PermissionStore::load(&db_context),
            CommandStore::load(&db_context),
            TemplateRenderer::create(&db_context),
        )
        .await;
        Ok(BotContext(Arc::new(InnerBotContext {
            db_context,
            sender,
            state: Default::default(),
            permissions: ArcSwap::from_pointee(permissions?),
            templates: ArcSwap::from_pointee(templates?),
            commands: ArcSwap::from_pointee(commands?),
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

    pub async fn reload_permissions(&self) -> Result<()> {
        self.permissions
            .store(Arc::new(PermissionStore::load(&self.db_context).await?));
        Ok(())
    }

    pub async fn reload_templates(&self) -> Result<()> {
        self.templates
            .store(Arc::new(TemplateRenderer::create(&self.db_context).await?));
        Ok(())
    }

    pub async fn reload_commands(&self) -> Result<()> {
        self.commands
            .store(Arc::new(CommandStore::load(&self.db_context).await?));
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChannelInfo {
    /// persisted channel data from the database
    pub data: Channel,
    /// channel state - should be available after the ROOMSTATE event
    /// on join is received
    pub state: Option<ChannelState>,
}

#[derive(Debug, Clone, Serialize)]
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
