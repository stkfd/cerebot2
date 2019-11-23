use std::fmt::Debug;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use fnv::FnvHashMap;
use futures::future::ready;
use futures::stream;
use futures::{Future, Stream, StreamExt, TryStreamExt};
use parking_lot::RwLock;
use tmi_rs::event::Event;
use tmi_rs::ClientMessage;

use crate::cerebot::BotContext;
use crate::error::Error;

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

    pub async fn dispatch(
        &self,
        evt: &Arc<Event<String>>,
        context: &BotContext,
    ) -> Result<(), Error> {
        let event_groups = self.event_groups.read();
        let mut futures = stream::iter(event_groups.values())
            .filter(|group| group.matcher.match_event(&evt))
            .map(|group| group.execute(evt, context))
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

pub trait EventHandler: Send + Sync {
    fn init(ctx: &BotContext) -> Self
    where
        Self: Sized;

    fn run(&self, event: &Arc<Event<String>>) -> Result<Response, Error>;
}

#[allow(dead_code)]
pub const fn ok_now() -> Result<Response, Error> {
    Ok(Response::OkNow)
}

#[allow(dead_code)]
pub fn ok_fut(
    fut: impl Future<Output = Result<(), Error>> + Send + 'static,
) -> Result<Response, Error> {
    Ok(Response::OkFuture(Box::pin(fut)))
}

#[allow(dead_code)]
pub fn respond_with(
    stream: impl Stream<Item = Result<ClientMessage<String>, Error>> + Send + 'static,
) -> Result<Response, Error> {
    Ok(Response::Response(Box::pin(stream)))
}

#[allow(dead_code)]
pub enum Response {
    OkNow,
    OkFuture(Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>),
    Response(Pin<Box<dyn Stream<Item = Result<ClientMessage<String>, Error>> + Send>>),
}

pub trait EventMatcher: Send + Sync {
    fn match_event(&self, e: &Arc<Event<String>>) -> Pin<Box<dyn Future<Output = bool> + Send>>;
}
impl<T, Fut> EventMatcher for T
where
    for<'x> T: Fn(&'x Arc<Event<String>>) -> Fut + Send + Sync,
    Fut: Future<Output = bool> + 'static + Send,
{
    fn match_event(&self, e: &Arc<Event<String>>) -> Pin<Box<dyn Future<Output = bool> + Send>> {
        Pin::from(Box::new(self(e)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandlerGroupId(usize);

struct EventHandlerGroup {
    matcher: Box<dyn EventMatcher>,
    handlers: RwLock<Vec<Arc<Box<dyn EventHandler>>>>,
}

impl EventHandlerGroup {
    async fn execute(&self, evt: &Arc<Event<String>>, context: &BotContext) -> Result<(), Error> {
        let handlers = self.handlers.read().iter().cloned().collect::<Vec<_>>();
        for handler in handlers {
            match handler.run(evt)? {
                Response::Response(stream) => {
                    stream
                        .inspect_err(|err| error!("Error in message handler response: {}", err))
                        .into_stream()
                        .filter_map(|msg_result| ready(msg_result.ok()))
                        .map(Ok)
                        .forward(&mut context.sender.clone())
                        .await
                        .map_err(Error::Tmi)?;
                }
                Response::OkFuture(fut) => {
                    fut.await?;
                }
                Response::OkNow => {}
            }
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
    use std::pin::Pin;
    use std::sync::Arc;

    use futures::future::ready;
    use tmi_rs::event::Event;
    use tokio::future::Future;

    use crate::dispatch::EventMatcher;

    pub struct MatchAll();
    impl EventMatcher for MatchAll {
        fn match_event(
            &self,
            _e: &Arc<Event<String>>,
        ) -> Pin<Box<dyn Future<Output = bool> + Send>> {
            Box::pin(ready(true))
        }
    }
}
