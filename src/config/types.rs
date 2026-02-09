use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;

use crate::args::{HttpMethod, OutputFormat};
use crate::sinks::config::SinksConfig;

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    pub method: Option<HttpMethod>,
    pub url: Option<String>,
    pub headers: Option<Vec<String>>,
    pub accept: Option<String>,
    pub content_type: Option<String>,
    pub data: Option<String>,
    pub data_file: Option<String>,
    pub data_lines: Option<String>,
    pub basic_auth: Option<String>,
    pub aws_session: Option<String>,
    pub aws_sigv4: Option<String>,
    pub duration: Option<u64>,
    pub requests: Option<u64>,
    pub timeout: Option<DurationValue>,
    pub connect_timeout: Option<DurationValue>,
    pub warmup: Option<DurationValue>,
    pub status: Option<u16>,
    pub redirect: Option<u32>,
    pub disable_keepalive: Option<bool>,
    pub disable_compression: Option<bool>,
    pub charts_path: Option<String>,
    pub no_charts: Option<bool>,
    pub no_ua: Option<bool>,
    pub authorized: Option<bool>,
    pub tmp_path: Option<String>,
    pub keep_tmp: Option<bool>,
    pub output: Option<String>,
    pub output_format: Option<OutputFormat>,
    pub export_csv: Option<String>,
    pub export_json: Option<String>,
    pub export_jsonl: Option<String>,
    pub db_url: Option<String>,
    pub log_shards: Option<usize>,
    pub no_ui: Option<bool>,
    pub ui_window_ms: Option<u64>,
    pub summary: Option<bool>,
    pub tls_min: Option<crate::args::TlsVersion>,
    pub tls_max: Option<crate::args::TlsVersion>,
    pub cacert: Option<String>,
    pub cert: Option<String>,
    pub key: Option<String>,
    pub insecure: Option<bool>,
    pub http2: Option<bool>,
    pub http3: Option<bool>,
    pub http_version: Option<crate::args::HttpVersion>,
    pub alpn: Option<Vec<String>>,
    #[serde(alias = "proxy")]
    pub proxy_url: Option<String>,
    pub proxy_headers: Option<Vec<String>>,
    pub proxy_http_version: Option<crate::args::HttpVersion>,
    pub proxy_http2: Option<bool>,
    #[serde(alias = "concurrency", alias = "connections")]
    pub max_tasks: Option<usize>,
    pub spawn_rate: Option<usize>,
    pub spawn_interval: Option<u64>,
    pub rate: Option<u64>,
    pub rpm: Option<u64>,
    pub connect_to: Option<Vec<String>>,
    pub host: Option<String>,
    pub ipv6: Option<bool>,
    pub ipv4: Option<bool>,
    pub no_pre_lookup: Option<bool>,
    pub no_color: Option<bool>,
    pub fps: Option<u32>,
    pub stats_success_breakdown: Option<bool>,
    pub unix_socket: Option<String>,
    pub load: Option<LoadConfig>,
    pub metrics_range: Option<String>,
    pub metrics_max: Option<usize>,
    pub scenario: Option<ScenarioConfig>,
    pub scenarios: Option<BTreeMap<String, ScenarioConfig>>,
    pub script: Option<String>,
    pub sinks: Option<SinksConfig>,
    pub distributed: Option<DistributedConfig>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LoadConfig {
    pub rate: Option<u64>,
    pub rpm: Option<u64>,
    pub stages: Option<Vec<LoadStageConfig>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct LoadStageConfig {
    pub duration: String,
    pub target: Option<u64>,
    pub rate: Option<u64>,
    pub rpm: Option<u64>,
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct ScenarioConfig {
    pub schema_version: Option<u32>,
    pub base_url: Option<String>,
    pub method: Option<HttpMethod>,
    pub headers: Option<Vec<String>>,
    pub data: Option<String>,
    pub vars: Option<BTreeMap<String, String>>,
    pub steps: Vec<ScenarioStepConfig>,
}

pub const SCENARIO_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Default, Deserialize, Clone)]
pub struct ScenarioStepConfig {
    pub name: Option<String>,
    pub method: Option<HttpMethod>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub headers: Option<Vec<String>>,
    pub data: Option<String>,
    pub assert_status: Option<u16>,
    pub assert_body_contains: Option<String>,
    pub think_time: Option<DurationValue>,
    pub vars: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DistributedConfig {
    pub role: Option<String>,
    pub controller_mode: Option<crate::args::ControllerMode>,
    pub listen: Option<String>,
    pub control_listen: Option<String>,
    pub control_auth_token: Option<String>,
    pub join: Option<String>,
    pub auth_token: Option<String>,
    pub agent_id: Option<String>,
    pub weight: Option<u64>,
    pub min_agents: Option<usize>,
    pub agent_wait_timeout_ms: Option<u64>,
    pub agent_standby: Option<bool>,
    pub agent_reconnect_ms: Option<u64>,
    pub agent_heartbeat_interval_ms: Option<u64>,
    pub agent_heartbeat_timeout_ms: Option<u64>,
    pub stream_summaries: Option<bool>,
    pub stream_interval_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum DurationValue {
    Seconds(u64),
    Text(String),
}

impl DurationValue {
    pub(crate) fn to_duration(&self) -> Result<Duration, String> {
        match self {
            DurationValue::Seconds(secs) => {
                if *secs == 0 {
                    Err("Duration must be > 0.".to_owned())
                } else {
                    Ok(Duration::from_secs(*secs))
                }
            }
            DurationValue::Text(text) => super::parse_duration_value(text),
        }
    }
}
