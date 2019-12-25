use persistence::commands::attributes::InsertCommandAttributes;

use crate::state::BotContext;
use crate::Result;

/// Match any characters that are not allowed in user input. Can be used to strip undesired unicode
/// symbols.
pub fn disallowed_input_chars(c: char) -> bool {
    !char::is_alphanumeric(c) && !char::is_ascii_punctuation(&c) && !char::is_ascii_whitespace(&c)
}

pub fn split_args(args_str: &str) -> Vec<String> {
    args_str
        .replace(disallowed_input_chars, "")
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

pub async fn initialize_command(
    ctx: &BotContext,
    data: InsertCommandAttributes<'static>,
    required_permission_names: Vec<impl AsRef<str> + Send + 'static>,
    aliases: Vec<impl AsRef<str> + Send + Sync + 'static>,
) -> Result<()> {
    let required_permissions: Vec<i32> = ctx
        .permissions
        .load()
        .get_permissions(required_permission_names.iter().map(|s| s.as_ref()))?
        .iter()
        .map(|permission| permission.id)
        .collect();

    persistence::commands::util::initialize_command(
        &ctx.db_context,
        data,
        required_permissions,
        aliases,
    )
    .await
    .map_err(Into::into)
}
