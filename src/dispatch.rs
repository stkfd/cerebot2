use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use fnv::FnvHashMap;
use futures::future::ready;
use futures::stream;
use futures::StreamExt;
use parking_lot::RwLock;
use tmi_rs::event::Event;

use async_trait::async_trait;

use crate::state::BotContext;
use crate::{AsyncResult, Result};

#[derive(Default)]
pub struct EventDispatch {
    next_id: AtomicUsize,
    event_groups: Arc<RwLock<FnvHashMap<HandlerGroupId, EventHandlerGroup>>>,
}

impl<'a> EventDispatch {
    fn next_id(&self) -> HandlerGroupId {
        HandlerGroupId(self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn register_matcher(&self, matcher: Box<dyn EventMatcher>) -> HandlerGroupId {
        let group_id = self.next_id();
        self.event_groups.write().insert(
            group_id,
            EventHandlerGroup {
                matcher,
                handlers: RwLock::new(vec![]),
            },
        );
        group_id
    }

    pub fn register_handler(&self, group_id: HandlerGroupId, handler: Box<dyn EventHandler>) {
        if let Some(group) = self.event_groups.read().get(&group_id) {
            group.handlers.write().push(Arc::new(handler));
        }
    }

    pub async fn dispatch(&self, evt: &Arc<Event<String>>, context: &BotContext) -> Result<()> {
        let event_groups = self.event_groups.read();
        let mut futures = stream::iter(event_groups.values())
            .filter(|group| group.matcher.match_event(&evt))
            .map(|group| group.execute(evt))
            .buffer_unordered(5);

        loop {
            let next: Option<_> = futures.next().await;
            if let Some(next) = next {
                next?;
            } else {
                break;
            }
        }
        Ok(())
    }
}

#[async_trait]
pub trait EventHandler: Send + Sync {
    async fn create(ctx: &BotContext) -> Result<Self>
    where
        Self: Sized;

    async fn run(&self, event: &Arc<Event<String>>) -> Result<()>;
}

#[inline]
pub fn ok() -> AsyncResult<'static, ()> {
    Box::pin(ready(Ok(())))
}

#[async_trait]
pub trait EventMatcher: Send + Sync {
    async fn match_event(&self, e: &Arc<Event<String>>) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandlerGroupId(usize);

struct EventHandlerGroup {
    matcher: Box<dyn EventMatcher>,
    handlers: RwLock<Vec<Arc<Box<dyn EventHandler>>>>,
}

impl EventHandlerGroup {
    async fn execute(&self, evt: &Arc<Event<String>>) -> Result<()> {
        let handlers = self.handlers.read().iter().cloned().collect::<Vec<_>>();
        for handler in handlers {
            handler.run(evt).await?;
        }
        Ok(())
    }
}

pub trait MatcherBuilder {
    fn match_events(
        &self,
        matcher: impl EventMatcher + 'static,
    ) -> (HandlerGroupId, &EventDispatch);
}

impl MatcherBuilder for EventDispatch {
    fn match_events(
        &self,
        matcher: impl EventMatcher + 'static,
    ) -> (HandlerGroupId, &EventDispatch) {
        (self.register_matcher(Box::new(matcher)), self)
    }
}

impl MatcherBuilder for (HandlerGroupId, &EventDispatch) {
    fn match_events(
        &self,
        matcher: impl EventMatcher + 'static,
    ) -> (HandlerGroupId, &EventDispatch) {
        (self.1.register_matcher(Box::new(matcher)), self.1)
    }
}

pub trait HandlerBuilder {
    fn handle(&self, handler: Box<dyn EventHandler>) -> (HandlerGroupId, &EventDispatch);
}

impl HandlerBuilder for (HandlerGroupId, &EventDispatch) {
    fn handle(&self, handler: Box<dyn EventHandler>) -> (HandlerGroupId, &EventDispatch) {
        self.1.register_handler(self.0, handler);
        (self.0, self.1)
    }
}

pub mod matchers {
    use std::sync::Arc;

    use tmi_rs::event::Event;

    use async_trait::async_trait;

    use crate::dispatch::EventMatcher;

    /// Match all events
    pub struct MatchAll;
    #[async_trait]
    impl EventMatcher for MatchAll {
        async fn match_event(&self, _e: &Arc<Event<String>>) -> bool {
            true
        }
    }

    /// Matches only channel messages and whispers
    pub struct MatchMessages;
    #[async_trait]
    impl EventMatcher for MatchMessages {
        async fn match_event(&self, e: &Arc<Event<String>>) -> bool {
            match &**e {
                Event::PrivMsg(_) | Event::Whisper(_) => true,
                _ => false
            }
        }
    }
}
