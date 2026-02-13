use crate::application::commands::{
    AgentRunCommand, ControllerRunCommand, LocalRunCommand, ReplayRunCommand, ServiceCommand,
};
use crate::args::{CleanupArgs, CompareArgs};

pub(in crate::entry) struct DumpUrlsPlan {
    pub(super) pattern: String,
    pub(super) count: usize,
    pub(super) max_repeat: u32,
}

pub(in crate::entry) enum RunPlan {
    Cleanup(CleanupArgs),
    Compare(CompareArgs),
    Replay(ReplayRunCommand),
    DumpUrls(DumpUrlsPlan),
    Service(ServiceCommand),
    Controller(ControllerRunCommand),
    Agent(AgentRunCommand),
    Local(LocalRunCommand),
}
