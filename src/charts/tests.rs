use std::future::Future;
use std::time::Duration;

use tempfile::tempdir;

use super::{plot_latency_percentiles, plot_metrics};
use crate::args::{HttpMethod, PositiveU64, PositiveUsize, TesterArgs};
use crate::metrics::MetricRecord;

fn sample_metrics() -> Vec<MetricRecord> {
    vec![
        MetricRecord {
            elapsed_ms: 0,
            latency_ms: 10,
            status_code: 200,
            timed_out: false,
            transport_error: false,
        },
        MetricRecord {
            elapsed_ms: 100,
            latency_ms: 15,
            status_code: 200,
            timed_out: false,
            transport_error: false,
        },
        MetricRecord {
            elapsed_ms: 200,
            latency_ms: 20,
            status_code: 200,
            timed_out: false,
            transport_error: false,
        },
        MetricRecord {
            elapsed_ms: 400,
            latency_ms: 30,
            status_code: 500,
            timed_out: false,
            transport_error: true,
        },
    ]
}

#[test]
fn plot_latency_percentiles_single_second() -> Result<(), String> {
    let metrics = sample_metrics();
    let dir = tempdir().map_err(|err| format!("Failed to create temp dir: {}", err))?;

    let base_path = dir.path().join("latency_percentiles");
    let base_path_str = match base_path.to_str() {
        Some(path) => path,
        None => return Err("Failed to convert path to string".to_owned()),
    };

    plot_latency_percentiles(&metrics, 200, base_path_str)
        .map_err(|err| format!("plot_latency_percentiles failed: {}", err))?;

    let p50_path = format!("{}_P50.png", base_path_str);
    let p90_path = format!("{}_P90.png", base_path_str);
    let p99_path = format!("{}_P99.png", base_path_str);

    if std::fs::metadata(p50_path).is_err() {
        return Err("Missing P50 output".to_owned());
    }
    if std::fs::metadata(p90_path).is_err() {
        return Err("Missing P90 output".to_owned());
    }
    if std::fs::metadata(p99_path).is_err() {
        return Err("Missing P99 output".to_owned());
    }

    Ok(())
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
fn plot_metrics_creates_files() -> Result<(), String> {
    run_async_test(async {
        let metrics = sample_metrics();
        let dir = tempdir().map_err(|err| format!("Failed to create temp dir: {}", err))?;

        let charts_path = match dir.path().to_str() {
            Some(path) => path.to_owned(),
            None => return Err("Failed to convert path to string".to_owned()),
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
            url: Some("http://localhost".to_owned()),
            headers: vec![],
            no_ua: false,
            authorized: false,
            data: String::new(),
            target_duration: PositiveU64::try_from(1)?,
            expected_status_code: 200,
            request_timeout: Duration::from_secs(10),
            charts_path: charts_path.clone(),
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
            agent_weight: PositiveU64::try_from(1)?,
            min_agents: PositiveUsize::try_from(1)?,
            agent_wait_timeout_ms: None,
            agent_standby: false,
            agent_reconnect_ms: PositiveU64::try_from(1000)?,
            agent_heartbeat_interval_ms: PositiveU64::try_from(1000)?,
            agent_heartbeat_timeout_ms: PositiveU64::try_from(3000)?,
            keep_tmp: false,
            warmup: None,
            export_csv: None,
            export_json: None,
            export_jsonl: None,
            log_shards: PositiveUsize::try_from(1)?,
            no_ui: true,
            ui_window_ms: PositiveU64::try_from(10_000)?,
            summary: false,
            tls_min: None,
            tls_max: None,
            http2: false,
            http3: false,
            alpn: vec![],
            proxy_url: None,
            max_tasks: PositiveUsize::try_from(1)?,
            spawn_rate_per_tick: PositiveUsize::try_from(1)?,
            tick_interval: PositiveU64::try_from(1)?,
            rate_limit: None,
            metrics_range: None,
            metrics_max: PositiveUsize::try_from(1_000_000)?,
            scenario: None,
            script: None,
            install_service: false,
            uninstall_service: false,
            service_name: None,
            sinks: None,
            distributed_silent: false,
            distributed_stream_summaries: false,
            distributed_stream_interval_ms: None,
        };

        plot_metrics(&metrics, &args)
            .await
            .map_err(|err| format!("plot_metrics failed: {}", err))?;

        let expected = vec![
            "average_response_time.png",
            "cumulative_successful_requests.png",
            "cumulative_error_rate.png",
            "latency_percentiles_P50.png",
            "latency_percentiles_P90.png",
            "latency_percentiles_P99.png",
            "requests_per_second.png",
            "timeouts_per_second.png",
            "error_rate_breakdown.png",
            "status_code_distribution.png",
            "inflight_requests.png",
            "cumulative_total_requests.png",
        ];

        for file in expected {
            let path = dir.path().join(file);
            std::fs::metadata(path)
                .map_err(|err| format!("Missing chart output: {} ({})", file, err))?;
        }

        Ok(())
    })
}
