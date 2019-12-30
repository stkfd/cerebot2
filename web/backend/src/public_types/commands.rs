use serde::Serialize;
use typescript_definitions::TypeScriptify;

#[derive(Serialize, TypeScriptify)]
pub struct CommandAttributes {
    pub id: i32,
    /// User facing description
    pub description: Option<String>,
    /// name of the command handler. Used to identify the right handler in the bot.
    pub handler_name: String,
    /// global switch to enable/disable a command
    pub enabled: bool,
    /// whether the command is active by default in all channels
    pub default_active: bool,
    /// minimum time between command uses
    pub cooldown: Option<isize>,
    /// whether the command can be used in whispers
    pub whisper_enabled: bool,
}
