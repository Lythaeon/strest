use std::time::Duration;

use crate::error::AppResult;
use crate::metrics::{LatencyHistogram, MetricRecord, MetricsSummary};

use super::super::summary as app_summary;

pub(crate) struct SummaryOutput {
    pub(crate) summary: MetricsSummary,
    pub(crate) histogram: LatencyHistogram,
    pub(crate) success_histogram: LatencyHistogram,
}

pub(crate) fn summarize(
    records: &[MetricRecord],
    expected_status_code: u16,
    window_start_ms: u64,
    window_end_ms: u64,
) -> AppResult<SummaryOutput> {
    let mut histogram = LatencyHistogram::new()?;
    let mut success_histogram = LatencyHistogram::new()?;

    let mut total_requests: u64 = 0;
    let mut successful_requests: u64 = 0;
    let mut timeout_requests: u64 = 0;
    let mut transport_errors: u64 = 0;
    let mut non_expected_status: u64 = 0;
    let mut latency_sum_ms: u128 = 0;
    let mut success_latency_sum_ms: u128 = 0;
    let mut min_latency_ms: u64 = u64::MAX;
    let mut max_latency_ms: u64 = 0;
    let mut success_min_latency_ms: u64 = u64::MAX;
    let mut success_max_latency_ms: u64 = 0;

    for record in records {
        total_requests = total_requests.saturating_add(1);
        latency_sum_ms = latency_sum_ms.saturating_add(u128::from(record.latency_ms));
        if record.latency_ms < min_latency_ms {
            min_latency_ms = record.latency_ms;
        }
        if record.latency_ms > max_latency_ms {
            max_latency_ms = record.latency_ms;
        }
        if record.status_code == expected_status_code
            && !record.timed_out
            && !record.transport_error
        {
            successful_requests = successful_requests.saturating_add(1);
            success_latency_sum_ms =
                success_latency_sum_ms.saturating_add(u128::from(record.latency_ms));
            if record.latency_ms < success_min_latency_ms {
                success_min_latency_ms = record.latency_ms;
            }
            if record.latency_ms > success_max_latency_ms {
                success_max_latency_ms = record.latency_ms;
            }
            success_histogram.record(record.latency_ms)?;
        }
        if record.timed_out {
            timeout_requests = timeout_requests.saturating_add(1);
        } else if record.transport_error {
            transport_errors = transport_errors.saturating_add(1);
        } else if record.status_code != expected_status_code {
            non_expected_status = non_expected_status.saturating_add(1);
        }
        histogram.record(record.latency_ms)?;
    }

    let duration_ms = window_end_ms.saturating_sub(window_start_ms);
    let duration = Duration::from_millis(duration_ms);
    let avg_latency_ms = if total_requests > 0 {
        let avg = latency_sum_ms
            .checked_div(u128::from(total_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };
    let success_avg_latency_ms = if successful_requests > 0 {
        let avg = success_latency_sum_ms
            .checked_div(u128::from(successful_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };
    let min_latency_ms = if total_requests > 0 {
        min_latency_ms
    } else {
        0
    };
    let max_latency_ms = if total_requests > 0 {
        max_latency_ms
    } else {
        0
    };
    let success_min_latency_ms = if successful_requests > 0 {
        success_min_latency_ms
    } else {
        0
    };
    let success_max_latency_ms = if successful_requests > 0 {
        success_max_latency_ms
    } else {
        0
    };
    let error_requests = total_requests.saturating_sub(successful_requests);

    Ok(SummaryOutput {
        summary: MetricsSummary {
            duration,
            total_requests,
            successful_requests,
            error_requests,
            timeout_requests,
            transport_errors,
            non_expected_status,
            min_latency_ms,
            max_latency_ms,
            avg_latency_ms,
            success_min_latency_ms,
            success_max_latency_ms,
            success_avg_latency_ms,
        },
        histogram,
        success_histogram,
    })
}

pub(super) fn compute_replay_percentiles(
    summary_output: &SummaryOutput,
    slice: &[MetricRecord],
    expected_status_code: u16,
) -> (u64, u64, u64, u64, u64, u64) {
    let (mut p50, mut p90, mut p99) = summary_output.histogram.percentiles();
    let (mut success_p50, mut success_p90, mut success_p99) =
        summary_output.success_histogram.percentiles();
    if summary_output.histogram.count() == 0 {
        let (fallback_p50, fallback_p90, fallback_p99) = app_summary::compute_percentiles(slice);
        p50 = fallback_p50;
        p90 = fallback_p90;
        p99 = fallback_p99;
    }
    if summary_output.success_histogram.count() == 0 {
        let success_records: Vec<MetricRecord> = slice
            .iter()
            .copied()
            .filter(|record| {
                record.status_code == expected_status_code
                    && !record.timed_out
                    && !record.transport_error
            })
            .collect();
        let (fallback_p50, fallback_p90, fallback_p99) =
            app_summary::compute_percentiles(&success_records);
        success_p50 = fallback_p50;
        success_p90 = fallback_p90;
        success_p99 = fallback_p99;
    }
    (p50, p90, p99, success_p50, success_p90, success_p99)
}
