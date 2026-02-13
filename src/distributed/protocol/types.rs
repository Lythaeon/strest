use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::args::{HttpMethod, LoadMode, Protocol, TlsVersion};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(in crate::distributed) enum WireMessage {
    Hello(HelloMessage),
    Config(Box<ConfigMessage>),
    Start(StartMessage),
    Stop(StopMessage),
    Heartbeat(HeartbeatMessage),
    Stream(Box<StreamMessage>),
    Report(Box<ReportMessage>),
    Error(ErrorMessage),
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct HelloMessage {
    pub(in crate::distributed) agent_id: String,
    pub(in crate::distributed) hostname: String,
    pub(in crate::distributed) cpu_cores: usize,
    pub(in crate::distributed) weight: u64,
    pub(in crate::distributed) auth_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct ConfigMessage {
    pub(in crate::distributed) run_id: String,
    pub(in crate::distributed) args: WireArgs,
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct StartMessage {
    pub(in crate::distributed) run_id: String,
    pub(in crate::distributed) start_after_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct StopMessage {
    pub(in crate::distributed) run_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct HeartbeatMessage {
    pub(in crate::distributed) sent_at_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct ReportMessage {
    pub(in crate::distributed) run_id: String,
    pub(in crate::distributed) agent_id: String,
    pub(in crate::distributed) summary: WireSummary,
    pub(in crate::distributed) histogram_b64: String,
    #[serde(default)]
    pub(in crate::distributed) success_histogram_b64: Option<String>,
    pub(in crate::distributed) runtime_errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct StreamMessage {
    pub(in crate::distributed) run_id: String,
    pub(in crate::distributed) agent_id: String,
    pub(in crate::distributed) summary: WireSummary,
    pub(in crate::distributed) histogram_b64: String,
    #[serde(default)]
    pub(in crate::distributed) success_histogram_b64: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::distributed) struct ErrorMessage {
    pub(in crate::distributed) message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(in crate::distributed) struct WireArgs {
    pub(in crate::distributed) method: HttpMethod,
    #[serde(default = "default_protocol")]
    pub(in crate::distributed) protocol: Protocol,
    #[serde(default = "default_load_mode")]
    pub(in crate::distributed) load_mode: LoadMode,
    pub(in crate::distributed) url: Option<String>,
    pub(in crate::distributed) headers: Vec<(String, String)>,
    pub(in crate::distributed) data: String,
    pub(in crate::distributed) target_duration: u64,
    pub(in crate::distributed) expected_status_code: u16,
    pub(in crate::distributed) request_timeout_ms: u64,
    pub(in crate::distributed) charts_path: String,
    pub(in crate::distributed) no_charts: bool,
    #[serde(default)]
    pub(in crate::distributed) verbose: bool,
    pub(in crate::distributed) tmp_path: String,
    pub(in crate::distributed) keep_tmp: bool,
    pub(in crate::distributed) warmup_ms: Option<u64>,
    pub(in crate::distributed) export_csv: Option<String>,
    pub(in crate::distributed) export_json: Option<String>,
    pub(in crate::distributed) log_shards: usize,
    pub(in crate::distributed) no_ui: bool,
    pub(in crate::distributed) summary: bool,
    pub(in crate::distributed) proxy_url: Option<String>,
    pub(in crate::distributed) max_tasks: usize,
    pub(in crate::distributed) spawn_rate_per_tick: usize,
    pub(in crate::distributed) tick_interval: u64,
    pub(in crate::distributed) rate_limit: Option<u64>,
    pub(in crate::distributed) load_profile: Option<WireLoadProfile>,
    pub(in crate::distributed) metrics_range: Option<(u64, u64)>,
    pub(in crate::distributed) metrics_max: usize,
    pub(in crate::distributed) scenario: Option<WireScenario>,
    pub(in crate::distributed) tls_min: Option<TlsVersion>,
    pub(in crate::distributed) tls_max: Option<TlsVersion>,
    pub(in crate::distributed) http2: bool,
    #[serde(default)]
    pub(in crate::distributed) http3: bool,
    pub(in crate::distributed) alpn: Vec<String>,
    #[serde(default)]
    pub(in crate::distributed) stream_summaries: bool,
    #[serde(default)]
    pub(in crate::distributed) stream_interval_ms: Option<u64>,
}

const fn default_protocol() -> Protocol {
    Protocol::Http
}

const fn default_load_mode() -> LoadMode {
    LoadMode::Arrival
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(in crate::distributed) struct WireLoadProfile {
    pub(in crate::distributed) initial_rpm: u64,
    pub(in crate::distributed) stages: Vec<WireLoadStage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(in crate::distributed) struct WireLoadStage {
    pub(in crate::distributed) duration_secs: u64,
    pub(in crate::distributed) target_rpm: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(in crate::distributed) struct WireScenario {
    pub(in crate::distributed) base_url: Option<String>,
    pub(in crate::distributed) vars: BTreeMap<String, String>,
    pub(in crate::distributed) steps: Vec<WireScenarioStep>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(in crate::distributed) struct WireScenarioStep {
    pub(in crate::distributed) name: Option<String>,
    pub(in crate::distributed) method: HttpMethod,
    pub(in crate::distributed) url: Option<String>,
    pub(in crate::distributed) path: Option<String>,
    pub(in crate::distributed) headers: Vec<(String, String)>,
    pub(in crate::distributed) body: Option<String>,
    pub(in crate::distributed) assert_status: Option<u16>,
    pub(in crate::distributed) assert_body_contains: Option<String>,
    pub(in crate::distributed) think_time_ms: Option<u64>,
    pub(in crate::distributed) vars: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(in crate::distributed) struct WireSummary {
    pub(in crate::distributed) duration_ms: u64,
    pub(in crate::distributed) total_requests: u64,
    pub(in crate::distributed) successful_requests: u64,
    pub(in crate::distributed) error_requests: u64,
    #[serde(default)]
    pub(in crate::distributed) timeout_requests: u64,
    #[serde(default)]
    pub(in crate::distributed) transport_errors: u64,
    #[serde(default)]
    pub(in crate::distributed) non_expected_status: u64,
    #[serde(default)]
    pub(in crate::distributed) success_min_latency_ms: u64,
    #[serde(default)]
    pub(in crate::distributed) success_max_latency_ms: u64,
    #[serde(default, with = "serde_u128")]
    pub(in crate::distributed) success_latency_sum_ms: u128,
    pub(in crate::distributed) min_latency_ms: u64,
    pub(in crate::distributed) max_latency_ms: u64,
    #[serde(with = "serde_u128")]
    pub(in crate::distributed) latency_sum_ms: u128,
}

mod serde_u128 {
    use serde::{Deserializer, Serializer, de};

    pub fn serialize<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl<'de> de::Visitor<'de> for Visitor {
            type Value = u128;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a u128 encoded as string or a non-negative integer")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(u128::from(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                value
                    .parse::<u128>()
                    .map_err(|err| de::Error::custom(format!("Invalid u128: {}", err)))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}
