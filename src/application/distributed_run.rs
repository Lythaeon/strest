use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::application::commands::{DistributedRunCommand, DistributedRunMode};
use crate::args::TesterArgs;
use crate::config::types::ScenarioConfig;
use crate::error::AppResult;

#[async_trait]
pub(crate) trait DistributedRunPort {
    async fn run_controller(
        &self,
        args: &TesterArgs,
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    ) -> AppResult<()>;

    async fn run_agent(&self, args: TesterArgs) -> AppResult<()>;
}

pub(crate) async fn execute<TPort>(
    command: DistributedRunCommand,
    distributed_port: &TPort,
) -> AppResult<()>
where
    TPort: DistributedRunPort + Sync,
{
    let (args, mode) = command.into_parts();
    match mode {
        DistributedRunMode::Controller { scenarios } => {
            distributed_port.run_controller(&args, scenarios).await
        }
        DistributedRunMode::Agent => distributed_port.run_agent(args).await,
    }
}
