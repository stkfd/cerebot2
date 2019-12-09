use std::fmt;

#[derive(Debug)]
pub enum CommandError {
    ReplyError(&'static str),
    ArgumentError(structopt::clap::Error),
}

impl std::error::Error for CommandError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CommandError::ArgumentError(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::ReplyError(msg) => write!(f, "Reply error: {}", msg),
            CommandError::ArgumentError(e) => write!(f, "{}", e),
        }
    }
}
