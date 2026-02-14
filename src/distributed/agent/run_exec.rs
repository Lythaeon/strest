use async_trait::async_trait;
use tokio::sync::{mpsc, watch};
use tracing::{debug, info};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};
use crate::metrics::StreamSnapshot;

use super::command::AgentCommand;
use super::wire::{send_wire, snapshot_to_wire_summary};
use crate::distributed::protocol::{
    ConfigMessage, ErrorMessage, ReportMessage, StreamMessage, WireMessage, WireSummary,
};
use crate::distributed::utils::duration_to_ms;
use crate::distributed::wire::apply_wire_args;

#[derive(Debug)]
pub(crate) struct AgentRunOutcome {
    pub(crate) summary: crate::metrics::MetricsSummary,
    pub(crate) histogram: crate::metrics::LatencyHistogram,
    pub(crate) success_histogram: crate::metrics::LatencyHistogram,
    pub(crate) latency_sum_ms: u128,
    pub(crate) success_latency_sum_ms: u128,
    pub(crate) runtime_errors: Vec<String>,
}

#[async_trait]
pub(crate) trait AgentLocalRunPort {
    async fn run_local(
        &self,
        args: TesterArgs,
        stream_tx: Option<mpsc::UnboundedSender<StreamSnapshot>>,
        external_shutdown: Option<watch::Receiver<bool>>,
    ) -> AppResult<AgentRunOutcome>;
}

pub(super) async fn run_agent_run<TLocalRunPort>(
    base_args: &TesterArgs,
    config: ConfigMessage,
    agent_id: String,
    out_tx: &mpsc::UnboundedSender<WireMessage>,
    cmd_rx: &mut mpsc::UnboundedReceiver<AgentCommand>,
    local_run_port: &TLocalRunPort,
) -> AppResult<()>
where
    TLocalRunPort: AgentLocalRunPort + Sync,
{
    let ConfigMessage { run_id, args } = config;
    let mut run_args = base_args.clone();
    if let Err(err) = apply_wire_args(&mut run_args, args) {
        let message = format!("Run {} config error: {}", run_id, err);
        if send_wire(out_tx, WireMessage::Error(ErrorMessage { message })).is_err() {
            // ignore
        }
        return Err(err);
    }
    info!("Starting run {} on agent {}", run_id, agent_id);
    run_args.distributed_silent = true;
    let streaming_enabled = run_args.distributed_stream_summaries;

    let (stop_tx, stop_rx) = watch::channel(false);

    let (stream_tx, mut stream_rx) = if streaming_enabled {
        let (stream_tx, stream_rx) = mpsc::unbounded_channel::<StreamSnapshot>();
        (Some(stream_tx), Some(stream_rx))
    } else {
        (None, None)
    };

    let mut run_future = Box::pin(local_run_port.run_local(run_args, stream_tx, Some(stop_rx)));
    let mut abort_reason: Option<AppError> = None;

    let run_outcome = loop {
        tokio::select! {
            result = &mut run_future => {
                break result;
            }
            command = cmd_rx.recv() => {
                handle_runtime_command(command, &run_id, &stop_tx, &mut abort_reason);
            }
            snapshot = async {
                if let Some(rx) = stream_rx.as_mut() {
                    rx.recv().await
                } else {
                    None
                }
            }, if streaming_enabled => {
                if let Some(snapshot) = snapshot {
                    let message = WireMessage::Stream(Box::new(StreamMessage {
                        run_id: run_id.clone(),
                        agent_id: agent_id.clone(),
                        summary: snapshot_to_wire_summary(&snapshot),
                        histogram_b64: snapshot.histogram_b64,
                        success_histogram_b64: None,
                    }));
                    if send_wire(out_tx, message).is_err() {
                        return Err(AppError::distributed(
                            DistributedError::ControllerConnectionClosed,
                        ));
                    }
                }
            }
        }

        if let Some(reason) = abort_reason.take() {
            match run_future.await {
                Ok(_) | Err(_) => {}
            }
            return Err(reason);
        }
    };

    let run_outcome = match run_outcome {
        Ok(outcome) => outcome,
        Err(err) => {
            let message = format!("Run {} failed: {}", run_id, err);
            if send_wire(out_tx, WireMessage::Error(ErrorMessage { message })).is_err() {
                // ignore
            }
            return Err(err);
        }
    };

    info!(
        "Run {} completed on agent {} (runtime_errors={})",
        run_id,
        agent_id,
        run_outcome.runtime_errors.len()
    );
    let histogram_b64 = run_outcome.histogram.encode_base64()?;
    let success_histogram_b64 = run_outcome.success_histogram.encode_base64()?;
    let summary = WireSummary {
        duration_ms: duration_to_ms(run_outcome.summary.duration),
        total_requests: run_outcome.summary.total_requests,
        successful_requests: run_outcome.summary.successful_requests,
        error_requests: run_outcome.summary.error_requests,
        timeout_requests: run_outcome.summary.timeout_requests,
        transport_errors: run_outcome.summary.transport_errors,
        non_expected_status: run_outcome.summary.non_expected_status,
        success_min_latency_ms: run_outcome.summary.success_min_latency_ms,
        success_max_latency_ms: run_outcome.summary.success_max_latency_ms,
        success_latency_sum_ms: run_outcome.success_latency_sum_ms,
        min_latency_ms: run_outcome.summary.min_latency_ms,
        max_latency_ms: run_outcome.summary.max_latency_ms,
        latency_sum_ms: run_outcome.latency_sum_ms,
    };

    let report = ReportMessage {
        run_id,
        agent_id,
        summary,
        histogram_b64,
        success_histogram_b64: Some(success_histogram_b64),
        runtime_errors: run_outcome.runtime_errors,
    };
    debug!("Sending report for run {}", report.run_id);
    if send_wire(out_tx, WireMessage::Report(Box::new(report))).is_err() {
        return Err(AppError::distributed(
            DistributedError::ControllerConnectionClosed,
        ));
    }

    Ok(())
}

fn handle_runtime_command(
    command: Option<AgentCommand>,
    run_id: &str,
    stop_tx: &watch::Sender<bool>,
    abort_reason: &mut Option<AppError>,
) {
    match command {
        Some(AgentCommand::Stop(stop)) => {
            if stop.run_id == run_id && stop_tx.send(true).is_err() {
                // Local run task already stopped.
            }
        }
        Some(AgentCommand::Disconnected(err)) | Some(AgentCommand::Error(err)) => {
            *abort_reason = Some(err);
            if stop_tx.send(true).is_err() {
                // Local run task already stopped.
            }
        }
        Some(AgentCommand::Config(_)) | Some(AgentCommand::Start(_)) => {
            *abort_reason = Some(AppError::distributed(
                DistributedError::UnexpectedControllerMessageWhileRunning,
            ));
            if stop_tx.send(true).is_err() {
                // Local run task already stopped.
            }
        }
        None => {
            *abort_reason = Some(AppError::distributed(
                DistributedError::ControllerConnectionClosed,
            ));
            if stop_tx.send(true).is_err() {
                // Local run task already stopped.
            }
        }
    }
}
