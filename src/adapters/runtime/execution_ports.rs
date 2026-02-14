use std::collections::BTreeMap;

use async_trait::async_trait;

use crate::application::distributed_run::DistributedRunPort;
use crate::application::local_run;
use crate::application::slice_execution::{
    CleanupPort, ComparePort, LocalRunPort, ReplayRunPort, ServicePort,
};
use crate::args::{CleanupArgs, CompareArgs, TesterArgs};
use crate::config::types::ScenarioConfig;
use crate::distributed::{AgentLocalRunPort, AgentRunOutcome};
use crate::error::AppResult;
use crate::metrics::StreamSnapshot;
use tokio::sync::{mpsc, watch};

pub(crate) struct RuntimeLocalPort;

#[async_trait]
impl LocalRunPort<TesterArgs, local_run::RunOutcome> for RuntimeLocalPort {
    async fn run_local(&self, adapter_args: TesterArgs) -> AppResult<local_run::RunOutcome> {
        crate::app::run_local(adapter_args, None, None).await
    }
}

pub(crate) struct RuntimeReplayPort;

#[async_trait]
impl ReplayRunPort<TesterArgs> for RuntimeReplayPort {
    async fn run_replay(&self, adapter_args: TesterArgs) -> AppResult<()> {
        crate::app::run_replay(&adapter_args).await
    }
}

pub(crate) struct RuntimeCleanupPort;

#[async_trait]
impl CleanupPort<CleanupArgs> for RuntimeCleanupPort {
    async fn run_cleanup(&self, cleanup_args: CleanupArgs) -> AppResult<()> {
        crate::app::run_cleanup(&cleanup_args).await
    }
}

pub(crate) struct RuntimeComparePort;

#[async_trait]
impl ComparePort<CompareArgs> for RuntimeComparePort {
    async fn run_compare(&self, compare_args: CompareArgs) -> AppResult<()> {
        crate::app::run_compare(&compare_args).await
    }
}

pub(crate) struct RuntimeServicePort;

impl ServicePort<TesterArgs> for RuntimeServicePort {
    fn handle_service_action(&self, adapter_args: TesterArgs) -> AppResult<()> {
        crate::service::handle_service_action(&adapter_args)
    }
}

pub(crate) struct RuntimeDistributedPort;

#[async_trait]
impl DistributedRunPort<TesterArgs> for RuntimeDistributedPort {
    async fn run_controller(
        &self,
        adapter_args: &TesterArgs,
        scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    ) -> AppResult<()> {
        crate::distributed::run_controller(adapter_args, scenarios).await
    }

    async fn run_agent(&self, adapter_args: &TesterArgs) -> AppResult<()> {
        let local_port = RuntimeAgentLocalRunPort;
        crate::distributed::run_agent(adapter_args.clone(), &local_port).await
    }
}

struct RuntimeAgentLocalRunPort;

#[async_trait]
impl AgentLocalRunPort for RuntimeAgentLocalRunPort {
    async fn run_local(
        &self,
        args: TesterArgs,
        stream_tx: Option<mpsc::UnboundedSender<StreamSnapshot>>,
        external_shutdown: Option<watch::Receiver<bool>>,
    ) -> AppResult<AgentRunOutcome> {
        let outcome = crate::app::run_local(args, stream_tx, external_shutdown).await?;
        Ok(AgentRunOutcome {
            summary: outcome.summary,
            histogram: outcome.histogram,
            success_histogram: outcome.success_histogram,
            latency_sum_ms: outcome.latency_sum_ms,
            success_latency_sum_ms: outcome.success_latency_sum_ms,
            runtime_errors: outcome.runtime_errors,
        })
    }
}

pub(crate) fn print_runtime_errors(errors: &[String]) {
    eprintln!("Runtime errors:");
    for error in errors {
        eprintln!("- {}", error);
    }
}
