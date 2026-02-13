use std::collections::BTreeMap;

use crate::metrics::AggregatedMetricSample;

fn sorted_samples(samples: &[AggregatedMetricSample]) -> Vec<AggregatedMetricSample> {
    let mut out = samples.to_vec();
    out.sort_by_key(|sample| sample.elapsed_ms);
    out
}

fn bucket_last_value(
    samples: &[AggregatedMetricSample],
    bucket_ms: u64,
    value: fn(&AggregatedMetricSample) -> u64,
) -> Vec<(u64, u64)> {
    let mut buckets: BTreeMap<u64, u64> = BTreeMap::new();
    let bucket_ms = bucket_ms.max(1);
    for sample in samples {
        let bucket = sample.elapsed_ms.checked_div(bucket_ms).unwrap_or(0);
        buckets.insert(bucket, value(sample));
    }
    buckets.into_iter().collect()
}

pub(super) fn bucket_last_value_u64(
    samples: &[AggregatedMetricSample],
    bucket_ms: u64,
    value: fn(&AggregatedMetricSample) -> u64,
) -> Vec<(u64, u64)> {
    bucket_last_value(samples, bucket_ms, value)
}

pub(super) fn compute_rps_series(samples: &[AggregatedMetricSample]) -> Vec<(u64, u64)> {
    let sorted = sorted_samples(samples);
    let mut buckets: BTreeMap<u64, u64> = BTreeMap::new();
    for window in sorted.windows(2) {
        let Some(prev) = window.first() else { continue };
        let Some(curr) = window.get(1) else { continue };
        if curr.elapsed_ms <= prev.elapsed_ms {
            continue;
        }
        let delta = curr.total_requests.saturating_sub(prev.total_requests);
        let delta_ms = curr.elapsed_ms.saturating_sub(prev.elapsed_ms).max(1);
        let rps = u64::try_from(
            u128::from(delta)
                .saturating_mul(1000)
                .checked_div(u128::from(delta_ms))
                .unwrap_or(0),
        )
        .unwrap_or(u64::MAX);
        let sec_bucket = curr.elapsed_ms.checked_div(1000).unwrap_or(0);
        buckets.insert(sec_bucket, rps);
    }
    buckets.into_iter().collect()
}
