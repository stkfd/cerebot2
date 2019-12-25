use serde::{Deserialize, Serialize};
use tokio_diesel::AsyncRunQueryDsl;

use crate::schema::*;
use crate::DbPool;
use crate::Result;

#[derive(Serialize, Deserialize, Debug, Clone, Queryable)]
pub struct CommandAlias {
    pub name: String,
    pub command_id: i32,
}

impl CommandAlias {
    pub async fn all(pool: &DbPool) -> Result<Vec<CommandAlias>> {
        command_aliases::table
            .load_async(pool)
            .await
            .map_err(Into::into)
    }
}
