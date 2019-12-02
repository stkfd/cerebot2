use std::sync::Arc;

use tmi_rs::event::Event;

pub struct CbEvent {
    event: Arc<Event<String>>,
}

impl CbEvent {
    pub fn inner(&self) -> &Arc<Event<String>> {
        &self.event
    }
}
