use clap::{Args, Subcommand};
use std::time::Duration;

use crate::args::{PositiveU64, PositiveUsize};

use super::super::defaults::{default_charts_path, default_tmp_path};
use super::super::parsers::{
    parse_bool_env, parse_duration_arg, parse_positive_u64, parse_positive_usize,
};

#[derive(Debug, Subcommand, Clone)]
pub enum Command {
    /// Quick baseline test with opinionated defaults
    Quick(PresetQuickArgs),
    /// Long-running steady test profile
    Soak(PresetSoakArgs),
    /// Short aggressive burst profile
    Spike(PresetSpikeArgs),
    /// Distributed controller preset (waits for N agents)
    Distributed(PresetDistributedArgs),
    /// Clean up temporary run data
    Cleanup(CleanupArgs),
    /// Compare two snapshot exports
    Compare(CompareArgs),
}

#[derive(Debug, Args, Clone)]
pub struct PresetQuickArgs {
    /// Target URL for the stress test
    #[arg(long, short)]
    pub url: String,

    /// Duration of test (seconds)
    #[arg(long = "duration", short = 't', default_value = "30", value_parser = parse_positive_u64)]
    pub target_duration: PositiveU64,

    /// Max number of concurrent request tasks
    #[arg(long = "max-tasks", default_value = "100", value_parser = parse_positive_usize)]
    pub max_tasks: PositiveUsize,

    /// Limit requests per second (optional)
    #[arg(long = "rate", short = 'q', value_parser = parse_positive_u64)]
    pub rate_limit: Option<PositiveU64>,
}

#[derive(Debug, Args, Clone)]
pub struct PresetSoakArgs {
    /// Target URL for the stress test
    #[arg(long, short)]
    pub url: String,

    /// Duration of test (seconds)
    #[arg(long = "duration", short = 't', default_value = "1800", value_parser = parse_positive_u64)]
    pub target_duration: PositiveU64,

    /// Max number of concurrent request tasks
    #[arg(long = "max-tasks", default_value = "300", value_parser = parse_positive_usize)]
    pub max_tasks: PositiveUsize,

    /// Limit requests per second (optional)
    #[arg(long = "rate", short = 'q', value_parser = parse_positive_u64)]
    pub rate_limit: Option<PositiveU64>,
}

#[derive(Debug, Args, Clone)]
pub struct PresetSpikeArgs {
    /// Target URL for the stress test
    #[arg(long, short)]
    pub url: String,

    /// Duration of test (seconds)
    #[arg(long = "duration", short = 't', default_value = "120", value_parser = parse_positive_u64)]
    pub target_duration: PositiveU64,

    /// Max number of concurrent request tasks
    #[arg(long = "max-tasks", default_value = "1000", value_parser = parse_positive_usize)]
    pub max_tasks: PositiveUsize,

    /// Tasks spawned per tick
    #[arg(long = "spawn-rate", default_value = "20", value_parser = parse_positive_usize)]
    pub spawn_rate_per_tick: PositiveUsize,

    /// Spawn interval in milliseconds
    #[arg(long = "spawn-interval", default_value = "100", value_parser = parse_positive_u64)]
    pub tick_interval: PositiveU64,
}

#[derive(Debug, Args, Clone)]
pub struct PresetDistributedArgs {
    /// Target URL for the stress test
    #[arg(long, short)]
    pub url: String,

    /// Expected number of agents before controller starts
    #[arg(long = "agents", default_value = "3", value_parser = parse_positive_usize)]
    pub agents: PositiveUsize,

    /// Duration of test (seconds)
    #[arg(long = "duration", short = 't', default_value = "300", value_parser = parse_positive_u64)]
    pub target_duration: PositiveU64,

    /// Controller listen address
    #[arg(long = "controller-listen", default_value = "0.0.0.0:9009")]
    pub controller_listen: String,

    /// Shared auth token for distributed mode (optional)
    #[arg(long = "auth-token")]
    pub auth_token: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct CleanupArgs {
    /// Path to temporary run data (directory)
    #[arg(long = "tmp-path", default_value_t = default_tmp_path())]
    pub tmp_path: String,

    /// Also clean chart run directories
    #[arg(long = "with-charts")]
    pub with_charts: bool,

    /// Path to chart output data (directory)
    #[arg(long = "charts-path", default_value_t = default_charts_path())]
    pub charts_path: String,

    /// Only remove entries older than this duration (supports ms/s/m/h)
    #[arg(long = "older-than", value_parser = parse_duration_arg)]
    pub older_than: Option<Duration>,

    /// Show what would be removed without deleting anything
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Actually delete files
    #[arg(long = "force")]
    pub force: bool,
}

#[derive(Debug, Args, Clone)]
pub struct CompareArgs {
    /// Left snapshot file (csv/json/jsonl)
    pub left: String,

    /// Right snapshot file (csv/json/jsonl)
    pub right: String,

    /// Expected HTTP status code
    #[arg(long = "status", short = 's', default_value = "200")]
    pub expected_status_code: u16,

    /// Replay step size for compare mode (supports ms/s/m/h)
    #[arg(long = "replay-step", value_parser = parse_duration_arg)]
    pub replay_step: Option<Duration>,

    /// UI chart window length in milliseconds (default: 10000)
    #[arg(long = "ui-window-ms", default_value = "10000", value_parser = parse_positive_u64)]
    pub ui_window_ms: PositiveU64,

    /// Disable UI rendering
    #[arg(long = "no-tui", alias = "no-ui")]
    pub no_ui: bool,

    /// Disable color output
    #[arg(long = "no-color", env = "NO_COLOR", value_parser = parse_bool_env)]
    pub no_color: bool,

    /// Label for the left series
    #[arg(long = "left-label")]
    pub left_label: Option<String>,

    /// Label for the right series
    #[arg(long = "right-label")]
    pub right_label: Option<String>,
}
