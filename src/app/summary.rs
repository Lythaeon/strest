use crate::{args::TesterArgs, metrics};

pub(crate) struct SummaryExtras {
    pub(crate) metrics_truncated: bool,
    pub(crate) charts_enabled: bool,
    pub(crate) p50: u64,
    pub(crate) p90: u64,
    pub(crate) p99: u64,
    pub(crate) success_p50: u64,
    pub(crate) success_p90: u64,
    pub(crate) success_p99: u64,
}

pub(crate) struct SummaryStats {
    pub(crate) success_rate_x100: u64,
    pub(crate) avg_rps_x100: u64,
    pub(crate) avg_rpm_x100: u64,
}

pub(crate) fn compute_summary_stats(summary: &metrics::MetricsSummary) -> SummaryStats {
    let duration_ms = summary.duration.as_millis().max(1);
    let total = summary.total_requests;
    let success = summary.successful_requests;

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

    SummaryStats {
        success_rate_x100,
        avg_rps_x100,
        avg_rpm_x100,
    }
}

pub(crate) fn print_summary(
    summary: &metrics::MetricsSummary,
    extras: &SummaryExtras,
    stats: &SummaryStats,
    args: &TesterArgs,
) {
    let total = summary.total_requests;
    let success = summary.successful_requests;
    let errors = summary.error_requests;

    println!("Duration: {}s", summary.duration.as_secs());
    println!("Total Requests: {}", total);
    println!(
        "Successful: {} ({}.{:02}%)",
        success,
        stats.success_rate_x100 / 100,
        stats.success_rate_x100 % 100
    );
    println!("Errors: {}", errors);
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
        extras.p50, extras.p90, extras.p99
    );
    println!(
        "P50/P90/P99 Latency (ok): {}ms / {}ms / {}ms",
        extras.success_p50, extras.success_p90, extras.success_p99
    );
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

    if !extras.charts_enabled {
        println!("Charts: disabled");
    } else if extras.metrics_truncated {
        println!(
            "Charts: enabled (truncated at {} metrics).",
            args.metrics_max.get()
        );
    } else {
        println!("Charts: enabled");
    }
}

pub(crate) fn compute_percentiles(records: &[metrics::MetricRecord]) -> (u64, u64, u64) {
    if records.is_empty() {
        return (0, 0, 0);
    }
    let mut latencies: Vec<u64> = records.iter().map(|record| record.latency_ms).collect();
    latencies.sort_unstable();

    let p50 = percentile(&latencies, 50);
    let p90 = percentile(&latencies, 90);
    let p99 = percentile(&latencies, 99);

    (p50, p90, p99)
}

fn percentile(values: &[u64], percentile: u64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let count = values.len().saturating_sub(1) as u64;
    let index = percentile
        .saturating_mul(count)
        .saturating_add(50)
        .checked_div(100)
        .unwrap_or(0);
    let idx = usize::try_from(index).unwrap_or_else(|_| values.len().saturating_sub(1));
    *values.get(idx).unwrap_or(&0)
}
