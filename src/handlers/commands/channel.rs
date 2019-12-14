use async_trait::async_trait;
use structopt::clap::AppSettings;
use structopt::StructOpt;

use crate::db::{
    create_permissions, AddPermission, Channel, InsertChannel, NewPermissionAttributes,
    PermissionState, UpdateChannelSettings,
};
use crate::db::{CommandAttributes, InsertCommandAttributes};
use crate::handlers::commands::{CommandContext, CommandHandler};
use crate::state::BotContext;
use crate::Result;
use futures::SinkExt;
use tmi_rs::ClientMessage;

#[derive(Debug)]
pub struct ChannelManagerCommand {
    ctx: BotContext,
}

const HANDLER_NAME: &str = "channel";

#[async_trait]
impl CommandHandler for ChannelManagerCommand {
    fn name(&self) -> &'static str {
        HANDLER_NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        let args = cmd.parse_args::<ChannelCommandArgs>(&self.ctx).await?;
        let mut sender = &self.ctx.sender;
        if let Some(args) = args {
            match args {
                ChannelCommandArgs::Info { channel } => {
                    cmd.check_permissions(&self.ctx, &["channels:read"], true).await?;

                    let channel_info = self.ctx.get_channel(&channel).await;
                    let reply = format!("{:?}", channel_info);
                    cmd.reply(&reply, sender).await?;
                }
                ChannelCommandArgs::Update { channel, settings } => {
                    cmd.check_permissions(&self.ctx, &["channels:manage"], true).await?;

                    if let Some(channel_info) = self.ctx.get_channel(&channel).await {
                        Channel::update_settings(
                            &self.ctx,
                            &channel_info,
                            settings.into_update_data(),
                        )
                        .await?;
                        cmd.reply("Channel updated.", sender).await?;
                    } else {
                        cmd.reply("No channel with that name found.", sender)
                            .await?;
                    }
                }
                ChannelCommandArgs::New { channel, settings } => {
                    cmd.check_permissions(&self.ctx, &["channels:manage", "channels:join"], true).await?;

                    Channel::create_channel(&self.ctx, settings.into_insert_data(channel.clone()))
                        .await?;
                    if let Some(channel_info) = self.ctx.get_channel(&channel).await {
                        if channel_info.data.join_on_start {
                            cmd.reply("Channel created, joining.", sender).await?;
                            sender
                                .send(ClientMessage::join(channel_info.data.name.as_str()))
                                .await?;
                        } else {
                            cmd.reply("Channel created.", sender).await?;
                        }
                    }
                }
            };
            Ok(())
        } else {
            Ok(())
        }
    }

    async fn create(ctx: &BotContext) -> Result<Box<dyn CommandHandler>>
    where
        Self: Sized,
    {
        create_permissions(
            ctx,
            vec![
                AddPermission {
                    attributes: NewPermissionAttributes {
                        name: "channels:manage",
                        description: Some("Manage the channels the bot operates in"),
                        default_state: PermissionState::Deny,
                    },
                    implied_by: vec!["root"],
                },
                AddPermission {
                    attributes: NewPermissionAttributes {
                        name: "channels:join",
                        description: Some("Make the bot join channels"),
                        default_state: PermissionState::Deny,
                    },
                    implied_by: vec!["root", "channels:manage"],
                },
                AddPermission {
                    attributes: NewPermissionAttributes {
                        name: "channels:read",
                        description: Some("Get information about the bot's channels"),
                        default_state: PermissionState::Deny,
                    },
                    implied_by: vec!["root", "channels:manage"],
                },
            ],
        )
        .await?;

        // register channel management command
        CommandAttributes::initialize(
            ctx,
            InsertCommandAttributes {
                handler_name: HANDLER_NAME.into(),
                description: Some("Manage the bot channels".into()),
                enabled: true,
                default_active: true,
                cooldown: None,
                whisper_enabled: true,
            },
            Vec::<String>::new(),
            vec!["channel", "ch"],
        )
        .await?;

        // register join command
        CommandAttributes::initialize(
            ctx,
            InsertCommandAttributes {
                handler_name: HANDLER_NAME.into(),
                description: Some("Join a channel".into()),
                enabled: true,
                default_active: true,
                cooldown: None,
                whisper_enabled: true,
            },
            vec!["channels:join"],
            vec!["join"],
        )
        .await?;

        Ok(Box::new(ChannelManagerCommand { ctx: ctx.clone() }) as Box<dyn CommandHandler>)
    }
}

/// Manage channel settings
#[derive(StructOpt, Debug)]
#[structopt(
    name = "channel",
    template("{bin} - {about} USAGE: {usage} {subcommands}"),
    setting(AppSettings::DisableVersion),
    setting(AppSettings::DisableHelpSubcommand)
)]
enum ChannelCommandArgs {
    /// Update channel settings
    #[structopt(
        setting(AppSettings::DisableVersion),
        template("{bin} - USAGE: {usage} {options}")
    )]
    Update {
        /// Channel to update
        channel: String,
        #[structopt(flatten)]
        settings: ChannelSettingsArgs,
    },
    /// Create a channel
    #[structopt(
        setting(AppSettings::DisableVersion),
        template("{bin} - USAGE: {usage} {options}")
    )]
    New {
        /// Channel to add
        channel: String,
        #[structopt(flatten)]
        settings: ChannelSettingsArgs,
    },
    /// Get settings for a channel
    #[structopt(
        setting(AppSettings::DisableVersion),
        template("{bin} - USAGE: {usage} {options}")
    )]
    Info {
        /// Channel to update
        channel: String,
    },
}

#[derive(StructOpt, Debug)]
struct ChannelSettingsArgs {
    /// Join the channel on startup
    #[structopt(long)]
    join: bool,

    /// Don't join the channel on startup
    #[structopt(long, conflicts_with = "join")]
    no_join: bool,

    /// Don't respond to any commands
    #[structopt(long)]
    silence: bool,

    /// Respond to commands
    #[structopt(long, conflicts_with = "silence")]
    respond: bool,

    /// Command prefix
    #[structopt(long)]
    prefix: Option<String>,

    /// Remove command prefix
    #[structopt(long, conflicts_with = "prefix")]
    no_prefix: bool,
}

impl ChannelSettingsArgs {
    fn into_update_data(self) -> UpdateChannelSettings {
        UpdateChannelSettings {
            join_on_start: if self.join {
                Some(true)
            } else if self.no_join {
                Some(false)
            } else {
                None
            },
            command_prefix: if self.prefix.is_some() {
                Some(self.prefix)
            } else if self.no_prefix {
                Some(None)
            } else {
                None
            },
            silent: if self.silence {
                Some(true)
            } else if self.respond {
                Some(false)
            } else {
                None
            },
        }
    }

    fn into_insert_data(self, name: String) -> InsertChannel {
        InsertChannel {
            twitch_room_id: None,
            name,
            join_on_start: if self.join {
                Some(true)
            } else if self.no_join {
                Some(false)
            } else {
                None
            },
            command_prefix: self.prefix,
            silent: if self.silence {
                Some(true)
            } else if self.respond {
                Some(false)
            } else {
                None
            },
        }
    }
}
