use serde::{Deserialize, Serialize};
use tokio_diesel::AsyncRunQueryDsl;

use crate::schema::*;
use crate::DbPool;
use crate::Result;
use diesel::sql_query;
use diesel::sql_types::Integer;

#[derive(Serialize, Deserialize, Debug, Clone, Queryable, QueryableByName)]
#[table_name = "command_aliases"]
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

    /// Get a list of all channel commands active for the given channel
    pub async fn channel_commands(pool: &DbPool, channel_id: i32) -> Result<Vec<CommandAlias>> {
        sql_query(
                "select distinct on (att.id) ca.name, ca.command_id \
                from command_attributes att \
                left join channel_command_config conf on conf.channel_id = $1 and att.id = conf.command_id \
                left join command_aliases ca on att.id = ca.command_id \
                where ca.name is not null and enabled = true and (default_active = true or conf.active = true) \
                order by att.id, ca.name desc;"
            )
            .bind::<Integer, _>(channel_id)
            .load_async::<CommandAlias>(pool)
            .await
            .map_err(Into::into)
    }
}
