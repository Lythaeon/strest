use crate::app::replay::summary;
use crate::app::summary::{PERCENT_DIVISOR, compute_percentiles, compute_summary_stats};
use crate::args::CompareArgs;
use crate::error::AppResult;
use crate::metrics::MetricRecord;

pub(super) fn print_compare_summary(
    label: &str,
    records: &[MetricRecord],
    expected_status_code: u16,
    start_ms: u64,
    end_ms: u64,
    _args: &CompareArgs,
) -> AppResult<()> {
    let summary_output = summary::summarize(records, expected_status_code, start_ms, end_ms)?;
    let stats = compute_summary_stats(&summary_output.summary);
    let (p50, p90, p99) = compute_percentiles(records);
    println!("Snapshot: {label}");
    println!(
        "Duration: {}ms",
        summary_output.summary.duration.as_millis()
    );
    println!("Total Requests: {}", summary_output.summary.total_requests);
    println!(
        "Successful: {} ({}.{:02}%)",
        summary_output.summary.successful_requests,
        stats.success_rate_x100 / PERCENT_DIVISOR,
        stats.success_rate_x100 % PERCENT_DIVISOR
    );
    println!("Errors: {}", summary_output.summary.error_requests);
    println!("Timeouts: {}", summary_output.summary.timeout_requests);
    println!(
        "Transport Errors: {}",
        summary_output.summary.transport_errors
    );
    println!(
        "Non-Expected Status: {}",
        summary_output.summary.non_expected_status
    );
    println!(
        "Avg Latency (all): {}ms",
        summary_output.summary.avg_latency_ms
    );
    println!(
        "Avg Latency (ok): {}ms",
        summary_output.summary.success_avg_latency_ms
    );
    println!(
        "Min/Max Latency (all): {}ms / {}ms",
        summary_output.summary.min_latency_ms, summary_output.summary.max_latency_ms
    );
    println!(
        "Min/Max Latency (ok): {}ms / {}ms",
        summary_output.summary.success_min_latency_ms,
        summary_output.summary.success_max_latency_ms
    );
    println!(
        "P50/P90/P99 Latency (all): {}ms / {}ms / {}ms",
        p50, p90, p99
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
    println!();
    Ok(())
}
