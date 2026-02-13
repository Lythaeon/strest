use crate::args::{OutputFormat, PositiveU64, TesterArgs, TimeUnit};
use crate::metrics::{self, MetricsRange};

use super::{PERCENT_DIVISOR, SummaryExtras, SummaryStats};

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

    lines.push(chart_status_line(
        args,
        extras.charts_output_path.as_deref(),
        extras.metrics_truncated,
    ));

    if args.show_selections {
        lines.extend(selection_lines(args, extras.charts_output_path.as_deref()));
    }

    lines
}

pub(crate) fn selection_lines(args: &TesterArgs, charts_output_path: Option<&str>) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("Selections:".to_owned());
    lines.push(format!("protocol: {}", args.protocol.as_str()));
    lines.push(format!("load_mode: {}", args.load_mode.as_str()));
    lines.push(format!("url: {}", args.url.as_deref().unwrap_or("none")));
    lines.push(format!("method: {:?}", args.method));
    lines.push(format!("duration_s: {}", args.target_duration.get()));
    lines.push(format!("requests: {}", format_opt_u64(args.requests)));
    lines.push(format!(
        "rate_limit_rps: {}",
        format_opt_u64(args.rate_limit)
    ));
    lines.push(format!("max_tasks: {}", args.max_tasks.get()));
    lines.push(format!("spawn_rate: {}", args.spawn_rate_per_tick.get()));
    lines.push(format!("spawn_interval_ms: {}", args.tick_interval.get()));
    lines.push(format!("expected_status: {}", args.expected_status_code));
    lines.push(format!(
        "request_timeout_ms: {}",
        args.request_timeout.as_millis()
    ));
    lines.push(format!(
        "connect_timeout_ms: {}",
        args.connect_timeout.as_millis()
    ));
    lines.push(format!("redirect_limit: {}", args.redirect_limit));
    lines.push(format!("no_tui: {}", args.no_ui));
    lines.push(format!("summary: {}", args.summary));
    lines.push(format!("no_charts: {}", args.no_charts));
    lines.push(format!("charts_path: {}", args.charts_path));
    lines.push(format!(
        "charts_latency_bucket_ms: {}",
        args.charts_latency_bucket_ms.get()
    ));
    lines.push(format!("tmp_path: {}", args.tmp_path));
    lines.push(format!("keep_tmp: {}", args.keep_tmp));
    lines.push(format!(
        "metrics_range: {}",
        format_metrics_range(&args.metrics_range)
    ));
    lines.push(format!("metrics_max: {}", args.metrics_max.get()));
    lines.push(format!(
        "output_format: {}",
        format_output_format(args.output_format)
    ));
    lines.push(format!(
        "output: {}",
        args.output.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "export_csv: {}",
        args.export_csv.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "export_json: {}",
        args.export_json.as_deref().unwrap_or("none")
    ));
    lines.push(format!(
        "export_jsonl: {}",
        args.export_jsonl.as_deref().unwrap_or("none")
    ));
    lines.push(format!("no_color: {}", args.no_color));
    lines.push(format!(
        "charts_output: {}",
        charts_output_path.unwrap_or("none")
    ));
    lines
}

pub(crate) fn chart_status_line(
    args: &TesterArgs,
    charts_output_path: Option<&str>,
    metrics_truncated: bool,
) -> String {
    if args.no_charts {
        return "Charts: disabled (--no-charts selected)".to_owned();
    }
    if let Some(path) = charts_output_path {
        return format!("Charts: saved in {}", path);
    }
    if metrics_truncated {
        return format!(
            "Charts: enabled (truncated at {} metrics).",
            args.metrics_max.get()
        );
    }
    "Charts: enabled".to_owned()
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

fn format_opt_u64(value: Option<PositiveU64>) -> String {
    value
        .map(|val| val.get().to_string())
        .unwrap_or_else(|| "none".to_owned())
}

fn format_metrics_range(range: &Option<MetricsRange>) -> String {
    range.as_ref().map_or_else(
        || "none".to_owned(),
        |range| format!("{}-{}", range.0.start(), range.0.end()),
    )
}

const fn format_output_format(format: Option<OutputFormat>) -> &'static str {
    match format {
        Some(OutputFormat::Text) => "text",
        Some(OutputFormat::Json) => "json",
        Some(OutputFormat::Jsonl) => "jsonl",
        Some(OutputFormat::Csv) => "csv",
        Some(OutputFormat::Quiet) => "quiet",
        None => "none",
    }
}
