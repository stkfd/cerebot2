use std::sync::Arc;

use fnv::FnvHashMap;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use serde_json::{to_value, Value as JsonValue};
use tera::Tera;

use async_trait::async_trait;

use crate::db::commands::templates::CommandTemplate;
use crate::error::Error;
use crate::event::CbEvent;
use crate::state::{BotContext, DbContext};
use crate::Result;
use crate::util::split_args;

pub struct TemplateRenderer {
    tera: Tera,
    context_requests: FnvHashMap<i32, JsonValue>,
    context_providers: Vec<Arc<dyn ContextProvider>>,
}

impl TemplateRenderer {
    /// Create a new renderer instance and load the templates
    pub async fn create(db_context: &DbContext) -> Result<Self> {
        let tera = Tera::default();

        let mut instance = TemplateRenderer {
            tera,
            context_requests: Default::default(),
            context_providers: vec![],
        };
        instance.load_templates(db_context).await?;
        instance.register_context_provider(UserProvider);
        instance.register_context_provider(ChannelInfoProvider);
        instance.register_context_provider(ArgsProvider);

        Ok(instance)
    }

    /// Render a template.
    pub async fn render(
        &self,
        command_id: i32,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<String> {
        let context_request = self.context_requests.get(&command_id);
        let mut context = tera::Context::new();
        if let Some(context_request) = context_request {
            self.build_context(&mut context, context_request, event, bot)
                .await?;
        }
        debug!("Built template context: {:?}", context);
        self.tera
            .render(&format!("{}", command_id), &context)
            .map_err(Into::into)
    }

    pub fn register_context_provider(&mut self, provider_fn: impl ContextProvider + 'static) {
        self.context_providers.push(Arc::new(provider_fn));
    }

    async fn build_context(
        &self,
        context: &mut tera::Context,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<()> {
        let mut fut_unordered = self
            .context_providers
            .iter()
            .map(|provider| provider.run(request, event, bot))
            .collect::<FuturesUnordered<_>>();
        while let Some(result) = fut_unordered.next().await {
            if let Some((key, value)) = result? {
                context.insert(key, &value);
            }
        }
        Ok(())
    }

    /// Load the command templates from the database
    async fn load_templates(&mut self, db_context: &DbContext) -> Result<()> {
        use crate::schema::command_attributes;
        use diesel::query_dsl::*;
        use diesel::ExpressionMethods;

        let db_pool = db_context.db_pool.clone();
        let templates: Vec<CommandTemplate> = tokio::task::spawn_blocking(move || {
            command_attributes::table
                .filter(command_attributes::template.is_not_null())
                .select(CommandTemplate::COLUMNS)
                .load(&*db_pool.get()?)
                .map_err::<Error, _>(Into::into)
        })
        .await??;

        for template in templates {
            if let Some(request) = template.template_context {
                self.context_requests.insert(template.id, request);
            }
            self.tera
                .add_raw_template(&format!("{}", template.id), &template.template.unwrap())?;
        }
        Ok(())
    }
}

#[async_trait]
pub trait ContextProvider: Send + Sync {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>>;
}

pub struct UserProvider;
#[async_trait]
impl ContextProvider for UserProvider {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>> {
        if let JsonValue::Bool(true) = request["sender"] {
            Ok(Some((
                "sender".to_string(),
                to_value(event.user(bot).await?).unwrap(),
            )))
        } else {
            Ok(None)
        }
    }
}

pub struct ChannelInfoProvider;
#[async_trait]
impl ContextProvider for ChannelInfoProvider {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>> {
        if let JsonValue::Bool(true) = request["channel"] {
            Ok(Some((
                "channel".to_string(),
                to_value(event.channel_info(bot).await?.as_deref()).unwrap(),
            )))
        } else {
            Ok(None)
        }
    }
}

pub struct ArgsProvider;
#[async_trait]
impl ContextProvider for ArgsProvider {
    async fn run(
        &self,
        request: &JsonValue,
        event: &CbEvent,
        _bot: &BotContext,
    ) -> Result<Option<(String, JsonValue)>> {
        if let JsonValue::String(s) = &request["args"] {
            let message = event.message();
            let args_str = message.map(|msg| {
                if let Some(index) = msg.find(char::is_whitespace) {
                    msg.split_at(index).1
                } else {
                    ""
                }
            });
            if s == "complete" {
                Ok(Some(("args".to_string(), to_value(args_str).unwrap())))
            } else if s == "array" {
                let value = to_value(
                    args_str.map(|args| split_args(args))
                ).unwrap();
                Ok(Some(("args".to_string(), value)))
            } else {
                Ok(None)
            }
        }
        else {
            Ok(None)
        }
    }
}
