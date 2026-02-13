use crate::error::AppError;

use crate::distributed::protocol::{ConfigMessage, StartMessage, StopMessage};

pub(super) enum AgentCommand {
    Config(Box<ConfigMessage>),
    Start(StartMessage),
    Stop(StopMessage),
    Error(AppError),
    Disconnected(AppError),
}
