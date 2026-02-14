mod finalize;

use std::io::IsTerminal;
#[cfg(feature = "wasm")]
use std::sync::Mutex;
use std::time::Duration;

use crate::{
    app::{logs, progress},
    application::local_run::{
        self, FinalizeRunInput, LocalRunExecutionCommand, LocalRunSettings, MetricsCollectorInput,
        MetricsPort, OutputPort, ShutdownPort, TrafficPort,
    },
    args::TesterArgs,
    domain::run::ProtocolKind,
    error::{AppError, AppResult},
    metrics::{self, Metrics},
    protocol,
    shutdown::{ShutdownReceiver, ShutdownSender},
    system::shutdown_handlers,
    ui::{model::UiData, render::setup_render_ui},
};
use async_trait::async_trait;
use tokio::sync::{mpsc, watch};
use tokio::time::Instant;

#[cfg(not(feature = "wasm"))]
use crate::error::ScriptError;
#[cfg(feature = "wasm")]
use crate::wasm_plugins::WasmPluginHost;

use super::alloc::{setup_alloc_profiler_dump_task, setup_alloc_profiler_task};
use super::rss::setup_rss_log_task;
use finalize::{FinalizeContext, finalize_run as finalize_local_run};

pub(crate) type RunOutcome = local_run::RunOutcome;

pub(crate) async fn run_local(
    args: TesterArgs,
    stream_tx: Option<mpsc::UnboundedSender<metrics::StreamSnapshot>>,
    external_shutdown: Option<watch::Receiver<bool>>,
) -> AppResult<RunOutcome> {
    let protocol = args.protocol.to_domain();
    let settings = local_run_settings(&args);
    let command =
        LocalRunExecutionCommand::new(protocol, settings, args, stream_tx, external_shutdown);
    let shutdown_adapter = RuntimeShutdownAdapter;
    let traffic_adapter = RuntimeTrafficAdapter;
    let metrics_adapter = RuntimeMetricsAdapter;
    let output_adapter = RuntimeOutputAdapter::new();

    local_run::execute(
        command,
        &shutdown_adapter,
        &traffic_adapter,
        &metrics_adapter,
        &output_adapter,
    )
    .await
}

fn local_run_settings(args: &TesterArgs) -> LocalRunSettings {
    LocalRunSettings {
        no_color: args.no_color,
        no_ui: args.no_ui,
        no_splash: args.no_splash,
        no_charts: args.no_charts,
        summary: args.summary,
        show_selections: args.show_selections,
        verbose: args.verbose,
        target_duration_secs: args.target_duration.get(),
        ui_window_ms: args.ui_window_ms.get(),
        rss_log_ms: args.rss_log_ms.as_ref().map(|value| value.get()),
        alloc_profiler_ms: args.alloc_profiler_ms.as_ref().map(|value| value.get()),
        alloc_profiler_dump_ms: args
            .alloc_profiler_dump_ms
            .as_ref()
            .map(|value| value.get()),
        alloc_profiler_dump_path: args.alloc_profiler_dump_path.clone(),
        metrics_max: args.metrics_max.get(),
    }
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

impl TrafficPort<TesterArgs> for RuntimeTrafficAdapter {
    fn setup_request_sender(
        &self,
        protocol: ProtocolKind,
        adapter_args: &TesterArgs,
        shutdown_tx: &ShutdownSender,
        metrics_tx: &mpsc::Sender<Metrics>,
        log_sink: Option<&std::sync::Arc<metrics::LogSink>>,
    ) -> AppResult<tokio::task::JoinHandle<()>> {
        protocol::setup_request_sender(protocol, adapter_args, shutdown_tx, metrics_tx, log_sink)
    }
}

struct RuntimeMetricsAdapter;

impl MetricsPort<TesterArgs> for RuntimeMetricsAdapter {
    fn setup_metrics_collector(
        &self,
        input: MetricsCollectorInput<'_, TesterArgs>,
    ) -> tokio::task::JoinHandle<metrics::MetricsReport> {
        let MetricsCollectorInput {
            adapter_args,
            run_start,
            shutdown_tx,
            metrics_rx,
            ui_tx,
            stream_tx,
            ..
        } = input;
        metrics::setup_metrics_collector(
            adapter_args,
            run_start,
            shutdown_tx,
            metrics_rx,
            ui_tx,
            stream_tx,
        )
    }
}

struct RuntimeOutputAdapter {
    #[cfg(feature = "wasm")]
    plugin_host: Mutex<Option<WasmPluginHost>>,
}

impl RuntimeOutputAdapter {
    const fn new() -> Self {
        Self {
            #[cfg(feature = "wasm")]
            plugin_host: Mutex::new(None),
        }
    }
}

#[async_trait]
impl OutputPort<TesterArgs> for RuntimeOutputAdapter {
    fn prepare_run(&self, adapter_args: &TesterArgs) -> AppResult<()> {
        #[cfg(feature = "wasm")]
        {
            let mut plugin_host = WasmPluginHost::from_paths(&adapter_args.plugin)?;
            if let Some(host) = plugin_host.as_mut() {
                host.on_run_start(adapter_args)?;
            }
            let mut guard = self.plugin_host.lock().map_err(|err| {
                AppError::from(std::io::Error::other(format!(
                    "WASM plugin state lock poisoned: {}",
                    err
                )))
            })?;
            *guard = plugin_host;
        }

        #[cfg(not(feature = "wasm"))]
        if !adapter_args.plugin.is_empty() {
            return Err(AppError::script(ScriptError::WasmFeatureDisabled));
        }

        Ok(())
    }

    fn stdout_is_terminal(&self) -> bool {
        std::io::stdout().is_terminal()
    }

    fn setup_ui_channel(&self, settings: &LocalRunSettings) -> watch::Sender<UiData> {
        let initial_ui = UiData {
            target_duration: Duration::from_secs(settings.target_duration_secs),
            ui_window_ms: settings.ui_window_ms,
            no_color: settings.no_color,
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
        interval_ms: Option<u64>,
    ) -> tokio::task::JoinHandle<()> {
        let interval = interval_ms.and_then(|value| crate::args::PositiveU64::try_from(value).ok());
        setup_rss_log_task(shutdown_tx, no_ui, interval.as_ref())
    }

    fn setup_alloc_profiler_task(
        &self,
        shutdown_tx: &ShutdownSender,
        interval_ms: Option<u64>,
    ) -> tokio::task::JoinHandle<()> {
        let interval = interval_ms.and_then(|value| crate::args::PositiveU64::try_from(value).ok());
        setup_alloc_profiler_task(shutdown_tx, interval.as_ref())
    }

    fn setup_alloc_profiler_dump_task(
        &self,
        shutdown_tx: &ShutdownSender,
        interval_ms: Option<u64>,
        dump_path: &str,
    ) -> tokio::task::JoinHandle<()> {
        let interval = interval_ms.and_then(|value| crate::args::PositiveU64::try_from(value).ok());
        setup_alloc_profiler_dump_task(shutdown_tx, interval.as_ref(), dump_path)
    }

    async fn setup_log_sinks(
        &self,
        adapter_args: &TesterArgs,
        run_start: Instant,
        charts_enabled: bool,
        summary_enabled: bool,
    ) -> AppResult<logs::LogSetup> {
        logs::setup_log_sinks(adapter_args, run_start, charts_enabled, summary_enabled).await
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
        adapter_args: &TesterArgs,
        run_start: Instant,
        shutdown_tx: &ShutdownSender,
    ) -> tokio::task::JoinHandle<()> {
        progress::setup_progress_indicator(adapter_args, run_start, shutdown_tx)
    }

    async fn finalize_run(&self, input: FinalizeRunInput<'_, TesterArgs>) -> AppResult<RunOutcome> {
        let FinalizeRunInput {
            adapter_args,
            charts_enabled,
            summary_enabled,
            metrics_max,
            runtime_errors,
            report,
            log_handles,
            log_paths,
            ..
        } = input;

        #[cfg(feature = "wasm")]
        let mut plugin_host = {
            let mut guard = self.plugin_host.lock().map_err(|err| {
                AppError::from(std::io::Error::other(format!(
                    "WASM plugin state lock poisoned: {}",
                    err
                )))
            })?;
            guard.take()
        };

        let outcome = finalize_local_run(FinalizeContext {
            args: adapter_args,
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
        .await;

        #[cfg(feature = "wasm")]
        {
            let mut guard = self.plugin_host.lock().map_err(|err| {
                AppError::from(std::io::Error::other(format!(
                    "WASM plugin state lock poisoned: {}",
                    err
                )))
            })?;
            *guard = plugin_host;
        }

        outcome
    }
}
