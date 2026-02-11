use std::time::Duration;

use crate::args::TesterArgs;
use crate::error::AppResult;
use crate::metrics::MetricRecord;
use crate::ui::model::{ReplayUi, UiData};

use super::super::summary as app_summary;
use super::state::{ReplayWindow, SnapshotMarkers};
use super::{summary, window_slice};

/// Milliseconds per second for rate windows.
const MS_PER_SEC: u64 = 1000;
/// Seconds per minute for RPM conversion.
const SECS_PER_MIN: u64 = 60;

pub(super) fn render_once(
    records: &[MetricRecord],
    args: &TesterArgs,
    start_ms: u64,
    end_ms: u64,
) -> AppResult<()> {
    let slice = window_slice(records, start_ms, end_ms);
    let summary_output = summary::summarize(slice, args.expected_status_code, start_ms, end_ms)?;
    let stats = app_summary::compute_summary_stats(&summary_output.summary);
    let (p50, p90, p99, success_p50, success_p90, success_p99) =
        summary::compute_replay_percentiles(&summary_output, slice, args.expected_status_code);
    let extras = app_summary::SummaryExtras {
        metrics_truncated: false,
        charts_enabled: false,
        p50,
        p90,
        p99,
        success_p50,
        success_p90,
        success_p99,
    };
    for line in app_summary::summary_lines(&summary_output.summary, &extras, &stats, args) {
        println!("{line}");
    }
    Ok(())
}

pub(super) fn build_ui_data(
    records: &[MetricRecord],
    args: &TesterArgs,
    state: &ReplayWindow,
    markers: &SnapshotMarkers,
    default_range: Option<(u64, u64)>,
) -> AppResult<UiData> {
    let slice = window_slice(records, state.start_ms, state.cursor_ms);
    let summary_output = summary::summarize(
        slice,
        args.expected_status_code,
        state.start_ms,
        state.cursor_ms,
    )?;
    let (p50, p90, p99, p50_ok, p90_ok, p99_ok) =
        summary::compute_replay_percentiles(&summary_output, slice, args.expected_status_code);

    let ui_window_ms = args.ui_window_ms.get();
    let chart_start = state.cursor_ms.saturating_sub(ui_window_ms);
    let chart_slice = window_slice(records, chart_start, state.cursor_ms);
    let latencies = chart_slice
        .iter()
        .map(|record| (record.elapsed_ms, record.latency_ms))
        .collect();

    let rps_start = state.cursor_ms.saturating_sub(MS_PER_SEC);
    let rps_slice = window_slice(records, rps_start, state.cursor_ms);
    let rps = u64::try_from(rps_slice.len()).unwrap_or(u64::MAX);
    let rpm = rps.saturating_mul(SECS_PER_MIN);

    let (snapshot_start_ms, snapshot_end_ms) = if markers.start.is_some() || markers.end.is_some() {
        (markers.start, markers.end)
    } else if let Some((start, end)) = default_range {
        (Some(start), Some(end))
    } else {
        (None, None)
    };

    Ok(UiData {
        elapsed_time: Duration::from_millis(state.cursor_ms.saturating_sub(state.start_ms)),
        target_duration: Duration::from_millis(state.end_ms.saturating_sub(state.start_ms)),
        current_requests: summary_output.summary.total_requests,
        successful_requests: summary_output.summary.successful_requests,
        timeout_requests: summary_output.summary.timeout_requests,
        transport_errors: summary_output.summary.transport_errors,
        non_expected_status: summary_output.summary.non_expected_status,
        ui_window_ms,
        no_color: args.no_color,
        latencies,
        p50,
        p90,
        p99,
        p50_ok,
        p90_ok,
        p99_ok,
        rps,
        rpm,
        replay: Some(ReplayUi {
            playing: state.playing,
            window_start_ms: state.start_ms,
            window_end_ms: state.end_ms,
            cursor_ms: state.cursor_ms,
            snapshot_start_ms,
            snapshot_end_ms,
        }),
    })
}
