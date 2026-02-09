use super::protocol::WireArgs;
use super::wire::{apply_wire_args, build_wire_args};
use super::{run_agent, run_controller};
use crate::args::{HttpMethod, PositiveU64, PositiveUsize, TesterArgs};
use crate::sinks::config::{PrometheusSinkConfig, SinksConfig};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

fn positive_u64(value: u64) -> Result<PositiveU64, String> {
    PositiveU64::try_from(value)
}

fn positive_usize(value: usize) -> Result<PositiveUsize, String> {
    PositiveUsize::try_from(value)
}

fn base_args(url: String, tmp_path: String) -> Result<TesterArgs, String> {
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
        url: Some(url),
        headers: vec![],
        accept_header: None,
        content_type: None,
        no_ua: false,
        authorized: false,
        data: String::new(),
        basic_auth: None,
        aws_session: None,
        aws_sigv4: None,
        data_file: None,
        data_lines: None,
        target_duration: positive_u64(1)?,
        requests: None,
        expected_status_code: 200,
        request_timeout: Duration::from_secs(2),
        redirect_limit: 10,
        disable_keepalive: false,
        disable_compression: false,
        http_version: None,
        connect_timeout: Duration::from_secs(5),
        charts_path: "./charts".to_owned(),
        no_charts: true,
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
        export_csv: None,
        export_json: None,
        export_jsonl: None,
        db_url: None,
        log_shards: positive_usize(1)?,
        no_ui: true,
        ui_window_ms: positive_u64(10_000)?,
        summary: false,
        tls_min: None,
        tls_max: None,
        cacert: None,
        cert: None,
        key: None,
        insecure: false,
        http2: false,
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
        scenario: None,
        script: None,
        install_service: false,
        uninstall_service: false,
        service_name: None,
        sinks: None,
        distributed_silent: false,
        distributed_stream_summaries: false,
        distributed_stream_interval_ms: None,
    })
}

fn run_async_test<F>(future: F) -> Result<(), String>
where
    F: std::future::Future<Output = Result<(), String>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("Failed to build runtime: {}", err))?;
    runtime.block_on(future)
}

fn allocate_port() -> Result<u16, String> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("Failed to bind port: {}", err))?;
    let port = listener
        .local_addr()
        .map_err(|err| format!("Failed to read local addr: {}", err))?
        .port();
    Ok(port)
}

async fn spawn_http_server() -> Result<(String, watch::Sender<bool>), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|err| format!("Failed to bind test server: {}", err))?;
    let addr = listener
        .local_addr()
        .map_err(|err| format!("Failed to read server addr: {}", err))?;
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

async fn spawn_http_server_or_skip() -> Result<Option<(String, watch::Sender<bool>)>, String> {
    match spawn_http_server().await {
        Ok(result) => Ok(Some(result)),
        Err(err) if err.contains("Operation not permitted") => {
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

async fn run_distributed(
    controller_args: TesterArgs,
    agent_args: TesterArgs,
) -> Result<(), String> {
    let controller_handle =
        tokio::spawn(async move { run_controller(&controller_args, None).await });
    tokio::time::sleep(Duration::from_millis(200)).await;
    let agent_result = run_agent(agent_args).await;
    let controller_result = controller_handle
        .await
        .map_err(|err| format!("Controller task join failed: {}", err))?;
    agent_result?;
    controller_result?;
    Ok(())
}

#[test]
fn wire_args_roundtrip_preserves_stream_settings() -> Result<(), String> {
    let tmp_path = "./tmp".to_owned();
    let mut args = base_args("http://localhost".to_owned(), tmp_path.clone())?;
    args.distributed_stream_summaries = true;
    args.distributed_stream_interval_ms = Some(positive_u64(150)?);

    let wire = build_wire_args(&args);
    let mut applied = base_args("http://localhost".to_owned(), tmp_path)?;
    apply_wire_args(&mut applied, wire)?;

    if !applied.distributed_stream_summaries {
        return Err("Expected stream summaries to be true".to_owned());
    }
    let interval = match applied.distributed_stream_interval_ms {
        Some(value) => value.get(),
        None => return Err("Expected stream interval to be set".to_owned()),
    };
    if interval != 150 {
        return Err(format!("Unexpected stream interval: {}", interval));
    }
    Ok(())
}

#[test]
fn wire_args_deserialize_missing_stream_interval() -> Result<(), String> {
    let args = base_args("http://localhost".to_owned(), "./tmp".to_owned())?;
    let wire = build_wire_args(&args);
    let mut value =
        serde_json::to_value(&wire).map_err(|err| format!("Serialize failed: {}", err))?;
    match value.as_object_mut() {
        Some(map) => {
            map.remove("stream_interval_ms");
        }
        None => return Err("Expected wire args to serialize to object".to_owned()),
    }
    let decoded: WireArgs =
        serde_json::from_value(value).map_err(|err| format!("Deserialize failed: {}", err))?;
    if decoded.stream_interval_ms.is_some() {
        return Err("Expected stream_interval_ms to default to None".to_owned());
    }
    Ok(())
}

#[test]
fn tcp_streaming_controller_writes_sink() -> Result<(), String> {
    run_async_test(async {
        let Some((url, shutdown_tx)) = spawn_http_server_or_skip().await? else {
            return Ok(());
        };
        let controller_port = allocate_port()?;
        let controller_addr = format!("127.0.0.1:{}", controller_port);
        let tmp_dir =
            tempfile::tempdir().map_err(|err| format!("Failed to create temp dir: {}", err))?;
        let tmp_path = tmp_dir
            .path()
            .to_str()
            .ok_or_else(|| "Failed to convert tmp path".to_owned())?
            .to_owned();
        let sink_path = tmp_dir.path().join("controller.prom");
        let sink_path_str = sink_path
            .to_str()
            .ok_or_else(|| "Failed to convert sink path".to_owned())?
            .to_owned();

        let mut controller_args = base_args(url.clone(), tmp_path.clone())?;
        controller_args.controller_listen = Some(controller_addr.clone());
        controller_args.sinks = Some(SinksConfig {
            update_interval_ms: Some(200),
            prometheus: Some(PrometheusSinkConfig {
                path: sink_path_str.clone(),
            }),
            otel: None,
            influx: None,
        });
        controller_args.distributed_stream_summaries = true;
        controller_args.distributed_stream_interval_ms = Some(positive_u64(200)?);

        let mut agent_args = base_args(url, tmp_path)?;
        agent_args.agent_join = Some(controller_addr);

        let run_result = tokio::time::timeout(
            Duration::from_secs(12),
            run_distributed(controller_args, agent_args),
        )
        .await
        .map_err(|err| format!("Timed out waiting for distributed run: {}", err))?;
        run_result?;

        shutdown_tx
            .send(true)
            .map_err(|err| format!("Failed to shutdown server: {}", err))?;

        let metadata = tokio::fs::metadata(&sink_path_str)
            .await
            .map_err(|err| format!("Missing controller sink: {}", err))?;
        if metadata.len() == 0 {
            return Err("Expected controller sink to be non-empty".to_owned());
        }
        Ok(())
    })
}

#[test]
fn tcp_non_streaming_writes_agent_and_controller_sinks() -> Result<(), String> {
    run_async_test(async {
        let Some((url, shutdown_tx)) = spawn_http_server_or_skip().await? else {
            return Ok(());
        };
        let controller_port = allocate_port()?;
        let controller_addr = format!("127.0.0.1:{}", controller_port);
        let tmp_dir =
            tempfile::tempdir().map_err(|err| format!("Failed to create temp dir: {}", err))?;
        let tmp_path = tmp_dir
            .path()
            .to_str()
            .ok_or_else(|| "Failed to convert tmp path".to_owned())?
            .to_owned();
        let controller_sink = tmp_dir.path().join("controller.prom");
        let controller_sink_str = controller_sink
            .to_str()
            .ok_or_else(|| "Failed to convert controller sink path".to_owned())?
            .to_owned();
        let agent_sink = tmp_dir.path().join("agent.prom");
        let agent_sink_str = agent_sink
            .to_str()
            .ok_or_else(|| "Failed to convert agent sink path".to_owned())?
            .to_owned();

        let mut controller_args = base_args(url.clone(), tmp_path.clone())?;
        controller_args.controller_listen = Some(controller_addr.clone());
        controller_args.sinks = Some(SinksConfig {
            update_interval_ms: None,
            prometheus: Some(PrometheusSinkConfig {
                path: controller_sink_str.clone(),
            }),
            otel: None,
            influx: None,
        });

        let mut agent_args = base_args(url, tmp_path)?;
        agent_args.agent_join = Some(controller_addr);
        agent_args.sinks = Some(SinksConfig {
            update_interval_ms: None,
            prometheus: Some(PrometheusSinkConfig {
                path: agent_sink_str.clone(),
            }),
            otel: None,
            influx: None,
        });

        let run_result = tokio::time::timeout(
            Duration::from_secs(12),
            run_distributed(controller_args, agent_args),
        )
        .await
        .map_err(|err| format!("Timed out waiting for distributed run: {}", err))?;
        run_result?;

        shutdown_tx
            .send(true)
            .map_err(|err| format!("Failed to shutdown server: {}", err))?;

        let controller_meta = tokio::fs::metadata(&controller_sink_str)
            .await
            .map_err(|err| format!("Missing controller sink: {}", err))?;
        if controller_meta.len() == 0 {
            return Err("Expected controller sink to be non-empty".to_owned());
        }
        let agent_meta = tokio::fs::metadata(&agent_sink_str)
            .await
            .map_err(|err| format!("Missing agent sink: {}", err))?;
        if agent_meta.len() == 0 {
            return Err("Expected agent sink to be non-empty".to_owned());
        }
        Ok(())
    })
}
