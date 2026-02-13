use clap::ArgMatches;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, ConfigError};
use crate::metrics::MetricsRange;

use super::super::super::types::ConfigFile;
use super::super::load::{parse_load_profile, parse_simple_load};
use super::super::util::{
    ensure_positive_u64, ensure_positive_usize, is_cli, parse_connect_to_entries,
};

pub(super) fn apply_runtime_network_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &ConfigFile,
) -> AppResult<()> {
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

    if !is_cli(matches, "burst_delay")
        && let Some(delay) = config.burst_delay.as_ref()
    {
        args.burst_delay = Some(delay.to_duration()?);
    }

    if !is_cli(matches, "burst_rate")
        && let Some(rate) = config.burst_rate
    {
        args.burst_rate = ensure_positive_usize(rate, "burst_rate")?;
    }

    if !is_cli(matches, "latency_correction")
        && let Some(value) = config.latency_correction
    {
        args.latency_correction = value;
    }

    if !is_cli(matches, "connect_to")
        && let Some(entries) = config.connect_to.as_ref()
    {
        args.connect_to = parse_connect_to_entries(entries)?;
    }

    if !is_cli(matches, "host_header")
        && let Some(host) = config.host.clone()
    {
        args.host_header = Some(host);
    }

    if !is_cli(matches, "ipv6_only")
        && let Some(ipv6) = config.ipv6
    {
        args.ipv6_only = ipv6;
    }

    if !is_cli(matches, "ipv4_only")
        && let Some(ipv4) = config.ipv4
    {
        args.ipv4_only = ipv4;
    }

    if !is_cli(matches, "no_pre_lookup")
        && let Some(no_pre_lookup) = config.no_pre_lookup
    {
        args.no_pre_lookup = no_pre_lookup;
    }

    if !is_cli(matches, "no_color")
        && let Some(no_color) = config.no_color
    {
        args.no_color = no_color;
    }

    if !is_cli(matches, "ui_fps")
        && let Some(fps) = config.fps
    {
        args.ui_fps = fps;
    }

    if !is_cli(matches, "stats_success_breakdown")
        && let Some(flag) = config.stats_success_breakdown
    {
        args.stats_success_breakdown = flag;
    }

    if !is_cli(matches, "unix_socket")
        && let Some(path) = config.unix_socket.clone()
    {
        args.unix_socket = Some(path);
    }

    if args.ipv4_only && args.ipv6_only {
        return Err(AppError::config(ConfigError::Conflict {
            left: "ipv4",
            right: "ipv6",
        }));
    }

    if !is_cli(matches, "metrics_range")
        && let Some(range) = config.metrics_range.as_ref()
    {
        args.metrics_range =
            Some(range.parse::<MetricsRange>().map_err(|err| {
                AppError::config(ConfigError::InvalidMetricsRange { source: err })
            })?);
    }

    Ok(())
}
