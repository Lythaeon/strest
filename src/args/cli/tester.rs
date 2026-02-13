use clap::Parser;
use std::time::Duration;

use crate::metrics::MetricsRange;
use crate::sinks::config::SinksConfig;

use super::super::defaults::{default_charts_path, default_tmp_path};
use super::super::parsers::{
    parse_bool_env, parse_connect_to, parse_duration_arg, parse_header, parse_positive_u64,
    parse_positive_usize, parse_tls_version,
};
use super::super::types::{
    ConnectToMapping, ControllerMode, HttpMethod, HttpVersion, LoadMode, LoadProfile, OutputFormat,
    PositiveU64, PositiveUsize, Protocol, Scenario, TimeUnit, TlsVersion,
};
use super::presets::Command;

#[derive(Debug, Parser, Clone)]
#[clap(
    version,
    about = "Blazing-fast async HTTP load tester in Rust - lock-free design, real-time stats, distributed runs, and optional chart exports for high-load API testing.",
    next_help_heading = "Advanced Options"
)]
pub struct TesterArgs {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// HTTP method to use
    #[arg(
        long,
        short = 'X',
        default_value = "get",
        ignore_case = true,
        help_heading = "Common Options"
    )]
    pub method: HttpMethod,

    /// Network protocol adapter for this run
    #[arg(
        long = "protocol",
        default_value = "http",
        value_enum,
        help_heading = "Common Options"
    )]
    pub protocol: Protocol,

    /// Load model intent (for presets/workflows and metadata)
    #[arg(
        long = "load-mode",
        default_value = "arrival",
        value_enum,
        help_heading = "Common Options"
    )]
    pub load_mode: LoadMode,

    /// Target URL for the stress test
    #[arg(long, short, help_heading = "Common Options")]
    pub url: Option<String>,

    /// Read URLs from file (newline-delimited)
    #[arg(
        long = "urls-from-file",
        conflicts_with = "rand_regex_url",
        requires = "url"
    )]
    pub urls_from_file: bool,

    /// Generate URLs from a rand_regex pattern (uses --url as the pattern)
    #[arg(
        long = "rand-regex-url",
        conflicts_with = "urls_from_file",
        requires = "url"
    )]
    pub rand_regex_url: bool,

    /// Maximum extra repeat count for rand_regex quantifiers
    #[arg(long = "max-repeat", default_value = "4", value_parser = parse_positive_usize)]
    pub max_repeat: PositiveUsize,

    /// Dump generated URLs and exit (requires --rand-regex-url)
    #[arg(long = "dump-urls", value_parser = parse_positive_usize, requires = "rand_regex_url")]
    pub dump_urls: Option<PositiveUsize>,

    /// HTTP headers in 'Key: Value' format (repeatable)
    #[arg(long, short = 'H', value_parser = parse_header, help_heading = "Common Options")]
    pub headers: Vec<(String, String)>,

    /// HTTP Accept header (shortcut)
    #[arg(long = "accept", short = 'A')]
    pub accept_header: Option<String>,

    /// Content-Type header (shortcut)
    #[arg(long = "content-type", short = 'T')]
    pub content_type: Option<String>,

    /// Disable the default User-Agent header (strest-loadtest/<version> (+https://github.com/Lythaeon/strest)); requires --authorized
    #[arg(long = "no-ua", alias = "no-default-ua")]
    pub no_ua: bool,

    /// Confirm you have authorization to run tests when disabling the default User-Agent
    #[arg(long = "authorized")]
    pub authorized: bool,

    /// Request body data (for POST/PUT)
    #[arg(long, short, default_value = "", help_heading = "Common Options")]
    pub data: String,

    /// Specify HTTP multipart form data (repeatable, curl-compatible)
    #[arg(long = "form", short = 'F', conflicts_with_all = ["data", "data_file", "data_lines"])]
    pub form: Vec<String>,

    /// Basic authentication (username:password), or AWS credentials (access_key:secret_key)
    #[arg(long = "basic-auth", short = 'a')]
    pub basic_auth: Option<String>,

    /// AWS session token
    #[arg(long = "aws-session")]
    pub aws_session: Option<String>,

    /// AWS SigV4 signing params (format: aws:amz:region:service)
    #[arg(long = "aws-sigv4")]
    pub aws_sigv4: Option<String>,

    /// Request body from file
    #[arg(long = "data-file", short = 'D', conflicts_with_all = ["data", "data_lines"])]
    pub data_file: Option<String>,

    /// Request body from file line by line
    #[arg(long = "data-lines", short = 'Z', conflicts_with_all = ["data", "data_file"])]
    pub data_lines: Option<String>,

    /// Duration of test (seconds)
    #[arg(
        long = "duration",
        short = 't',
        default_value = "30",
        value_parser = parse_positive_u64,
        help_heading = "Common Options"
    )]
    pub target_duration: PositiveU64,

    /// Wait for ongoing requests after the duration is reached
    #[arg(long = "wait-ongoing-requests-after-deadline")]
    pub wait_ongoing_requests_after_deadline: bool,

    /// Stop after N total requests
    #[arg(long = "requests", short = 'n', value_parser = parse_positive_u64, help_heading = "Common Options")]
    pub requests: Option<PositiveU64>,

    /// Expected HTTP status code
    #[arg(
        long = "status",
        short = 's',
        default_value = "200",
        help_heading = "Common Options"
    )]
    pub expected_status_code: u16,

    /// Request timeout (supports ms/s/m/h)
    #[arg(
        long = "timeout",
        default_value = "10s",
        value_parser = parse_duration_arg,
        help_heading = "Common Options"
    )]
    pub request_timeout: Duration,

    /// Limit the number of redirects to follow (0 disables redirects)
    #[arg(long = "redirect", default_value = "10")]
    pub redirect_limit: u32,

    /// Disable keep-alive (prevents re-use of TCP connections)
    #[arg(long = "disable-keepalive")]
    pub disable_keepalive: bool,

    /// Disable compression (gzip, brotli, deflate)
    #[arg(long = "disable-compression")]
    pub disable_compression: bool,

    /// Max idle connections per host in the HTTP pool (0 disables idle pooling)
    #[arg(long = "pool-max-idle-per-host", value_parser = parse_positive_usize)]
    pub pool_max_idle_per_host: Option<PositiveUsize>,

    /// Idle connection timeout for the HTTP pool (ms)
    #[arg(long = "pool-idle-timeout-ms", value_parser = parse_positive_u64)]
    pub pool_idle_timeout_ms: Option<PositiveU64>,

    /// Prefer HTTP version (0.9, 1.0, 1.1, 2, 3)
    #[arg(long = "http-version", value_enum)]
    pub http_version: Option<HttpVersion>,

    /// Timeout for establishing a new connection (supports ms/s/m/h)
    #[arg(
        long = "connect-timeout",
        default_value = "5s",
        value_parser = parse_duration_arg
    )]
    pub connect_timeout: Duration,

    /// Path to save charts to
    #[arg(long, short = 'c', default_value_t = default_charts_path())]
    pub charts_path: String,

    /// Disable chart generation
    #[arg(long, help_heading = "Common Options")]
    pub no_charts: bool,

    /// Latency percentile chart bucket size in milliseconds
    #[arg(long = "charts-latency-bucket-ms", default_value = "100", value_parser = parse_positive_u64)]
    pub charts_latency_bucket_ms: PositiveU64,

    /// Enable verbose logging (sets log level to debug unless overridden by STREST_LOG/RUST_LOG)
    #[arg(long, short = 'v', alias = "debug", help_heading = "Common Options")]
    pub verbose: bool,

    /// Path to config file (TOML/JSON). Defaults to ./strest.toml or ./strest.json if present.
    #[arg(long, help_heading = "Common Options")]
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

    /// Output file to write results to
    #[arg(long = "output", short = 'o', help_heading = "Common Options")]
    pub output: Option<String>,

    /// Output format
    #[arg(long = "output-format", value_enum, help_heading = "Common Options")]
    pub output_format: Option<OutputFormat>,

    /// Time unit for text output (ns, us, ms, s, m, h)
    #[arg(long = "time-unit", value_enum, help_heading = "Common Options")]
    pub time_unit: Option<TimeUnit>,

    /// Export metrics to CSV (uses the same bounds as charts)
    #[arg(long = "export-csv")]
    pub export_csv: Option<String>,

    /// Export metrics to JSON (uses the same bounds as charts)
    #[arg(long = "export-json")]
    pub export_json: Option<String>,

    /// Export metrics to JSONL (newline-delimited JSON)
    #[arg(long = "export-jsonl")]
    pub export_jsonl: Option<String>,

    /// Write per-request metrics to a sqlite database
    #[arg(long = "db-url")]
    pub db_url: Option<String>,

    /// Number of log shards to use for metrics logging (default: 1)
    #[arg(long = "log-shards", default_value = "1", value_parser = parse_positive_usize)]
    pub log_shards: PositiveUsize,

    /// Disable UI rendering
    #[arg(long = "no-tui", alias = "no-ui", help_heading = "Common Options")]
    pub no_ui: bool,

    /// Skip the startup splash screen
    #[arg(long = "no-splash")]
    pub no_splash: bool,

    /// UI chart window length in milliseconds (default: 10000)
    #[arg(
        long = "ui-window-ms",
        default_value = "10000",
        value_parser = parse_positive_u64,
        help_heading = "Common Options"
    )]
    pub ui_window_ms: PositiveU64,

    /// Print summary at the end of the run (implied by --no-tui)
    #[arg(long = "summary", help_heading = "Common Options")]
    pub summary: bool,

    /// Include a full selection summary in the final output
    #[arg(long = "show-selections")]
    pub show_selections: bool,

    /// Replay a previous run from tmp logs or exported CSV/JSON
    #[arg(long = "replay", help_heading = "Advanced Options")]
    pub replay: bool,

    /// Replay window start (e.g., 10s, 2m, min)
    #[arg(long = "replay-start", help_heading = "Advanced Options")]
    pub replay_start: Option<String>,

    /// Replay window end (e.g., 30s, max)
    #[arg(long = "replay-end", help_heading = "Advanced Options")]
    pub replay_end: Option<String>,

    /// Step size for rewind/forward during replay (supports ms/s/m/h)
    #[arg(long = "replay-step", value_parser = parse_duration_arg, help_heading = "Advanced Options")]
    pub replay_step: Option<Duration>,

    /// Snapshot interval during replay (supports ms/s/m/h)
    #[arg(long = "replay-snapshot-interval", value_parser = parse_duration_arg, help_heading = "Advanced Options")]
    pub replay_snapshot_interval: Option<Duration>,

    /// Snapshot window start during replay (e.g., 10s, min)
    #[arg(long = "replay-snapshot-start", help_heading = "Advanced Options")]
    pub replay_snapshot_start: Option<String>,

    /// Snapshot window end during replay (e.g., 2m, max)
    #[arg(long = "replay-snapshot-end", help_heading = "Advanced Options")]
    pub replay_snapshot_end: Option<String>,

    /// Snapshot output path (defaults to ~/.strest/snapshots)
    #[arg(long = "replay-snapshot-out", help_heading = "Advanced Options")]
    pub replay_snapshot_out: Option<String>,

    /// Snapshot format (json, jsonl, csv)
    #[arg(
        long = "replay-snapshot-format",
        default_value = "json",
        help_heading = "Advanced Options"
    )]
    pub replay_snapshot_format: String,

    /// Minimum TLS version (1.0, 1.1, 1.2, 1.3)
    #[arg(long = "tls-min", value_parser = parse_tls_version)]
    pub tls_min: Option<TlsVersion>,

    /// Maximum TLS version (1.0, 1.1, 1.2, 1.3)
    #[arg(long = "tls-max", value_parser = parse_tls_version)]
    pub tls_max: Option<TlsVersion>,

    /// (TLS) Use the specified certificate file to verify the peer
    #[arg(long = "cacert")]
    pub cacert: Option<String>,

    /// (TLS) Use the specified client certificate file (requires --key)
    #[arg(long = "cert")]
    pub cert: Option<String>,

    /// (TLS) Use the specified client key file (requires --cert)
    #[arg(long = "key")]
    pub key: Option<String>,

    /// (TLS) Accept invalid certs
    #[arg(long = "insecure")]
    pub insecure: bool,

    /// Enable HTTP/2 (adaptive)
    #[arg(long = "http2")]
    pub http2: bool,

    /// Number of parallel HTTP/2 requests per connection
    #[arg(long = "http2-parallel", default_value = "1", value_parser = parse_positive_usize)]
    pub http2_parallel: PositiveUsize,

    /// ALPN protocols to advertise (repeatable, e.g. --alpn h2 --alpn http/1.1)
    #[arg(long = "alpn")]
    pub alpn: Vec<String>,

    /// Proxy URL (optional)
    #[arg(long = "proxy", short = 'p', alias = "proxy-url")]
    pub proxy_url: Option<String>,

    /// Proxy HTTP header (repeatable, 'Key: Value')
    #[arg(long = "proxy-header", value_parser = parse_header)]
    pub proxy_headers: Vec<(String, String)>,

    /// Proxy HTTP version (0.9, 1.0, 1.1, 2)
    #[arg(long = "proxy-http-version", value_enum)]
    pub proxy_http_version: Option<HttpVersion>,

    /// Use HTTP/2 to connect to proxy (shorthand for --proxy-http-version=2)
    #[arg(long = "proxy-http2")]
    pub proxy_http2: bool,

    /// Max number of concurrent request tasks (default: 1000)
    #[arg(
        long = "max-tasks",
        short = 'm',
        aliases = ["concurrency", "connections"],
        default_value = "1000",
        value_parser = parse_positive_usize,
        help_heading = "Common Options"
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
    #[arg(long = "rate", short = 'q', value_parser = parse_positive_u64, required = false, help_heading = "Common Options")]
    pub rate_limit: Option<PositiveU64>,

    /// Burst delay (ignored if --rate is set)
    #[arg(long = "burst-delay", value_parser = parse_duration_arg)]
    pub burst_delay: Option<Duration>,

    /// Burst rate (requests per burst; ignored if --rate is set)
    #[arg(long = "burst-rate", default_value = "1", value_parser = parse_positive_usize)]
    pub burst_rate: PositiveUsize,

    /// Correct latency to avoid coordinated omission (ignored if --rate is not set)
    #[arg(long = "latency-correction")]
    pub latency_correction: bool,

    /// Override DNS resolution and port for a host (repeatable)
    #[arg(long = "connect-to", value_parser = parse_connect_to)]
    pub connect_to: Vec<ConnectToMapping>,

    /// Override the Host header
    #[arg(long = "host")]
    pub host_header: Option<String>,

    /// Lookup only ipv6
    #[arg(long = "ipv6")]
    pub ipv6_only: bool,

    /// Lookup only ipv4
    #[arg(long = "ipv4")]
    pub ipv4_only: bool,

    /// Do not perform a DNS pre-lookup
    #[arg(long = "no-pre-lookup")]
    pub no_pre_lookup: bool,

    /// Disable color output
    #[arg(long = "no-color", env = "NO_COLOR", value_parser = parse_bool_env)]
    pub no_color: bool,

    /// Frame per second for the UI
    #[arg(long = "fps", default_value = "16")]
    pub ui_fps: u32,

    /// Include successful vs non-successful status breakdown in stats
    #[arg(long = "stats-success-breakdown")]
    pub stats_success_breakdown: bool,

    /// Connect to a unix socket instead of TCP (http only)
    #[arg(long = "unix-socket")]
    pub unix_socket: Option<String>,

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

    /// Log RSS periodically when UI is disabled (Linux only, ms)
    #[arg(long = "rss-log-ms", value_parser = parse_positive_u64)]
    pub rss_log_ms: Option<PositiveU64>,

    /// Log allocator stats periodically (requires alloc-profiler feature, ms)
    #[arg(long = "alloc-profiler-ms", value_parser = parse_positive_u64)]
    pub alloc_profiler_ms: Option<PositiveU64>,

    /// Dump jemalloc heap profiles periodically (requires alloc-profiler feature, ms)
    #[arg(long = "alloc-profiler-dump-ms", value_parser = parse_positive_u64)]
    pub alloc_profiler_dump_ms: Option<PositiveU64>,

    /// Directory to write heap profile dumps (requires alloc-profiler feature)
    #[arg(long = "alloc-profiler-dump-path", default_value = "./alloc-prof")]
    pub alloc_profiler_dump_path: String,

    #[arg(skip)]
    pub scenario: Option<Scenario>,

    /// WASM script that generates a scenario definition (experimental)
    #[arg(long = "script")]
    pub script: Option<String>,

    /// WASM lifecycle plugins to load (repeatable, experimental; requires `wasm` feature)
    #[arg(
        long = "plugin",
        alias = "plugin-wasm",
        help_heading = "Advanced Options"
    )]
    pub plugin: Vec<String>,

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
