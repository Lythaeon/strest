use super::workload::render_template;
use super::*;
use crate::args::{HttpMethod, PositiveU64, PositiveUsize, TesterArgs};
use crate::metrics::Metrics;
use std::future::Future;
use std::time::Duration;

fn positive_u64(value: u64) -> Result<PositiveU64, String> {
    PositiveU64::try_from(value)
}

fn positive_usize(value: usize) -> Result<PositiveUsize, String> {
    PositiveUsize::try_from(value)
}

fn base_args(url: String) -> Result<TesterArgs, String> {
    Ok(TesterArgs {
        method: HttpMethod::Get,
        url: Some(url),
        headers: vec![],
        data: String::new(),
        target_duration: positive_u64(1)?,
        expected_status_code: 200,
        request_timeout: Duration::from_secs(10),
        charts_path: "./charts".to_owned(),
        no_charts: true,
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
        log_shards: positive_usize(1)?,
        no_ui: true,
        summary: false,
        tls_min: None,
        tls_max: None,
        http2: false,
        http3: false,
        alpn: vec![],
        proxy_url: None,
        max_tasks: positive_usize(1)?,
        spawn_rate_per_tick: positive_usize(1)?,
        tick_interval: positive_u64(10)?,
        rate_limit: None,
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
fn invalid_proxy_sends_shutdown() -> Result<(), String> {
    run_async_test(async {
        let mut args = base_args("http://localhost".to_owned())?;
        args.proxy_url = Some("not a url".to_owned());
        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<u16>(1);
        let (metrics_tx, _metrics_rx) = tokio::sync::mpsc::channel::<Metrics>(1);

        let result = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None);
        if result.is_ok() {
            return Err("Expected error for invalid proxy".to_owned());
        }
        Ok(())
    })
}

#[test]
fn invalid_url_sends_shutdown() -> Result<(), String> {
    run_async_test(async {
        let args = base_args("http://".to_owned())?;

        let (shutdown_tx, _) = tokio::sync::broadcast::channel::<u16>(1);
        let (metrics_tx, _metrics_rx) = tokio::sync::mpsc::channel::<Metrics>(10);

        let result = setup_request_sender(&args, &shutdown_tx, &metrics_tx, None);
        if result.is_ok() {
            return Err("Expected error for invalid URL".to_owned());
        }
        Ok(())
    })
}

#[test]
fn rate_controller_ramps_tokens() -> Result<(), String> {
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
        return Err(format!("Expected 10 tokens, got {}", first));
    }
    if second != 15 {
        return Err(format!("Expected 15 tokens, got {}", second));
    }
    if third != 20 {
        return Err(format!("Expected 20 tokens, got {}", third));
    }

    Ok(())
}

#[test]
fn render_template_substitutes_vars() -> Result<(), String> {
    let vars = std::collections::BTreeMap::from([
        ("user".to_owned(), "alice".to_owned()),
        ("seq".to_owned(), "42".to_owned()),
    ]);
    let rendered = render_template("{{user}}-{{seq}}", &vars);
    if rendered != "alice-42" {
        return Err(format!("Unexpected render: {}", rendered));
    }
    Ok(())
}

#[test]
fn resolve_alpn_detects_http2_only() -> Result<(), String> {
    let selection = resolve_alpn(&["h2".to_owned()])?;
    if !matches!(selection.choice, AlpnChoice::Http2Only) {
        return Err("Expected Http2Only".to_owned());
    }
    if selection.has_h3 {
        return Err("Expected has_h3 to be false".to_owned());
    }
    Ok(())
}
