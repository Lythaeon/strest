use std::time::Duration;

use crate::args::TesterArgs;
use crate::metrics::MetricsSummary;

use super::protocol::WireSummary;

/// Minimum non-zero duration used to avoid divide-by-zero.
const MIN_DURATION_MS: u128 = 1;
/// Percent scale for success rate (x100 = 10_000).
const SUCCESS_RATE_SCALE: u128 = 10_000;
/// Scale for average RPS in hundredths.
const RPS_SCALE: u128 = 100_000;
/// Divisor to format x100 values as `xx.yy`.
const PERCENT_DIVISOR: u64 = 100;
/// RPM conversion factor from RPS.
const RPM_PER_RPS: u64 = 60;

pub(super) fn merge_summaries(summaries: &[WireSummary]) -> MetricsSummary {
    let mut total_requests = 0u64;
    let mut successful_requests = 0u64;
    let mut error_requests = 0u64;
    let mut timeout_requests = 0u64;
    let mut transport_errors = 0u64;
    let mut non_expected_status = 0u64;
    let mut min_latency_ms = u64::MAX;
    let mut max_latency_ms = 0u64;
    let mut latency_sum_ms = 0u128;
    let mut success_min_latency_ms = u64::MAX;
    let mut success_max_latency_ms = 0u64;
    let mut success_latency_sum_ms = 0u128;
    let mut duration_ms = 0u64;

    for summary in summaries {
        total_requests = total_requests.saturating_add(summary.total_requests);
        successful_requests = successful_requests.saturating_add(summary.successful_requests);
        error_requests = error_requests.saturating_add(summary.error_requests);
        timeout_requests = timeout_requests.saturating_add(summary.timeout_requests);
        transport_errors = transport_errors.saturating_add(summary.transport_errors);
        non_expected_status = non_expected_status.saturating_add(summary.non_expected_status);
        if summary.total_requests > 0 {
            min_latency_ms = min_latency_ms.min(summary.min_latency_ms);
            max_latency_ms = max_latency_ms.max(summary.max_latency_ms);
        }
        if summary.successful_requests > 0 {
            success_min_latency_ms = success_min_latency_ms.min(summary.success_min_latency_ms);
            success_max_latency_ms = success_max_latency_ms.max(summary.success_max_latency_ms);
        }
        latency_sum_ms = latency_sum_ms.saturating_add(summary.latency_sum_ms);
        success_latency_sum_ms =
            success_latency_sum_ms.saturating_add(summary.success_latency_sum_ms);
        duration_ms = duration_ms.max(summary.duration_ms);
    }

    let avg_latency_ms = if total_requests > 0 {
        let avg = latency_sum_ms
            .checked_div(u128::from(total_requests))
            .unwrap_or(0);
        u64::try_from(avg).unwrap_or(u64::MAX)
    } else {
        0
    };
    let success_avg_latency_ms = if successful_requests > 0 {
        let avg = success_latency_sum_ms
            .checked_div(u128::from(successful_requests))
            .unwrap_or(0);
        u64::try_from(avg).unwrap_or(u64::MAX)
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

    MetricsSummary {
        duration: Duration::from_millis(duration_ms),
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
    }
}

pub(super) struct SummaryStats {
    pub(super) success_rate_x100: u64,
    pub(super) avg_rps_x100: u64,
    pub(super) avg_rpm_x100: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Percentiles {
    pub(super) p50: u64,
    pub(super) p90: u64,
    pub(super) p99: u64,
}

#[derive(Clone, Copy)]
pub(super) struct SummaryPercentiles {
    pub(super) all: Percentiles,
    pub(super) ok: Percentiles,
}

pub(super) fn compute_summary_stats(summary: &MetricsSummary) -> SummaryStats {
    let duration_ms = summary.duration.as_millis().max(MIN_DURATION_MS);
    let total = summary.total_requests;
    let success = summary.successful_requests;

    let success_rate_x100 = if total > 0 {
        let scaled = u128::from(success)
            .saturating_mul(SUCCESS_RATE_SCALE)
            .checked_div(u128::from(total))
            .unwrap_or(0);
        u64::try_from(scaled).unwrap_or(u64::MAX)
    } else {
        0
    };

    let avg_rps_x100 = if total > 0 {
        let scaled = u128::from(total)
            .saturating_mul(RPS_SCALE)
            .checked_div(duration_ms)
            .unwrap_or(0);
        u64::try_from(scaled).unwrap_or(u64::MAX)
    } else {
        0
    };
    let avg_rpm_x100 = avg_rps_x100.saturating_mul(RPM_PER_RPS);

    SummaryStats {
        success_rate_x100,
        avg_rps_x100,
        avg_rpm_x100,
    }
}

pub(super) fn print_summary(
    summary: &MetricsSummary,
    percentiles: SummaryPercentiles,
    args: &TesterArgs,
    charts_written: bool,
) {
    let stats = compute_summary_stats(summary);

    println!("Duration: {}s", summary.duration.as_secs());
    println!("Total Requests: {}", summary.total_requests);
    println!(
        "Successful: {} ({}.{:02}%)",
        summary.successful_requests,
        stats.success_rate_x100 / PERCENT_DIVISOR,
        stats.success_rate_x100 % PERCENT_DIVISOR
    );
    println!("Errors: {}", summary.error_requests);
    println!("Timeouts: {}", summary.timeout_requests);
    println!("Transport Errors: {}", summary.transport_errors);
    println!("Non-Expected Status: {}", summary.non_expected_status);
    println!("Avg Latency (all): {}ms", summary.avg_latency_ms);
    println!("Avg Latency (ok): {}ms", summary.success_avg_latency_ms);
    println!(
        "Min/Max Latency (all): {}ms / {}ms",
        summary.min_latency_ms, summary.max_latency_ms
    );
    println!(
        "Min/Max Latency (ok): {}ms / {}ms",
        summary.success_min_latency_ms, summary.success_max_latency_ms
    );
    println!(
        "P50/P90/P99 Latency (all): {}ms / {}ms / {}ms",
        percentiles.all.p50, percentiles.all.p90, percentiles.all.p99
    );
    println!(
        "P50/P90/P99 Latency (ok): {}ms / {}ms / {}ms",
        percentiles.ok.p50, percentiles.ok.p90, percentiles.ok.p99
    );
    println!(
        "Avg RPS: {}.{:02}",
        stats.avg_rps_x100 / PERCENT_DIVISOR,
        stats.avg_rps_x100 % PERCENT_DIVISOR
    );
    println!(
        "Avg RPM: {}.{:02}",
        stats.avg_rpm_x100 / PERCENT_DIVISOR,
        stats.avg_rpm_x100 % PERCENT_DIVISOR
    );

    if args.no_charts {
        println!("Charts: disabled");
    } else if charts_written {
        println!("Charts: saved in {}", args.charts_path);
    } else {
        println!("Charts: unavailable (enable --stream-summaries)");
    }
}
