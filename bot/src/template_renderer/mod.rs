use std::sync::Arc;

use fnv::FnvHashMap;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use serde_json::Value as JsonValue;
use tera::Tera;

use persistence::commands::templates::CommandTemplate;
use persistence::DbContext;

use crate::event::CbEvent;
use crate::state::BotContext;
use crate::Result;

use self::context_providers::*;

mod context_providers;

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
        let templates: Vec<CommandTemplate> = CommandTemplate::all(&db_context.db_pool).await?;

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
