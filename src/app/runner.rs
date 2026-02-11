use std::error::Error;
use std::io::IsTerminal;
use std::path::Path;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, watch};
use tokio::time::Instant;
use tracing::{info, warn};

use crate::{
    args::{OutputFormat, TesterArgs},
    charts, http,
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
        no_color: args.no_color,
        ..UiData::default()
    };
    let (ui_tx, _) = watch::channel(initial_ui);
    let (metrics_tx, metrics_rx) = mpsc::channel::<Metrics>(10_000);

    let ui_enabled = !args.no_ui && std::io::stdout().is_terminal();
    if !ui_enabled && !args.no_ui {
        info!("UI disabled because stdout is not a TTY.");
    }
    let charts_enabled = !args.no_charts;

    let summary_enabled = args.summary || args.no_ui || !ui_enabled;
    let rss_handle = setup_rss_log_task(&shutdown_tx, args.no_ui, args.rss_log_ms.as_ref());
    let alloc_handle = setup_alloc_profiler_task(&shutdown_tx, args.alloc_profiler_ms.as_ref());
    let alloc_dump_handle = setup_alloc_profiler_dump_task(
        &shutdown_tx,
        args.alloc_profiler_dump_ms.as_ref(),
        &args.alloc_profiler_dump_path,
    );

    if ui_enabled
        && !args.no_splash
        && let Err(err) = crate::ui::render::run_splash_screen(args.no_color).await
    {
        warn!("Failed to render splash screen: {}", err);
    }

    let run_start = Instant::now();
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
    let (_, _, _, _, _, _, _, metrics_result, request_result) = tokio::join!(
        keyboard_shutdown_handle,
        signal_shutdown_handle,
        render_ui_handle,
        progress_handle,
        rss_handle,
        alloc_handle,
        alloc_dump_handle,
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
        _chart_records_unused,
        _metrics_truncated_unused,
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
                charts::plot_streaming_metrics(&chart_data, &args).await?;
                info!("Charts saved in {}", args.charts_path);
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

    if let Some(path) = args.output.as_deref()
        && matches!(
            args.output_format,
            Some(OutputFormat::Text | OutputFormat::Quiet)
        )
        && let Err(err) = export_text_summary(
            path,
            &summary,
            &summary_stats,
            &args,
            &summary::SummaryExtras {
                metrics_truncated,
                charts_enabled,
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
    }

    if let Some(path) = args.export_json.as_deref()
        && let Err(err) = export::export_json(path, &summary, &chart_records).await
    {
        runtime_errors.push(format!("Failed to export JSON: {}", err));
    }

    if let Some(path) = args.export_jsonl.as_deref()
        && let Err(err) = export::export_jsonl(path, &summary, &chart_records).await
    {
        runtime_errors.push(format!("Failed to export JSONL: {}", err));
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

fn setup_rss_log_task(
    shutdown_tx: &broadcast::Sender<u16>,
    no_ui: bool,
    interval_ms: Option<&crate::args::PositiveU64>,
) -> tokio::task::JoinHandle<()> {
    if !no_ui {
        return tokio::spawn(async {});
    }
    let Some(interval_ms) = interval_ms.map(|value| value.get()) else {
        return tokio::spawn(async {});
    };
    let shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = interval.tick() => {
                    if let Some(rss_bytes) = read_rss_bytes() {
                        let rss_mb_x100 = u128::from(rss_bytes)
                            .saturating_mul(100)
                            .checked_div(1024 * 1024)
                            .unwrap_or(0);
                        let whole = rss_mb_x100 / 100;
                        let frac = rss_mb_x100 % 100;
                        info!("rss_mb={}.{:02}", whole, frac);
                    } else {
                        break;
                    }
                }
            }
        }
    })
}

fn read_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let mut parts = statm.split_whitespace();
        let _size = parts.next()?;
        let resident = parts.next()?.parse::<u64>().ok()?;
        // Safety: sysconf is safe to call; we only read the page size.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page_size <= 0 {
            return None;
        }
        let page_size = u64::try_from(page_size).ok()?;
        Some(resident.saturating_mul(page_size))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

fn setup_alloc_profiler_task(
    shutdown_tx: &broadcast::Sender<u16>,
    interval_ms: Option<&crate::args::PositiveU64>,
) -> tokio::task::JoinHandle<()> {
    let Some(interval_ms) = interval_ms.map(|value| value.get()) else {
        return tokio::spawn(async {});
    };
    setup_alloc_profiler_task_inner(shutdown_tx, interval_ms)
}

#[cfg(not(feature = "alloc-profiler"))]
fn setup_alloc_profiler_task_inner(
    _shutdown_tx: &broadcast::Sender<u16>,
    _interval_ms: u64,
) -> tokio::task::JoinHandle<()> {
    warn!("alloc-profiler-ms set but alloc-profiler feature is disabled.");
    tokio::spawn(async {})
}

#[cfg(feature = "alloc-profiler")]
fn setup_alloc_profiler_task_inner(
    shutdown_tx: &broadcast::Sender<u16>,
    interval_ms: u64,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = interval.tick() => {
                    if let Err(err) = log_alloc_stats() {
                        warn!("alloc-profiler failed: {}", err);
                        break;
                    }
                }
            }
        }
    })
}

fn setup_alloc_profiler_dump_task(
    shutdown_tx: &broadcast::Sender<u16>,
    interval_ms: Option<&crate::args::PositiveU64>,
    dump_path: &str,
) -> tokio::task::JoinHandle<()> {
    let Some(interval_ms) = interval_ms.map(|value| value.get()) else {
        return tokio::spawn(async {});
    };
    setup_alloc_profiler_dump_task_inner(shutdown_tx, interval_ms, dump_path)
}

#[cfg(not(feature = "alloc-profiler"))]
fn setup_alloc_profiler_dump_task_inner(
    _shutdown_tx: &broadcast::Sender<u16>,
    _interval_ms: u64,
    _dump_path: &str,
) -> tokio::task::JoinHandle<()> {
    warn!("alloc-profiler-dump-ms set but alloc-profiler feature is disabled.");
    tokio::spawn(async {})
}

#[cfg(feature = "alloc-profiler")]
fn setup_alloc_profiler_dump_task_inner(
    shutdown_tx: &broadcast::Sender<u16>,
    interval_ms: u64,
    dump_path: &str,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    let dump_path = dump_path.to_owned();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        if let Err(err) = tokio::fs::create_dir_all(&dump_path).await {
            warn!(
                "alloc-profiler failed to create dump dir {}: {}",
                dump_path, err
            );
            return;
        }
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = interval.tick() => {
                    if let Err(err) = dump_alloc_profile(&dump_path) {
                        warn!("alloc-profiler dump failed: {}", err);
                        break;
                    }
                }
            }
        }
    })
}

#[cfg(feature = "alloc-profiler")]
fn log_alloc_stats() -> Result<(), String> {
    jemalloc_ctl::epoch::advance().map_err(|err| format!("epoch advance failed: {}", err))?;
    let allocated = jemalloc_ctl::stats::allocated::read()
        .map_err(|err| format!("allocated read failed: {}", err))?;
    let active = jemalloc_ctl::stats::active::read()
        .map_err(|err| format!("active read failed: {}", err))?;
    let resident = jemalloc_ctl::stats::resident::read()
        .map_err(|err| format!("resident read failed: {}", err))?;
    let mapped = jemalloc_ctl::stats::mapped::read()
        .map_err(|err| format!("mapped read failed: {}", err))?;
    let metadata = jemalloc_ctl::stats::metadata::read()
        .map_err(|err| format!("metadata read failed: {}", err))?;
    info!(
        "alloc_bytes={},active_bytes={},resident_bytes={},mapped_bytes={},metadata_bytes={}",
        allocated, active, resident, mapped, metadata
    );
    Ok(())
}

#[cfg(feature = "alloc-profiler")]
fn dump_alloc_profile(dir: &str) -> Result<(), String> {
    use std::ffi::CString;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("timestamp error: {}", err))?
        .as_millis();
    let path = std::path::Path::new(dir).join(format!("heap-{}.prof", stamp));
    let path_cstr = CString::new(path.to_string_lossy().as_bytes())
        .map_err(|err| format!("invalid dump path: {}", err))?;
    ensure_prof_enabled()?;
    jemalloc_ctl::epoch::advance().map_err(|err| format!("epoch advance failed: {}", err))?;
    // Safety: prof.dump expects a C string pointing to the output file path.
    unsafe {
        jemalloc_ctl::raw::write(b"prof.dump\0", path_cstr.as_ptr())
            .map_err(|err| format!("prof dump failed: {}", err))?;
    }
    info!("alloc_profiler_dump={}", path.display());
    Ok(())
}

#[cfg(feature = "alloc-profiler")]
fn ensure_prof_enabled() -> Result<(), String> {
    let config_prof = unsafe { jemalloc_ctl::raw::read::<bool>(b"config.prof\0") }
        .map_err(|err| format!("prof config read failed: {}", err))?;
    if !config_prof {
        return Err("jemalloc profiling not compiled (config.prof=false)".to_owned());
    }
    let opt_prof = unsafe { jemalloc_ctl::raw::read::<bool>(b"opt.prof\0") }
        .map_err(|err| format!("opt.prof read failed: {}", err))?;
    if !opt_prof {
        return Err(
            "jemalloc profiling disabled (opt.prof=false). Set MALLOC_CONF=prof:true".to_owned(),
        );
    }
    let active = unsafe { jemalloc_ctl::raw::read::<bool>(b"prof.active\0") }
        .map_err(|err| format!("prof.active read failed: {}", err))?;
    if !active {
        // Safety: prof.active expects a boolean value.
        unsafe {
            jemalloc_ctl::raw::write(b"prof.active\0", true)
                .map_err(|err| format!("prof.active write failed: {}", err))?;
        }
    }
    Ok(())
}
