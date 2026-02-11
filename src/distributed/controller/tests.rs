use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use tokio::sync::watch;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult};
use crate::metrics::LatencyHistogram;
use crate::ui::model::UiData;

use super::super::protocol::WireSummary;
use super::shared::{AgentSnapshot, aggregate_snapshots, update_ui};

fn build_hist(values: &[u64]) -> AppResult<LatencyHistogram> {
    let mut hist = LatencyHistogram::new()?;
    for value in values {
        hist.record(*value)?;
    }
    Ok(hist)
}

fn base_args() -> AppResult<TesterArgs> {
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
        method: crate::args::HttpMethod::Get,
        url: Some("http://localhost".to_owned()),
        urls_from_file: false,
        rand_regex_url: false,
        max_repeat: crate::args::PositiveUsize::try_from(4)?,
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
        target_duration: crate::args::PositiveU64::try_from(1)?,
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
        charts_latency_bucket_ms: crate::args::PositiveU64::try_from(100)?,
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
        agent_weight: crate::args::PositiveU64::try_from(1)?,
        min_agents: crate::args::PositiveUsize::try_from(1)?,
        agent_wait_timeout_ms: None,
        agent_standby: false,
        agent_reconnect_ms: crate::args::PositiveU64::try_from(1000)?,
        agent_heartbeat_interval_ms: crate::args::PositiveU64::try_from(1000)?,
        agent_heartbeat_timeout_ms: crate::args::PositiveU64::try_from(3000)?,
        keep_tmp: false,
        warmup: None,
        output: None,
        output_format: None,
        time_unit: None,
        export_csv: None,
        export_json: None,
        export_jsonl: None,
        db_url: None,
        log_shards: crate::args::PositiveUsize::try_from(1)?,
        no_ui: true,
        no_splash: true,
        ui_window_ms: crate::args::PositiveU64::try_from(10_000)?,
        summary: false,
        tls_min: None,
        tls_max: None,
        cacert: None,
        cert: None,
        key: None,
        insecure: false,
        http2: false,
        http2_parallel: crate::args::PositiveUsize::try_from(1)?,
        http3: false,
        alpn: vec![],
        proxy_url: None,
        proxy_headers: vec![],
        proxy_http_version: None,
        proxy_http2: false,
        max_tasks: crate::args::PositiveUsize::try_from(1)?,
        spawn_rate_per_tick: crate::args::PositiveUsize::try_from(1)?,
        tick_interval: crate::args::PositiveU64::try_from(100)?,
        rate_limit: None,
        burst_delay: None,
        burst_rate: crate::args::PositiveUsize::try_from(1)?,
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
        metrics_max: crate::args::PositiveUsize::try_from(1_000)?,
        rss_log_ms: None,
        alloc_profiler_ms: None,
        alloc_profiler_dump_ms: None,
        alloc_profiler_dump_path: "./alloc-prof".to_owned(),
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

#[test]
fn aggregate_snapshots_merges_summary() -> AppResult<()> {
    let summary_a = WireSummary {
        duration_ms: 1000,
        total_requests: 10,
        successful_requests: 9,
        error_requests: 1,
        timeout_requests: 1,
        transport_errors: 0,
        non_expected_status: 0,
        success_min_latency_ms: 10,
        success_max_latency_ms: 50,
        success_latency_sum_ms: 900,
        min_latency_ms: 10,
        max_latency_ms: 50,
        latency_sum_ms: 1000,
    };
    let summary_b = WireSummary {
        duration_ms: 1500,
        total_requests: 20,
        successful_requests: 19,
        error_requests: 1,
        timeout_requests: 2,
        transport_errors: 1,
        non_expected_status: 0,
        success_min_latency_ms: 5,
        success_max_latency_ms: 40,
        success_latency_sum_ms: 1900,
        min_latency_ms: 5,
        max_latency_ms: 40,
        latency_sum_ms: 600,
    };

    let hist_a = build_hist(&[10, 20])?;
    let hist_b = build_hist(&[30, 40])?;
    let success_hist_a = build_hist(&[10, 20])?;
    let success_hist_b = build_hist(&[30, 40])?;

    let mut agent_states = HashMap::new();
    agent_states.insert(
        "a".to_owned(),
        AgentSnapshot {
            summary: summary_a,
            histogram: hist_a,
            success_histogram: success_hist_a,
        },
    );
    agent_states.insert(
        "b".to_owned(),
        AgentSnapshot {
            summary: summary_b,
            histogram: hist_b,
            success_histogram: success_hist_b,
        },
    );

    let (summary, merged_hist, _success_hist) = aggregate_snapshots(&agent_states)?;
    if summary.total_requests != 30 {
        return Err(AppError::distributed(format!(
            "Unexpected total_requests: {}",
            summary.total_requests
        )));
    }
    if summary.successful_requests != 28 {
        return Err(AppError::distributed(format!(
            "Unexpected successful_requests: {}",
            summary.successful_requests
        )));
    }
    if summary.error_requests != 2 {
        return Err(AppError::distributed(format!(
            "Unexpected error_requests: {}",
            summary.error_requests
        )));
    }
    if summary.timeout_requests != 3 {
        return Err(AppError::distributed(format!(
            "Unexpected timeout_requests: {}",
            summary.timeout_requests
        )));
    }
    if summary.success_avg_latency_ms != 100 {
        return Err(AppError::distributed(format!(
            "Unexpected success_avg_latency_ms: {}",
            summary.success_avg_latency_ms
        )));
    }
    if summary.min_latency_ms != 5 {
        return Err(AppError::distributed(format!(
            "Unexpected min_latency_ms: {}",
            summary.min_latency_ms
        )));
    }
    if summary.max_latency_ms != 50 {
        return Err(AppError::distributed(format!(
            "Unexpected max_latency_ms: {}",
            summary.max_latency_ms
        )));
    }
    if summary.avg_latency_ms != 53 {
        return Err(AppError::distributed(format!(
            "Unexpected avg_latency_ms: {}",
            summary.avg_latency_ms
        )));
    }
    if merged_hist.count() != 4 {
        return Err(AppError::distributed(format!(
            "Unexpected merged histogram count: {}",
            merged_hist.count()
        )));
    }
    Ok(())
}

#[test]
fn update_ui_emits_aggregated_stats() -> AppResult<()> {
    let args = base_args()?;
    let (ui_tx, ui_rx) = watch::channel(UiData::default());

    let summary = WireSummary {
        duration_ms: 1000,
        total_requests: 10,
        successful_requests: 9,
        error_requests: 1,
        timeout_requests: 0,
        transport_errors: 0,
        non_expected_status: 1,
        success_min_latency_ms: 10,
        success_max_latency_ms: 50,
        success_latency_sum_ms: 900,
        min_latency_ms: 10,
        max_latency_ms: 50,
        latency_sum_ms: 1000,
    };
    let hist = build_hist(&[10, 20, 30])?;
    let success_hist = build_hist(&[10, 20, 30])?;

    let mut agent_states = HashMap::new();
    agent_states.insert(
        "a".to_owned(),
        AgentSnapshot {
            summary,
            histogram: hist,
            success_histogram: success_hist,
        },
    );

    let mut latency_window = VecDeque::new();
    update_ui(&ui_tx, &args, &agent_states, &mut latency_window);

    let snapshot = ui_rx.borrow().clone();
    if snapshot.current_requests != 10 {
        return Err(AppError::distributed(format!(
            "Unexpected current_requests: {}",
            snapshot.current_requests
        )));
    }
    if snapshot.successful_requests != 9 {
        return Err(AppError::distributed(format!(
            "Unexpected successful_requests: {}",
            snapshot.successful_requests
        )));
    }
    if snapshot.p50 == 0 {
        return Err(AppError::distributed("Expected non-zero p50 latency"));
    }
    if snapshot.rps == 0 {
        return Err(AppError::distributed("Expected non-zero rps"));
    }
    Ok(())
}
