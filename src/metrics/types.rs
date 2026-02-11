use std::collections::BTreeMap;
use std::ops::RangeInclusive;

use crate::error::ValidationError;
use std::time::Duration;

use tokio::time::Instant;

#[derive(Clone, Copy, Debug)]
pub struct Metrics {
    pub start: Instant,
    pub response_time: Duration,
    pub status_code: u16,
    pub timed_out: bool,
    pub transport_error: bool,
    pub response_bytes: u64,
    pub in_flight_ops: u64,
}

impl Metrics {
    #[must_use]
    pub fn new(
        start: Instant,
        status_code: u16,
        timed_out: bool,
        transport_error: bool,
        response_bytes: u64,
        in_flight_ops: u64,
    ) -> Self {
        Self {
            start,
            response_time: start.elapsed(),
            status_code,
            timed_out,
            transport_error,
            response_bytes,
            in_flight_ops,
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
    pub transport_errors: u64,
    pub non_expected_status: u64,
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
    pub transport_errors: u64,
    pub non_expected_status: u64,
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
    pub timed_out: bool,
    pub transport_error: bool,
}

#[derive(Debug, Clone)]
pub struct MetricsRange(pub RangeInclusive<u64>);

#[derive(Debug)]
pub struct StreamingChartData {
    pub avg_buckets: BTreeMap<u64, (u128, u64)>,
    pub total_buckets: BTreeMap<u64, u64>,
    pub success_buckets: BTreeMap<u64, u64>,
    pub error_buckets: BTreeMap<u64, u64>,
    pub rps_counts: Vec<u32>,
    pub timeouts: Vec<u32>,
    pub transports: Vec<u32>,
    pub non_expected: Vec<u32>,
    pub status_2xx: Vec<u32>,
    pub status_3xx: Vec<u32>,
    pub status_4xx: Vec<u32>,
    pub status_5xx: Vec<u32>,
    pub status_other: Vec<u32>,
    pub inflight: Vec<u32>,
    pub latency_buckets_ms: Vec<u64>,
    pub latency_bucket_ms: u64,
    pub p50: Vec<u64>,
    pub p90: Vec<u64>,
    pub p99: Vec<u64>,
    pub p50_ok: Vec<u64>,
    pub p90_ok: Vec<u64>,
    pub p99_ok: Vec<u64>,
}

impl std::str::FromStr for MetricsRange {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (start_str, end_str) = s
            .split_once('-')
            .ok_or(ValidationError::MetricsRangeFormat)?;
        let start: u64 = start_str
            .parse()
            .map_err(|err| ValidationError::MetricsRangeInvalidStart { source: err })?;
        let end: u64 = end_str
            .parse()
            .map_err(|err| ValidationError::MetricsRangeInvalidEnd { source: err })?;
        if start > end {
            return Err(ValidationError::MetricsRangeStartAfterEnd);
        }
        Ok(MetricsRange(start..=end))
    }
}
