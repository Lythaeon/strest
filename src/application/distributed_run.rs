use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::application::commands::{DistributedRunCommand, DistributedRunMode};
use crate::config::types::ScenarioConfig;
use crate::error::AppResult;

#[async_trait]
pub(crate) trait DistributedRunPort<TAdapterArgs> {
    async fn run_controller(
        &self,
        adapter_args: &TAdapterArgs,
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    ) -> AppResult<()>;

    async fn run_agent(&self, adapter_args: &TAdapterArgs) -> AppResult<()>;
}

pub(crate) async fn execute<TPort, TAdapterArgs>(
    command: DistributedRunCommand,
    adapter_args: TAdapterArgs,
    distributed_port: &TPort,
) -> AppResult<()>
where
    TPort: DistributedRunPort<TAdapterArgs> + Sync,
{
    match command.into_mode() {
        DistributedRunMode::Controller { scenarios } => {
            distributed_port
                .run_controller(&adapter_args, scenarios)
                .await
        }
        DistributedRunMode::Agent => distributed_port.run_agent(&adapter_args).await,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{DistributedRunPort, execute};
    use crate::application::commands::DistributedRunCommand;
    use crate::domain::run::{LoadMode, ProtocolKind, RunConfig};
    use crate::error::AppResult;

    struct FakeDistributedPort {
        controller_called: AtomicBool,
        agent_called: AtomicBool,
        seen_args: Arc<Mutex<Vec<String>>>,
        seen_scenarios_len: Arc<Mutex<Option<usize>>>,
    }

    #[async_trait::async_trait]
    impl DistributedRunPort<String> for FakeDistributedPort {
        async fn run_controller(
            &self,
            adapter_args: &String,
            scenarios: Option<BTreeMap<String, crate::config::types::ScenarioConfig>>,
        ) -> AppResult<()> {
            self.controller_called.store(true, Ordering::SeqCst);
            if let Ok(mut seen) = self.seen_args.lock() {
                seen.push(adapter_args.clone());
            }
            if let Ok(mut seen_len) = self.seen_scenarios_len.lock() {
                *seen_len = Some(scenarios.map(|items| items.len()).unwrap_or(0));
            }
            Ok(())
        }

        async fn run_agent(&self, adapter_args: &String) -> AppResult<()> {
            self.agent_called.store(true, Ordering::SeqCst);
            if let Ok(mut seen) = self.seen_args.lock() {
                seen.push(adapter_args.clone());
            }
            Ok(())
        }
    }

    fn run_config() -> RunConfig {
        RunConfig {
            protocol: ProtocolKind::Http,
            load_mode: LoadMode::Arrival,
            target_url: Some("http://localhost".to_owned()),
            scenario: None,
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_dispatches_controller_mode() -> AppResult<()> {
        let mut scenarios = BTreeMap::new();
        scenarios.insert(
            "default".to_owned(),
            crate::config::types::ScenarioConfig::default(),
        );
        let command = DistributedRunCommand::new_controller(run_config(), false, Some(scenarios));

        let seen_args = Arc::new(Mutex::new(Vec::new()));
        let seen_scenarios_len = Arc::new(Mutex::new(None));
        let port = FakeDistributedPort {
            controller_called: AtomicBool::new(false),
            agent_called: AtomicBool::new(false),
            seen_args: seen_args.clone(),
            seen_scenarios_len: seen_scenarios_len.clone(),
        };

        execute(command, "controller".to_owned(), &port).await?;

        if !port.controller_called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "expected controller mode to call controller port",
            ));
        }
        if port.agent_called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "agent port should not be called for controller mode",
            ));
        }
        if let Ok(seen) = seen_args.lock()
            && seen.as_slice() != ["controller"]
        {
            return Err(crate::error::AppError::validation(
                "expected controller args to be forwarded",
            ));
        }
        if let Ok(seen_len) = seen_scenarios_len.lock()
            && *seen_len != Some(1)
        {
            return Err(crate::error::AppError::validation(
                "expected one controller scenario",
            ));
        }

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_dispatches_agent_mode() -> AppResult<()> {
        let command = DistributedRunCommand::new_agent(run_config(), false);

        let seen_args = Arc::new(Mutex::new(Vec::new()));
        let seen_scenarios_len = Arc::new(Mutex::new(None));
        let port = FakeDistributedPort {
            controller_called: AtomicBool::new(false),
            agent_called: AtomicBool::new(false),
            seen_args: seen_args.clone(),
            seen_scenarios_len,
        };

        execute(command, "agent".to_owned(), &port).await?;

        if !port.agent_called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "expected agent mode to call agent port",
            ));
        }
        if port.controller_called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "controller port should not be called for agent mode",
            ));
        }
        if let Ok(seen) = seen_args.lock()
            && seen.as_slice() != ["agent"]
        {
            return Err(crate::error::AppError::validation(
                "expected agent args to be forwarded",
            ));
        }

        Ok(())
    }
}
