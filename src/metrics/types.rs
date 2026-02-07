use std::ops::RangeInclusive;
use std::time::Duration;

use tokio::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub start: Instant,
    pub response_time: Duration,
    pub status_code: u16,
    pub timed_out: bool,
}

impl Metrics {
    #[must_use]
    pub fn new(start: Instant, status_code: u16, timed_out: bool) -> Self {
        Self {
            start,
            response_time: start.elapsed(),
            status_code,
            timed_out,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MetricsSummary {
    pub duration: Duration,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub error_requests: u64,
    pub timeout_requests: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub avg_latency_ms: u64,
    pub success_min_latency_ms: u64,
    pub success_max_latency_ms: u64,
    pub success_avg_latency_ms: u64,
}

#[derive(Debug)]
pub struct MetricsReport {
    pub summary: MetricsSummary,
}

#[derive(Debug, Clone)]
pub struct StreamSnapshot {
    pub duration: Duration,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub error_requests: u64,
    pub timeout_requests: u64,
    pub min_latency_ms: u64,
    pub max_latency_ms: u64,
    pub latency_sum_ms: u128,
    pub success_min_latency_ms: u64,
    pub success_max_latency_ms: u64,
    pub success_latency_sum_ms: u128,
    pub histogram_b64: String,
}

#[derive(Debug, Clone)]
pub struct AggregatedMetricSample {
    pub elapsed_ms: u64,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub error_requests: u64,
    pub avg_latency_ms: u64,
    pub p50_latency_ms: u64,
    pub p90_latency_ms: u64,
    pub p99_latency_ms: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct MetricRecord {
    pub elapsed_ms: u64,
    pub latency_ms: u64,
    pub status_code: u16,
}

#[derive(Debug, Clone)]
pub struct MetricsRange(pub RangeInclusive<u64>);

impl std::str::FromStr for MetricsRange {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start_str, end_str) = s
            .split_once('-')
            .ok_or_else(|| "Expected format start-end (e.g., 10-30)".to_owned())?;
        let start: u64 = start_str
            .parse()
            .map_err(|err| format!("Invalid start value: {}", err))?;
        let end: u64 = end_str
            .parse()
            .map_err(|err| format!("Invalid end value: {}", err))?;
        if start > end {
            return Err("Start must be <= end".to_owned());
        }
        Ok(MetricsRange(start..=end))
    }
}
