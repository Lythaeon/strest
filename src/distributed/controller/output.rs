use std::collections::{HashMap, VecDeque};
use std::io::IsTerminal;
use std::time::Duration;

use tokio::sync::watch;

use crate::args::TesterArgs;
use crate::charts;
use crate::distributed::summary::{
    Percentiles, SummaryPercentiles, compute_summary_stats, print_summary,
};
use crate::error::AppResult;
use crate::metrics::AggregatedMetricSample;
use crate::shutdown::ShutdownSender;
use crate::sinks::config::SinkStats;
use crate::sinks::writers::write_sinks;
use crate::ui::{model::UiData, render::setup_render_ui};

use super::shared::{AgentSnapshot, aggregate_snapshots, record_aggregated_sample, update_ui};

pub(in crate::distributed::controller) enum OutputEvent {
    AgentStateUpdated,
    SinkTick,
}

pub(in crate::distributed::controller) struct DistributedOutputState {
    charts_enabled: bool,
    sink_updates_enabled: bool,
    sink_dirty: bool,
    aggregated_samples: Vec<AggregatedMetricSample>,
    ui_tx: Option<watch::Sender<UiData>>,
    shutdown_tx: Option<ShutdownSender>,
    ui_latency_window: VecDeque<(u64, u64)>,
    ui_rps_window: VecDeque<(u64, u64)>,
}

pub(in crate::distributed::controller) fn setup_output_state(
    args: &TesterArgs,
) -> DistributedOutputState {
    let streaming_enabled = args.distributed_stream_summaries;
    let ui_enabled = streaming_enabled && !args.no_ui && std::io::stdout().is_terminal();
    let (ui_tx, shutdown_tx) = if ui_enabled {
        let target_duration = Duration::from_secs(args.target_duration.get());
        let (shutdown_tx, _) = crate::system::shutdown_handlers::shutdown_channel();
        let (ui_tx, _) = watch::channel(UiData {
            target_duration,
            ui_window_ms: args.ui_window_ms.get(),
            no_color: args.no_color,
            ..UiData::default()
        });
        let _ui_handle = setup_render_ui(&shutdown_tx, &ui_tx);
        (Some(ui_tx), Some(shutdown_tx))
    } else {
        (None, None)
    };

    DistributedOutputState {
        charts_enabled: !args.no_charts && streaming_enabled,
        sink_updates_enabled: streaming_enabled && args.sinks.is_some(),
        sink_dirty: false,
        aggregated_samples: Vec::new(),
        ui_tx,
        shutdown_tx,
        ui_latency_window: VecDeque::new(),
        ui_rps_window: VecDeque::new(),
    }
}

pub(in crate::distributed::controller) async fn handle_output_event(
    args: &TesterArgs,
    state: &mut DistributedOutputState,
    agent_states: &HashMap<String, AgentSnapshot>,
    runtime_errors: &mut Vec<String>,
    event: OutputEvent,
) {
    match event {
        OutputEvent::AgentStateUpdated => {
            if state.charts_enabled {
                record_aggregated_sample(&mut state.aggregated_samples, agent_states);
            }
            if let Some(ui_tx) = state.ui_tx.as_ref() {
                update_ui(
                    ui_tx,
                    args,
                    agent_states,
                    &mut state.ui_latency_window,
                    &mut state.ui_rps_window,
                );
            }
            if state.sink_updates_enabled {
                state.sink_dirty = true;
            }
        }
        OutputEvent::SinkTick => {
            if state.sink_updates_enabled && state.sink_dirty {
                if let Err(err) = write_streaming_sinks(args, agent_states).await {
                    runtime_errors.push(err.to_string());
                } else {
                    state.sink_dirty = false;
                }
            }
        }
    }
}

pub(in crate::distributed::controller) async fn finalize_output(
    args: &TesterArgs,
    state: &mut DistributedOutputState,
    agent_states: &HashMap<String, AgentSnapshot>,
    runtime_errors: &mut Vec<String>,
) {
    if agent_states.is_empty() {
        runtime_errors.push("No successful agent reports received.".to_owned());
        send_shutdown_signal(state);
        return;
    }

    let Ok((summary, merged_hist, success_hist)) = aggregate_snapshots(agent_states) else {
        runtime_errors.push("Failed to aggregate agent summaries.".to_owned());
        send_shutdown_signal(state);
        return;
    };

    let (p50, p90, p99) = merged_hist.percentiles();
    let (success_p50, success_p90, success_p99) = success_hist.percentiles();
    let stats = compute_summary_stats(&summary);
    let mut charts_output_path: Option<String> = None;
    if state.charts_enabled {
        match write_aggregated_charts(&state.aggregated_samples, args).await {
            Ok(path) => charts_output_path = path,
            Err(err) => runtime_errors.push(err.to_string()),
        }
    }

    let percentiles = SummaryPercentiles {
        all: Percentiles { p50, p90, p99 },
        ok: Percentiles {
            p50: success_p50,
            p90: success_p90,
            p99: success_p99,
        },
    };

    print_summary(&summary, percentiles, args, charts_output_path.as_deref());

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
        if let Err(err) = write_sinks(sinks, &sink_stats).await {
            runtime_errors.push(format!("Sinks: {}", err));
        }
    }

    send_shutdown_signal(state);
}

fn send_shutdown_signal(state: &DistributedOutputState) {
    if let Some(shutdown_tx) = state.shutdown_tx.as_ref() {
        drop(shutdown_tx.send(()));
    }
}

async fn write_streaming_sinks(
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
        write_sinks(sinks, &sink_stats).await?;
    }
    Ok(())
}

async fn write_aggregated_charts(
    samples: &[AggregatedMetricSample],
    args: &TesterArgs,
) -> AppResult<Option<String>> {
    if args.no_charts || samples.len() < 2 {
        return Ok(None);
    }
    charts::plot_aggregated_metrics(samples, args).await
}
