use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::task;

use crate::schema::*;
use crate::state::DbContext;
use crate::Result;

#[derive(Serialize, Deserialize, Debug, Clone, Queryable)]
pub struct CommandAlias {
    pub name: String,
    pub command_id: i32,
}

impl CommandAlias {
    pub async fn all(ctx: &DbContext) -> Result<Vec<CommandAlias>> {
        let ctx = ctx.clone();
        task::spawn_blocking(move || {
            let pg = &*ctx.db_pool.get()?;
            command_aliases::table.load(pg).map_err(Into::into)
        })
        .await?
    }
}
