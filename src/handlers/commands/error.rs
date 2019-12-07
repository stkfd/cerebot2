use std::fmt;

#[derive(Debug)]
pub enum CommandError {
    ReplyError(&'static str),
}

impl std::error::Error for CommandError {}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::ReplyError(msg) => write!(f, "Reply error: {}", msg),
        }
    }
}
