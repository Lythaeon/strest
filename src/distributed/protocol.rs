use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::args::{HttpMethod, TlsVersion};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(super) enum WireMessage {
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
pub(super) struct HelloMessage {
    pub(super) agent_id: String,
    pub(super) hostname: String,
    pub(super) cpu_cores: usize,
    pub(super) weight: u64,
    pub(super) auth_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ConfigMessage {
    pub(super) run_id: String,
    pub(super) args: WireArgs,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StartMessage {
    pub(super) run_id: String,
    pub(super) start_after_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StopMessage {
    pub(super) run_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct HeartbeatMessage {
    pub(super) sent_at_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ReportMessage {
    pub(super) run_id: String,
    pub(super) agent_id: String,
    pub(super) summary: WireSummary,
    pub(super) histogram_b64: String,
    pub(super) runtime_errors: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct StreamMessage {
    pub(super) run_id: String,
    pub(super) agent_id: String,
    pub(super) summary: WireSummary,
    pub(super) histogram_b64: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ErrorMessage {
    pub(super) message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct WireArgs {
    pub(super) method: HttpMethod,
    pub(super) url: Option<String>,
    pub(super) headers: Vec<(String, String)>,
    pub(super) data: String,
    pub(super) target_duration: u64,
    pub(super) expected_status_code: u16,
    pub(super) request_timeout_ms: u64,
    pub(super) charts_path: String,
    pub(super) no_charts: bool,
    #[serde(default)]
    pub(super) verbose: bool,
    pub(super) tmp_path: String,
    pub(super) keep_tmp: bool,
    pub(super) warmup_ms: Option<u64>,
    pub(super) export_csv: Option<String>,
    pub(super) export_json: Option<String>,
    pub(super) log_shards: usize,
    pub(super) no_ui: bool,
    pub(super) summary: bool,
    pub(super) proxy_url: Option<String>,
    pub(super) max_tasks: usize,
    pub(super) spawn_rate_per_tick: usize,
    pub(super) tick_interval: u64,
    pub(super) rate_limit: Option<u64>,
    pub(super) load_profile: Option<WireLoadProfile>,
    pub(super) metrics_range: Option<(u64, u64)>,
    pub(super) metrics_max: usize,
    pub(super) scenario: Option<WireScenario>,
    pub(super) tls_min: Option<TlsVersion>,
    pub(super) tls_max: Option<TlsVersion>,
    pub(super) http2: bool,
    #[serde(default)]
    pub(super) http3: bool,
    pub(super) alpn: Vec<String>,
    #[serde(default)]
    pub(super) stream_summaries: bool,
    #[serde(default)]
    pub(super) stream_interval_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct WireLoadProfile {
    pub(super) initial_rpm: u64,
    pub(super) stages: Vec<WireLoadStage>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct WireLoadStage {
    pub(super) duration_secs: u64,
    pub(super) target_rpm: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct WireScenario {
    pub(super) base_url: Option<String>,
    pub(super) vars: BTreeMap<String, String>,
    pub(super) steps: Vec<WireScenarioStep>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct WireScenarioStep {
    pub(super) name: Option<String>,
    pub(super) method: HttpMethod,
    pub(super) url: Option<String>,
    pub(super) path: Option<String>,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Option<String>,
    pub(super) assert_status: Option<u16>,
    pub(super) assert_body_contains: Option<String>,
    pub(super) think_time_ms: Option<u64>,
    pub(super) vars: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(super) struct WireSummary {
    pub(super) duration_ms: u64,
    pub(super) total_requests: u64,
    pub(super) successful_requests: u64,
    pub(super) error_requests: u64,
    pub(super) min_latency_ms: u64,
    pub(super) max_latency_ms: u64,
    #[serde(with = "serde_u128")]
    pub(super) latency_sum_ms: u128,
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

pub(super) async fn read_message(
    reader: &mut BufReader<tokio::net::tcp::OwnedReadHalf>,
) -> Result<WireMessage, String> {
    const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
    let mut buffer: Vec<u8> = Vec::with_capacity(1024);
    let bytes = reader
        .read_until(b'\n', &mut buffer)
        .await
        .map_err(|err| format!("Failed to read message: {}", err))?;
    if bytes == 0 {
        return Err("Connection closed.".to_owned());
    }
    if buffer.len() > MAX_MESSAGE_BYTES {
        return Err(format!(
            "Message exceeded max size ({} bytes).",
            MAX_MESSAGE_BYTES
        ));
    }
    if buffer.ends_with(b"\n") {
        buffer.pop();
        if buffer.ends_with(b"\r") {
            buffer.pop();
        }
    }
    let line =
        std::str::from_utf8(&buffer).map_err(|err| format!("Invalid UTF-8 message: {}", err))?;
    serde_json::from_str::<WireMessage>(line).map_err(|err| format!("Invalid message: {}", err))
}

pub(super) async fn send_message(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    message: &WireMessage,
) -> Result<(), String> {
    let mut payload =
        serde_json::to_string(message).map_err(|err| format!("Encode failed: {}", err))?;
    payload.push('\n');
    writer
        .write_all(payload.as_bytes())
        .await
        .map_err(|err| format!("Failed to send message: {}", err))
}
