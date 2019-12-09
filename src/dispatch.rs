use std::fmt::Debug;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use fnv::FnvHashMap;
use futures::future::ready;
use futures::stream;
use futures::StreamExt;

use async_trait::async_trait;

use crate::state::BotContext;
use crate::sync::RwLock;
use crate::{AsyncResult, Result};
use futures::executor::block_on;

#[derive(Debug)]
pub struct EventDispatch<T: Send + Sync> {
    next_id: AtomicUsize,
    event_groups: Arc<RwLock<FnvHashMap<HandlerGroupId, EventHandlerGroup<T>>>>,
}

impl<T: Send + Sync> Default for EventDispatch<T> {
    fn default() -> Self {
        EventDispatch {
            next_id: Default::default(),
            event_groups: Default::default(),
        }
    }
}

impl<'a, T: Send + Sync> EventDispatch<T> {
    fn next_id(&self) -> HandlerGroupId {
        HandlerGroupId(self.next_id.fetch_add(1, Ordering::SeqCst))
    }

    pub fn register_matcher(&self, matcher: Box<dyn EventMatcher<T>>) -> HandlerGroupId {
        let group_id = self.next_id();
        block_on(self.event_groups.write()).insert(
            group_id,
            EventHandlerGroup {
                matcher,
                handlers: RwLock::new(vec![]),
            },
        );
        group_id
    }

    pub fn register_handler(&self, group_id: HandlerGroupId, handler: Box<dyn EventHandler<T>>) {
        if let Some(group) = block_on(self.event_groups.read()).get(&group_id) {
            block_on(group.handlers.write()).push(Arc::new(handler));
        }
    }

    pub async fn dispatch(&self, evt: T) -> Result<()> {
        let event_groups = self.event_groups.read().await;
        let mut futures = stream::iter(event_groups.values())
            .filter(|group| group.matcher.match_event(&evt))
            .map(|group| group.execute(&evt))
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
pub trait EventHandler<T>: Send + Sync + Debug {
    async fn create(ctx: &BotContext) -> Result<Self>
    where
        Self: Sized;

    async fn run(&self, event: &T) -> Result<()>;
}

#[inline]
pub fn ok() -> AsyncResult<'static, ()> {
    Box::pin(ready(Ok(())))
}

#[async_trait]
pub trait EventMatcher<T>: Send + Sync + Debug {
    async fn match_event(&self, e: &T) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandlerGroupId(usize);

#[derive(Debug)]
struct EventHandlerGroup<T> {
    matcher: Box<dyn EventMatcher<T>>,
    handlers: RwLock<Vec<Arc<Box<dyn EventHandler<T>>>>>,
}

impl<T> EventHandlerGroup<T> {
    async fn execute(&self, evt: &T) -> Result<()> {
        let handlers = self
            .handlers
            .read()
            .await
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for handler in handlers {
            handler.run(evt).await?;
        }
        Ok(())
    }
}

pub trait MatcherBuilder<T: Send + Sync> {
    fn match_events(
        &self,
        matcher: impl EventMatcher<T> + 'static,
    ) -> (HandlerGroupId, &EventDispatch<T>);
}

impl<T: Send + Sync> MatcherBuilder<T> for EventDispatch<T> {
    fn match_events(
        &self,
        matcher: impl EventMatcher<T> + 'static,
    ) -> (HandlerGroupId, &EventDispatch<T>) {
        (self.register_matcher(Box::new(matcher)), self)
    }
}

impl<T: Send + Sync> MatcherBuilder<T> for (HandlerGroupId, &EventDispatch<T>) {
    fn match_events(
        &self,
        matcher: impl EventMatcher<T> + 'static,
    ) -> (HandlerGroupId, &EventDispatch<T>) {
        (self.1.register_matcher(Box::new(matcher)), self.1)
    }
}

pub trait HandlerBuilder<T: Send + Sync> {
    fn handle(&self, handler: Box<dyn EventHandler<T>>) -> (HandlerGroupId, &EventDispatch<T>);
}

impl<T: Send + Sync> HandlerBuilder<T> for (HandlerGroupId, &EventDispatch<T>) {
    fn handle(&self, handler: Box<dyn EventHandler<T>>) -> (HandlerGroupId, &EventDispatch<T>) {
        self.1.register_handler(self.0, handler);
        (self.0, self.1)
    }
}

pub mod matchers {
    use tmi_rs::event::Event;

    use async_trait::async_trait;

    use crate::dispatch::EventMatcher;
    use crate::event::CbEvent;

    /// Match all events
    #[derive(Debug)]
    pub struct MatchAll;
    #[async_trait]
    impl<T: Send + Sync> EventMatcher<T> for MatchAll {
        async fn match_event(&self, _e: &T) -> bool {
            true
        }
    }

    /// Matches only channel messages and whispers
    #[derive(Debug)]
    pub struct MatchMessages;
    #[async_trait]
    impl EventMatcher<CbEvent> for MatchMessages {
        async fn match_event(&self, e: &CbEvent) -> bool {
            match &**e {
                Event::PrivMsg(_) | Event::Whisper(_) => true,
                _ => false,
            }
        }
    }
}
