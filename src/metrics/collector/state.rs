use std::collections::VecDeque;
use std::time::Duration;

use tokio::time::Instant;

use crate::ui::model::StatusCounts;

use super::super::LatencyHistogram;

pub(super) struct UiAggregationState {
    pub(super) current_requests: u64,
    pub(super) successful_requests: u64,
    pub(super) timeout_requests: u64,
    pub(super) transport_errors: u64,
    pub(super) non_expected_status: u64,
    pub(super) in_flight_ops: u64,
    pub(super) ui_window: Duration,
    pub(super) latency_sum_ms: u128,
    pub(super) success_latency_sum_ms: u128,
    pub(super) min_latency_ms: u64,
    pub(super) max_latency_ms: u64,
    pub(super) success_min_latency_ms: u64,
    pub(super) success_max_latency_ms: u64,
    pub(super) latency_window: VecDeque<(Instant, u64)>,
    pub(super) latency_window_ok: VecDeque<(Instant, u64)>,
    pub(super) rps_window: VecDeque<(Instant, u64)>,
    pub(super) rps_samples: VecDeque<(Instant, u64)>,
    pub(super) status_counts: StatusCounts,
    pub(super) bytes_window: VecDeque<(Instant, u64)>,
    pub(super) bytes_samples: VecDeque<(Instant, u64)>,
    pub(super) total_bytes: u128,
    pub(super) histogram: Option<LatencyHistogram>,
    pub(super) success_histogram: Option<LatencyHistogram>,
}

impl UiAggregationState {
    pub(super) fn new(ui_window: Duration) -> Self {
        let histogram = match LatencyHistogram::new() {
            Ok(histogram) => Some(histogram),
            Err(err) => {
                tracing::warn!("Failed to initialize latency histogram: {}", err);
                None
            }
        };
        let success_histogram = match LatencyHistogram::new() {
            Ok(success_histogram) => Some(success_histogram),
            Err(err) => {
                tracing::warn!("Failed to initialize success latency histogram: {}", err);
                None
            }
        };

        Self {
            current_requests: 0,
            successful_requests: 0,
            timeout_requests: 0,
            transport_errors: 0,
            non_expected_status: 0,
            in_flight_ops: 0,
            ui_window,
            latency_sum_ms: 0,
            success_latency_sum_ms: 0,
            min_latency_ms: u64::MAX,
            max_latency_ms: 0,
            success_min_latency_ms: u64::MAX,
            success_max_latency_ms: 0,
            latency_window: VecDeque::new(),
            latency_window_ok: VecDeque::new(),
            rps_window: VecDeque::new(),
            rps_samples: VecDeque::new(),
            status_counts: StatusCounts::default(),
            bytes_window: VecDeque::new(),
            bytes_samples: VecDeque::new(),
            total_bytes: 0,
            histogram,
            success_histogram,
        }
    }
}
