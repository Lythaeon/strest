use std::time::Duration;

use crate::args::{LoadProfile, PositiveU64, PositiveUsize, Scenario, ScenarioStep, TesterArgs};
use crate::metrics::MetricsRange;

use super::protocol::{WireArgs, WireLoadProfile, WireLoadStage, WireScenario, WireScenarioStep};
use super::utils::duration_to_ms;

pub(super) fn build_wire_args(args: &TesterArgs) -> WireArgs {
    WireArgs {
        method: args.method,
        url: args.url.clone(),
        headers: args.headers.clone(),
        data: args.data.clone(),
        target_duration: args.target_duration.get(),
        expected_status_code: args.expected_status_code,
        request_timeout_ms: duration_to_ms(args.request_timeout),
        charts_path: args.charts_path.clone(),
        no_charts: true,
        verbose: args.verbose,
        tmp_path: args.tmp_path.clone(),
        keep_tmp: args.keep_tmp,
        warmup_ms: args.warmup.map(duration_to_ms),
        export_csv: None,
        export_json: None,
        log_shards: args.log_shards.get(),
        no_ui: true,
        summary: true,
        proxy_url: args.proxy_url.clone(),
        max_tasks: args.max_tasks.get(),
        spawn_rate_per_tick: args.spawn_rate_per_tick.get(),
        tick_interval: args.tick_interval.get(),
        rate_limit: args.rate_limit.map(u64::from),
        load_profile: args.load_profile.as_ref().map(to_wire_load_profile),
        metrics_range: args.metrics_range.as_ref().map(|range| {
            let start = *range.0.start();
            let end = *range.0.end();
            (start, end)
        }),
        metrics_max: 1,
        scenario: args.scenario.as_ref().map(to_wire_scenario),
        tls_min: args.tls_min,
        tls_max: args.tls_max,
        http2: args.http2,
        http3: args.http3,
        alpn: args.alpn.clone(),
        stream_summaries: args.distributed_stream_summaries,
        stream_interval_ms: args.distributed_stream_interval_ms.map(u64::from),
    }
}

pub(super) fn apply_wire_args(args: &mut TesterArgs, wire: WireArgs) -> Result<(), String> {
    args.method = wire.method;
    args.url = wire.url;
    args.headers = wire.headers;
    args.data = wire.data;
    args.target_duration = PositiveU64::try_from(wire.target_duration)
        .map_err(|err| format!("Wire target_duration must be >= 1: {}", err))?;
    args.expected_status_code = wire.expected_status_code;
    args.request_timeout = Duration::from_millis(wire.request_timeout_ms);
    args.charts_path = wire.charts_path;
    args.no_charts = wire.no_charts;
    args.verbose = wire.verbose;
    args.tmp_path = wire.tmp_path;
    args.keep_tmp = wire.keep_tmp;
    args.warmup = wire.warmup_ms.map(Duration::from_millis);
    args.export_csv = wire.export_csv;
    args.export_json = wire.export_json;
    args.log_shards = PositiveUsize::try_from(wire.log_shards)
        .map_err(|err| format!("Wire log_shards must be >= 1: {}", err))?;
    args.no_ui = wire.no_ui;
    args.summary = wire.summary;
    args.proxy_url = wire.proxy_url;
    args.max_tasks = PositiveUsize::try_from(wire.max_tasks)
        .map_err(|err| format!("Wire max_tasks must be >= 1: {}", err))?;
    args.spawn_rate_per_tick = PositiveUsize::try_from(wire.spawn_rate_per_tick)
        .map_err(|err| format!("Wire spawn_rate_per_tick must be >= 1: {}", err))?;
    args.tick_interval = PositiveU64::try_from(wire.tick_interval)
        .map_err(|err| format!("Wire tick_interval must be >= 1: {}", err))?;
    args.rate_limit = match wire.rate_limit {
        Some(value) => Some(
            PositiveU64::try_from(value)
                .map_err(|err| format!("Wire rate_limit must be >= 1: {}", err))?,
        ),
        None => None,
    };
    args.load_profile = wire.load_profile.map(from_wire_load_profile);
    args.metrics_range = wire
        .metrics_range
        .map(|(start, end)| MetricsRange(start..=end));
    args.metrics_max = PositiveUsize::try_from(wire.metrics_max)
        .map_err(|err| format!("Wire metrics_max must be >= 1: {}", err))?;
    args.scenario = wire.scenario.map(from_wire_scenario);
    args.tls_min = wire.tls_min;
    args.tls_max = wire.tls_max;
    args.http2 = wire.http2;
    args.http3 = wire.http3;
    args.alpn = wire.alpn;
    args.distributed_stream_summaries = wire.stream_summaries;
    args.distributed_stream_interval_ms = match wire.stream_interval_ms {
        Some(value) => Some(
            PositiveU64::try_from(value)
                .map_err(|err| format!("Wire stream_interval_ms must be >= 1: {}", err))?,
        ),
        None => None,
    };
    Ok(())
}

pub(super) fn to_wire_load_profile(profile: &LoadProfile) -> WireLoadProfile {
    WireLoadProfile {
        initial_rpm: profile.initial_rpm,
        stages: profile
            .stages
            .iter()
            .map(|stage| WireLoadStage {
                duration_secs: stage.duration.as_secs(),
                target_rpm: stage.target_rpm,
            })
            .collect(),
    }
}

pub(super) fn from_wire_load_profile(profile: WireLoadProfile) -> LoadProfile {
    LoadProfile {
        initial_rpm: profile.initial_rpm,
        stages: profile
            .stages
            .into_iter()
            .map(|stage| crate::args::LoadStage {
                duration: Duration::from_secs(stage.duration_secs.max(1)),
                target_rpm: stage.target_rpm,
            })
            .collect(),
    }
}

pub(super) fn to_wire_scenario(scenario: &Scenario) -> WireScenario {
    WireScenario {
        base_url: scenario.base_url.clone(),
        vars: scenario.vars.clone(),
        steps: scenario
            .steps
            .iter()
            .map(|step| WireScenarioStep {
                name: step.name.clone(),
                method: step.method,
                url: step.url.clone(),
                path: step.path.clone(),
                headers: step.headers.clone(),
                body: step.body.clone(),
                assert_status: step.assert_status,
                assert_body_contains: step.assert_body_contains.clone(),
                think_time_ms: step.think_time.map(duration_to_ms),
                vars: step.vars.clone(),
            })
            .collect(),
    }
}

pub(super) fn from_wire_scenario(scenario: WireScenario) -> Scenario {
    Scenario {
        base_url: scenario.base_url,
        vars: scenario.vars,
        steps: scenario
            .steps
            .into_iter()
            .map(|step| ScenarioStep {
                name: step.name,
                method: step.method,
                url: step.url,
                path: step.path,
                headers: step.headers,
                body: step.body,
                assert_status: step.assert_status,
                assert_body_contains: step.assert_body_contains,
                think_time: step.think_time_ms.map(Duration::from_millis),
                vars: step.vars,
            })
            .collect(),
    }
}
