use clap::ArgMatches;
use clap::parser::ValueSource;

use crate::args::{
    LoadProfile, LoadStage, PositiveU64, PositiveUsize, Scenario, ScenarioStep, TesterArgs,
    parse_header,
};
use crate::metrics::MetricsRange;

use super::types::{
    ConfigFile, DistributedConfig, LoadConfig, LoadStageConfig, SCENARIO_SCHEMA_VERSION,
    ScenarioConfig,
};

/// Applies configuration values to CLI arguments.
///
/// # Errors
///
/// Returns an error when config values are invalid or conflict with CLI options.
pub fn apply_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &ConfigFile,
) -> Result<(), String> {
    if config.load.is_some() && (config.rate.is_some() || config.rpm.is_some()) {
        return Err("Config cannot set both 'load' and top-level 'rate'/'rpm' options.".to_owned());
    }
    if !is_cli(matches, "method")
        && let Some(method) = config.method
    {
        args.method = method;
    }

    if !is_cli(matches, "url")
        && let Some(url) = config.url.clone()
    {
        args.url = Some(url);
    }

    if !is_cli(matches, "headers")
        && let Some(headers) = config.headers.as_ref()
    {
        let mut parsed = Vec::with_capacity(headers.len());
        for header in headers {
            parsed.push(parse_header(header)?);
        }
        args.headers = parsed;
    }

    if !is_cli(matches, "data")
        && let Some(data) = config.data.clone()
    {
        args.data = data;
    }

    if !is_cli(matches, "target_duration")
        && let Some(duration) = config.duration
    {
        args.target_duration = ensure_positive_u64(duration, "duration")?;
    }

    if !is_cli(matches, "request_timeout")
        && let Some(timeout) = config.timeout.as_ref()
    {
        args.request_timeout = timeout.to_duration()?;
    }

    if !is_cli(matches, "warmup")
        && let Some(warmup) = config.warmup.as_ref()
    {
        args.warmup = Some(warmup.to_duration()?);
    }

    if !is_cli(matches, "expected_status_code")
        && let Some(status) = config.status
    {
        args.expected_status_code = status;
    }

    if !is_cli(matches, "charts_path")
        && let Some(path) = config.charts_path.clone()
    {
        args.charts_path = path;
    }

    if !is_cli(matches, "no_charts")
        && let Some(no_charts) = config.no_charts
    {
        args.no_charts = no_charts;
    }

    if !is_cli(matches, "tmp_path")
        && let Some(path) = config.tmp_path.clone()
    {
        args.tmp_path = path;
    }

    if !is_cli(matches, "keep_tmp")
        && let Some(keep) = config.keep_tmp
    {
        args.keep_tmp = keep;
    }

    if !is_cli(matches, "export_csv")
        && let Some(path) = config.export_csv.clone()
    {
        args.export_csv = Some(path);
    }

    if !is_cli(matches, "export_json")
        && let Some(path) = config.export_json.clone()
    {
        args.export_json = Some(path);
    }

    if !is_cli(matches, "log_shards")
        && let Some(log_shards) = config.log_shards
    {
        args.log_shards = ensure_positive_usize(log_shards, "log_shards")?;
    }

    if !is_cli(matches, "no_ui")
        && let Some(no_ui) = config.no_ui
    {
        args.no_ui = no_ui;
    }

    if !is_cli(matches, "ui_window_ms")
        && let Some(window_ms) = config.ui_window_ms
    {
        args.ui_window_ms = ensure_positive_u64(window_ms, "ui_window_ms")?;
    }

    if !is_cli(matches, "summary")
        && let Some(summary) = config.summary
    {
        args.summary = summary;
    }

    if !is_cli(matches, "tls_min")
        && let Some(version) = config.tls_min
    {
        args.tls_min = Some(version);
    }

    if !is_cli(matches, "tls_max")
        && let Some(version) = config.tls_max
    {
        args.tls_max = Some(version);
    }

    if !is_cli(matches, "http2")
        && let Some(http2) = config.http2
    {
        args.http2 = http2;
    }

    if !is_cli(matches, "http3")
        && let Some(http3) = config.http3
    {
        args.http3 = http3;
    }

    if !is_cli(matches, "alpn")
        && let Some(alpn) = config.alpn.clone()
    {
        args.alpn = alpn;
    }

    if !is_cli(matches, "proxy_url")
        && let Some(proxy) = config.proxy_url.clone()
    {
        args.proxy_url = Some(proxy);
    }

    if !is_cli(matches, "max_tasks")
        && let Some(max_tasks) = config.max_tasks
    {
        args.max_tasks = ensure_positive_usize(max_tasks, "max_tasks")?;
    }

    if !is_cli(matches, "spawn_rate_per_tick")
        && let Some(spawn_rate) = config.spawn_rate
    {
        args.spawn_rate_per_tick = ensure_positive_usize(spawn_rate, "spawn_rate")?;
    }

    if !is_cli(matches, "tick_interval")
        && let Some(interval) = config.spawn_interval
    {
        args.tick_interval = ensure_positive_u64(interval, "spawn_interval")?;
    }

    if !is_cli(matches, "rate_limit") {
        if let Some(load) = config.load.as_ref() {
            if args.load_profile.is_none() {
                args.load_profile = Some(parse_load_profile(load)?);
            }
        } else if config.rate.is_some() || config.rpm.is_some() {
            args.load_profile = Some(parse_simple_load(config.rate, config.rpm)?);
        }
    }

    if !is_cli(matches, "metrics_range")
        && let Some(range) = config.metrics_range.as_ref()
    {
        args.metrics_range = Some(range.parse::<MetricsRange>()?);
    }

    if !is_cli(matches, "metrics_max")
        && let Some(max) = config.metrics_max
    {
        args.metrics_max = ensure_positive_usize(max, "metrics_max")?;
    }

    if !is_cli(matches, "script")
        && let Some(script) = config.script.clone()
    {
        args.script = Some(script);
    }

    if let Some(scenario) = config.scenario.as_ref() {
        args.scenario = Some(parse_scenario(scenario, args)?);
    }

    if let Some(sinks) = config.sinks.as_ref() {
        args.sinks = Some(sinks.clone());
    }

    if let Some(distributed) = config.distributed.as_ref() {
        apply_distributed_config(args, matches, distributed)?;
    }

    Ok(())
}

fn apply_distributed_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &DistributedConfig,
) -> Result<(), String> {
    if !is_cli(matches, "controller_mode")
        && let Some(mode) = config.controller_mode
    {
        args.controller_mode = mode;
    }

    let role = config
        .role
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase());

    let mut role_applied = false;
    match role.as_deref() {
        Some("controller") => {
            role_applied = true;
            if !is_cli(matches, "controller_listen")
                && let Some(listen) = config.listen.clone()
            {
                args.controller_listen = Some(listen);
            }
        }
        Some("agent") => {
            role_applied = true;
            if !is_cli(matches, "agent_join")
                && let Some(join) = config.join.clone()
            {
                args.agent_join = Some(join);
            }
        }
        Some(other) => {
            return Err(format!(
                "Invalid distributed.role '{}'. Use 'controller' or 'agent'.",
                other
            ));
        }
        None => {}
    }

    if !role_applied {
        if !is_cli(matches, "controller_listen")
            && let Some(listen) = config.listen.clone()
        {
            args.controller_listen = Some(listen);
        }

        if !is_cli(matches, "agent_join")
            && let Some(join) = config.join.clone()
        {
            args.agent_join = Some(join);
        }
    }

    if !is_cli(matches, "control_listen")
        && let Some(listen) = config.control_listen.clone()
    {
        args.control_listen = Some(listen);
    }

    if !is_cli(matches, "control_auth_token")
        && let Some(token) = config.control_auth_token.clone()
    {
        args.control_auth_token = Some(token);
    }

    if !is_cli(matches, "auth_token")
        && let Some(token) = config.auth_token.clone()
    {
        args.auth_token = Some(token);
    }

    if !is_cli(matches, "agent_id")
        && let Some(agent_id) = config.agent_id.clone()
    {
        args.agent_id = Some(agent_id);
    }

    if !is_cli(matches, "agent_weight")
        && let Some(weight) = config.weight
    {
        args.agent_weight = ensure_positive_u64(weight, "distributed.weight")?;
    }

    if !is_cli(matches, "min_agents")
        && let Some(min_agents) = config.min_agents
    {
        args.min_agents = ensure_positive_usize(min_agents, "distributed.min_agents")?;
    }

    if !is_cli(matches, "agent_wait_timeout_ms")
        && let Some(timeout_ms) = config.agent_wait_timeout_ms
    {
        args.agent_wait_timeout_ms = Some(ensure_positive_u64(
            timeout_ms,
            "distributed.agent_wait_timeout_ms",
        )?);
    }

    if !is_cli(matches, "agent_standby")
        && let Some(standby) = config.agent_standby
    {
        args.agent_standby = standby;
    }

    if !is_cli(matches, "agent_reconnect_ms")
        && let Some(interval_ms) = config.agent_reconnect_ms
    {
        args.agent_reconnect_ms =
            ensure_positive_u64(interval_ms, "distributed.agent_reconnect_ms")?;
    }

    if !is_cli(matches, "agent_heartbeat_interval_ms")
        && let Some(interval_ms) = config.agent_heartbeat_interval_ms
    {
        args.agent_heartbeat_interval_ms =
            ensure_positive_u64(interval_ms, "distributed.agent_heartbeat_interval_ms")?;
    }

    if !is_cli(matches, "agent_heartbeat_timeout_ms")
        && let Some(timeout_ms) = config.agent_heartbeat_timeout_ms
    {
        args.agent_heartbeat_timeout_ms =
            ensure_positive_u64(timeout_ms, "distributed.agent_heartbeat_timeout_ms")?;
    }

    if !is_cli(matches, "distributed_stream_summaries")
        && let Some(stream_summaries) = config.stream_summaries
    {
        args.distributed_stream_summaries = stream_summaries;
    }

    if !is_cli(matches, "distributed_stream_interval_ms")
        && let Some(interval_ms) = config.stream_interval_ms
    {
        args.distributed_stream_interval_ms = Some(ensure_positive_u64(
            interval_ms,
            "distributed.stream_interval_ms",
        )?);
    }

    Ok(())
}

fn is_cli(matches: &ArgMatches, name: &str) -> bool {
    matches.value_source(name) == Some(ValueSource::CommandLine)
}

fn ensure_positive_u64(value: u64, field: &str) -> Result<PositiveU64, String> {
    PositiveU64::try_from(value).map_err(|err| format!("Config '{}' must be >= 1: {}", field, err))
}

fn ensure_positive_usize(value: usize, field: &str) -> Result<PositiveUsize, String> {
    PositiveUsize::try_from(value)
        .map_err(|err| format!("Config '{}' must be >= 1: {}", field, err))
}

fn parse_load_profile(load: &LoadConfig) -> Result<LoadProfile, String> {
    let initial_rpm = resolve_rpm(load.rate, load.rpm, "load")?.unwrap_or(0);

    let mut stages = Vec::new();
    if let Some(stage_configs) = load.stages.as_ref() {
        for (idx, stage) in stage_configs.iter().enumerate() {
            let duration = super::parse_duration_value(&stage.duration)?;
            let target_rpm = resolve_stage_rpm(stage, idx)?;
            stages.push(LoadStage {
                duration,
                target_rpm,
            });
        }
    }

    if initial_rpm == 0 && stages.is_empty() {
        return Err("Load profile requires a rate/rpm or at least one stage.".to_owned());
    }

    Ok(LoadProfile {
        initial_rpm,
        stages,
    })
}

fn parse_simple_load(rate: Option<u64>, rpm: Option<u64>) -> Result<LoadProfile, String> {
    let initial_rpm = resolve_rpm(rate, rpm, "rate/rpm")?.unwrap_or(0);
    if initial_rpm == 0 {
        return Err("Config rate/rpm must be >= 1.".to_owned());
    }

    Ok(LoadProfile {
        initial_rpm,
        stages: Vec::new(),
    })
}

fn resolve_stage_rpm(stage: &LoadStageConfig, idx: usize) -> Result<u64, String> {
    let mut configured = 0u8;
    if stage.target.is_some() {
        configured = configured.saturating_add(1);
    }
    if stage.rate.is_some() {
        configured = configured.saturating_add(1);
    }
    if stage.rpm.is_some() {
        configured = configured.saturating_add(1);
    }

    let stage_index = idx.saturating_add(1);
    if configured == 0 {
        return Err(format!(
            "Stage {} must define one of target, rate, or rpm.",
            stage_index
        ));
    }
    if configured > 1 {
        return Err(format!(
            "Stage {} cannot combine target, rate, and rpm.",
            stage_index
        ));
    }

    if let Some(rpm) = stage.rpm {
        return Ok(rpm);
    }

    let rate = stage.target.or(stage.rate).unwrap_or(0);
    Ok(rate.saturating_mul(60))
}

fn resolve_rpm(rate: Option<u64>, rpm: Option<u64>, context: &str) -> Result<Option<u64>, String> {
    if rate.is_some() && rpm.is_some() {
        return Err(format!(
            "Config '{}' cannot define both rate and rpm.",
            context
        ));
    }
    if let Some(rpm) = rpm {
        return Ok(Some(rpm));
    }
    if let Some(rate) = rate {
        return Ok(Some(rate.saturating_mul(60)));
    }
    Ok(None)
}

pub(crate) fn parse_scenario(
    config: &ScenarioConfig,
    args: &TesterArgs,
) -> Result<Scenario, String> {
    if let Some(schema_version) = config.schema_version
        && schema_version != SCENARIO_SCHEMA_VERSION
    {
        return Err(format!(
            "Unsupported scenario schema_version {}.",
            schema_version
        ));
    }

    if config.steps.is_empty() {
        return Err("Scenario must include at least one step.".to_owned());
    }

    let base_url = config.base_url.clone().or_else(|| args.url.clone());
    let default_method = config.method.unwrap_or(args.method);
    let default_body = config.data.clone().unwrap_or_else(|| args.data.clone());

    let default_headers = if let Some(headers) = config.headers.as_ref() {
        let mut parsed = Vec::with_capacity(headers.len());
        for header in headers {
            parsed.push(parse_header(header)?);
        }
        parsed
    } else {
        args.headers.clone()
    };

    let vars = config.vars.clone().unwrap_or_default();
    let mut steps = Vec::with_capacity(config.steps.len());

    for (idx, step) in config.steps.iter().enumerate() {
        let method = step.method.unwrap_or(default_method);
        let mut headers = default_headers.clone();
        if let Some(step_headers) = step.headers.as_ref() {
            for header in step_headers {
                headers.push(parse_header(header)?);
            }
        }

        let think_time = match step.think_time.as_ref() {
            Some(value) => Some(value.to_duration()?),
            None => None,
        };

        let url = step.url.clone();
        let path = step.path.clone();
        if url.is_none() && path.is_none() && base_url.is_none() {
            return Err(format!(
                "Scenario step {} must define url/path or set scenario.base_url.",
                idx.saturating_add(1)
            ));
        }

        steps.push(ScenarioStep {
            name: step.name.clone(),
            method,
            url,
            path,
            headers,
            body: step.data.clone().map_or_else(
                || {
                    if default_body.is_empty() {
                        None
                    } else {
                        Some(default_body.clone())
                    }
                },
                Some,
            ),
            assert_status: step.assert_status,
            assert_body_contains: step.assert_body_contains.clone(),
            think_time,
            vars: step.vars.clone().unwrap_or_default(),
        });
    }

    Ok(Scenario {
        base_url,
        vars,
        steps,
    })
}
