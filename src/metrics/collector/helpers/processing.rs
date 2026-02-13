use std::collections::VecDeque;
use std::time::Duration;

use tokio::time::Instant;

use crate::ui::model::StatusCounts;

use super::super::super::Metrics;
use super::super::state::UiAggregationState;
use super::windows::{prune_bytes_window, prune_latency_window, prune_rps_window};

pub(in crate::metrics::collector) fn process_metric_ui(
    msg: Metrics,
    now: Instant,
    expected_status_code: u16,
    state: &mut UiAggregationState,
) {
    let status_code = msg.status_code;
    let latency_ms = u64::try_from(msg.response_time.as_millis()).unwrap_or(u64::MAX);
    state.in_flight_ops = msg.in_flight_ops;

    state.current_requests = state.current_requests.saturating_add(1);

    let is_success = status_code == expected_status_code && !msg.timed_out && !msg.transport_error;
    if is_success {
        state.successful_requests = state.successful_requests.saturating_add(1);
        state.success_latency_sum_ms = state
            .success_latency_sum_ms
            .saturating_add(u128::from(latency_ms));
        if latency_ms < state.success_min_latency_ms {
            state.success_min_latency_ms = latency_ms;
        }
        if latency_ms > state.success_max_latency_ms {
            state.success_max_latency_ms = latency_ms;
        }
        state.latency_window_ok.push_back((now, latency_ms));
        prune_latency_window(&mut state.latency_window_ok, now, state.ui_window);
        if let Some(histogram) = state.success_histogram.as_mut()
            && let Err(err) = histogram.record(latency_ms)
        {
            tracing::warn!("Disabling success latency histogram after error: {}", err);
            state.success_histogram = None;
        }
    } else if msg.timed_out {
        state.timeout_requests = state.timeout_requests.saturating_add(1);
    } else if msg.transport_error {
        state.transport_errors = state.transport_errors.saturating_add(1);
    } else if status_code != expected_status_code {
        state.non_expected_status = state.non_expected_status.saturating_add(1);
    }

    increment_status_counts(
        &mut state.status_counts,
        bucket_status(status_code, msg.timed_out, msg.transport_error),
    );

    state.latency_window.push_back((now, latency_ms));
    prune_latency_window(&mut state.latency_window, now, state.ui_window);

    record_rps(&mut state.rps_window, now);
    prune_rps_window(&mut state.rps_window, now);

    state.total_bytes = state
        .total_bytes
        .saturating_add(u128::from(msg.response_bytes));
    record_bytes(&mut state.bytes_window, now, msg.response_bytes);
    prune_bytes_window(&mut state.bytes_window, now);

    state.latency_sum_ms = state.latency_sum_ms.saturating_add(u128::from(latency_ms));
    if latency_ms < state.min_latency_ms {
        state.min_latency_ms = latency_ms;
    }
    if latency_ms > state.max_latency_ms {
        state.max_latency_ms = latency_ms;
    }

    if let Some(histogram) = state.histogram.as_mut()
        && let Err(err) = histogram.record(latency_ms)
    {
        tracing::warn!("Disabling latency histogram after error: {}", err);
        state.histogram = None;
    }
}

fn record_rps(window: &mut VecDeque<(Instant, u64)>, now: Instant) {
    if let Some((ts, count)) = window.back_mut() {
        if now.duration_since(*ts) < Duration::from_millis(100) {
            *count = count.saturating_add(1);
        } else {
            window.push_back((now, 1));
        }
    } else {
        window.push_back((now, 1));
    }
}

#[derive(Clone, Copy)]
enum StatusBucket {
    Status2xx,
    Status3xx,
    Status4xx,
    Status5xx,
    Other,
}

const fn bucket_status(status_code: u16, timed_out: bool, transport_error: bool) -> StatusBucket {
    if timed_out || transport_error {
        return StatusBucket::Other;
    }
    match status_code {
        200..=299 => StatusBucket::Status2xx,
        300..=399 => StatusBucket::Status3xx,
        400..=499 => StatusBucket::Status4xx,
        500..=599 => StatusBucket::Status5xx,
        _ => StatusBucket::Other,
    }
}

const fn increment_status_counts(counts: &mut StatusCounts, bucket: StatusBucket) {
    match bucket {
        StatusBucket::Status2xx => counts.status_2xx = counts.status_2xx.saturating_add(1),
        StatusBucket::Status3xx => counts.status_3xx = counts.status_3xx.saturating_add(1),
        StatusBucket::Status4xx => counts.status_4xx = counts.status_4xx.saturating_add(1),
        StatusBucket::Status5xx => counts.status_5xx = counts.status_5xx.saturating_add(1),
        StatusBucket::Other => counts.status_other = counts.status_other.saturating_add(1),
    }
}

fn record_bytes(window: &mut VecDeque<(Instant, u64)>, now: Instant, bytes: u64) {
    if let Some((ts, count)) = window.back_mut()
        && now.duration_since(*ts) < Duration::from_millis(100)
    {
        *count = count.saturating_add(bytes);
        return;
    }
    window.push_back((now, bytes));
}
