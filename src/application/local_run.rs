use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;
use tracing::{info, warn};

use crate::app::logs;
use crate::args::TesterArgs;
use crate::domain::run::ProtocolKind;
use crate::error::{AppError, AppResult, ValidationError};
use crate::metrics::{self, Metrics};
use crate::shutdown::{ShutdownReceiver, ShutdownSender};
use crate::ui::model::UiData;

#[cfg(not(feature = "wasm"))]
use crate::error::ScriptError;
#[cfg(feature = "wasm")]
use crate::wasm_plugins::WasmPluginHost;

#[derive(Debug)]
pub(crate) struct RunOutcome {
    pub summary: metrics::MetricsSummary,
    pub histogram: metrics::LatencyHistogram,
    pub success_histogram: metrics::LatencyHistogram,
    pub latency_sum_ms: u128,
    pub success_latency_sum_ms: u128,
    pub runtime_errors: Vec<String>,
}

pub(crate) struct LocalRunExecutionCommand {
    protocol: ProtocolKind,
    args: TesterArgs,
    stream_tx: Option<mpsc::UnboundedSender<metrics::StreamSnapshot>>,
    external_shutdown: Option<watch::Receiver<bool>>,
}

impl LocalRunExecutionCommand {
    #[must_use]
    pub(crate) const fn new(
        protocol: ProtocolKind,
        args: TesterArgs,
        stream_tx: Option<mpsc::UnboundedSender<metrics::StreamSnapshot>>,
        external_shutdown: Option<watch::Receiver<bool>>,
    ) -> Self {
        Self {
            protocol,
            args,
            stream_tx,
            external_shutdown,
        }
    }

    #[must_use]
    pub(crate) fn into_parts(
        self,
    ) -> (
        ProtocolKind,
        TesterArgs,
        Option<mpsc::UnboundedSender<metrics::StreamSnapshot>>,
        Option<watch::Receiver<bool>>,
    ) {
        (
            self.protocol,
            self.args,
            self.stream_tx,
            self.external_shutdown,
        )
    }
}

pub(crate) trait ShutdownPort {
    fn shutdown_channel(&self) -> (ShutdownSender, ShutdownReceiver);
    fn bridge_external_shutdown(
        &self,
        shutdown_tx: &ShutdownSender,
        external_shutdown: watch::Receiver<bool>,
    ) -> tokio::task::JoinHandle<()>;
    fn setup_keyboard_shutdown_handler(
        &self,
        shutdown_tx: &ShutdownSender,
    ) -> tokio::task::JoinHandle<()>;
    fn setup_signal_shutdown_handler(
        &self,
        shutdown_tx: &ShutdownSender,
    ) -> tokio::task::JoinHandle<()>;
}

pub(crate) trait TrafficPort {
    fn setup_request_sender(
        &self,
        protocol: ProtocolKind,
        args: &TesterArgs,
        shutdown_tx: &ShutdownSender,
        metrics_tx: &mpsc::Sender<Metrics>,
        log_sink: Option<&Arc<metrics::LogSink>>,
    ) -> AppResult<tokio::task::JoinHandle<()>>;
}

pub(crate) trait MetricsPort {
    fn setup_metrics_collector(
        &self,
        input: MetricsCollectorInput<'_>,
    ) -> tokio::task::JoinHandle<metrics::MetricsReport>;
}

pub(crate) struct FinalizeRunInput<'args> {
    pub args: &'args TesterArgs,
    pub charts_enabled: bool,
    pub summary_enabled: bool,
    pub metrics_max: usize,
    pub runtime_errors: Vec<String>,
    pub report: metrics::MetricsReport,
    pub log_handles: Vec<tokio::task::JoinHandle<AppResult<metrics::LogResult>>>,
    pub log_paths: Vec<PathBuf>,
    #[cfg(feature = "wasm")]
    pub plugin_host: Option<&'args mut WasmPluginHost>,
}

pub(crate) struct MetricsCollectorInput<'args> {
    pub args: &'args TesterArgs,
    pub run_start: Instant,
    pub shutdown_tx: &'args ShutdownSender,
    pub metrics_rx: mpsc::Receiver<Metrics>,
    pub ui_tx: &'args watch::Sender<UiData>,
    pub stream_tx: Option<mpsc::UnboundedSender<metrics::StreamSnapshot>>,
}

#[async_trait]
pub(crate) trait OutputPort {
    fn stdout_is_terminal(&self) -> bool;
    fn setup_ui_channel(&self, args: &TesterArgs) -> watch::Sender<UiData>;
    async fn run_splash_screen(&self, no_color: bool) -> AppResult<bool>;
    fn setup_rss_log_task(
        &self,
        shutdown_tx: &ShutdownSender,
        no_ui: bool,
        interval_ms: Option<&crate::args::PositiveU64>,
    ) -> tokio::task::JoinHandle<()>;
    fn setup_alloc_profiler_task(
        &self,
        shutdown_tx: &ShutdownSender,
        interval_ms: Option<&crate::args::PositiveU64>,
    ) -> tokio::task::JoinHandle<()>;
    fn setup_alloc_profiler_dump_task(
        &self,
        shutdown_tx: &ShutdownSender,
        interval_ms: Option<&crate::args::PositiveU64>,
        dump_path: &str,
    ) -> tokio::task::JoinHandle<()>;
    async fn setup_log_sinks(
        &self,
        args: &TesterArgs,
        run_start: Instant,
        charts_enabled: bool,
        summary_enabled: bool,
    ) -> AppResult<logs::LogSetup>;
    fn setup_render_ui(
        &self,
        shutdown_tx: &ShutdownSender,
        ui_tx: &watch::Sender<UiData>,
    ) -> tokio::task::JoinHandle<()>;
    fn setup_progress_indicator(
        &self,
        args: &TesterArgs,
        run_start: Instant,
        shutdown_tx: &ShutdownSender,
    ) -> tokio::task::JoinHandle<()>;
    async fn finalize_run(&self, input: FinalizeRunInput<'_>) -> AppResult<RunOutcome>;
}

/// Executes the local run use-case against injected ports.
///
/// # Errors
///
/// Returns an error when plugin hooks fail, transport setup fails, or
/// downstream output finalization fails.
pub(crate) async fn execute<TShutdown, TTraffic, TMetrics, TOutput>(
    command: LocalRunExecutionCommand,
    shutdown_port: &TShutdown,
    traffic_port: &TTraffic,
    metrics_port: &TMetrics,
    output_port: &TOutput,
) -> AppResult<RunOutcome>
where
    TShutdown: ShutdownPort,
    TTraffic: TrafficPort,
    TMetrics: MetricsPort,
    TOutput: OutputPort + Sync,
{
    let (protocol, args, stream_tx, external_shutdown) = command.into_parts();

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

    let (shutdown_tx, _) = shutdown_port.shutdown_channel();
    if let Some(external_shutdown) = external_shutdown {
        let _bridge_handle =
            shutdown_port.bridge_external_shutdown(&shutdown_tx, external_shutdown);
    }

    let ui_tx = output_port.setup_ui_channel(&args);
    let (metrics_tx, metrics_rx) = mpsc::channel::<Metrics>(10_000);

    let ui_enabled = !args.no_ui && output_port.stdout_is_terminal();
    if !ui_enabled && !args.no_ui {
        info!("UI disabled because stdout is not a TTY.");
    }
    let charts_enabled = !args.no_charts;
    let summary_enabled = args.summary || args.show_selections;

    let rss_handle =
        output_port.setup_rss_log_task(&shutdown_tx, args.no_ui, args.rss_log_ms.as_ref());
    let alloc_handle =
        output_port.setup_alloc_profiler_task(&shutdown_tx, args.alloc_profiler_ms.as_ref());
    let alloc_dump_handle = output_port.setup_alloc_profiler_dump_task(
        &shutdown_tx,
        args.alloc_profiler_dump_ms.as_ref(),
        &args.alloc_profiler_dump_path,
    );

    if ui_enabled && !args.no_splash {
        match output_port.run_splash_screen(args.no_color).await {
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
    } = output_port
        .setup_log_sinks(&args, run_start, charts_enabled, summary_enabled)
        .await?;

    let request_sender_handle = match traffic_port.setup_request_sender(
        protocol,
        &args,
        &shutdown_tx,
        &metrics_tx,
        log_sink.as_ref(),
    ) {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!("Failed to setup request sender: {}", err);
            return Err(err);
        }
    };
    drop(metrics_tx);

    let keyboard_shutdown_handle = if ui_enabled {
        shutdown_port.setup_keyboard_shutdown_handler(&shutdown_tx)
    } else {
        tokio::spawn(async {})
    };
    let signal_shutdown_handle = shutdown_port.setup_signal_shutdown_handler(&shutdown_tx);
    let render_ui_handle = if ui_enabled {
        output_port.setup_render_ui(&shutdown_tx, &ui_tx)
    } else {
        tokio::spawn(async {})
    };
    let progress_handle = if args.no_ui && !args.verbose {
        output_port.setup_progress_indicator(&args, run_start, &shutdown_tx)
    } else {
        tokio::spawn(async {})
    };
    let metrics_handle = metrics_port.setup_metrics_collector(MetricsCollectorInput {
        args: &args,
        run_start,
        shutdown_tx: &shutdown_tx,
        metrics_rx,
        ui_tx: &ui_tx,
        stream_tx,
    });
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

    output_port
        .finalize_run(FinalizeRunInput {
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

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::Duration;

    use super::*;
    use crate::app::logs::LogSetup;

    struct FakeShutdownPort;

    impl ShutdownPort for FakeShutdownPort {
        fn shutdown_channel(&self) -> (ShutdownSender, ShutdownReceiver) {
            crate::system::shutdown_handlers::shutdown_channel()
        }

        fn bridge_external_shutdown(
            &self,
            _shutdown_tx: &ShutdownSender,
            _external_shutdown: watch::Receiver<bool>,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }

        fn setup_keyboard_shutdown_handler(
            &self,
            _shutdown_tx: &ShutdownSender,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }

        fn setup_signal_shutdown_handler(
            &self,
            _shutdown_tx: &ShutdownSender,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }
    }

    struct FakeTrafficPort;

    impl TrafficPort for FakeTrafficPort {
        fn setup_request_sender(
            &self,
            _protocol: ProtocolKind,
            _args: &TesterArgs,
            _shutdown_tx: &ShutdownSender,
            _metrics_tx: &mpsc::Sender<Metrics>,
            _log_sink: Option<&Arc<metrics::LogSink>>,
        ) -> AppResult<tokio::task::JoinHandle<()>> {
            Ok(tokio::spawn(async {}))
        }
    }

    struct FakeMetricsPort;

    impl MetricsPort for FakeMetricsPort {
        fn setup_metrics_collector(
            &self,
            _input: MetricsCollectorInput<'_>,
        ) -> tokio::task::JoinHandle<metrics::MetricsReport> {
            tokio::spawn(async {
                metrics::MetricsReport {
                    summary: logs::empty_summary(),
                }
            })
        }
    }

    struct FakeOutputPort {
        stdout_terminal: bool,
        splash_cancelled: bool,
        finalize_called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl OutputPort for FakeOutputPort {
        fn stdout_is_terminal(&self) -> bool {
            self.stdout_terminal
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

        async fn run_splash_screen(&self, _no_color: bool) -> AppResult<bool> {
            if self.splash_cancelled {
                Ok(false)
            } else {
                Ok(true)
            }
        }

        fn setup_rss_log_task(
            &self,
            _shutdown_tx: &ShutdownSender,
            _no_ui: bool,
            _interval_ms: Option<&crate::args::PositiveU64>,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }

        fn setup_alloc_profiler_task(
            &self,
            _shutdown_tx: &ShutdownSender,
            _interval_ms: Option<&crate::args::PositiveU64>,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }

        fn setup_alloc_profiler_dump_task(
            &self,
            _shutdown_tx: &ShutdownSender,
            _interval_ms: Option<&crate::args::PositiveU64>,
            _dump_path: &str,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }

        async fn setup_log_sinks(
            &self,
            _args: &TesterArgs,
            _run_start: Instant,
            _charts_enabled: bool,
            _summary_enabled: bool,
        ) -> AppResult<LogSetup> {
            Ok(LogSetup {
                log_sink: None,
                handles: Vec::new(),
                paths: Vec::new(),
            })
        }

        fn setup_render_ui(
            &self,
            _shutdown_tx: &ShutdownSender,
            _ui_tx: &watch::Sender<UiData>,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }

        fn setup_progress_indicator(
            &self,
            _args: &TesterArgs,
            _run_start: Instant,
            _shutdown_tx: &ShutdownSender,
        ) -> tokio::task::JoinHandle<()> {
            tokio::spawn(async {})
        }

        async fn finalize_run(&self, _input: FinalizeRunInput<'_>) -> AppResult<RunOutcome> {
            self.finalize_called.store(true, Ordering::SeqCst);
            Ok(RunOutcome {
                summary: logs::empty_summary(),
                histogram: metrics::LatencyHistogram::new()?,
                success_histogram: metrics::LatencyHistogram::new()?,
                latency_sum_ms: 0,
                success_latency_sum_ms: 0,
                runtime_errors: Vec::new(),
            })
        }
    }

    fn parse_args() -> AppResult<TesterArgs> {
        crate::args::parse_test_args(["strest", "--url", "http://localhost"])
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_runs_and_finalizes_with_ports() -> AppResult<()> {
        let finalize_called = Arc::new(AtomicBool::new(false));
        let output_port = FakeOutputPort {
            stdout_terminal: false,
            splash_cancelled: false,
            finalize_called: finalize_called.clone(),
        };
        let args = parse_args()?;
        let command = LocalRunExecutionCommand::new(args.protocol.to_domain(), args, None, None);

        let outcome = execute(
            command,
            &FakeShutdownPort,
            &FakeTrafficPort,
            &FakeMetricsPort,
            &output_port,
        )
        .await?;

        if !outcome.runtime_errors.is_empty() {
            return Err(AppError::validation("expected no runtime errors"));
        }
        if !finalize_called.load(Ordering::SeqCst) {
            return Err(AppError::validation("expected finalize to be called"));
        }
        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_returns_cancelled_when_splash_is_cancelled() -> AppResult<()> {
        let finalize_called = Arc::new(AtomicBool::new(false));
        let output_port = FakeOutputPort {
            stdout_terminal: true,
            splash_cancelled: true,
            finalize_called: finalize_called.clone(),
        };
        let args = parse_args()?;
        let command = LocalRunExecutionCommand::new(args.protocol.to_domain(), args, None, None);

        let result = execute(
            command,
            &FakeShutdownPort,
            &FakeTrafficPort,
            &FakeMetricsPort,
            &output_port,
        )
        .await;
        let err = match result {
            Ok(_) => {
                return Err(AppError::validation(
                    "expected splash cancellation to stop local run",
                ));
            }
            Err(err) => err,
        };

        if !matches!(err, AppError::Validation(ValidationError::RunCancelled)) {
            return Err(AppError::validation("expected run cancellation error"));
        }
        if finalize_called.load(Ordering::SeqCst) {
            return Err(AppError::validation(
                "did not expect finalize when splash is cancelled",
            ));
        }
        Ok(())
    }
}
