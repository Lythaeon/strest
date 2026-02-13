use std::collections::BTreeMap;

use crate::args::{CleanupArgs, CompareArgs, TesterArgs};
use crate::config::types::ScenarioConfig;
use crate::error::{AppError, AppResult, ValidationError};

pub(in crate::entry) struct DumpUrlsPlan {
    pub(super) pattern: String,
    pub(super) count: usize,
    pub(super) max_repeat: u32,
}

pub(in crate::entry) struct LocalArgs {
    pub(super) args: TesterArgs,
}

impl LocalArgs {
    pub(super) fn new(mut args: TesterArgs) -> AppResult<Self> {
        if args.url.is_none() && args.scenario.is_none() {
            tracing::error!("Missing URL (set --url or provide in config).");
            return Err(AppError::validation(ValidationError::MissingUrl));
        }
        args.distributed_stream_summaries = false;
        Ok(Self { args })
    }
}

pub(in crate::entry) enum RunPlan {
    Cleanup(CleanupArgs),
    Compare(CompareArgs),
    Replay(TesterArgs),
    DumpUrls(DumpUrlsPlan),
    Service(TesterArgs),
    Controller {
        args: TesterArgs,
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    },
    Agent(TesterArgs),
    Local(LocalArgs),
}
