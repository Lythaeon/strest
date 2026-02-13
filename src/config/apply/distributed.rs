use clap::ArgMatches;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, ConfigError};

use super::super::types::DistributedConfig;
use super::util::{ensure_positive_u64, ensure_positive_usize, is_cli};

pub(super) fn apply_distributed_config(
    args: &mut TesterArgs,
    matches: &ArgMatches,
    config: &DistributedConfig,
) -> AppResult<()> {
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
            return Err(AppError::config(ConfigError::InvalidDistributedRole {
                value: other.to_owned(),
            }));
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
