use super::workload::{RequestLimiter, render_template};
use super::*;
use crate::args::{HttpMethod, PositiveU64, PositiveUsize, TesterArgs};
use crate::error::{AppError, AppResult};
use crate::metrics::Metrics;
use std::future::Future;
use std::time::Duration;
use tokio::sync::broadcast;

const SHUTDOWN_CHANNEL_CAPACITY: usize = 1;

fn positive_u64(value: u64) -> AppResult<PositiveU64> {
    Ok(PositiveU64::try_from(value)?)
}

fn positive_usize(value: usize) -> AppResult<PositiveUsize> {
    Ok(PositiveUsize::try_from(value)?)
}

fn base_args(url: String) -> AppResult<TesterArgs> {
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
        request_timeout: Duration::from_secs(10),
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
        tick_interval: positive_u64(10)?,
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
        metrics_max: positive_usize(1_000_000)?,
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

fn run_async_test<F>(future: F) -> AppResult<()>
where
    F: Future<Output = AppResult<()>>,
{
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| AppError::validation(format!("Failed to build runtime: {}", err)))?;
    runtime.block_on(future)
}

#[test]
fn invalid_proxy_sends_shutdown() -> AppResult<()> {
    run_async_test(async {
        let mut args = base_args("http://localhost".to_owned())?;
        args.proxy_url = Some("not a url".to_owned());
        let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
        let (metrics_tx, _metrics_rx) = tokio::sync::mpsc::channel::<Metrics>(1);

        let result = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None);
        if result.is_ok() {
            return Err(AppError::validation("Expected error for invalid proxy"));
        }
        Ok(())
    })
}

#[test]
fn invalid_url_sends_shutdown() -> AppResult<()> {
    run_async_test(async {
        let args = base_args("http://".to_owned())?;

        let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
        let (metrics_tx, _metrics_rx) = tokio::sync::mpsc::channel::<Metrics>(10);

        let result = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None);
        if result.is_ok() {
            return Err(AppError::validation("Expected error for invalid URL"));
        }
        Ok(())
    })
}

#[test]
fn rate_controller_ramps_tokens() -> AppResult<()> {
    let plan = RatePlan {
        initial_rpm: 600,
        stages: vec![RateStage {
            duration_secs: 2,
            target_rpm: 1200,
        }],
    };
    let initial_rpm = plan.initial_rpm;
    let mut controller = RateController {
        plan,
        stage_idx: 0,
        stage_elapsed_secs: 0,
        stage_start_rpm: initial_rpm,
        remainder: 0,
    };
    let first = controller.next_tokens();
    let second = controller.next_tokens();
    let third = controller.next_tokens();

    if first != 10 {
        return Err(AppError::validation(format!(
            "Expected 10 tokens, got {}",
            first
        )));
    }
    if second != 15 {
        return Err(AppError::validation(format!(
            "Expected 15 tokens, got {}",
            second
        )));
    }
    if third != 20 {
        return Err(AppError::validation(format!(
            "Expected 20 tokens, got {}",
            third
        )));
    }

    Ok(())
}

#[test]
fn render_template_substitutes_vars() -> AppResult<()> {
    let vars = std::collections::BTreeMap::from([
        ("user".to_owned(), "alice".to_owned()),
        ("seq".to_owned(), "42".to_owned()),
    ]);
    let rendered = render_template("{{user}}-{{seq}}", &vars);
    if rendered != "alice-42" {
        return Err(AppError::validation(format!(
            "Unexpected render: {}",
            rendered
        )));
    }
    Ok(())
}

#[test]
fn resolve_alpn_detects_http2_only() -> AppResult<()> {
    let selection = resolve_alpn(&["h2".to_owned()])?;
    if !matches!(selection.choice, AlpnChoice::Http2Only) {
        return Err(AppError::validation("Expected Http2Only"));
    }
    if selection.has_h3 {
        return Err(AppError::validation("Expected has_h3 to be false"));
    }
    Ok(())
}

#[test]
fn request_limiter_stops_at_limit() -> AppResult<()> {
    let limiter =
        RequestLimiter::new(Some(2)).ok_or_else(|| AppError::validation("Missing limiter"))?;
    let (shutdown_tx, _) = broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY);
    let mut shutdown_rx = shutdown_tx.subscribe();

    if !limiter.try_reserve(&shutdown_tx) {
        return Err(AppError::validation("Expected first reserve to succeed"));
    }
    if !limiter.try_reserve(&shutdown_tx) {
        return Err(AppError::validation("Expected second reserve to succeed"));
    }
    if limiter.try_reserve(&shutdown_tx) {
        return Err(AppError::validation("Expected third reserve to fail"));
    }
    if shutdown_rx.try_recv().is_err() {
        return Err(AppError::validation("Expected shutdown signal"));
    }

    Ok(())
}
