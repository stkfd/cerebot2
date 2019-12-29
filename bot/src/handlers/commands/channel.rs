use futures::SinkExt;
use structopt::StructOpt;
use tmi_rs::ClientMessage;

use async_trait::async_trait;
use persistence::channel::{Channel, InsertChannel, UpdateChannelSettings};
use persistence::commands::attributes::InsertCommandAttributes;
use persistence::permissions::{
    create_permissions, AddPermission, NewPermissionAttributes, PermissionState,
};

use crate::handlers::commands::*;
use crate::state::{BotContext, ChannelInfo};
use crate::util::initialize_command;
use crate::Result;

#[derive(Debug)]
pub struct ChannelManagerCommand {
    ctx: BotContext,
}

const NAME: &str = "channel";

#[async_trait]
impl CommandHandler for ChannelManagerCommand {
    fn name(&self) -> &'static str {
        NAME
    }

    async fn run(&self, cmd: &CommandContext<'_>) -> Result<()> {
        let args = cmd.parse_args::<ChannelCommandArgs>(&self.ctx).await?;
        let mut sender = &self.ctx.sender;
        if let Some(args) = args {
            match args {
                ChannelCommandArgs::Info { channel } => {
                    cmd.check_permissions(&self.ctx, &["channels:read"], true)
                        .await?;

                    let channel_info = Channel::get(&self.ctx.db_context, &channel).await?;
                    let reply = format!("{:?}", channel_info);
                    cmd.reply(&reply, sender).await?;
                }
                ChannelCommandArgs::Update { channel, settings } => {
                    cmd.check_permissions(&self.ctx, &["channels:manage"], true)
                        .await?;

                    if let Some(channel_data) = Channel::get(&self.ctx.db_context, &channel).await?
                    {
                        // update DB
                        let updated_channel = Channel::update_settings(
                            &self.ctx.db_context,
                            &channel_data.name,
                            settings.into_update_data(),
                        )
                        .await?;

                        // update the bot's internal channel map
                        self.ctx
                            .update_channel(ChannelInfo {
                                data: updated_channel,
                                state: self
                                    .ctx
                                    .get_channel(&channel_data.name)
                                    .await
                                    .and_then(|c| c.state.clone()),
                            })
                            .await;
                        cmd.reply("Channel updated.", sender).await?;
                    } else {
                        cmd.reply("No channel with that name found.", sender)
                            .await?;
                    }
                }
                ChannelCommandArgs::New { channel, settings } => {
                    cmd.check_permissions(&self.ctx, &["channels:manage", "channels:join"], true)
                        .await?;

                    let inserted_channel = Channel::create_channel(
                        &self.ctx.db_context,
                        settings.into_insert_data(channel.clone()),
                    )
                    .await?;

                    // update the bot's internal channel map
                    self.ctx
                        .update_channel(ChannelInfo {
                            data: inserted_channel,
                            state: None,
                        })
                        .await;

                    // join the channel if join on start is set
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
                ChannelCommandArgs::Join { channel } => {
                    cmd.check_permissions(&self.ctx, &["channels:manage", "channels:join"], true)
                        .await?;

                    if let Some(channel_data) = Channel::get(&self.ctx.db_context, &channel).await?
                    {
                        sender
                            .send(ClientMessage::join(channel_data.name.as_str()))
                            .await?;

                        let reply = format!("Joined {}!", channel_data.name);
                        cmd.reply(&reply, &self.ctx.sender).await?;
                    } else {
                        cmd.reply("Channel not found.", &self.ctx.sender).await?;
                    }
                }
                ChannelCommandArgs::Part { channel } => {
                    if let Some(channel_data) = Channel::get(&self.ctx.db_context, &channel).await?
                    {
                        sender
                            .send(ClientMessage::Part(channel_data.name.clone()))
                            .await?;
                        let reply = format!("Left {}!", channel_data.name);
                        cmd.reply(&reply, &self.ctx.sender).await?;
                    } else {
                        cmd.reply("Channel not found.", &self.ctx.sender).await?;
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
            &ctx.db_context,
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
        initialize_command(
            &ctx,
            InsertCommandAttributes {
                handler_name: NAME.into(),
                description: Some("Manage the bot channels".into()),
                enabled: true,
                default_active: true,
                cooldown: None,
                whisper_enabled: true,
            },
            Vec::<String>::new(), // permissions checked inside the handler
            vec!["channel", "ch"],
        )
        .await?;

        Ok(Box::new(ChannelManagerCommand { ctx: ctx.clone() }) as Box<dyn CommandHandler>)
    }
}

/// Manage channel settings
#[derive(StructOpt, Debug)]
#[structopt(name = "channel", template("{usage} {subcommands} {unified}"))]
enum ChannelCommandArgs {
    #[structopt(template(OPTS_HELP_TEMPLATE))]
    Join { channel: String },
    #[structopt(template(OPTS_HELP_TEMPLATE))]
    Part { channel: String },
    #[structopt(template(OPTS_HELP_TEMPLATE))]
    Update {
        channel: String,
        #[structopt(flatten)]
        settings: ChannelSettingsArgs,
    },
    #[structopt(template(OPTS_HELP_TEMPLATE))]
    New {
        channel: String,
        #[structopt(flatten)]
        settings: ChannelSettingsArgs,
    },
    #[structopt(template(OPTS_HELP_TEMPLATE))]
    Info { channel: String },
}

#[derive(StructOpt, Debug)]
struct ChannelSettingsArgs {
    #[structopt(long)]
    join: bool,

    #[structopt(long, conflicts_with = "join")]
    no_join: bool,

    #[structopt(long)]
    silence: bool,

    #[structopt(long, conflicts_with = "silence")]
    respond: bool,

    #[structopt(long)]
    prefix: Option<String>,

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
