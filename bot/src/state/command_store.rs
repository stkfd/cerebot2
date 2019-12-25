use fnv::FnvHashMap;

use persistence::commands::alias::CommandAlias;
use persistence::commands::attributes::CommandAttributes;
use persistence::DbContext;

use crate::Result;

pub struct CommandStore {
    /// Map of command alias -> command_id pairs
    aliases: FnvHashMap<String, i32>,
    /// Map of command_id -> CommandAttributes to hold command configurations
    commands: FnvHashMap<i32, CommandAttributes>,
}

impl CommandStore {
    pub async fn load(ctx: &DbContext) -> Result<Self> {
        let aliases = CommandAlias::all(&ctx.db_pool)
            .await?
            .into_iter()
            .map(|alias| (alias.name, alias.command_id))
            .collect();

        let commands = CommandAttributes::all(&ctx.db_pool)
            .await?
            .into_iter()
            .map(|attr| (attr.id, attr))
            .collect();

        Ok(CommandStore { aliases, commands })
    }

    pub fn get_by_alias(&self, name: &str) -> Option<&CommandAttributes> {
        self.aliases
            .get(name)
            .and_then(|command_id| self.commands.get(command_id))
    }
}
