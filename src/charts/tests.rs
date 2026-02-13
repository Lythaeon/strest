use std::fmt::Write as _;
use std::future::Future;
use std::time::Duration;

use tempfile::tempdir;
use tokio::io::AsyncWriteExt;

use super::{LatencyPercentilesSeries, is_chart_run_dir_name, plot_streaming_metrics};
use crate::app::logs;
use crate::args::{HttpMethod, LoadMode, PositiveU64, PositiveUsize, Protocol, TesterArgs};
use crate::error::{AppError, AppResult};
use crate::metrics::{MetricRecord, StreamingChartData};

fn sample_metrics() -> Vec<MetricRecord> {
    vec![
        MetricRecord {
            elapsed_ms: 0,
            latency_ms: 10,
            status_code: 200,
            timed_out: false,
            transport_error: false,
            response_bytes: 0,
            in_flight_ops: 0,
        },
        MetricRecord {
            elapsed_ms: 100,
            latency_ms: 15,
            status_code: 200,
            timed_out: false,
            transport_error: false,
            response_bytes: 0,
            in_flight_ops: 0,
        },
        MetricRecord {
            elapsed_ms: 200,
            latency_ms: 20,
            status_code: 200,
            timed_out: false,
            transport_error: false,
            response_bytes: 0,
            in_flight_ops: 0,
        },
        MetricRecord {
            elapsed_ms: 400,
            latency_ms: 30,
            status_code: 500,
            timed_out: false,
            transport_error: true,
            response_bytes: 0,
            in_flight_ops: 0,
        },
    ]
}

#[test]
fn plot_latency_percentiles_single_second() -> AppResult<()> {
    run_async_test(async {
        let metrics = sample_metrics();
        let (dir, data) = build_streaming_data(&metrics, 200).await?;

        let base_path = dir.path().join("latency_percentiles");
        let base_path_str = match base_path.to_str() {
            Some(path) => path,
            None => return Err(AppError::metrics("Failed to convert path to string")),
        };
        let series = LatencyPercentilesSeries {
            buckets_ms: &data.latency_buckets_ms,
            bucket_ms: data.latency_bucket_ms,
            p50: &data.p50,
            p90: &data.p90,
            p99: &data.p99,
            p50_ok: &data.p50_ok,
            p90_ok: &data.p90_ok,
            p99_ok: &data.p99_ok,
        };

        super::plot_latency_percentiles_series(&series, base_path_str).map_err(|err| {
            AppError::metrics(format!("plot_latency_percentiles_series failed: {}", err))
        })?;

        let p50_path = format!("{}_P50_all.png", base_path_str);
        let p50_ok_path = format!("{}_P50_ok.png", base_path_str);
        let p90_path = format!("{}_P90_all.png", base_path_str);
        let p90_ok_path = format!("{}_P90_ok.png", base_path_str);
        let p99_path = format!("{}_P99_all.png", base_path_str);
        let p99_ok_path = format!("{}_P99_ok.png", base_path_str);

        if std::fs::metadata(p50_path).is_err() {
            return Err(AppError::metrics("Missing P50 output"));
        }
        if std::fs::metadata(p50_ok_path).is_err() {
            return Err(AppError::metrics("Missing P50 ok output"));
        }
        if std::fs::metadata(p90_path).is_err() {
            return Err(AppError::metrics("Missing P90 output"));
        }
        if std::fs::metadata(p90_ok_path).is_err() {
            return Err(AppError::metrics("Missing P90 ok output"));
        }
        if std::fs::metadata(p99_path).is_err() {
            return Err(AppError::metrics("Missing P99 output"));
        }
        if std::fs::metadata(p99_ok_path).is_err() {
            return Err(AppError::metrics("Missing P99 ok output"));
        }

        Ok(())
    })
}

fn run_async_test<F>(future: F) -> AppResult<()>
where
    F: Future<Output = AppResult<()>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::metrics(format!("Failed to build runtime: {}", err)))?;
    runtime.block_on(future)
}

async fn build_streaming_data(
    metrics: &[MetricRecord],
    expected_status_code: u16,
) -> AppResult<(tempfile::TempDir, StreamingChartData)> {
    let dir = tempdir()
        .map_err(|err| AppError::metrics(format!("Failed to create temp dir: {}", err)))?;
    let log_path = dir.path().join("metrics.log");
    let mut file = tokio::fs::File::create(&log_path)
        .await
        .map_err(|err| AppError::metrics(format!("Failed to create log: {}", err)))?;
    let mut content = String::new();
    for metric in metrics {
        writeln!(
            &mut content,
            "{},{},{},{},{}",
            metric.elapsed_ms,
            metric.latency_ms,
            metric.status_code,
            u8::from(metric.timed_out),
            u8::from(metric.transport_error)
        )
        .map_err(|err| AppError::metrics(format!("Failed to format log line: {}", err)))?;
    }
    file.write_all(content.as_bytes())
        .await
        .map_err(|err| AppError::metrics(format!("Failed to write log: {}", err)))?;
    file.flush()
        .await
        .map_err(|err| AppError::metrics(format!("Failed to flush log: {}", err)))?;
    let data =
        logs::load_chart_data_streaming(&[log_path], expected_status_code, &None, 100).await?;
    Ok((dir, data))
}

#[test]
fn plot_metrics_creates_files() -> AppResult<()> {
    run_async_test(async {
        let metrics = sample_metrics();
        let dir = tempdir()
            .map_err(|err| AppError::metrics(format!("Failed to create temp dir: {}", err)))?;

        let charts_path = match dir.path().to_str() {
            Some(path) => path.to_owned(),
            None => return Err(AppError::metrics("Failed to convert path to string")),
        };

        let args = TesterArgs {
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
            url: Some("http://localhost".to_owned()),
            urls_from_file: false,
            rand_regex_url: false,
            max_repeat: PositiveUsize::try_from(4)?,
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
            target_duration: PositiveU64::try_from(1)?,
            wait_ongoing_requests_after_deadline: false,
            requests: None,
            expected_status_code: 200,
            request_timeout: Duration::from_secs(10),
            redirect_limit: 10,
            disable_keepalive: false,
            disable_compression: false,
            pool_max_idle_per_host: None,
            pool_idle_timeout_ms: None,
            http_version: None,
            connect_timeout: Duration::from_secs(5),
            charts_path: charts_path.clone(),
            no_charts: false,
            charts_latency_bucket_ms: PositiveU64::try_from(100)?,
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
            agent_weight: PositiveU64::try_from(1)?,
            min_agents: PositiveUsize::try_from(1)?,
            agent_wait_timeout_ms: None,
            agent_standby: false,
            agent_reconnect_ms: PositiveU64::try_from(1000)?,
            agent_heartbeat_interval_ms: PositiveU64::try_from(1000)?,
            agent_heartbeat_timeout_ms: PositiveU64::try_from(3000)?,
            keep_tmp: false,
            warmup: None,
            output: None,
            output_format: None,
            time_unit: None,
            export_csv: None,
            export_json: None,
            export_jsonl: None,
            db_url: None,
            log_shards: PositiveUsize::try_from(1)?,
            no_ui: true,
            no_splash: true,
            ui_window_ms: PositiveU64::try_from(10_000)?,
            summary: false,
            show_selections: false,
            tls_min: None,
            tls_max: None,
            cacert: None,
            cert: None,
            key: None,
            insecure: false,
            http2: false,
            http2_parallel: PositiveUsize::try_from(1)?,
            http3: false,
            alpn: vec![],
            proxy_url: None,
            proxy_headers: vec![],
            proxy_http_version: None,
            proxy_http2: false,
            max_tasks: PositiveUsize::try_from(1)?,
            spawn_rate_per_tick: PositiveUsize::try_from(1)?,
            tick_interval: PositiveU64::try_from(1)?,
            rate_limit: None,
            burst_delay: None,
            burst_rate: PositiveUsize::try_from(1)?,
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
            metrics_max: PositiveUsize::try_from(1_000_000)?,
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
        };

        let (_dir, data) = build_streaming_data(&metrics, args.expected_status_code).await?;

        let output_dir = plot_streaming_metrics(&data, &args)
            .await
            .map_err(|err| AppError::metrics(format!("plot_streaming_metrics failed: {}", err)))?
            .ok_or_else(|| AppError::metrics("Expected chart output directory"))?;

        let expected = vec![
            "average_response_time.png",
            "cumulative_successful_requests.png",
            "cumulative_error_rate.png",
            "latency_percentiles_P50_all.png",
            "latency_percentiles_P50_ok.png",
            "latency_percentiles_P90_all.png",
            "latency_percentiles_P90_ok.png",
            "latency_percentiles_P99_all.png",
            "latency_percentiles_P99_ok.png",
            "requests_per_second.png",
            "timeouts_per_second.png",
            "error_rate_breakdown.png",
            "status_code_distribution.png",
            "inflight_requests.png",
            "cumulative_total_requests.png",
        ];

        for file in expected {
            let path = std::path::Path::new(&output_dir).join(file);
            std::fs::metadata(path).map_err(|err| {
                AppError::metrics(format!("Missing chart output: {} ({})", file, err))
            })?;
        }

        Ok(())
    })
}

#[test]
fn chart_run_dir_name_validation() -> AppResult<()> {
    if !is_chart_run_dir_name("run-2026-02-12_15-30-07_example.com-443") {
        return Err(AppError::metrics("Expected valid chart run directory name"));
    }
    if is_chart_run_dir_name("api.example.com") {
        return Err(AppError::metrics(
            "Unexpected valid name without run prefix",
        ));
    }
    if is_chart_run_dir_name("run-abc-api.example.com") {
        return Err(AppError::metrics(
            "Unexpected valid name with non-numeric timestamp",
        ));
    }
    Ok(())
}
