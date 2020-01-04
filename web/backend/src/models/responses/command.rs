use persistence::commands::attributes::{CommandAttributes, CommandDetails, CommandWithAliases};
use persistence::commands::channel_config::ChannelCommandConfigNamed;
use persistence::commands::templates::CommandTemplate;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiCommand {
    #[serde(flatten)]
    pub attributes: ApiCommandAttributes,
    pub aliases: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDetailedCommand {
    #[serde(flatten)]
    pub attributes: ApiCommandAttributes,
    #[serde(flatten)]
    pub template: ApiCommandTemplate,
    pub aliases: Vec<String>,
    pub channel_config: Vec<ApiChannelCommandConfig>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiChannelCommandConfig {
    pub channel_id: i32,
    pub channel_name: String,
    pub active: Option<bool>,
    pub cooldown: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCommandTemplate {
    pub template: Option<String>,
    pub template_context: Option<serde_json::Value>,
}

impl From<CommandWithAliases> for ApiCommand {
    fn from(source: CommandWithAliases) -> Self {
        ApiCommand {
            attributes: source.attributes.into(),
            aliases: source.aliases,
        }
    }
}

impl From<CommandTemplate> for ApiCommandTemplate {
    fn from(source: CommandTemplate) -> Self {
        ApiCommandTemplate {
            template: source.template,
            template_context: source.template_context,
        }
    }
}

impl From<(CommandDetails, Vec<ChannelCommandConfigNamed>)> for ApiDetailedCommand {
    fn from((command, channel_config): (CommandDetails, Vec<ChannelCommandConfigNamed>)) -> Self {
        ApiDetailedCommand {
            attributes: command.attributes.into(),
            template: command.template.into(),
            aliases: command.aliases,
            channel_config: channel_config
                .into_iter()
                .map(|conf| ApiChannelCommandConfig {
                    channel_id: conf.channel_id,
                    channel_name: conf.channel_name,
                    active: conf.active,
                    cooldown: conf.cooldown.map(|d| d.as_millis() as u64),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCommandAttributes {
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
    pub cooldown: Option<u64>,
    /// whether the command can be used in whispers
    pub whisper_enabled: bool,
}

impl From<CommandAttributes> for ApiCommandAttributes {
    fn from(attributes: CommandAttributes) -> Self {
        ApiCommandAttributes {
            id: attributes.id,
            description: attributes.description,
            handler_name: attributes.handler_name,
            enabled: attributes.enabled,
            default_active: attributes.default_active,
            cooldown: attributes
                .cooldown
                .map(|duration| duration.as_millis() as u64),
            whisper_enabled: attributes.whisper_enabled,
        }
    }
}
