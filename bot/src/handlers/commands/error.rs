use persistence::commands::permission::PermissionRequirement;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandError {
    #[error("Reply error: {0}")]
    ReplyError(&'static str),
    #[error("Quote mismatch in command arguments")]
    QuoteMismatch,
    #[error("{0}")]
    ArgumentError(structopt::clap::Error),
    #[error("Permission requirement {0:?} is not fulfilled")]
    PermissionRequired(PermissionRequirement),
    #[error("Netflix API error: {0}")]
    UnogsError(#[from] unogs_client::Error),
    #[error("RapidApi key is not configured")]
    RapidApiNotConfigured,
    #[error("RapidApi daily request quota exceeded")]
    RapidApiQuotaLimit,
}
