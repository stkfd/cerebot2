use diesel::{ExpressionMethods, QueryDsl, Queryable};
use tokio_diesel::AsyncRunQueryDsl;

use crate::schema::command_attributes;
use crate::DbPool;
use crate::Result;

#[derive(Debug, Clone, Queryable, QueryableByName)]
#[table_name = "command_attributes"]
pub struct CommandTemplate {
    pub id: i32,
    pub template: Option<String>,
    pub template_context: Option<serde_json::Value>,
}

pub type TemplateColumns = (
    command_attributes::id,
    command_attributes::template,
    command_attributes::template_context,
);

impl CommandTemplate {
    pub const COLUMNS: TemplateColumns = (
        command_attributes::id,
        command_attributes::template,
        command_attributes::template_context,
    );

    pub async fn all(pool: &DbPool) -> Result<Vec<CommandTemplate>> {
        command_attributes::table
            .filter(command_attributes::template.is_not_null())
            .select(CommandTemplate::COLUMNS)
            .load_async(pool)
            .await
            .map_err(Into::into)
    }
}
