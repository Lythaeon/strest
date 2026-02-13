use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use tokio::sync::watch;

use crate::args::TesterArgs;
use crate::ui::model::UiData;

use super::super::super::summary::compute_summary_stats;
use super::aggregate_snapshots;
use super::events::AgentSnapshot;

pub(in crate::distributed::controller) fn update_ui(
    ui_tx: &watch::Sender<UiData>,
    args: &TesterArgs,
    agent_states: &HashMap<String, AgentSnapshot>,
    latency_window: &mut VecDeque<(u64, u64)>,
    rps_window: &mut VecDeque<(u64, u64)>,
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

    let rps = stats.avg_rps_x100 / 100;
    rps_window.push_back((elapsed_ms, rps));
    while rps_window.front().is_some_and(|(ts, _)| *ts < window_start) {
        rps_window.pop_front();
    }
    let rps_series: Vec<(u64, u64)> = rps_window.iter().copied().collect();

    drop(ui_tx.send(UiData {
        elapsed_time: summary.duration,
        target_duration: Duration::from_secs(args.target_duration.get()),
        current_requests: summary.total_requests,
        successful_requests: summary.successful_requests,
        timeout_requests: summary.timeout_requests,
        transport_errors: summary.transport_errors,
        non_expected_status: summary.non_expected_status,
        in_flight_ops: 0,
        ui_window_ms,
        no_color: args.no_color,
        latencies,
        rps_series,
        status_counts: None,
        data_usage: None,
        p50,
        p90,
        p99,
        p50_ok,
        p90_ok,
        p99_ok,
        rps,
        rpm: stats.avg_rpm_x100 / 100,
        replay: None,
        compare: None,
    }));
}
