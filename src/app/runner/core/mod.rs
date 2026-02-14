mod finalize;

use std::io::IsTerminal;
use std::time::Duration;

use crate::{
    app::{logs, progress},
    application::local_run::{
        self, FinalizeRunInput, LocalRunExecutionCommand, MetricsCollectorInput, MetricsPort,
        OutputPort, ShutdownPort, TrafficPort,
    },
    args::TesterArgs,
    error::AppResult,
    metrics::{self, Metrics},
    protocol,
    shutdown::{ShutdownReceiver, ShutdownSender},
    system::shutdown_handlers,
    ui::{model::UiData, render::setup_render_ui},
};
use async_trait::async_trait;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

use super::alloc::{setup_alloc_profiler_dump_task, setup_alloc_profiler_task};
use super::rss::setup_rss_log_task;
use finalize::{FinalizeContext, finalize_run as finalize_local_run};

pub(crate) type RunOutcome = local_run::RunOutcome;

pub(crate) async fn run_local(
    args: TesterArgs,
    stream_tx: Option<mpsc::UnboundedSender<metrics::StreamSnapshot>>,
    external_shutdown: Option<watch::Receiver<bool>>,
) -> AppResult<RunOutcome> {
    let command = LocalRunExecutionCommand::new(args, stream_tx, external_shutdown);
    let shutdown_adapter = RuntimeShutdownAdapter;
    let traffic_adapter = RuntimeTrafficAdapter;
    let metrics_adapter = RuntimeMetricsAdapter;
    let output_adapter = RuntimeOutputAdapter;

    local_run::execute(
        command,
        &shutdown_adapter,
        &traffic_adapter,
        &metrics_adapter,
        &output_adapter,
    )
    .await
}

struct RuntimeShutdownAdapter;

impl ShutdownPort for RuntimeShutdownAdapter {
    fn shutdown_channel(&self) -> (ShutdownSender, ShutdownReceiver) {
        shutdown_handlers::shutdown_channel()
    }

    fn bridge_external_shutdown(
        &self,
        shutdown_tx: &ShutdownSender,
        mut external_shutdown: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()> {
        let shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            loop {
                if external_shutdown.changed().await.is_err() {
                    break;
                }
                if *external_shutdown.borrow() {
                    if shutdown_tx.send(()).is_err() {
                        // Run is already stopping.
                    }
                    break;
                }
            }
        })
    }

    fn setup_keyboard_shutdown_handler(
        &self,
        shutdown_tx: &ShutdownSender,
    ) -> tokio::task::JoinHandle<()> {
        shutdown_handlers::setup_keyboard_shutdown_handler(shutdown_tx)
    }

    fn setup_signal_shutdown_handler(
        &self,
        shutdown_tx: &ShutdownSender,
    ) -> tokio::task::JoinHandle<()> {
        shutdown_handlers::setup_signal_shutdown_handler(shutdown_tx)
    }
}

struct RuntimeTrafficAdapter;

impl TrafficPort for RuntimeTrafficAdapter {
    fn setup_request_sender(
        &self,
        args: &TesterArgs,
        shutdown_tx: &ShutdownSender,
        metrics_tx: &mpsc::Sender<Metrics>,
        log_sink: Option<&std::sync::Arc<metrics::LogSink>>,
    ) -> AppResult<tokio::task::JoinHandle<()>> {
        protocol::setup_request_sender(args, shutdown_tx, metrics_tx, log_sink)
    }
}

struct RuntimeMetricsAdapter;

impl MetricsPort for RuntimeMetricsAdapter {
    fn setup_metrics_collector(
        &self,
        input: MetricsCollectorInput<'_>,
    ) -> tokio::task::JoinHandle<metrics::MetricsReport> {
        let MetricsCollectorInput {
            args,
            run_start,
            shutdown_tx,
            metrics_rx,
            ui_tx,
            stream_tx,
        } = input;
        metrics::setup_metrics_collector(args, run_start, shutdown_tx, metrics_rx, ui_tx, stream_tx)
    }
}

struct RuntimeOutputAdapter;

#[async_trait]
impl OutputPort for RuntimeOutputAdapter {
    fn stdout_is_terminal(&self) -> bool {
        std::io::stdout().is_terminal()
    }

    fn setup_ui_channel(&self, args: &TesterArgs) -> watch::Sender<UiData> {
        let initial_ui = UiData {
            target_duration: Duration::from_secs(args.target_duration.get()),
            ui_window_ms: args.ui_window_ms.get(),
            no_color: args.no_color,
            ..UiData::default()
        };
        let (ui_tx, _) = watch::channel(initial_ui);
        ui_tx
    }

    async fn run_splash_screen(&self, no_color: bool) -> AppResult<bool> {
        crate::ui::render::run_splash_screen(no_color).await
    }

    fn setup_rss_log_task(
        &self,
        shutdown_tx: &ShutdownSender,
        no_ui: bool,
        interval_ms: Option<&crate::args::PositiveU64>,
    ) -> tokio::task::JoinHandle<()> {
        setup_rss_log_task(shutdown_tx, no_ui, interval_ms)
    }

    fn setup_alloc_profiler_task(
        &self,
        shutdown_tx: &ShutdownSender,
        interval_ms: Option<&crate::args::PositiveU64>,
    ) -> tokio::task::JoinHandle<()> {
        setup_alloc_profiler_task(shutdown_tx, interval_ms)
    }

    fn setup_alloc_profiler_dump_task(
        &self,
        shutdown_tx: &ShutdownSender,
        interval_ms: Option<&crate::args::PositiveU64>,
        dump_path: &str,
    ) -> tokio::task::JoinHandle<()> {
        setup_alloc_profiler_dump_task(shutdown_tx, interval_ms, dump_path)
    }

    async fn setup_log_sinks(
        &self,
        args: &TesterArgs,
        run_start: Instant,
        charts_enabled: bool,
        summary_enabled: bool,
    ) -> AppResult<logs::LogSetup> {
        logs::setup_log_sinks(args, run_start, charts_enabled, summary_enabled).await
    }

    fn setup_render_ui(
        &self,
        shutdown_tx: &ShutdownSender,
        ui_tx: &watch::Sender<UiData>,
    ) -> tokio::task::JoinHandle<()> {
        setup_render_ui(shutdown_tx, ui_tx)
    }

    fn setup_progress_indicator(
        &self,
        args: &TesterArgs,
        run_start: Instant,
        shutdown_tx: &ShutdownSender,
    ) -> tokio::task::JoinHandle<()> {
        progress::setup_progress_indicator(args, run_start, shutdown_tx)
    }

    async fn finalize_run(&self, input: FinalizeRunInput<'_>) -> AppResult<RunOutcome> {
        let FinalizeRunInput {
            args,
            charts_enabled,
            summary_enabled,
            metrics_max,
            runtime_errors,
            report,
            log_handles,
            log_paths,
            #[cfg(feature = "wasm")]
            plugin_host,
        } = input;
        finalize_local_run(FinalizeContext {
            args,
            charts_enabled,
            summary_enabled,
            metrics_max,
            runtime_errors,
            report,
            log_handles,
            log_paths,
            #[cfg(feature = "wasm")]
            plugin_host,
        })
        .await
    }
}
