use super::*;
use crate::args::{HttpMethod, PositiveU64, PositiveUsize, TesterArgs};
use crate::ui::model::UiData;
use std::future::Future;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

fn positive_u64(value: u64) -> Result<PositiveU64, String> {
    PositiveU64::try_from(value)
}

fn positive_usize(value: usize) -> Result<PositiveUsize, String> {
    PositiveUsize::try_from(value)
}

fn base_args() -> Result<TesterArgs, String> {
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
        url: Some("http://localhost".to_owned()),
        headers: vec![],
        accept_header: None,
        content_type: None,
        no_ua: false,
        authorized: false,
        data: String::new(),
        data_file: None,
        data_lines: None,
        target_duration: positive_u64(1)?,
        requests: None,
        expected_status_code: 200,
        request_timeout: Duration::from_secs(10),
        redirect_limit: 10,
        disable_keepalive: false,
        disable_compression: false,
        http_version: None,
        connect_timeout: Duration::from_secs(5),
        charts_path: "./charts".to_owned(),
        no_charts: false,
        verbose: false,
        config: None,
        tmp_path: "./tmp".to_owned(),
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
        export_csv: None,
        export_json: None,
        export_jsonl: None,
        log_shards: positive_usize(1)?,
        no_ui: true,
        ui_window_ms: positive_u64(10_000)?,
        summary: false,
        tls_min: None,
        tls_max: None,
        http2: false,
        http3: false,
        alpn: vec![],
        proxy_url: None,
        proxy_headers: vec![],
        proxy_http_version: None,
        proxy_http2: false,
        max_tasks: positive_usize(1)?,
        spawn_rate_per_tick: positive_usize(1)?,
        tick_interval: positive_u64(1)?,
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
        metrics_max: positive_usize(1_000_000)?,
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
    F: Future<Output = Result<(), String>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("Failed to build runtime: {}", err))?;
    runtime.block_on(future)
}

#[test]
fn shuts_down_on_timer_without_metrics() -> Result<(), String> {
    run_async_test(async {
        let args = base_args()?;
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<u16>(1);
        let (_ui_tx, _ui_rx) = watch::channel(UiData::default());
        let (metrics_tx, metrics_rx) = tokio::sync::mpsc::channel::<Metrics>(10);

        let handle = setup_metrics_collector(
            &args,
            tokio::time::Instant::now(),
            &shutdown_tx,
            metrics_rx,
            &_ui_tx,
            None,
        );
        drop(metrics_tx);

        tokio::time::sleep(Duration::from_millis(1200)).await;

        let report = tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .map_err(|err| format!("Timed out waiting for collector to finish: {}", err))?
            .map_err(|err| format!("Collector join error: {}", err))?;

        if report.summary.total_requests == 0 {
            Ok(())
        } else {
            Err(format!(
                "Expected no metrics, got {}",
                report.summary.total_requests
            ))
        }
    })
}

#[test]
fn read_metrics_log_respects_range() -> Result<(), String> {
    run_async_test(async {
        let dir = tempfile::tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
        let log_path = dir.path().join("metrics.log");
        let mut file = tokio::fs::File::create(&log_path)
            .await
            .map_err(|err| format!("Failed to create log: {}", err))?;
        file.write_all(b"500,5,200\n1500,9,200\n")
            .await
            .map_err(|err| format!("Failed to write log: {}", err))?;
        file.flush()
            .await
            .map_err(|err| format!("Failed to flush log: {}", err))?;

        let range = Some(MetricsRange(0..=0));
        let result = read_metrics_log(&log_path, 200, &range, 10, None).await?;

        if result.records.len() == 1 && result.summary.total_requests == 2 {
            Ok(())
        } else {
            Err(format!(
                "Expected 1 record and 2 total requests, got {} and {}",
                result.records.len(),
                result.summary.total_requests
            ))
        }
    })
}

#[test]
fn updates_ui_data_on_tick() -> Result<(), String> {
    run_async_test(async {
        let mut args = base_args()?;
        args.target_duration = positive_u64(5)?;

        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<u16>(1);
        let (ui_tx, mut ui_rx) = watch::channel(UiData::default());
        let (metrics_tx, metrics_rx) = tokio::sync::mpsc::channel::<Metrics>(10);

        let handle = setup_metrics_collector(
            &args,
            tokio::time::Instant::now(),
            &shutdown_tx,
            metrics_rx,
            &ui_tx,
            None,
        );

        tokio::time::sleep(Duration::from_millis(150)).await;
        match metrics_tx.try_send(Metrics {
            start: tokio::time::Instant::now(),
            response_time: Duration::from_millis(12),
            status_code: 200,
            timed_out: false,
            transport_error: false,
        }) {
            Ok(()) => {}
            Err(err) => return Err(format!("Failed to send metric: {}", err)),
        }

        tokio::time::sleep(Duration::from_millis(200)).await;
        match ui_rx.changed().await {
            Ok(()) => {}
            Err(err) => return Err(format!("UI channel closed: {}", err)),
        }
        let ui_snapshot = ui_rx.borrow().clone();
        if ui_snapshot.current_requests < 1 {
            return Err("Expected at least one request".to_owned());
        }
        if ui_snapshot.rps > ui_snapshot.current_requests {
            return Err("RPS should not exceed total requests".to_owned());
        }

        if shutdown_tx.send(1).is_err() {
            return Err("Failed to send shutdown".to_owned());
        }
        drop(metrics_tx);

        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .map_err(|err| format!("Timed out waiting for collector to finish: {}", err))?
            .map_err(|err| format!("Collector join error: {}", err))?;
        Ok(())
    })
}

#[test]
fn read_metrics_log_respects_metrics_max() -> Result<(), String> {
    run_async_test(async {
        let dir = tempfile::tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
        let log_path = dir.path().join("metrics.log");
        let mut file = tokio::fs::File::create(&log_path)
            .await
            .map_err(|err| format!("Failed to create log: {}", err))?;
        file.write_all(b"0,5,200\n1,6,200\n")
            .await
            .map_err(|err| format!("Failed to write log: {}", err))?;
        file.flush()
            .await
            .map_err(|err| format!("Failed to flush log: {}", err))?;

        let result = read_metrics_log(&log_path, 200, &None, 0, None).await?;
        if !result.records.is_empty() {
            return Err("Expected no records when metrics_max is 0".to_owned());
        }
        if result.summary.total_requests != 2 {
            return Err(format!(
                "Expected total_requests 2, got {}",
                result.summary.total_requests
            ));
        }
        Ok(())
    })
}

#[test]
fn read_metrics_log_marks_truncated() -> Result<(), String> {
    run_async_test(async {
        let dir = tempfile::tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
        let log_path = dir.path().join("metrics.log");
        let mut file = tokio::fs::File::create(&log_path)
            .await
            .map_err(|err| format!("Failed to create log: {}", err))?;
        file.write_all(b"0,5,200\n1,6,200\n2,7,500\n")
            .await
            .map_err(|err| format!("Failed to write log: {}", err))?;
        file.flush()
            .await
            .map_err(|err| format!("Failed to flush log: {}", err))?;

        let result = read_metrics_log(&log_path, 200, &None, 2, None).await?;
        if !result.metrics_truncated {
            return Err("Expected metrics_truncated to be true".to_owned());
        }
        if result.records.len() != 2 {
            return Err(format!("Expected 2 records, got {}", result.records.len()));
        }
        if result.summary.total_requests != 3 {
            return Err(format!(
                "Expected total_requests 3, got {}",
                result.summary.total_requests
            ));
        }
        Ok(())
    })
}

#[test]
fn read_metrics_log_empty_file() -> Result<(), String> {
    run_async_test(async {
        let dir = tempfile::tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
        let log_path = dir.path().join("metrics.log");
        tokio::fs::File::create(&log_path)
            .await
            .map_err(|err| format!("Failed to create log: {}", err))?;

        let result = read_metrics_log(&log_path, 200, &None, 10, None).await?;
        if !result.records.is_empty() {
            return Err("Expected no records".to_owned());
        }
        if result.summary.total_requests != 0 {
            return Err(format!(
                "Expected total_requests 0, got {}",
                result.summary.total_requests
            ));
        }
        Ok(())
    })
}

#[test]
fn metrics_logger_summarizes_and_limits_records() -> Result<(), String> {
    run_async_test(async {
        let dir = tempfile::tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
        let log_path = dir.path().join("metrics.log");
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let run_start = tokio::time::Instant::now();
        let logger_config = MetricsLoggerConfig {
            run_start,
            warmup: None,
            expected_status_code: 200,
            metrics_range: None,
            metrics_max: 1,
        };
        let handle = setup_metrics_logger(log_path, logger_config, rx);

        let first = Metrics {
            start: run_start,
            response_time: Duration::from_millis(5),
            status_code: 200,
            timed_out: false,
            transport_error: false,
        };
        let second_start = run_start
            .checked_add(Duration::from_millis(10))
            .ok_or_else(|| "Failed to add duration".to_owned())?;
        let second = Metrics {
            start: second_start,
            response_time: Duration::from_millis(7),
            status_code: 500,
            timed_out: true,
            transport_error: false,
        };

        if tx.send(first).is_err() {
            return Err("Failed to send first metric".to_owned());
        }
        if tx.send(second).is_err() {
            return Err("Failed to send second metric".to_owned());
        }
        drop(tx);

        let result = handle
            .await
            .map_err(|err| format!("Log join error: {}", err))?
            .map_err(|err| format!("Log error: {}", err))?;

        if result.summary.total_requests != 2 {
            return Err(format!(
                "Expected 2 total requests, got {}",
                result.summary.total_requests
            ));
        }
        if result.summary.successful_requests != 1 {
            return Err(format!(
                "Expected 1 successful request, got {}",
                result.summary.successful_requests
            ));
        }
        if result.summary.timeout_requests != 1 {
            return Err(format!(
                "Expected 1 timeout request, got {}",
                result.summary.timeout_requests
            ));
        }
        if result.records.len() != 1 {
            return Err(format!(
                "Expected 1 record due to metrics_max, got {}",
                result.records.len()
            ));
        }
        Ok(())
    })
}
