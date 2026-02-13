use std::time::Duration;

use crate::error::AppResult;
use crate::metrics;

use super::LogMergeResult;

pub(super) fn merge_log_results(
    results: Vec<metrics::LogResult>,
    metrics_max: usize,
) -> AppResult<LogMergeResult> {
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
    let mut duration = Duration::ZERO;
    let mut records = Vec::new();
    let mut metrics_truncated = false;
    let mut histogram = metrics::LatencyHistogram::new()?;
    let mut success_histogram = metrics::LatencyHistogram::new()?;

    for result in results {
        total_requests = total_requests.saturating_add(result.summary.total_requests);
        successful_requests =
            successful_requests.saturating_add(result.summary.successful_requests);
        timeout_requests = timeout_requests.saturating_add(result.summary.timeout_requests);
        transport_errors = transport_errors.saturating_add(result.summary.transport_errors);
        non_expected_status =
            non_expected_status.saturating_add(result.summary.non_expected_status);
        latency_sum_ms = latency_sum_ms.saturating_add(result.latency_sum_ms);
        success_latency_sum_ms =
            success_latency_sum_ms.saturating_add(result.success_latency_sum_ms);
        if result.summary.total_requests > 0 {
            min_latency_ms = min_latency_ms.min(result.summary.min_latency_ms);
            max_latency_ms = max_latency_ms.max(result.summary.max_latency_ms);
        }
        if result.summary.successful_requests > 0 {
            success_min_latency_ms =
                success_min_latency_ms.min(result.summary.success_min_latency_ms);
            success_max_latency_ms =
                success_max_latency_ms.max(result.summary.success_max_latency_ms);
        }
        duration = duration.max(result.summary.duration);
        metrics_truncated = metrics_truncated || result.metrics_truncated;
        records.extend(result.records);
        histogram.merge(&result.histogram)?;
        success_histogram.merge(&result.success_histogram)?;
    }

    if metrics_max > 0 && records.len() > metrics_max {
        records.truncate(metrics_max);
        metrics_truncated = true;
    }
    records.sort_by_key(|record| record.elapsed_ms);

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

    Ok((
        metrics::MetricsSummary {
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
        records,
        metrics_truncated,
        histogram,
        latency_sum_ms,
        success_latency_sum_ms,
        success_histogram,
    ))
}

pub(super) const fn empty_summary() -> metrics::MetricsSummary {
    metrics::MetricsSummary {
        duration: Duration::ZERO,
        total_requests: 0,
        successful_requests: 0,
        error_requests: 0,
        timeout_requests: 0,
        transport_errors: 0,
        non_expected_status: 0,
        min_latency_ms: 0,
        max_latency_ms: 0,
        avg_latency_ms: 0,
        success_min_latency_ms: 0,
        success_max_latency_ms: 0,
        success_avg_latency_ms: 0,
    }
}
