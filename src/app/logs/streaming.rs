use std::collections::BTreeMap;
use std::path::PathBuf;

use tokio::io::BufReader;

use crate::error::{AppError, AppResult, MetricsError};
use crate::metrics;

use super::parsing::{
    HeapItem, LogCursor, PercentileSeries, ensure_len, inc_slot, read_next_record,
};

pub(super) async fn load_chart_data_streaming(
    paths: &[PathBuf],
    expected_status_code: u16,
    metrics_range: &Option<metrics::MetricsRange>,
    latency_bucket_ms: u64,
) -> AppResult<metrics::StreamingChartData> {
    let mut cursors: Vec<LogCursor> = Vec::with_capacity(paths.len());
    for path in paths {
        let file = tokio::fs::File::open(path).await.map_err(|err| {
            AppError::metrics(MetricsError::Io {
                context: "open metrics log",
                source: err,
            })
        })?;
        cursors.push(LogCursor {
            reader: BufReader::new(file),
            line: String::new(),
        });
    }

    let mut heap: std::collections::BinaryHeap<std::cmp::Reverse<HeapItem>> =
        std::collections::BinaryHeap::new();

    for (idx, cursor) in cursors.iter_mut().enumerate() {
        if let Some(record) = read_next_record(cursor).await? {
            heap.push(std::cmp::Reverse(HeapItem {
                elapsed_ms: record.elapsed_ms,
                idx,
                record,
            }));
        }
    }

    let mut avg_buckets: BTreeMap<u64, (u128, u64)> = BTreeMap::new();
    let mut total_buckets: BTreeMap<u64, u64> = BTreeMap::new();
    let mut success_buckets: BTreeMap<u64, u64> = BTreeMap::new();
    let mut error_buckets: BTreeMap<u64, u64> = BTreeMap::new();
    let mut rps_counts: Vec<u32> = Vec::new();
    let mut timeouts: Vec<u32> = Vec::new();
    let mut transports: Vec<u32> = Vec::new();
    let mut non_expected: Vec<u32> = Vec::new();
    let mut status_2xx: Vec<u32> = Vec::new();
    let mut status_3xx: Vec<u32> = Vec::new();
    let mut status_4xx: Vec<u32> = Vec::new();
    let mut status_5xx: Vec<u32> = Vec::new();
    let mut status_other: Vec<u32> = Vec::new();
    let mut inflight_deltas: Vec<i64> = Vec::new();

    let mut latency_buckets_ms: Vec<u64> = Vec::new();
    let mut p50: Vec<u64> = Vec::new();
    let mut p90: Vec<u64> = Vec::new();
    let mut p99: Vec<u64> = Vec::new();
    let mut p50_ok: Vec<u64> = Vec::new();
    let mut p90_ok: Vec<u64> = Vec::new();
    let mut p99_ok: Vec<u64> = Vec::new();

    let bucket_ms = latency_bucket_ms.max(1);
    let mut current_bucket: Option<u64> = None;
    let mut latencies: Vec<u64> = Vec::new();
    let mut latencies_ok: Vec<u64> = Vec::new();
    let mut series = PercentileSeries {
        latency_seconds: &mut latency_buckets_ms,
        p50: &mut p50,
        p90: &mut p90,
        p99: &mut p99,
        p50_ok: &mut p50_ok,
        p90_ok: &mut p90_ok,
        p99_ok: &mut p99_ok,
    };

    while let Some(std::cmp::Reverse(item)) = heap.pop() {
        let record = item.record;
        let sec = record.elapsed_ms / 1000;
        let bucket = record.elapsed_ms.checked_div(bucket_ms).unwrap_or(0);

        if let Some(metrics::MetricsRange(range)) = metrics_range.as_ref()
            && !range.contains(&sec)
        {
            if let Some(cursor) = cursors.get_mut(item.idx)
                && let Some(next) = read_next_record(cursor).await?
            {
                heap.push(std::cmp::Reverse(HeapItem {
                    elapsed_ms: next.elapsed_ms,
                    idx: item.idx,
                    record: next,
                }));
            }
            continue;
        }

        match current_bucket {
            Some(active) if bucket != active => {
                series.push_percentiles_for_sec(
                    active.saturating_mul(bucket_ms),
                    &mut latencies,
                    &mut latencies_ok,
                );
                current_bucket = Some(bucket);
            }
            None => current_bucket = Some(bucket),
            _ => {}
        }

        let bucket_100ms = record.elapsed_ms / 100;
        let entry = avg_buckets.entry(bucket_100ms).or_insert((0, 0));
        entry.0 = entry.0.saturating_add(u128::from(record.latency_ms));
        entry.1 = entry.1.saturating_add(1);

        let total_entry = total_buckets.entry(bucket_100ms).or_insert(0);
        *total_entry = total_entry.saturating_add(1);

        if record.status_code == expected_status_code {
            let success_entry = success_buckets.entry(bucket_100ms).or_insert(0);
            *success_entry = success_entry.saturating_add(1);
        }
        if record.status_code != expected_status_code {
            let error_entry = error_buckets.entry(bucket_100ms).or_insert(0);
            *error_entry = error_entry.saturating_add(1);
        }

        let sec_idx = usize::try_from(sec).unwrap_or(usize::MAX);
        let sec_len = sec_idx.saturating_add(1);
        ensure_len(&mut rps_counts, sec_len);
        inc_slot(&mut rps_counts, sec_idx);

        ensure_len(&mut timeouts, sec_len);
        ensure_len(&mut transports, sec_len);
        ensure_len(&mut non_expected, sec_len);

        if record.timed_out {
            inc_slot(&mut timeouts, sec_idx);
        } else if record.transport_error {
            inc_slot(&mut transports, sec_idx);
        } else if record.status_code != expected_status_code {
            inc_slot(&mut non_expected, sec_idx);
        }

        ensure_len(&mut status_2xx, sec_len);
        ensure_len(&mut status_3xx, sec_len);
        ensure_len(&mut status_4xx, sec_len);
        ensure_len(&mut status_5xx, sec_len);
        ensure_len(&mut status_other, sec_len);

        match record.status_code {
            200..=299 => inc_slot(&mut status_2xx, sec_idx),
            300..=399 => inc_slot(&mut status_3xx, sec_idx),
            400..=499 => inc_slot(&mut status_4xx, sec_idx),
            500..=599 => inc_slot(&mut status_5xx, sec_idx),
            _ => inc_slot(&mut status_other, sec_idx),
        }

        let start_sec = sec_idx;
        let end_total_ms = record.elapsed_ms.saturating_add(record.latency_ms);
        let end_sec = usize::try_from(end_total_ms / 1000).unwrap_or(usize::MAX);
        let end_idx = end_sec.saturating_add(1);
        if inflight_deltas.len() <= end_idx {
            inflight_deltas.resize(end_idx.saturating_add(1), 0);
        }
        if let Some(slot) = inflight_deltas.get_mut(start_sec) {
            *slot = slot.saturating_add(1);
        }
        if let Some(slot) = inflight_deltas.get_mut(end_idx) {
            *slot = slot.saturating_sub(1);
        }

        latencies.push(record.latency_ms);
        if record.status_code == expected_status_code
            && !record.timed_out
            && !record.transport_error
        {
            latencies_ok.push(record.latency_ms);
        }

        if let Some(cursor) = cursors.get_mut(item.idx)
            && let Some(next) = read_next_record(cursor).await?
        {
            heap.push(std::cmp::Reverse(HeapItem {
                elapsed_ms: next.elapsed_ms,
                idx: item.idx,
                record: next,
            }));
        }
    }

    if let Some(active) = current_bucket {
        series.push_percentiles_for_sec(
            active.saturating_mul(bucket_ms),
            &mut latencies,
            &mut latencies_ok,
        );
    }

    let mut inflight: Vec<u32> = Vec::with_capacity(inflight_deltas.len());
    let mut current: i64 = 0;
    for delta in inflight_deltas {
        current = current.saturating_add(delta);
        inflight.push(u32::try_from(current.max(0)).unwrap_or(u32::MAX));
    }

    Ok(metrics::StreamingChartData {
        avg_buckets,
        total_buckets,
        success_buckets,
        error_buckets,
        rps_counts,
        timeouts,
        transports,
        non_expected,
        status_2xx,
        status_3xx,
        status_4xx,
        status_5xx,
        status_other,
        inflight,
        latency_buckets_ms,
        latency_bucket_ms: bucket_ms,
        p50,
        p90,
        p99,
        p50_ok,
        p90_ok,
        p99_ok,
    })
}
