use std::time::Duration;

use crate::{
    args::PositiveU64,
    sinks::config::{SinkStats, SinksConfig},
};

use super::super::super::StreamSnapshot;
use super::super::state::UiAggregationState;

const SINK_UPDATE_INTERVAL: Duration = Duration::from_secs(1);
const STREAM_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

pub(in crate::metrics::collector) fn build_sink_stats(
    state: &UiAggregationState,
    duration: Duration,
) -> SinkStats {
    let total_requests = state.current_requests;
    let successful_requests = state.successful_requests;
    let error_requests = total_requests.saturating_sub(successful_requests);

    let avg_latency_ms = if total_requests > 0 {
        let avg = state
            .latency_sum_ms
            .checked_div(u128::from(total_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let min_latency_ms = if total_requests > 0 {
        state.min_latency_ms
    } else {
        0
    };
    let max_latency_ms = if total_requests > 0 {
        state.max_latency_ms
    } else {
        0
    };

    let (p50_latency_ms, p90_latency_ms, p99_latency_ms) = state
        .histogram
        .as_ref()
        .map(|histogram| histogram.percentiles())
        .unwrap_or((0, 0, 0));

    let (success_rate_x100, avg_rps_x100, avg_rpm_x100) =
        compute_rate_stats(total_requests, successful_requests, duration);

    SinkStats {
        duration,
        total_requests,
        successful_requests,
        error_requests,
        timeout_requests: state.timeout_requests,
        min_latency_ms,
        max_latency_ms,
        avg_latency_ms,
        p50_latency_ms,
        p90_latency_ms,
        p99_latency_ms,
        success_rate_x100,
        avg_rps_x100,
        avg_rpm_x100,
    }
}

pub(in crate::metrics::collector) fn build_stream_snapshot(
    state: &UiAggregationState,
    duration: Duration,
) -> Option<StreamSnapshot> {
    let histogram = state.histogram.as_ref()?;
    let histogram_b64 = match histogram.encode_base64() {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!("Failed to encode histogram for stream snapshot: {}", err);
            return None;
        }
    };

    let total_requests = state.current_requests;
    let successful_requests = state.successful_requests;
    let error_requests = total_requests.saturating_sub(successful_requests);
    let min_latency_ms = if total_requests > 0 {
        state.min_latency_ms
    } else {
        0
    };
    let max_latency_ms = if total_requests > 0 {
        state.max_latency_ms
    } else {
        0
    };
    let success_min_latency_ms = if successful_requests > 0 {
        state.success_min_latency_ms
    } else {
        0
    };
    let success_max_latency_ms = if successful_requests > 0 {
        state.success_max_latency_ms
    } else {
        0
    };

    Some(StreamSnapshot {
        duration,
        total_requests,
        successful_requests,
        error_requests,
        timeout_requests: state.timeout_requests,
        transport_errors: state.transport_errors,
        non_expected_status: state.non_expected_status,
        min_latency_ms,
        max_latency_ms,
        latency_sum_ms: state.latency_sum_ms,
        success_min_latency_ms,
        success_max_latency_ms,
        success_latency_sum_ms: state.success_latency_sum_ms,
        histogram_b64,
    })
}

fn compute_rate_stats(total: u64, success: u64, duration: Duration) -> (u64, u64, u64) {
    let duration_ms = duration.as_millis().max(1);

    let success_rate_x100 = if total > 0 {
        let scaled = u128::from(success)
            .saturating_mul(10_000)
            .checked_div(u128::from(total))
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let avg_rps_x100 = if total > 0 {
        let scaled = u128::from(total)
            .saturating_mul(100_000)
            .checked_div(duration_ms)
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let avg_rpm_x100 = avg_rps_x100.saturating_mul(60);

    (success_rate_x100, avg_rps_x100, avg_rpm_x100)
}

pub(in crate::metrics::collector) fn resolve_sink_interval(
    config: &Option<SinksConfig>,
) -> Duration {
    let Some(config) = config.as_ref() else {
        return SINK_UPDATE_INTERVAL;
    };
    match config.update_interval_ms {
        Some(0) => {
            tracing::warn!(
                "sinks.update_interval_ms must be > 0; using default {}ms",
                SINK_UPDATE_INTERVAL.as_millis()
            );
            SINK_UPDATE_INTERVAL
        }
        Some(ms) => Duration::from_millis(ms),
        None => SINK_UPDATE_INTERVAL,
    }
}

pub(in crate::metrics::collector) const fn resolve_stream_interval(
    interval: Option<&PositiveU64>,
) -> Duration {
    let Some(value) = interval else {
        return STREAM_UPDATE_INTERVAL;
    };
    Duration::from_millis(value.get())
}
