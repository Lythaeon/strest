use std::collections::VecDeque;
use std::time::Duration;

use tokio::time::Instant;

pub(in crate::metrics::collector) fn prune_latency_window(
    window: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    window_span: Duration,
) {
    while window
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > window_span)
    {
        window.pop_front();
    }
}

pub(in crate::metrics::collector) fn prune_rps_window(
    window: &mut VecDeque<(Instant, u64)>,
    now: Instant,
) {
    while window
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > Duration::from_secs(60))
    {
        window.pop_front();
    }
}

pub(in crate::metrics::collector) fn prune_bytes_window(
    window: &mut VecDeque<(Instant, u64)>,
    now: Instant,
) {
    while window
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > Duration::from_secs(60))
    {
        window.pop_front();
    }
}

pub(in crate::metrics::collector) fn record_rps_sample(
    samples: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    rps: u64,
    window_span: Duration,
) {
    samples.push_back((now, rps));
    prune_rps_samples(samples, now, window_span);
}

fn prune_rps_samples(samples: &mut VecDeque<(Instant, u64)>, now: Instant, window_span: Duration) {
    while samples
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > window_span)
    {
        samples.pop_front();
    }
}

pub(in crate::metrics::collector) fn record_bytes_sample(
    samples: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    bytes_per_sec: u64,
    window_span: Duration,
) {
    samples.push_back((now, bytes_per_sec));
    prune_bytes_samples(samples, now, window_span);
}

fn prune_bytes_samples(
    samples: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    window_span: Duration,
) {
    while samples
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > window_span)
    {
        samples.pop_front();
    }
}

pub(in crate::metrics::collector) fn compute_percentiles(
    window: &VecDeque<(Instant, u64)>,
) -> (u64, u64, u64) {
    if window.is_empty() {
        return (0, 0, 0);
    }

    let mut values: Vec<u64> = window.iter().map(|(_, latency)| *latency).collect();
    values.sort_unstable();

    let p50 = percentile(&values, 50);
    let p90 = percentile(&values, 90);
    let p99 = percentile(&values, 99);

    (p50, p90, p99)
}

fn percentile(data: &[u64], percentile: u64) -> u64 {
    if data.is_empty() {
        return 0;
    }
    let count = data.len().saturating_sub(1) as u64;
    let index = (percentile.saturating_mul(count).saturating_add(50) / 100) as usize;
    *data.get(index).unwrap_or(&0)
}
