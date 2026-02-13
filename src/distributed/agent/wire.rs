use tokio::sync::mpsc;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};
use crate::metrics::StreamSnapshot;

use crate::distributed::protocol::{HelloMessage, WireMessage, WireSummary};
use crate::distributed::utils::duration_to_ms;

pub(super) fn send_wire(
    tx: &mpsc::UnboundedSender<WireMessage>,
    message: WireMessage,
) -> AppResult<()> {
    tx.send(message)
        .map_err(|_err| AppError::distributed(DistributedError::ControllerConnectionClosed))
}

pub(super) fn snapshot_to_wire_summary(snapshot: &StreamSnapshot) -> WireSummary {
    WireSummary {
        duration_ms: duration_to_ms(snapshot.duration),
        total_requests: snapshot.total_requests,
        successful_requests: snapshot.successful_requests,
        error_requests: snapshot.error_requests,
        timeout_requests: snapshot.timeout_requests,
        transport_errors: snapshot.transport_errors,
        non_expected_status: snapshot.non_expected_status,
        success_min_latency_ms: snapshot.success_min_latency_ms,
        success_max_latency_ms: snapshot.success_max_latency_ms,
        success_latency_sum_ms: snapshot.success_latency_sum_ms,
        min_latency_ms: snapshot.min_latency_ms,
        max_latency_ms: snapshot.max_latency_ms,
        latency_sum_ms: snapshot.latency_sum_ms,
    }
}

pub(super) fn build_hello(args: &TesterArgs, agent_id: &str) -> HelloMessage {
    HelloMessage {
        agent_id: agent_id.to_owned(),
        hostname: std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_owned()),
        cpu_cores: std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
        weight: args.agent_weight.get(),
        auth_token: args.auth_token.clone(),
    }
}

pub(super) fn build_agent_id(args: &TesterArgs) -> String {
    if let Some(id) = args.agent_id.as_ref() {
        return id.clone();
    }
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "agent".to_owned());
    format!("{}-{}", host, std::process::id())
}
