use std::path::{Path, PathBuf};

use tracing::info;

use crate::{
    app::{cleanup, export, logs, summary},
    args::{OutputFormat, TesterArgs},
    charts,
    error::AppResult,
    metrics,
    sinks::{config::SinkStats, writers},
};

use super::RunOutcome;
#[cfg(feature = "wasm")]
use crate::wasm_plugins::WasmPluginHost;

pub(super) struct FinalizeContext<'args> {
    pub(super) args: &'args TesterArgs,
    pub(super) charts_enabled: bool,
    pub(super) summary_enabled: bool,
    pub(super) metrics_max: usize,
    pub(super) runtime_errors: Vec<String>,
    pub(super) report: metrics::MetricsReport,
    pub(super) log_handles: Vec<tokio::task::JoinHandle<AppResult<metrics::LogResult>>>,
    pub(super) log_paths: Vec<PathBuf>,
    #[cfg(feature = "wasm")]
    pub(super) plugin_host: Option<&'args mut WasmPluginHost>,
}

pub(super) async fn finalize_run(ctx: FinalizeContext<'_>) -> AppResult<RunOutcome> {
    let FinalizeContext {
        args,
        charts_enabled,
        summary_enabled,
        metrics_max,
        mut runtime_errors,
        report,
        log_handles,
        log_paths,
        #[cfg(feature = "wasm")]
        plugin_host,
    } = ctx;
    #[cfg(feature = "wasm")]
    let mut plugin_host = plugin_host;
    let mut log_results = Vec::new();
    for handle in log_handles {
        match handle.await {
            Ok(Ok(result)) => log_results.push(result),
            Ok(Err(err)) => {
                runtime_errors.push(format!("Metrics log task failed: {}", err));
            }
            Err(err) => {
                runtime_errors.push(format!("Log task join failed: {}", err));
            }
        }
    }

    let (
        summary,
        _chart_records_unused,
        _metrics_truncated_unused,
        histogram,
        latency_sum_ms,
        success_latency_sum_ms,
        success_histogram,
    ) = if !log_results.is_empty() {
        logs::merge_log_results(log_results, metrics_max)?
    } else {
        (
            report.summary,
            Vec::new(),
            false,
            metrics::LatencyHistogram::new()?,
            0,
            0,
            metrics::LatencyHistogram::new()?,
        )
    };
    let latency_sum_ms = if latency_sum_ms == 0 && summary.total_requests > 0 {
        u128::from(summary.avg_latency_ms).saturating_mul(u128::from(summary.total_requests))
    } else {
        latency_sum_ms
    };
    let success_latency_sum_ms = if success_latency_sum_ms == 0 && summary.successful_requests > 0 {
        u128::from(summary.success_avg_latency_ms)
            .saturating_mul(u128::from(summary.successful_requests))
    } else {
        success_latency_sum_ms
    };
    let need_chart_records =
        args.export_csv.is_some() || args.export_json.is_some() || args.export_jsonl.is_some();
    let (chart_records, metrics_truncated) = if need_chart_records && !log_paths.is_empty() {
        match logs::load_log_records(&log_paths, &args.metrics_range, metrics_max).await {
            Ok((records, truncated)) => (records, truncated),
            Err(err) => {
                runtime_errors.push(format!("Failed to load metrics logs: {}", err));
                (Vec::new(), false)
            }
        }
    } else {
        (Vec::new(), false)
    };

    let (mut p50, mut p90, mut p99) = histogram.percentiles();
    let (mut success_p50, mut success_p90, mut success_p99) = success_histogram.percentiles();
    if histogram.count() == 0 && !chart_records.is_empty() {
        let (fallback_p50, fallback_p90, fallback_p99) =
            summary::compute_percentiles(&chart_records);
        p50 = fallback_p50;
        p90 = fallback_p90;
        p99 = fallback_p99;
    }
    if success_histogram.count() == 0 && summary.successful_requests > 0 {
        let expected_status = args.expected_status_code;
        let success_records: Vec<metrics::MetricRecord> = chart_records
            .iter()
            .copied()
            .filter(|record| record.status_code == expected_status)
            .collect();
        let (fallback_p50, fallback_p90, fallback_p99) =
            summary::compute_percentiles(&success_records);
        success_p50 = fallback_p50;
        success_p90 = fallback_p90;
        success_p99 = fallback_p99;
    }

    let mut charts_output_path: Option<String> = None;
    if charts_enabled && !log_paths.is_empty() {
        info!("Plotting charts...");

        match logs::load_chart_data_streaming(
            &log_paths,
            args.expected_status_code,
            &args.metrics_range,
            args.charts_latency_bucket_ms.get(),
        )
        .await
        {
            Ok(chart_data) => {
                if let Some(path) = charts::plot_streaming_metrics(&chart_data, args).await? {
                    info!("Charts saved in {}", path);
                    #[cfg(feature = "wasm")]
                    if let Some(host) = plugin_host.as_mut()
                        && let Err(err) = host.on_artifact("charts", &path)
                    {
                        runtime_errors.push(format!("WASM plugin chart hook failed: {}", err));
                    }
                    charts_output_path = Some(path);
                }
            }
            Err(err) => {
                runtime_errors.push(format!("Failed to build charts: {}", err));
            }
        }
    }

    let summary_stats = summary::compute_summary_stats(&summary);

    if summary_enabled
        && !args.distributed_silent
        && args.output_format != Some(OutputFormat::Quiet)
    {
        let extras = summary::SummaryExtras {
            metrics_truncated,
            charts_output_path: charts_output_path.clone(),
            p50,
            p90,
            p99,
            success_p50,
            success_p90,
            success_p99,
        };
        summary::print_summary(&summary, &extras, &summary_stats, args);
    }

    if let Some(path) = args.output.as_deref()
        && matches!(
            args.output_format,
            Some(OutputFormat::Text | OutputFormat::Quiet)
        )
        && let Err(err) = export_text_summary(
            path,
            &summary,
            &summary_stats,
            args,
            &summary::SummaryExtras {
                metrics_truncated,
                charts_output_path: charts_output_path.clone(),
                p50,
                p90,
                p99,
                success_p50,
                success_p90,
                success_p99,
            },
        )
        .await
    {
        runtime_errors.push(format!("Failed to write output: {}", err));
    }

    if let Some(path) = args.export_csv.as_deref()
        && let Err(err) = export::export_csv(path, &chart_records).await
    {
        runtime_errors.push(format!("Failed to export CSV: {}", err));
    } else {
        #[cfg(feature = "wasm")]
        if let Some(path) = args.export_csv.as_deref()
            && let Some(host) = plugin_host.as_mut()
            && let Err(err) = host.on_artifact("export_csv", path)
        {
            runtime_errors.push(format!("WASM plugin CSV hook failed: {}", err));
        }
    }

    if let Some(path) = args.export_json.as_deref()
        && let Err(err) = export::export_json(path, &summary, &chart_records).await
    {
        runtime_errors.push(format!("Failed to export JSON: {}", err));
    } else {
        #[cfg(feature = "wasm")]
        if let Some(path) = args.export_json.as_deref()
            && let Some(host) = plugin_host.as_mut()
            && let Err(err) = host.on_artifact("export_json", path)
        {
            runtime_errors.push(format!("WASM plugin JSON hook failed: {}", err));
        }
    }

    if let Some(path) = args.export_jsonl.as_deref()
        && let Err(err) = export::export_jsonl(path, &summary, &chart_records).await
    {
        runtime_errors.push(format!("Failed to export JSONL: {}", err));
    } else {
        #[cfg(feature = "wasm")]
        if let Some(path) = args.export_jsonl.as_deref()
            && let Some(host) = plugin_host.as_mut()
            && let Err(err) = host.on_artifact("export_jsonl", path)
        {
            runtime_errors.push(format!("WASM plugin JSONL hook failed: {}", err));
        }
    }

    if let Some(sinks_config) = args.sinks.as_ref() {
        let sink_stats = SinkStats {
            duration: summary.duration,
            total_requests: summary.total_requests,
            successful_requests: summary.successful_requests,
            error_requests: summary.error_requests,
            timeout_requests: summary.timeout_requests,
            min_latency_ms: summary.min_latency_ms,
            max_latency_ms: summary.max_latency_ms,
            avg_latency_ms: summary.avg_latency_ms,
            p50_latency_ms: p50,
            p90_latency_ms: p90,
            p99_latency_ms: p99,
            success_rate_x100: summary_stats.success_rate_x100,
            avg_rps_x100: summary_stats.avg_rps_x100,
            avg_rpm_x100: summary_stats.avg_rpm_x100,
        };
        if let Err(err) = writers::write_sinks(sinks_config, &sink_stats).await {
            runtime_errors.push(format!("Failed to write sinks: {}", err));
        }
    }

    #[cfg(feature = "wasm")]
    if let Some(host) = plugin_host.as_mut() {
        if let Err(err) = host.on_metrics_summary(&summary) {
            runtime_errors.push(format!("WASM plugin summary hook failed: {}", err));
        }
        if let Err(err) = host.on_run_end(&runtime_errors) {
            runtime_errors.push(format!("WASM plugin run_end hook failed: {}", err));
        }
    }

    if !args.keep_tmp && !log_paths.is_empty() {
        for log_path in &log_paths {
            if let Err(err) = cleanup::cleanup_tmp(log_path, Path::new(&args.tmp_path)).await {
                runtime_errors.push(format!("Failed to cleanup tmp data: {}", err));
            }
        }
    }

    Ok(RunOutcome {
        summary,
        histogram,
        success_histogram,
        latency_sum_ms,
        success_latency_sum_ms,
        runtime_errors,
    })
}

async fn export_text_summary(
    path: &str,
    summary: &metrics::MetricsSummary,
    stats: &summary::SummaryStats,
    args: &TesterArgs,
    extras: &summary::SummaryExtras,
) -> Result<(), std::io::Error> {
    if matches!(args.output_format, Some(OutputFormat::Quiet)) {
        tokio::fs::write(path, "").await?;
        return Ok(());
    }
    let lines = summary::summary_lines(summary, extras, stats, args);
    let content = lines.join("\n");
    tokio::fs::write(path, content).await?;
    Ok(())
}
