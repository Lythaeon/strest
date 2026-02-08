use std::collections::BTreeMap;
use std::time::Duration;

use serde::Deserialize;

use crate::args::HttpMethod;
use crate::sinks::config::SinksConfig;

#[derive(Debug, Default, Deserialize)]
pub struct ConfigFile {
    pub method: Option<HttpMethod>,
    pub url: Option<String>,
    pub headers: Option<Vec<String>>,
    pub data: Option<String>,
    pub duration: Option<u64>,
    pub timeout: Option<DurationValue>,
    pub warmup: Option<DurationValue>,
    pub status: Option<u16>,
    pub charts_path: Option<String>,
    pub no_charts: Option<bool>,
    pub tmp_path: Option<String>,
    pub keep_tmp: Option<bool>,
    pub export_csv: Option<String>,
    pub export_json: Option<String>,
    pub log_shards: Option<usize>,
    pub no_ui: Option<bool>,
    pub ui_window_ms: Option<u64>,
    pub summary: Option<bool>,
    pub tls_min: Option<crate::args::TlsVersion>,
    pub tls_max: Option<crate::args::TlsVersion>,
    pub http2: Option<bool>,
    pub http3: Option<bool>,
    pub alpn: Option<Vec<String>>,
    #[serde(alias = "proxy")]
    pub proxy_url: Option<String>,
    #[serde(alias = "concurrency")]
    pub max_tasks: Option<usize>,
    pub spawn_rate: Option<usize>,
    pub spawn_interval: Option<u64>,
    pub rate: Option<u64>,
    pub rpm: Option<u64>,
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
