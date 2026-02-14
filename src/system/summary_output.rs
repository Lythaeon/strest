use crate::args::{OutputFormat, PositiveU64, TesterArgs};
use crate::metrics::MetricsRange;

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
