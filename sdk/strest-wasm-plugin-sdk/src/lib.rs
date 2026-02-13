use std::io::{Read, Write};

use serde::Deserialize;

pub const API_VERSION: i32 = 1;
pub const ENV_API_VERSION: &str = "STREST_PLUGIN_API_VERSION";
pub const ENV_HOOK: &str = "STREST_PLUGIN_HOOK";

#[derive(Debug, Clone, Deserialize)]
pub struct RunStartPayload {
    pub event: String,
    pub protocol: String,
    pub load_mode: String,
    pub url: Option<String>,
    pub duration_seconds: u64,
    pub max_tasks: usize,
    pub summary: bool,
    pub no_ui: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsSummaryPayload {
    pub event: String,
    pub duration_ms: u64,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub error_requests: u64,
    pub timeout_requests: u64,
    pub transport_errors: u64,
    pub non_expected_status: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub avg_latency_ms: u64,
    pub success_min_latency_ms: u64,
    pub success_max_latency_ms: u64,
    pub success_avg_latency_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactPayload {
    pub event: String,
    pub kind: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RunEndPayload {
    pub event: String,
    pub runtime_error_count: usize,
    pub runtime_errors: Vec<String>,
}

pub trait Plugin {
    fn on_run_start(&mut self, _payload: &RunStartPayload) -> Result<(), String> {
        Ok(())
    }

    fn on_metrics_summary(&mut self, _payload: &MetricsSummaryPayload) -> Result<(), String> {
        Ok(())
    }

    fn on_artifact(&mut self, _payload: &ArtifactPayload) -> Result<(), String> {
        Ok(())
    }

    fn on_run_end(&mut self, _payload: &RunEndPayload) -> Result<(), String> {
        Ok(())
    }
}

pub fn run_plugin<P: Plugin>(plugin: &mut P) -> i32 {
    match run_plugin_inner(plugin) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(&mut std::io::stderr(), "{}", message);
            1
        }
    }
}

fn run_plugin_inner<P: Plugin>(plugin: &mut P) -> Result<(), String> {
    let api_version = std::env::var(ENV_API_VERSION)
        .map_err(|err| format!("missing {} env: {}", ENV_API_VERSION, err))?;
    let parsed_api_version = api_version
        .parse::<i32>()
        .map_err(|err| format!("invalid {} value: {}", ENV_API_VERSION, err))?;
    if parsed_api_version != API_VERSION {
        return Err(format!(
            "unsupported API version {} (expected {})",
            parsed_api_version, API_VERSION
        ));
    }

    let hook = std::env::var(ENV_HOOK).map_err(|err| format!("missing {} env: {}", ENV_HOOK, err))?;
    let mut payload = String::new();
    std::io::stdin()
        .read_to_string(&mut payload)
        .map_err(|err| format!("failed reading payload from stdin: {}", err))?;

    match hook.as_str() {
        "strest_on_run_start" => {
            let payload: RunStartPayload = parse_payload(&payload, "RunStartPayload")?;
            plugin.on_run_start(&payload)
        }
        "strest_on_metrics_summary" => {
            let payload: MetricsSummaryPayload = parse_payload(&payload, "MetricsSummaryPayload")?;
            plugin.on_metrics_summary(&payload)
        }
        "strest_on_artifact" => {
            let payload: ArtifactPayload = parse_payload(&payload, "ArtifactPayload")?;
            plugin.on_artifact(&payload)
        }
        "strest_on_run_end" => {
            let payload: RunEndPayload = parse_payload(&payload, "RunEndPayload")?;
            plugin.on_run_end(&payload)
        }
        unknown => Err(format!("unknown hook '{}'", unknown)),
    }
}

fn parse_payload<T>(payload: &str, type_name: &'static str) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(payload).map_err(|err| format!("failed to parse {}: {}", type_name, err))
}
