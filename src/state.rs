use fnv::FnvHashMap;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct BotState {
    channels: FnvHashMap<String, ChannelState>,
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

impl BotState {
    /// Restarts the bot after handling the current message
    pub fn restart(&self) {
        self.restart.store(true, Ordering::SeqCst)
    }

    pub fn should_restart(&self) -> bool {
        self.restart.load(Ordering::SeqCst)
    }
}

pub struct ChannelState {
    pub slow: Option<usize>,
    pub followers_only: Option<isize>,
    pub subs_only: bool,
    pub r9k: bool,
    pub emote_only: bool,
}
