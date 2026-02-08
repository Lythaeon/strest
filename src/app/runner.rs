use std::error::Error;
use std::io::IsTerminal;
use std::path::Path;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::Instant;
use tracing::info;

use crate::{
    args::TesterArgs,
    charts::plot_metrics,
    http,
    metrics::{self, Metrics},
    sinks::{config::SinkStats, writers},
    ui::{model::UiData, render::setup_render_ui},
};

use super::{cleanup, export, logs, progress, summary};

pub(crate) struct RunOutcome {
    pub summary: metrics::MetricsSummary,
    pub histogram: metrics::LatencyHistogram,
    pub success_histogram: metrics::LatencyHistogram,
    pub latency_sum_ms: u128,
    pub success_latency_sum_ms: u128,
    pub runtime_errors: Vec<String>,
}

pub(crate) async fn run_local(
    args: TesterArgs,
    stream_tx: Option<mpsc::UnboundedSender<metrics::StreamSnapshot>>,
    mut external_shutdown: Option<watch::Receiver<bool>>,
) -> Result<RunOutcome, Box<dyn Error>> {
    let (shutdown_tx, _) = broadcast::channel::<u16>(1);
    if let Some(mut external_shutdown) = external_shutdown.take() {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            loop {
                if external_shutdown.changed().await.is_err() {
                    break;
                }
                if *external_shutdown.borrow() {
                    drop(shutdown_tx.send(1));
                    break;
                }
            }
        });
    }
    let initial_ui = UiData {
        target_duration: Duration::from_secs(args.target_duration.get()),
        ui_window_ms: args.ui_window_ms.get(),
        ..UiData::default()
    };
    let (ui_tx, _) = watch::channel(initial_ui);
    let (metrics_tx, metrics_rx) = mpsc::channel::<Metrics>(10_000);

    let ui_enabled = !args.no_ui && std::io::stdout().is_terminal();
    if !ui_enabled && !args.no_ui {
        info!("UI disabled because stdout is not a TTY.");
    }
    let charts_enabled = !args.no_charts;

    let run_start = Instant::now();
    let summary_enabled = args.summary || args.no_ui || !ui_enabled;
    let logs::LogSetup {
        log_sink,
        handles: log_handles,
        paths: log_paths,
    } = logs::setup_log_sinks(&args, run_start, charts_enabled, summary_enabled).await?;

    let request_sender_handle =
        match http::setup_request_sender(&args, &shutdown_tx, &metrics_tx, log_sink.as_ref()) {
            Ok(handle) => handle,
            Err(err) => {
                eprintln!("Failed to setup request sender: {}", err);
                return Err(err.into());
            }
        };
    drop(metrics_tx);

    let keyboard_shutdown_handle = if ui_enabled {
        crate::shutdown::setup_keyboard_shutdown_handler(&shutdown_tx)
    } else {
        tokio::spawn(async {})
    };
    let signal_shutdown_handle = crate::shutdown::setup_signal_shutdown_handler(&shutdown_tx);
    let render_ui_handle = if ui_enabled {
        setup_render_ui(&args, &shutdown_tx, &ui_tx)
    } else {
        tokio::spawn(async {})
    };
    let progress_handle = if args.no_ui && !args.verbose {
        progress::setup_progress_indicator(&args, run_start, &shutdown_tx)
    } else {
        tokio::spawn(async {})
    };
    let metrics_handle = metrics::setup_metrics_collector(
        &args,
        run_start,
        &shutdown_tx,
        metrics_rx,
        &ui_tx,
        stream_tx,
    );
    let metrics_max = args.metrics_max.get();
    let (_, _, _, _, metrics_result, request_result) = tokio::join!(
        keyboard_shutdown_handle,
        signal_shutdown_handle,
        render_ui_handle,
        progress_handle,
        metrics_handle,
        request_sender_handle
    );

    drop(log_sink);

    let mut runtime_errors = Vec::new();
    if let Err(err) = request_result {
        runtime_errors.push(format!("Request sender task failed: {}", err));
    }

    let report = match metrics_result {
        Ok(report) => report,
        Err(err) => {
            runtime_errors.push(format!("Metrics collector task failed: {}", err));
            metrics::MetricsReport {
                summary: logs::empty_summary(),
            }
        }
    };

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
        chart_records,
        metrics_truncated,
        histogram,
        latency_sum_ms,
        success_latency_sum_ms,
        success_histogram,
    ) = if !log_results.is_empty() {
        logs::merge_log_results(log_results, metrics_max).map_err(std::io::Error::other)?
    } else {
        (
            report.summary,
            Vec::new(),
            false,
            metrics::LatencyHistogram::new().map_err(std::io::Error::other)?,
            0,
            0,
            metrics::LatencyHistogram::new().map_err(std::io::Error::other)?,
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
    let (mut p50, mut p90, mut p99) = histogram.percentiles();
    let (mut success_p50, mut success_p90, mut success_p99) = success_histogram.percentiles();
    if histogram.count() == 0 {
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

    if charts_enabled && !chart_records.is_empty() {
        info!("Plotting charts...");

        plot_metrics(&chart_records, &args).await?;

        info!("Charts saved in {}", args.charts_path);
    }

    let summary_stats = summary::compute_summary_stats(&summary);

    if summary_enabled && !args.distributed_silent {
        let extras = summary::SummaryExtras {
            metrics_truncated,
            charts_enabled,
            p50,
            p90,
            p99,
            success_p50,
            success_p90,
            success_p99,
        };
        summary::print_summary(&summary, &extras, &summary_stats, &args);
    }

    if let Some(path) = args.export_csv.as_deref()
        && let Err(err) = export::export_csv(path, &chart_records).await
    {
        runtime_errors.push(format!("Failed to export CSV: {}", err));
    }

    if let Some(path) = args.export_json.as_deref()
        && let Err(err) = export::export_json(path, &summary, &chart_records).await
    {
        runtime_errors.push(format!("Failed to export JSON: {}", err));
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
