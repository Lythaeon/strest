use crate::args::TimeUnit;
use crate::{args::TesterArgs, metrics};

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
/// Milliseconds per second.
const MS_PER_SEC_U64: u64 = 1_000;
/// Microseconds per millisecond.
const US_PER_MS: u128 = 1_000;
/// Nanoseconds per millisecond.
const NS_PER_MS: u128 = 1_000_000;
/// Milliseconds per minute.
const MS_PER_MIN: u64 = 60_000;
/// Milliseconds per hour.
const MS_PER_HOUR: u64 = 3_600_000;
/// Minimum unit divisor to avoid divide-by-zero.
const MIN_UNIT_MS: u64 = 1;
/// Fraction scale for formatted durations.
const FRACTION_SCALE: u64 = 1_000;
/// Standard percentile labels.
const PERCENTILE_P50: u64 = 50;
const PERCENTILE_P90: u64 = 90;
const PERCENTILE_P99: u64 = 99;
/// Rounding offset for percentile selection.
const PERCENTILE_ROUNDING: u64 = 50;

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
    let duration_ms = summary.duration.as_millis().max(MIN_DURATION_MS);
    let total = summary.total_requests;
    let success = summary.successful_requests;

    let success_rate_x100 = if total > 0 {
        let scaled = u128::from(success)
            .saturating_mul(SUCCESS_RATE_SCALE)
            .checked_div(u128::from(total))
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let avg_rps_x100 = if total > 0 {
        let scaled = u128::from(total)
            .saturating_mul(RPS_SCALE)
            .checked_div(duration_ms)
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
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

pub(crate) fn print_summary(
    summary: &metrics::MetricsSummary,
    extras: &SummaryExtras,
    stats: &SummaryStats,
    args: &TesterArgs,
) {
    for line in summary_lines(summary, extras, stats, args) {
        println!("{}", line);
    }
}

pub(crate) fn summary_lines(
    summary: &metrics::MetricsSummary,
    extras: &SummaryExtras,
    stats: &SummaryStats,
    args: &TesterArgs,
) -> Vec<String> {
    let mut lines = Vec::new();
    let total = summary.total_requests;
    let success = summary.successful_requests;
    let errors = summary.error_requests;
    let time_unit = args.time_unit;

    if let Some(unit) = time_unit {
        let duration_ms = u64::try_from(summary.duration.as_millis()).unwrap_or(u64::MAX);
        lines.push(format!(
            "Duration: {}",
            format_duration_ms(duration_ms, unit)
        ));
        lines.push(format!("Total Requests: {}", total));
        lines.push(format!(
            "Successful: {} ({}.{:02}%)",
            success,
            stats.success_rate_x100 / PERCENT_DIVISOR,
            stats.success_rate_x100 % PERCENT_DIVISOR
        ));
        lines.push(format!("Errors: {}", errors));
        lines.push(format!("Timeouts: {}", summary.timeout_requests));
        lines.push(format!("Transport Errors: {}", summary.transport_errors));
        lines.push(format!(
            "Non-Expected Status: {}",
            summary.non_expected_status
        ));
        lines.push(format!(
            "Avg Latency (all): {}",
            format_duration_ms(summary.avg_latency_ms, unit)
        ));
        lines.push(format!(
            "Avg Latency (ok): {}",
            format_duration_ms(summary.success_avg_latency_ms, unit)
        ));
        lines.push(format!(
            "Min/Max Latency (all): {} / {}",
            format_duration_ms(summary.min_latency_ms, unit),
            format_duration_ms(summary.max_latency_ms, unit)
        ));
        lines.push(format!(
            "Min/Max Latency (ok): {} / {}",
            format_duration_ms(summary.success_min_latency_ms, unit),
            format_duration_ms(summary.success_max_latency_ms, unit)
        ));
        lines.push(format!(
            "P50/P90/P99 Latency (all): {} / {} / {}",
            format_duration_ms(extras.p50, unit),
            format_duration_ms(extras.p90, unit),
            format_duration_ms(extras.p99, unit)
        ));
        lines.push(format!(
            "P50/P90/P99 Latency (ok): {} / {} / {}",
            format_duration_ms(extras.success_p50, unit),
            format_duration_ms(extras.success_p90, unit),
            format_duration_ms(extras.success_p99, unit)
        ));
    } else {
        lines.push(format!("Duration: {}s", summary.duration.as_secs()));
        lines.push(format!("Total Requests: {}", total));
        lines.push(format!(
            "Successful: {} ({}.{:02}%)",
            success,
            stats.success_rate_x100 / PERCENT_DIVISOR,
            stats.success_rate_x100 % PERCENT_DIVISOR
        ));
        lines.push(format!("Errors: {}", errors));
        lines.push(format!("Timeouts: {}", summary.timeout_requests));
        lines.push(format!("Transport Errors: {}", summary.transport_errors));
        lines.push(format!(
            "Non-Expected Status: {}",
            summary.non_expected_status
        ));
        lines.push(format!("Avg Latency (all): {}ms", summary.avg_latency_ms));
        lines.push(format!(
            "Avg Latency (ok): {}ms",
            summary.success_avg_latency_ms
        ));
        lines.push(format!(
            "Min/Max Latency (all): {}ms / {}ms",
            summary.min_latency_ms, summary.max_latency_ms
        ));
        lines.push(format!(
            "Min/Max Latency (ok): {}ms / {}ms",
            summary.success_min_latency_ms, summary.success_max_latency_ms
        ));
        lines.push(format!(
            "P50/P90/P99 Latency (all): {}ms / {}ms / {}ms",
            extras.p50, extras.p90, extras.p99
        ));
        lines.push(format!(
            "P50/P90/P99 Latency (ok): {}ms / {}ms / {}ms",
            extras.success_p50, extras.success_p90, extras.success_p99
        ));
    }

    lines.push(format!(
        "Avg RPS: {}.{:02}",
        stats.avg_rps_x100 / PERCENT_DIVISOR,
        stats.avg_rps_x100 % PERCENT_DIVISOR
    ));
    lines.push(format!(
        "Avg RPM: {}.{:02}",
        stats.avg_rpm_x100 / PERCENT_DIVISOR,
        stats.avg_rpm_x100 % PERCENT_DIVISOR
    ));

    if !extras.charts_enabled {
        lines.push("Charts: disabled".to_owned());
    } else if extras.metrics_truncated {
        lines.push(format!(
            "Charts: enabled (truncated at {} metrics).",
            args.metrics_max.get()
        ));
    } else {
        lines.push("Charts: enabled".to_owned());
    }

    lines
}

fn format_duration_ms(value_ms: u64, unit: TimeUnit) -> String {
    match unit {
        TimeUnit::Ns => format!("{}ns", u128::from(value_ms).saturating_mul(NS_PER_MS)),
        TimeUnit::Us => format!("{}us", u128::from(value_ms).saturating_mul(US_PER_MS)),
        TimeUnit::Ms => format!("{}ms", value_ms),
        TimeUnit::S => format_fraction_ms(value_ms, MS_PER_SEC_U64, "s"),
        TimeUnit::M => format_fraction_ms(value_ms, MS_PER_MIN, "m"),
        TimeUnit::H => format_fraction_ms(value_ms, MS_PER_HOUR, "h"),
    }
}

fn format_fraction_ms(value_ms: u64, unit_ms: u64, suffix: &str) -> String {
    let unit_ms = unit_ms.max(MIN_UNIT_MS);
    let whole = value_ms.checked_div(unit_ms).unwrap_or(0);
    let remainder = value_ms.checked_rem(unit_ms).unwrap_or(0);
    let thousandths = remainder
        .checked_mul(FRACTION_SCALE)
        .and_then(|value| value.checked_div(unit_ms))
        .unwrap_or(0);
    format!("{}.{:03}{}", whole, thousandths, suffix)
}

pub(crate) fn compute_percentiles(records: &[metrics::MetricRecord]) -> (u64, u64, u64) {
    if records.is_empty() {
        return (0, 0, 0);
    }
    let mut latencies: Vec<u64> = records.iter().map(|record| record.latency_ms).collect();
    latencies.sort_unstable();

    let p50 = percentile(&latencies, PERCENTILE_P50);
    let p90 = percentile(&latencies, PERCENTILE_P90);
    let p99 = percentile(&latencies, PERCENTILE_P99);

    (p50, p90, p99)
}

fn percentile(values: &[u64], percentile: u64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let count = values.len().saturating_sub(1) as u64;
    let index = percentile
        .saturating_mul(count)
        .saturating_add(PERCENTILE_ROUNDING)
        .checked_div(PERCENT_DIVISOR)
        .unwrap_or(0);
    let idx = usize::try_from(index).unwrap_or_else(|_| values.len().saturating_sub(1));
    *values.get(idx).unwrap_or(&0)
}
