use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Duration;

use tokio::sync::watch;
use tracing::{debug, info, warn};

use crate::args::TesterArgs;
use crate::charts;
use crate::error::AppResult;
use crate::metrics::{AggregatedMetricSample, LatencyHistogram};
use crate::sinks::config::{SinkStats, SinksConfig};
use crate::ui::model::UiData;

use super::super::protocol::{ReportMessage, StreamMessage, WireSummary};
use super::super::summary::{compute_summary_stats, merge_summaries};

pub(super) const REPORT_GRACE_SECS: u64 = 30;
pub(super) const DEFAULT_SINK_INTERVAL: Duration = Duration::from_secs(1);
pub(super) const DEFAULT_START_AFTER_MS: u64 = 3000;

pub(super) struct AgentSnapshot {
    pub(super) summary: WireSummary,
    pub(super) histogram: LatencyHistogram,
    pub(super) success_histogram: LatencyHistogram,
}

pub(super) enum AgentEvent {
    Heartbeat {
        agent_id: String,
    },
    Stream {
        agent_id: String,
        message: StreamMessage,
    },
    Report {
        agent_id: String,
        message: ReportMessage,
    },
    Error {
        agent_id: String,
        message: String,
    },
    Disconnected {
        agent_id: String,
        message: String,
    },
}

pub(super) fn handle_agent_event(
    event: AgentEvent,
    expected_run_id: &str,
    pending_agents: &mut HashSet<String>,
    agent_states: &mut HashMap<String, AgentSnapshot>,
    runtime_errors: &mut Vec<String>,
    sink_dirty: &mut bool,
) {
    match event {
        AgentEvent::Stream { agent_id, message } => {
            debug!(
                "Stream snapshot from agent {} for run {}",
                agent_id, message.run_id
            );
            if message.run_id != expected_run_id {
                runtime_errors.push(format!("Agent {} returned mismatched run id.", agent_id));
                return;
            }
            if message.agent_id != agent_id {
                runtime_errors.push(format!(
                    "Agent {} reported unexpected id {}.",
                    agent_id, message.agent_id
                ));
                return;
            }
            match LatencyHistogram::decode_base64(&message.histogram_b64) {
                Ok(histogram) => {
                    let success_histogram = match message.success_histogram_b64.as_deref() {
                        Some(encoded) => match LatencyHistogram::decode_base64(encoded) {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram decode failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                        None => match LatencyHistogram::new() {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram init failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                    };
                    agent_states.insert(
                        agent_id,
                        AgentSnapshot {
                            summary: message.summary,
                            histogram,
                            success_histogram,
                        },
                    );
                    *sink_dirty = true;
                }
                Err(err) => runtime_errors.push(format!(
                    "Agent {} histogram decode failed: {}",
                    agent_id, err
                )),
            }
        }
        AgentEvent::Report { agent_id, message } => {
            info!(
                "Report received from agent {} for run {}",
                agent_id, message.run_id
            );
            if message.run_id != expected_run_id {
                runtime_errors.push(format!("Agent {} returned mismatched run id.", agent_id));
                pending_agents.remove(&agent_id);
                return;
            }
            if message.agent_id != agent_id {
                runtime_errors.push(format!(
                    "Agent {} reported unexpected id {}.",
                    agent_id, message.agent_id
                ));
                pending_agents.remove(&agent_id);
                return;
            }
            if !message.runtime_errors.is_empty() {
                for err in message.runtime_errors {
                    runtime_errors.push(format!("Agent {}: {}", agent_id, err));
                }
            }
            match LatencyHistogram::decode_base64(&message.histogram_b64) {
                Ok(histogram) => {
                    let success_histogram = match message.success_histogram_b64.as_deref() {
                        Some(encoded) => match LatencyHistogram::decode_base64(encoded) {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram decode failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                        None => match LatencyHistogram::new() {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram init failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                    };
                    agent_states.insert(
                        agent_id.clone(),
                        AgentSnapshot {
                            summary: message.summary,
                            histogram,
                            success_histogram,
                        },
                    );
                    *sink_dirty = true;
                }
                Err(err) => runtime_errors.push(format!(
                    "Agent {} histogram decode failed: {}",
                    agent_id, err
                )),
            }
            pending_agents.remove(&agent_id);
        }
        AgentEvent::Error { agent_id, message } => {
            warn!("Agent {} error: {}", agent_id, message);
            runtime_errors.push(format!("Agent {}: {}", agent_id, message));
            pending_agents.remove(&agent_id);
        }
        AgentEvent::Disconnected { agent_id, message } => {
            warn!("Agent {} disconnected: {}", agent_id, message);
            runtime_errors.push(format!("Agent {} disconnected: {}", agent_id, message));
            pending_agents.remove(&agent_id);
        }
        AgentEvent::Heartbeat { .. } => {}
    }
}

pub(super) const fn event_agent_id(event: &AgentEvent) -> &str {
    match event {
        AgentEvent::Heartbeat { agent_id }
        | AgentEvent::Stream { agent_id, .. }
        | AgentEvent::Report { agent_id, .. }
        | AgentEvent::Error { agent_id, .. }
        | AgentEvent::Disconnected { agent_id, .. } => agent_id.as_str(),
    }
}

pub(super) fn update_ui(
    ui_tx: &watch::Sender<UiData>,
    args: &TesterArgs,
    agent_states: &HashMap<String, AgentSnapshot>,
    latency_window: &mut VecDeque<(u64, u64)>,
) {
    let Ok((summary, merged_hist, success_hist)) = aggregate_snapshots(agent_states) else {
        return;
    };
    let (p50, p90, p99) = merged_hist.percentiles();
    let (p50_ok, p90_ok, p99_ok) = success_hist.percentiles();
    let stats = compute_summary_stats(&summary);
    let elapsed_ms = summary.duration.as_millis().min(u128::from(u64::MAX)) as u64;
    let ui_window_ms = args.ui_window_ms.get();
    let window_start = elapsed_ms.saturating_sub(ui_window_ms);
    latency_window.push_back((elapsed_ms, summary.avg_latency_ms));
    while latency_window
        .front()
        .is_some_and(|(ts, _)| *ts < window_start)
    {
        latency_window.pop_front();
    }
    let latencies: Vec<(u64, u64)> = latency_window.iter().copied().collect();

    drop(ui_tx.send(UiData {
        elapsed_time: summary.duration,
        target_duration: Duration::from_secs(args.target_duration.get()),
        current_requests: summary.total_requests,
        successful_requests: summary.successful_requests,
        timeout_requests: summary.timeout_requests,
        transport_errors: summary.transport_errors,
        non_expected_status: summary.non_expected_status,
        ui_window_ms,
        no_color: args.no_color,
        latencies,
        p50,
        p90,
        p99,
        p50_ok,
        p90_ok,
        p99_ok,
        rps: stats.avg_rps_x100 / 100,
        rpm: stats.avg_rpm_x100 / 100,
        replay: None,
    }));
}

pub(super) async fn write_streaming_sinks(
    args: &TesterArgs,
    agent_states: &HashMap<String, AgentSnapshot>,
) -> AppResult<()> {
    if agent_states.is_empty() {
        return Ok(());
    }
    let (summary, merged_hist, _success_hist) = aggregate_snapshots(agent_states)?;
    let (p50, p90, p99) = merged_hist.percentiles();
    let stats = compute_summary_stats(&summary);
    if let Some(sinks) = args.sinks.as_ref() {
        let sink_stats = SinkStats {
            duration: summary.duration,
            total_requests: summary.total_requests,
            successful_requests: summary.successful_requests,
            error_requests: summary.error_requests,
            timeout_requests: summary.timeout_requests,
            min_latency_ms: summary.min_latency_ms,
            max_latency_ms: summary.max_latency_ms,
            avg_latency_ms: summary.avg_latency_ms,
            p50_latency_ms: p50,
            p90_latency_ms: p90,
            p99_latency_ms: p99,
            success_rate_x100: stats.success_rate_x100,
            avg_rps_x100: stats.avg_rps_x100,
            avg_rpm_x100: stats.avg_rpm_x100,
        };
        crate::sinks::writers::write_sinks(sinks, &sink_stats).await?;
    }
    Ok(())
}

pub(super) fn aggregate_snapshots(
    agent_states: &HashMap<String, AgentSnapshot>,
) -> AppResult<(
    crate::metrics::MetricsSummary,
    LatencyHistogram,
    LatencyHistogram,
)> {
    let mut summaries = Vec::with_capacity(agent_states.len());
    let mut merged_hist = LatencyHistogram::new()?;
    let mut merged_success_hist = LatencyHistogram::new()?;
    for snapshot in agent_states.values() {
        summaries.push(snapshot.summary.clone());
        merged_hist.merge(&snapshot.histogram)?;
        merged_success_hist.merge(&snapshot.success_histogram)?;
    }
    Ok((
        merge_summaries(&summaries),
        merged_hist,
        merged_success_hist,
    ))
}

pub(super) fn record_aggregated_sample(
    samples: &mut Vec<AggregatedMetricSample>,
    agent_states: &HashMap<String, AgentSnapshot>,
) {
    let Ok((summary, merged_hist, _success_hist)) = aggregate_snapshots(agent_states) else {
        return;
    };
    let (p50, p90, p99) = merged_hist.percentiles();
    let elapsed_ms = u64::try_from(summary.duration.as_millis()).unwrap_or(u64::MAX);
    let sample = AggregatedMetricSample {
        elapsed_ms,
        total_requests: summary.total_requests,
        successful_requests: summary.successful_requests,
        error_requests: summary.error_requests,
        avg_latency_ms: summary.avg_latency_ms,
        p50_latency_ms: p50,
        p90_latency_ms: p90,
        p99_latency_ms: p99,
    };

    if let Some(last) = samples.last()
        && last.elapsed_ms == sample.elapsed_ms
        && last.total_requests == sample.total_requests
        && last.successful_requests == sample.successful_requests
        && last.error_requests == sample.error_requests
        && last.avg_latency_ms == sample.avg_latency_ms
        && last.p50_latency_ms == sample.p50_latency_ms
        && last.p90_latency_ms == sample.p90_latency_ms
        && last.p99_latency_ms == sample.p99_latency_ms
    {
        return;
    }

    samples.push(sample);
}

pub(super) async fn write_aggregated_charts(
    samples: &[AggregatedMetricSample],
    args: &TesterArgs,
) -> AppResult<bool> {
    if args.no_charts {
        return Ok(false);
    }
    if samples.len() < 2 {
        return Ok(false);
    }
    charts::plot_aggregated_metrics(samples, args).await?;
    Ok(true)
}

pub(super) fn resolve_sink_interval(config: Option<&SinksConfig>) -> Duration {
    match config.and_then(|value| value.update_interval_ms) {
        Some(0) => {
            warn!(
                "sinks.update_interval_ms must be > 0; using default {}ms",
                DEFAULT_SINK_INTERVAL.as_millis()
            );
            DEFAULT_SINK_INTERVAL
        }
        Some(ms) => Duration::from_millis(ms),
        None => DEFAULT_SINK_INTERVAL,
    }
}

pub(super) fn resolve_agent_wait_timeout(args: &TesterArgs) -> Option<Duration> {
    args.agent_wait_timeout_ms
        .map(|value| Duration::from_millis(value.get()))
}

pub(super) fn resolve_heartbeat_check_interval(timeout: Duration) -> Duration {
    let timeout_ms = timeout.as_millis();
    let mut interval_ms = timeout_ms.saturating_div(2);
    if interval_ms < 200 {
        interval_ms = timeout_ms.max(1);
    }
    Duration::from_millis(u64::try_from(interval_ms).unwrap_or(1))
}
