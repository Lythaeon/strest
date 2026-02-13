use crate::metrics;

use super::PERCENT_DIVISOR;

/// Standard percentile labels.
const PERCENTILE_P50: u64 = 50;
const PERCENTILE_P90: u64 = 90;
const PERCENTILE_P99: u64 = 99;
/// Rounding offset for percentile selection.
const PERCENTILE_ROUNDING: u64 = 50;

pub(crate) fn compute_percentiles(records: &[metrics::MetricRecord]) -> (u64, u64, u64) {
    if records.is_empty() {
        return (0, 0, 0);
    }
    let mut latencies: Vec<u64> = records.iter().map(|record| record.latency_ms).collect();
    latencies.sort_unstable();

    let p50 = percentile(&latencies, PERCENTILE_P50);
    let p90 = percentile(&latencies, PERCENTILE_P90);
    let p99 = percentile(&latencies, PERCENTILE_P99);

    (p50, p90, p99)
}

fn percentile(values: &[u64], percentile: u64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let count = values.len().saturating_sub(1) as u64;
    let index = percentile
        .saturating_mul(count)
        .saturating_add(PERCENTILE_ROUNDING)
        .checked_div(PERCENT_DIVISOR)
        .unwrap_or(0);
    let idx = usize::try_from(index).unwrap_or_else(|_| values.len().saturating_sub(1));
    *values.get(idx).unwrap_or(&0)
}
