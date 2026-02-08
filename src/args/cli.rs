use clap::Parser;
use std::time::Duration;

use crate::metrics::MetricsRange;
use crate::sinks::config::SinksConfig;

use super::defaults::{default_charts_path, default_tmp_path};
use super::parsers::{
    parse_duration_arg, parse_header, parse_positive_u64, parse_positive_usize, parse_tls_version,
};
use super::types::{
    ControllerMode, HttpMethod, LoadProfile, PositiveU64, PositiveUsize, Scenario, TlsVersion,
};

#[derive(Debug, Parser, Clone)]
#[clap(
    version,
    about = "Blazing-fast async HTTP load tester in Rust - lock-free design, real-time stats, distributed runs, and optional chart exports for high-load API testing."
)]
pub struct TesterArgs {
    /// HTTP method to use
    #[arg(long, short = 'X', default_value = "get", ignore_case = true)]
    pub method: HttpMethod,

    /// Target URL for the stress test
    #[arg(long, short)]
    pub url: Option<String>,

    /// HTTP headers in 'Key: Value' format (repeatable)
    #[arg(long, short = 'H', value_parser = parse_header)]
    pub headers: Vec<(String, String)>,

    /// Request body data (for POST/PUT)
    #[arg(long, short, default_value = "")]
    pub data: String,

    /// Duration of test (seconds)
    #[arg(
        long = "duration",
        short = 't',
        default_value = "30",
        value_parser = parse_positive_u64
    )]
    pub target_duration: PositiveU64,

    /// Expected HTTP status code
    #[arg(long = "status", short = 's', default_value = "200")]
    pub expected_status_code: u16,

    /// Request timeout (supports ms/s/m/h)
    #[arg(
        long = "timeout",
        default_value = "10s",
        value_parser = parse_duration_arg
    )]
    pub request_timeout: Duration,

    /// Path to save charts to
    #[arg(long, short = 'c', default_value_t = default_charts_path())]
    pub charts_path: String,

    /// Disable chart generation
    #[arg(long, short = 'n')]
    pub no_charts: bool,

    /// Enable verbose logging (sets log level to debug unless overridden by STREST_LOG/RUST_LOG)
    #[arg(long, short = 'v')]
    pub verbose: bool,

    /// Path to config file (TOML/JSON). Defaults to ./strest.toml or ./strest.json if present.
    #[arg(long)]
    pub config: Option<String>,

    /// Path to store temporary run data
    #[arg(long = "tmp-path", default_value_t = default_tmp_path())]
    pub tmp_path: String,

    /// Keep temporary run data after completion
    #[arg(long = "keep-tmp")]
    pub keep_tmp: bool,

    /// Ignore the first N seconds for summary/charts/exports (supports ms/s/m/h)
    #[arg(long = "warmup", value_parser = parse_duration_arg)]
    pub warmup: Option<Duration>,

    /// Export metrics to CSV (uses the same bounds as charts)
    #[arg(long = "export-csv")]
    pub export_csv: Option<String>,

    /// Export metrics to JSON (uses the same bounds as charts)
    #[arg(long = "export-json")]
    pub export_json: Option<String>,

    /// Number of log shards to use for metrics logging (default: 1)
    #[arg(long = "log-shards", default_value = "1", value_parser = parse_positive_usize)]
    pub log_shards: PositiveUsize,

    /// Disable UI rendering
    #[arg(long = "no-ui")]
    pub no_ui: bool,

    /// Print summary at the end of the run (implied by --no-ui)
    #[arg(long = "summary")]
    pub summary: bool,

    /// Minimum TLS version (1.0, 1.1, 1.2, 1.3)
    #[arg(long = "tls-min", value_parser = parse_tls_version)]
    pub tls_min: Option<TlsVersion>,

    /// Maximum TLS version (1.0, 1.1, 1.2, 1.3)
    #[arg(long = "tls-max", value_parser = parse_tls_version)]
    pub tls_max: Option<TlsVersion>,

    /// Enable HTTP/2 (adaptive)
    #[arg(long = "http2")]
    pub http2: bool,

    /// ALPN protocols to advertise (repeatable, e.g. --alpn h2 --alpn http/1.1)
    #[arg(long = "alpn")]
    pub alpn: Vec<String>,

    /// Proxy URL (optional)
    #[arg(long = "proxy", short = 'p', alias = "proxy-url")]
    pub proxy_url: Option<String>,

    /// Max number of concurrent request tasks (default: 1000)
    #[arg(
        long = "max-tasks",
        short = 'm',
        alias = "concurrency",
        default_value = "1000",
        value_parser = parse_positive_usize
    )]
    pub max_tasks: PositiveUsize,

    /// Number of tasks to spawn per tick (default: 1)
    #[arg(
        long = "spawn-rate",
        short = 'r',
        default_value = "1",
        value_parser = parse_positive_usize
    )]
    pub spawn_rate_per_tick: PositiveUsize,

    /// Interval between ticks (milliseconds) (default: 100)
    #[arg(
        long = "spawn-interval",
        short = 'i',
        default_value = "100",
        value_parser = parse_positive_u64
    )]
    pub tick_interval: PositiveU64,

    /// Limit requests per second (optional)
    #[arg(long = "rate", value_parser = parse_positive_u64, required = false)]
    pub rate_limit: Option<PositiveU64>,

    #[arg(skip)]
    pub load_profile: Option<LoadProfile>,

    /// Listen address for distributed controller (e.g. 0.0.0.0:9009)
    #[arg(long = "controller-listen")]
    pub controller_listen: Option<String>,

    /// Controller mode for distributed runs (auto or manual)
    #[arg(long = "controller-mode", default_value = "auto", value_enum)]
    pub controller_mode: ControllerMode,

    /// Control-plane HTTP listen address (e.g. 127.0.0.1:9010)
    #[arg(long = "control-listen")]
    pub control_listen: Option<String>,

    /// Control-plane auth token (optional)
    #[arg(long = "control-auth-token")]
    pub control_auth_token: Option<String>,

    /// Controller address to join as an agent (e.g. 10.0.0.5:9009)
    #[arg(long = "agent-join")]
    pub agent_join: Option<String>,

    /// Shared auth token for distributed mode (optional)
    #[arg(long = "auth-token")]
    pub auth_token: Option<String>,

    /// Explicit agent id (optional)
    #[arg(long = "agent-id")]
    pub agent_id: Option<String>,

    /// Agent weight for load distribution (default: 1)
    #[arg(long = "agent-weight", default_value = "1", value_parser = parse_positive_u64)]
    pub agent_weight: PositiveU64,

    /// Minimum agents required before controller starts (default: 1)
    #[arg(long = "min-agents", default_value = "1", value_parser = parse_positive_usize)]
    pub min_agents: PositiveUsize,

    /// Max time to wait for min agents before starting (milliseconds, optional)
    #[arg(long = "agent-wait-timeout-ms", value_parser = parse_positive_u64)]
    pub agent_wait_timeout_ms: Option<PositiveU64>,

    /// Keep agents connected between distributed runs
    #[arg(long = "agent-standby")]
    pub agent_standby: bool,

    /// Reconnect interval for standby agents (milliseconds)
    #[arg(long = "agent-reconnect-ms", default_value = "1000", value_parser = parse_positive_u64)]
    pub agent_reconnect_ms: PositiveU64,

    /// Heartbeat interval for agents (milliseconds)
    #[arg(
        long = "agent-heartbeat-interval-ms",
        default_value = "1000",
        value_parser = parse_positive_u64
    )]
    pub agent_heartbeat_interval_ms: PositiveU64,

    /// Heartbeat timeout for agents (milliseconds)
    #[arg(
        long = "agent-heartbeat-timeout-ms",
        default_value = "3000",
        value_parser = parse_positive_u64
    )]
    pub agent_heartbeat_timeout_ms: PositiveU64,

    /// Stream summary interval in milliseconds for distributed mode (optional)
    /// Only applies when distributed stream summaries are enabled.
    #[arg(long = "stream-interval-ms", value_parser = parse_positive_u64)]
    pub distributed_stream_interval_ms: Option<PositiveU64>,

    /// Stream periodic summaries to the controller in distributed mode
    #[arg(long = "stream-summaries")]
    pub distributed_stream_summaries: bool,

    /// Enable HTTP/3 (requires rustls + http3 support)
    #[arg(long = "http3")]
    pub http3: bool,

    /// Range, in seconds, of metrics to collect for charts (e.g., 10-30)
    #[arg(long = "metrics-range", short = 'M', value_parser, required = false)]
    pub metrics_range: Option<MetricsRange>,

    /// Max number of metrics to keep for charts (default: 1000000)
    #[arg(
        long = "metrics-max",
        default_value = "1000000",
        value_parser = parse_positive_usize
    )]
    pub metrics_max: PositiveUsize,

    #[arg(skip)]
    pub scenario: Option<Scenario>,

    /// WASM script that generates a scenario definition (experimental)
    #[arg(long = "script")]
    pub script: Option<String>,

    /// Install the controller/agent as a system service (Linux only)
    #[arg(long = "install-service")]
    pub install_service: bool,

    /// Uninstall the controller/agent system service (Linux only)
    #[arg(long = "uninstall-service")]
    pub uninstall_service: bool,

    /// Override system service name (Linux only)
    #[arg(long = "service-name")]
    pub service_name: Option<String>,

    #[arg(skip)]
    pub sinks: Option<SinksConfig>,

    #[arg(skip)]
    pub distributed_silent: bool,
}
