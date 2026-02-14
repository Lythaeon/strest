use crate::application::commands::{DistributedRunCommand, LocalRunCommand, ReplayRunCommand};
use crate::args::{CleanupArgs, CompareArgs, TesterArgs};

pub(in crate::entry) struct DumpUrlsPlan {
    pub(super) pattern: String,
    pub(super) count: usize,
    pub(super) max_repeat: u32,
}

pub(in crate::entry) enum RunPlan {
    Cleanup(CleanupArgs),
    Compare(CompareArgs),
    Replay {
        command: ReplayRunCommand,
        args: TesterArgs,
    },
    DumpUrls(DumpUrlsPlan),
    Service(TesterArgs),
    Distributed {
        command: DistributedRunCommand,
        args: TesterArgs,
    },
    Local {
        command: LocalRunCommand,
        args: TesterArgs,
    },
}
