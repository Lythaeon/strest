use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use super::protocol::WireArgs;
use super::wire::{apply_wire_args, build_wire_args};
use super::{AgentLocalRunPort, AgentRunOutcome, run_agent, run_controller};
use crate::args::{HttpMethod, LoadMode, PositiveU64, PositiveUsize, Protocol, TesterArgs};
use crate::error::{AppError, AppResult};
use crate::metrics::StreamSnapshot;

mod sink_runs;
mod stability;
mod wire_args;

fn positive_u64(value: u64) -> AppResult<PositiveU64> {
    Ok(PositiveU64::try_from(value)?)
}

fn positive_usize(value: usize) -> AppResult<PositiveUsize> {
    Ok(PositiveUsize::try_from(value)?)
}

fn base_args(url: String, tmp_path: String) -> AppResult<TesterArgs> {
    Ok(TesterArgs {
        command: None,
        replay: false,
        replay_start: None,
        replay_end: None,
        replay_step: None,
        replay_snapshot_interval: None,
        replay_snapshot_start: None,
        replay_snapshot_end: None,
        replay_snapshot_out: None,
        replay_snapshot_format: "json".to_owned(),
        method: HttpMethod::Get,
        protocol: Protocol::Http,
        load_mode: LoadMode::Arrival,
        url: Some(url),
        urls_from_file: false,
        rand_regex_url: false,
        max_repeat: positive_usize(4)?,
        dump_urls: None,
        headers: vec![],
        accept_header: None,
        content_type: None,
        no_ua: false,
        authorized: false,
        data: String::new(),
        form: vec![],
        basic_auth: None,
        aws_session: None,
        aws_sigv4: None,
        data_file: None,
        data_lines: None,
        target_duration: positive_u64(1)?,
        wait_ongoing_requests_after_deadline: false,
        requests: None,
        expected_status_code: 200,
        request_timeout: Duration::from_secs(2),
        redirect_limit: 10,
        disable_keepalive: false,
        disable_compression: false,
        pool_max_idle_per_host: None,
        pool_idle_timeout_ms: None,
        http_version: None,
        connect_timeout: Duration::from_secs(5),
        charts_path: "./charts".to_owned(),
        no_charts: true,
        charts_latency_bucket_ms: positive_u64(100)?,
        verbose: false,
        config: None,
        tmp_path,
        load_profile: None,
        controller_listen: None,
        controller_mode: crate::args::ControllerMode::Auto,
        control_listen: None,
        control_auth_token: None,
        agent_join: None,
        auth_token: None,
        agent_id: None,
        agent_weight: positive_u64(1)?,
        min_agents: positive_usize(1)?,
        agent_wait_timeout_ms: None,
        agent_standby: false,
        agent_reconnect_ms: positive_u64(1000)?,
        agent_heartbeat_interval_ms: positive_u64(1000)?,
        agent_heartbeat_timeout_ms: positive_u64(3000)?,
        keep_tmp: false,
        warmup: None,
        output: None,
        output_format: None,
        time_unit: None,
        export_csv: None,
        export_json: None,
        export_jsonl: None,
        db_url: None,
        log_shards: positive_usize(1)?,
        no_ui: true,
        no_splash: true,
        ui_window_ms: positive_u64(10_000)?,
        summary: false,
        show_selections: false,
        tls_min: None,
        tls_max: None,
        cacert: None,
        cert: None,
        key: None,
        insecure: false,
        http2: false,
        http2_parallel: positive_usize(1)?,
        http3: false,
        alpn: vec![],
        proxy_url: None,
        proxy_headers: vec![],
        proxy_http_version: None,
        proxy_http2: false,
        max_tasks: positive_usize(1)?,
        spawn_rate_per_tick: positive_usize(1)?,
        tick_interval: positive_u64(100)?,
        rate_limit: None,
        burst_delay: None,
        burst_rate: positive_usize(1)?,
        latency_correction: false,
        connect_to: vec![],
        host_header: None,
        ipv6_only: false,
        ipv4_only: false,
        no_pre_lookup: false,
        no_color: false,
        ui_fps: 16,
        stats_success_breakdown: false,
        unix_socket: None,
        metrics_range: None,
        metrics_max: positive_usize(1_000)?,
        rss_log_ms: None,
        alloc_profiler_ms: None,
        alloc_profiler_dump_ms: None,
        alloc_profiler_dump_path: "./alloc-prof".to_owned(),
        scenario: None,
        script: None,
        plugin: vec![],
        install_service: false,
        uninstall_service: false,
        service_name: None,
        sinks: None,
        distributed_silent: false,
        distributed_stream_summaries: false,
        distributed_stream_interval_ms: None,
    })
}

fn run_async_test<F>(future: F) -> AppResult<()>
where
    F: std::future::Future<Output = AppResult<()>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::distributed(format!("Failed to build runtime: {}", err)))?;
    runtime.block_on(future)
}

fn allocate_port() -> AppResult<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|err| AppError::distributed(format!("Failed to bind port: {}", err)))?;
    let port = listener
        .local_addr()
        .map_err(|err| AppError::distributed(format!("Failed to read local addr: {}", err)))?
        .port();
    Ok(port)
}

async fn spawn_http_server() -> AppResult<(String, watch::Sender<bool>)> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| AppError::distributed(format!("Failed to bind test server: {}", err)))?;
    let addr = listener
        .local_addr()
        .map_err(|err| AppError::distributed(format!("Failed to read server addr: {}", err)))?;
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    tokio::spawn(async move {
        loop {
            tokio::select! {
                changed = shutdown_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }
                    if *shutdown_rx.borrow() {
                        break;
                    }
                }
                accept = listener.accept() => {
                    let (socket, _) = match accept {
                        Ok(result) => result,
                        Err(_) => break,
                    };
                    tokio::spawn(handle_http(socket));
                }
            }
        }
    });

    Ok((format!("http://{}", addr), shutdown_tx))
}

async fn spawn_http_server_or_skip() -> AppResult<Option<(String, watch::Sender<bool>)>> {
    match spawn_http_server().await {
        Ok(result) => Ok(Some(result)),
        Err(err) if err.to_string().contains("Operation not permitted") => {
            eprintln!("Skipping distributed test: {}", err);
            Ok(None)
        }
        Err(err) => Err(err),
    }
}

async fn handle_http(mut socket: TcpStream) {
    let mut buffer = [0u8; 1024];
    if socket.read(&mut buffer).await.is_err() {
        return;
    }
    if socket
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
        .await
        .is_err()
    {
        return;
    }
    let _shutdown_result = socket.shutdown().await;
}

async fn run_distributed(controller_args: TesterArgs, agent_args: TesterArgs) -> AppResult<()> {
    let controller_handle =
        tokio::spawn(async move { run_controller(&controller_args, None).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    let local_port = TestAgentLocalRunPort;
    let agent_result = run_agent(agent_args, &local_port).await;
    let controller_result = controller_handle
        .await
        .map_err(|err| AppError::distributed(format!("Controller task join failed: {}", err)))?;
    agent_result?;
    controller_result?;
    Ok(())
}

struct TestAgentLocalRunPort;

#[async_trait::async_trait]
impl AgentLocalRunPort for TestAgentLocalRunPort {
    async fn run_local(
        &self,
        args: TesterArgs,
        stream_tx: Option<tokio::sync::mpsc::UnboundedSender<StreamSnapshot>>,
        external_shutdown: Option<watch::Receiver<bool>>,
    ) -> AppResult<AgentRunOutcome> {
        let outcome = crate::app::run_local(args, stream_tx, external_shutdown).await?;
        Ok(AgentRunOutcome {
            summary: outcome.summary,
            histogram: outcome.histogram,
            success_histogram: outcome.success_histogram,
            latency_sum_ms: outcome.latency_sum_ms,
            success_latency_sum_ms: outcome.success_latency_sum_ms,
            runtime_errors: outcome.runtime_errors,
        })
    }
}
