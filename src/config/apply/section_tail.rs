use clap::ArgMatches;

use crate::args::TesterArgs;
use crate::error::AppResult;

use super::super::types::ConfigFile;
use super::distributed::apply_distributed_config;
use super::scenario::{ScenarioDefaults, parse_scenario};
use super::util::{ensure_positive_u64, ensure_positive_usize, is_cli};

pub(super) fn apply_tail_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &ConfigFile,
    scenario_defaults: &ScenarioDefaults,
) -> AppResult<()> {
    if !is_cli(matches, "metrics_max")
        && let Some(max) = config.metrics_max
    {
        args.metrics_max = ensure_positive_usize(max, "metrics_max")?;
    }

    if !is_cli(matches, "rss_log_ms")
        && let Some(value) = config.rss_log_ms
    {
        args.rss_log_ms = Some(ensure_positive_u64(value, "rss_log_ms")?);
    }

    if !is_cli(matches, "alloc_profiler_ms")
        && let Some(value) = config.alloc_profiler_ms
    {
        args.alloc_profiler_ms = Some(ensure_positive_u64(value, "alloc_profiler_ms")?);
    }

    if !is_cli(matches, "alloc_profiler_dump_ms")
        && let Some(value) = config.alloc_profiler_dump_ms
    {
        args.alloc_profiler_dump_ms = Some(ensure_positive_u64(value, "alloc_profiler_dump_ms")?);
    }

    if !is_cli(matches, "alloc_profiler_dump_path")
        && let Some(value) = &config.alloc_profiler_dump_path
    {
        args.alloc_profiler_dump_path = value.clone();
    }

    if !is_cli(matches, "script")
        && let Some(script) = config.script.clone()
    {
        args.script = Some(script);
    }

    if !is_cli(matches, "plugin")
        && let Some(plugins) = config.plugin.clone()
    {
        args.plugin = plugins;
    }

    if let Some(scenario) = config.scenario.as_ref() {
        args.scenario = Some(parse_scenario(scenario, scenario_defaults)?);
    }

    if let Some(sinks) = config.sinks.as_ref() {
        args.sinks = Some(sinks.clone());
    }

    if let Some(distributed) = config.distributed.as_ref() {
        apply_distributed_config(args, matches, distributed)?;
    }

    Ok(())
}
