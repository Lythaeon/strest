mod finalize;

use std::io::IsTerminal;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::Instant;
use tracing::{info, warn};

#[cfg(not(feature = "wasm"))]
use crate::error::ScriptError;
#[cfg(feature = "wasm")]
use crate::wasm_plugins::WasmPluginHost;
use crate::{
    args::TesterArgs,
    error::{AppError, AppResult, ValidationError},
    metrics::{self, Metrics},
    protocol,
    ui::{model::UiData, render::setup_render_ui},
};

use super::alloc::{setup_alloc_profiler_dump_task, setup_alloc_profiler_task};
use super::rss::setup_rss_log_task;
use crate::app::{logs, progress};
use finalize::{FinalizeContext, finalize_run};

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
) -> AppResult<RunOutcome> {
    #[cfg(feature = "wasm")]
    let mut plugin_host = WasmPluginHost::from_paths(&args.plugin)?;
    #[cfg(not(feature = "wasm"))]
    if !args.plugin.is_empty() {
        return Err(AppError::script(ScriptError::WasmFeatureDisabled));
    }

    #[cfg(feature = "wasm")]
    if let Some(host) = plugin_host.as_mut() {
        host.on_run_start(&args)?;
    }

    let (shutdown_tx, _) = crate::system::shutdown_handlers::shutdown_channel();
    if let Some(mut external_shutdown) = external_shutdown.take() {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            loop {
                if external_shutdown.changed().await.is_err() {
                    break;
                }
                if *external_shutdown.borrow() {
                    drop(shutdown_tx.send(()));
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

    let summary_enabled = args.summary || args.show_selections;
    let rss_handle = setup_rss_log_task(&shutdown_tx, args.no_ui, args.rss_log_ms.as_ref());
    let alloc_handle = setup_alloc_profiler_task(&shutdown_tx, args.alloc_profiler_ms.as_ref());
    let alloc_dump_handle = setup_alloc_profiler_dump_task(
        &shutdown_tx,
        args.alloc_profiler_dump_ms.as_ref(),
        &args.alloc_profiler_dump_path,
    );

    if ui_enabled && !args.no_splash {
        match crate::ui::render::run_splash_screen(args.no_color).await {
            Ok(true) => {}
            Ok(false) => {
                return Err(AppError::validation(ValidationError::RunCancelled));
            }
            Err(err) => {
                warn!("Failed to render splash screen: {}", err);
            }
        }
    }

    let run_start = Instant::now();
    let logs::LogSetup {
        log_sink,
        handles: log_handles,
        paths: log_paths,
    } = logs::setup_log_sinks(&args, run_start, charts_enabled, summary_enabled).await?;

    let request_sender_handle =
        match protocol::setup_request_sender(&args, &shutdown_tx, &metrics_tx, log_sink.as_ref()) {
            Ok(handle) => handle,
            Err(err) => {
                eprintln!("Failed to setup request sender: {}", err);
                return Err(err);
            }
        };
    drop(metrics_tx);

    let keyboard_shutdown_handle = if ui_enabled {
        crate::system::shutdown_handlers::setup_keyboard_shutdown_handler(&shutdown_tx)
    } else {
        tokio::spawn(async {})
    };
    let signal_shutdown_handle =
        crate::system::shutdown_handlers::setup_signal_shutdown_handler(&shutdown_tx);
    let render_ui_handle = if ui_enabled {
        setup_render_ui(&shutdown_tx, &ui_tx)
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

    finalize_run(FinalizeContext {
        args: &args,
        charts_enabled,
        summary_enabled,
        metrics_max,
        runtime_errors,
        report,
        log_handles,
        log_paths,
        #[cfg(feature = "wasm")]
        plugin_host: plugin_host.as_mut(),
    })
    .await
}
