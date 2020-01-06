use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwapOption;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use async_trait::async_trait;
use persistence::cache::Cacheable;
use persistence::impl_redis_bincode;
use unogs_client::genre_ids::Genre;
use unogs_client::{List, QuotaState, UnogsClient};

use crate::config::CerebotConfig;
use crate::error::Error;
use crate::handlers::commands::error::CommandError;
use crate::handlers::{CommandContext, CommandHandler};
use crate::state::BotContext;
use crate::util::initialize_command;
use crate::Result;
use persistence::commands::attributes::InsertCommandAttributes;
use rand::{thread_rng, Rng};

#[derive(Debug)]
pub struct NetflixCommandHandler {
    ctx: BotContext,
    api_client: OnceCell<UnogsClient>,
    genre_list: ArcSwapOption<GenreList>,
    quota: ArcSwapOption<QuotaState>,
}

const NAME: &str = "netflix";

#[async_trait]
impl CommandHandler for NetflixCommandHandler {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        let redis = &self.ctx.db_context.redis_pool;

        let is_loaded = self.genre_list.load().is_some();

        if is_loaded {
            if !GenreList::cache_exists(redis, ()).await? {
                self.fetch_genre_list().await?;
            }
        } else if let Some(list) = GenreList::cache_get(redis, ()).await? {
            self.genre_list.store(Some(Arc::new(list)));
        } else {
            self.fetch_genre_list().await?;
        }

        let genre_list = self.genre_list.load().clone().unwrap();

        let msg = {
            let mut rng = thread_rng();
            let genre = &genre_list[rng.gen_range(0, genre_list.len())];
            let id = &genre.ids[rng.gen_range(0, genre.ids.len())];

            format!(
                "{}: https://www.netflix.com/browse/genre/{}",
                htmlescape::decode_html(&genre.name)
                    .as_ref()
                    .unwrap_or(&genre.name),
                id
            )
        };
        cmd.reply(&msg, &self.ctx.sender).await
    }

    async fn create(bot: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        initialize_command(
            &bot,
            InsertCommandAttributes {
                handler_name: NAME.into(),
                description: Some("Get a random netflix genre".into()),
                enabled: true,
                default_active: true,
                cooldown: Some(10000),
                whisper_enabled: true,
            },
            Vec::<String>::new(),
            vec!["nfg", "netflixgenre"],
        )
        .await?;

        Ok(Box::new(NetflixCommandHandler {
            ctx: bot.clone(),
            api_client: Default::default(),
            genre_list: Default::default(),
            quota: ArcSwapOption::default(),
        }))
    }
}

impl NetflixCommandHandler {
    fn get_api_client(&self) -> Result<&UnogsClient> {
        let api_client = self.api_client.get_or_try_init::<_, Error>(|| {
            let key = CerebotConfig::get()?
                .rapidapi_key()
                .ok_or(CommandError::RapidApiNotConfigured)?;
            Ok(UnogsClient::new(key).map_err(CommandError::UnogsError)?)
        })?;
        Ok(api_client)
    }

    async fn fetch_genre_list(&self) -> Result<()> {
        if let Some(quota) = &*self.quota.load() {
            if quota.requests_remaining <= 0 {
                return Err(CommandError::RapidApiQuotaLimit.into());
            }
        }

        let response = self
            .get_api_client()?
            .genre_ids()
            .await
            .map_err(CommandError::UnogsError)?;
        self.quota.store(Some(Arc::new(response.quota)));
        let list = GenreList::from(response.content);
        list.cache_set(&self.ctx.db_context.redis_pool).await?;
        self.genre_list.store(Some(Arc::new(list)));
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct SerializableGenre {
    name: String,
    ids: Vec<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct GenreList(Vec<SerializableGenre>);

impl Deref for GenreList {
    type Target = Vec<SerializableGenre>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<List<Genre>> for GenreList {
    fn from(source: List<Genre>) -> Self {
        GenreList(
            source
                .items
                .into_iter()
                .map(|genre| SerializableGenre {
                    name: genre.name,
                    ids: genre.ids,
                })
                .collect(),
        )
    }
}

impl Cacheable<()> for GenreList {
    fn cache_key(&self) -> String {
        "netflix_genres".to_string()
    }

    fn cache_key_from_id(_: ()) -> String {
        "netflix_genres".to_string()
    }

    fn cache_life(&self) -> Duration {
        Duration::from_secs(60 * 60 * 24)
    }
}

impl_redis_bincode!(GenreList);
