use diesel::Queryable;

use crate::schema::command_attributes::dsl::*;

#[derive(Debug, Clone, Queryable)]
pub struct CommandTemplate {
    pub id: i32,
    pub template: Option<String>,
    pub template_context: Option<serde_json::Value>,
}

pub type TemplateColumns = (id, template, template_context);

impl CommandTemplate {
    pub const COLUMNS: TemplateColumns = (id, template, template_context);
}
