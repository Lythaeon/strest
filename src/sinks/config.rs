use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct SinksConfig {
    pub update_interval_ms: Option<u64>,
    pub prometheus: Option<PrometheusSinkConfig>,
    pub otel: Option<OtelSinkConfig>,
    pub influx: Option<InfluxSinkConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrometheusSinkConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OtelSinkConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InfluxSinkConfig {
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct SinkStats {
    pub duration: Duration,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub error_requests: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub avg_latency_ms: u64,
    pub p50_latency_ms: u64,
    pub p90_latency_ms: u64,
    pub p99_latency_ms: u64,
    pub success_rate_x100: u64,
    pub avg_rps_x100: u64,
    pub avg_rpm_x100: u64,
}
