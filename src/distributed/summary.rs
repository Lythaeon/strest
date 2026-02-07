use std::time::Duration;

use crate::args::TesterArgs;
use crate::metrics::MetricsSummary;

use super::protocol::WireSummary;

pub(super) fn merge_summaries(summaries: &[WireSummary]) -> MetricsSummary {
    let mut total_requests = 0u64;
    let mut successful_requests = 0u64;
    let mut error_requests = 0u64;
    let mut min_latency_ms = u64::MAX;
    let mut max_latency_ms = 0u64;
    let mut latency_sum_ms = 0u128;
    let mut duration_ms = 0u64;

    for summary in summaries {
        total_requests = total_requests.saturating_add(summary.total_requests);
        successful_requests = successful_requests.saturating_add(summary.successful_requests);
        error_requests = error_requests.saturating_add(summary.error_requests);
        if summary.total_requests > 0 {
            min_latency_ms = min_latency_ms.min(summary.min_latency_ms);
            max_latency_ms = max_latency_ms.max(summary.max_latency_ms);
        }
        latency_sum_ms = latency_sum_ms.saturating_add(summary.latency_sum_ms);
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

    let min_latency_ms = if total_requests > 0 {
        min_latency_ms
    } else {
        0
    };

    MetricsSummary {
        duration: Duration::from_millis(duration_ms),
        total_requests,
        successful_requests,
        error_requests,
        min_latency_ms,
        max_latency_ms,
        avg_latency_ms,
    }
}

pub(super) struct SummaryStats {
    pub(super) success_rate_x100: u64,
    pub(super) avg_rps_x100: u64,
    pub(super) avg_rpm_x100: u64,
}

pub(super) fn compute_summary_stats(summary: &MetricsSummary) -> SummaryStats {
    let duration_ms = summary.duration.as_millis().max(1);
    let total = summary.total_requests;
    let success = summary.successful_requests;

    let success_rate_x100 = if total > 0 {
        let scaled = u128::from(success)
            .saturating_mul(10_000)
            .checked_div(u128::from(total))
            .unwrap_or(0);
        u64::try_from(scaled).unwrap_or(u64::MAX)
    } else {
        0
    };

    let avg_rps_x100 = if total > 0 {
        let scaled = u128::from(total)
            .saturating_mul(100_000)
            .checked_div(duration_ms)
            .unwrap_or(0);
        u64::try_from(scaled).unwrap_or(u64::MAX)
    } else {
        0
    };
    let avg_rpm_x100 = avg_rps_x100.saturating_mul(60);

    SummaryStats {
        success_rate_x100,
        avg_rps_x100,
        avg_rpm_x100,
    }
}

pub(super) fn print_summary(
    summary: &MetricsSummary,
    p50: u64,
    p90: u64,
    p99: u64,
    args: &TesterArgs,
    charts_written: bool,
) {
    let stats = compute_summary_stats(summary);

    println!("Duration: {}s", summary.duration.as_secs());
    println!("Total Requests: {}", summary.total_requests);
    println!(
        "Successful: {} ({}.{:02}%)",
        summary.successful_requests,
        stats.success_rate_x100 / 100,
        stats.success_rate_x100 % 100
    );
    println!("Errors: {}", summary.error_requests);
    println!("Avg Latency: {}ms", summary.avg_latency_ms);
    println!(
        "Min/Max Latency: {}ms / {}ms",
        summary.min_latency_ms, summary.max_latency_ms
    );
    println!("P50/P90/P99 Latency: {}ms / {}ms / {}ms", p50, p90, p99);
    println!(
        "Avg RPS: {}.{:02}",
        stats.avg_rps_x100 / 100,
        stats.avg_rps_x100 % 100
    );
    println!(
        "Avg RPM: {}.{:02}",
        stats.avg_rpm_x100 / 100,
        stats.avg_rpm_x100 % 100
    );

    if args.no_charts {
        println!("Charts: disabled");
    } else if charts_written {
        println!("Charts: saved in {}", args.charts_path);
    } else {
        println!("Charts: unavailable (enable --stream-summaries)");
    }
}
