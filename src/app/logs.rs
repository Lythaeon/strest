use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::{args::TesterArgs, metrics};

pub(crate) struct LogSetup {
    pub(crate) log_sink: Option<Arc<metrics::LogSink>>,
    pub(crate) handles: Vec<tokio::task::JoinHandle<Result<metrics::LogResult, String>>>,
    pub(crate) paths: Vec<PathBuf>,
}

pub(crate) type LogMergeResult = (
    metrics::MetricsSummary,
    Vec<metrics::MetricRecord>,
    bool,
    metrics::LatencyHistogram,
    u128,
    u128,
    metrics::LatencyHistogram,
);

pub(crate) async fn setup_log_sinks(
    args: &TesterArgs,
    run_start: Instant,
    charts_enabled: bool,
    summary_enabled: bool,
) -> Result<LogSetup, Box<dyn std::error::Error>> {
    let log_enabled = charts_enabled
        || summary_enabled
        || args.export_csv.is_some()
        || args.export_json.is_some()
        || args.export_jsonl.is_some()
        || args.db_url.is_some();

    if !log_enabled {
        return Ok(LogSetup {
            log_sink: None,
            handles: Vec::new(),
            paths: Vec::new(),
        });
    }

    let tmp_dir = Path::new(&args.tmp_path);
    tokio::fs::create_dir_all(tmp_dir).await?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    let shards = args.log_shards.get();
    let metrics_max_per_shard = 0;

    let mut senders = Vec::with_capacity(shards);
    let mut handles = Vec::with_capacity(shards);
    let mut paths = Vec::with_capacity(shards);
    let db_url = args.db_url.clone();

    for shard in 0..shards {
        let file_name = format!("metrics-{}-{}-{}.log", std::process::id(), stamp, shard);
        let log_path = tmp_dir.join(file_name);
        let (log_tx, log_rx) = mpsc::channel(10_000);
        senders.push(log_tx);
        paths.push(log_path.clone());
        let logger_config = metrics::MetricsLoggerConfig {
            run_start,
            warmup: args.warmup,
            expected_status_code: args.expected_status_code,
            metrics_range: args.metrics_range.clone(),
            metrics_max: metrics_max_per_shard,
            db_url: if shard == 0 { db_url.clone() } else { None },
        };
        let handle = metrics::setup_metrics_logger(log_path, logger_config, log_rx);
        handles.push(handle);
    }

    Ok(LogSetup {
        log_sink: Some(Arc::new(metrics::LogSink::new(senders))),
        handles,
        paths,
    })
}

pub(crate) fn merge_log_results(
    results: Vec<metrics::LogResult>,
    metrics_max: usize,
) -> Result<LogMergeResult, String> {
    let mut total_requests: u64 = 0;
    let mut successful_requests: u64 = 0;
    let mut timeout_requests: u64 = 0;
    let mut transport_errors: u64 = 0;
    let mut non_expected_status: u64 = 0;
    let mut latency_sum_ms: u128 = 0;
    let mut success_latency_sum_ms: u128 = 0;
    let mut min_latency_ms: u64 = u64::MAX;
    let mut max_latency_ms: u64 = 0;
    let mut success_min_latency_ms: u64 = u64::MAX;
    let mut success_max_latency_ms: u64 = 0;
    let mut duration = Duration::ZERO;
    let mut records = Vec::new();
    let mut metrics_truncated = false;
    let mut histogram = metrics::LatencyHistogram::new()?;
    let mut success_histogram = metrics::LatencyHistogram::new()?;

    for result in results {
        total_requests = total_requests.saturating_add(result.summary.total_requests);
        successful_requests =
            successful_requests.saturating_add(result.summary.successful_requests);
        timeout_requests = timeout_requests.saturating_add(result.summary.timeout_requests);
        transport_errors = transport_errors.saturating_add(result.summary.transport_errors);
        non_expected_status =
            non_expected_status.saturating_add(result.summary.non_expected_status);
        latency_sum_ms = latency_sum_ms.saturating_add(result.latency_sum_ms);
        success_latency_sum_ms =
            success_latency_sum_ms.saturating_add(result.success_latency_sum_ms);
        if result.summary.total_requests > 0 {
            min_latency_ms = min_latency_ms.min(result.summary.min_latency_ms);
            max_latency_ms = max_latency_ms.max(result.summary.max_latency_ms);
        }
        if result.summary.successful_requests > 0 {
            success_min_latency_ms =
                success_min_latency_ms.min(result.summary.success_min_latency_ms);
            success_max_latency_ms =
                success_max_latency_ms.max(result.summary.success_max_latency_ms);
        }
        duration = duration.max(result.summary.duration);
        metrics_truncated = metrics_truncated || result.metrics_truncated;
        records.extend(result.records);
        histogram.merge(&result.histogram)?;
        success_histogram.merge(&result.success_histogram)?;
    }

    if metrics_max > 0 && records.len() > metrics_max {
        records.truncate(metrics_max);
        metrics_truncated = true;
    }
    records.sort_by_key(|record| record.elapsed_ms);

    let avg_latency_ms = if total_requests > 0 {
        let avg = latency_sum_ms
            .checked_div(u128::from(total_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };
    let success_avg_latency_ms = if successful_requests > 0 {
        let avg = success_latency_sum_ms
            .checked_div(u128::from(successful_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let min_latency_ms = if total_requests > 0 {
        min_latency_ms
    } else {
        0
    };
    let success_min_latency_ms = if successful_requests > 0 {
        success_min_latency_ms
    } else {
        0
    };
    let success_max_latency_ms = if successful_requests > 0 {
        success_max_latency_ms
    } else {
        0
    };
    let error_requests = total_requests.saturating_sub(successful_requests);

    Ok((
        metrics::MetricsSummary {
            duration,
            total_requests,
            successful_requests,
            error_requests,
            timeout_requests,
            transport_errors,
            non_expected_status,
            min_latency_ms,
            max_latency_ms,
            avg_latency_ms,
            success_min_latency_ms,
            success_max_latency_ms,
            success_avg_latency_ms,
        },
        records,
        metrics_truncated,
        histogram,
        latency_sum_ms,
        success_latency_sum_ms,
        success_histogram,
    ))
}

pub(crate) const fn empty_summary() -> metrics::MetricsSummary {
    metrics::MetricsSummary {
        duration: Duration::ZERO,
        total_requests: 0,
        successful_requests: 0,
        error_requests: 0,
        timeout_requests: 0,
        transport_errors: 0,
        non_expected_status: 0,
        min_latency_ms: 0,
        max_latency_ms: 0,
        avg_latency_ms: 0,
        success_min_latency_ms: 0,
        success_max_latency_ms: 0,
        success_avg_latency_ms: 0,
    }
}

struct LogCursor {
    reader: BufReader<tokio::fs::File>,
    line: String,
}

#[derive(Clone, Copy, Debug)]
struct LogRecord {
    elapsed_ms: u64,
    latency_ms: u64,
    status_code: u16,
    timed_out: bool,
    transport_error: bool,
}

fn parse_log_line(line: &str) -> Option<LogRecord> {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return None;
    }
    let mut parts = trimmed.split(',');
    let elapsed_ms = parts.next()?.parse::<u64>().ok()?;
    let latency_ms = parts.next()?.parse::<u64>().ok()?;
    let status_code = parts.next()?.parse::<u16>().ok()?;
    let timed_out = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .is_some_and(|value| value != 0);
    let transport_error = parts
        .next()
        .and_then(|value| value.parse::<u8>().ok())
        .is_some_and(|value| value != 0);
    Some(LogRecord {
        elapsed_ms,
        latency_ms,
        status_code,
        timed_out,
        transport_error,
    })
}

async fn read_next_record(cursor: &mut LogCursor) -> Result<Option<LogRecord>, String> {
    loop {
        cursor.line.clear();
        let bytes = cursor
            .reader
            .read_line(&mut cursor.line)
            .await
            .map_err(|err| format!("Failed to read metrics log: {}", err))?;
        if bytes == 0 {
            return Ok(None);
        }
        if let Some(record) = parse_log_line(&cursor.line) {
            return Ok(Some(record));
        }
    }
}

#[derive(Clone, Debug)]
struct HeapItem {
    elapsed_ms: u64,
    idx: usize,
    record: LogRecord,
}

impl Eq for HeapItem {}

impl PartialEq for HeapItem {
    fn eq(&self, other: &Self) -> bool {
        self.elapsed_ms == other.elapsed_ms && self.idx == other.idx
    }
}

impl Ord for HeapItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.elapsed_ms
            .cmp(&other.elapsed_ms)
            .then_with(|| self.idx.cmp(&other.idx))
    }
}

impl PartialOrd for HeapItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn percentile(values: &mut [u64], percentile: u64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let count = values.len().saturating_sub(1) as u64;
    let index = percentile
        .saturating_mul(count)
        .saturating_add(50)
        .checked_div(100)
        .unwrap_or(0);
    let idx = usize::try_from(index).unwrap_or_else(|_| values.len().saturating_sub(1));
    *values.get(idx).unwrap_or(&0)
}

fn ensure_len(vec: &mut Vec<u32>, len: usize) {
    if vec.len() < len {
        vec.resize(len, 0);
    }
}

fn inc_slot(vec: &mut [u32], idx: usize) {
    if let Some(slot) = vec.get_mut(idx) {
        *slot = slot.saturating_add(1);
    }
}

pub(crate) async fn load_chart_data_streaming(
    paths: &[PathBuf],
    expected_status_code: u16,
    metrics_range: &Option<metrics::MetricsRange>,
) -> Result<metrics::StreamingChartData, String> {
    let mut cursors: Vec<LogCursor> = Vec::with_capacity(paths.len());
    for path in paths {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|err| format!("Failed to open metrics log {}: {}", path.display(), err))?;
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

    let mut latency_seconds: Vec<u64> = Vec::new();
    let mut p50: Vec<u64> = Vec::new();
    let mut p90: Vec<u64> = Vec::new();
    let mut p99: Vec<u64> = Vec::new();
    let mut p50_ok: Vec<u64> = Vec::new();
    let mut p90_ok: Vec<u64> = Vec::new();
    let mut p99_ok: Vec<u64> = Vec::new();

    let mut current_sec: Option<u64> = None;
    let mut latencies: Vec<u64> = Vec::new();
    let mut latencies_ok: Vec<u64> = Vec::new();

    while let Some(std::cmp::Reverse(item)) = heap.pop() {
        let record = item.record;
        let sec = record.elapsed_ms / 1000;

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

        match current_sec {
            Some(active) if sec != active => {
                let mut values = std::mem::take(&mut latencies);
                let mut values_ok = std::mem::take(&mut latencies_ok);
                latency_seconds.push(active);
                p50.push(percentile(&mut values, 50));
                p90.push(percentile(&mut values, 90));
                p99.push(percentile(&mut values, 99));
                p50_ok.push(percentile(&mut values_ok, 50));
                p90_ok.push(percentile(&mut values_ok, 90));
                p99_ok.push(percentile(&mut values_ok, 99));
                current_sec = Some(sec);
            }
            None => current_sec = Some(sec),
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

    if let Some(active) = current_sec {
        let mut values = std::mem::take(&mut latencies);
        let mut values_ok = std::mem::take(&mut latencies_ok);
        latency_seconds.push(active);
        p50.push(percentile(&mut values, 50));
        p90.push(percentile(&mut values, 90));
        p99.push(percentile(&mut values, 99));
        p50_ok.push(percentile(&mut values_ok, 50));
        p90_ok.push(percentile(&mut values_ok, 90));
        p99_ok.push(percentile(&mut values_ok, 99));
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
        latency_seconds,
        p50,
        p90,
        p99,
        p50_ok,
        p90_ok,
        p99_ok,
    })
}

pub(crate) async fn load_log_records(
    paths: &[PathBuf],
    metrics_range: &Option<metrics::MetricsRange>,
    metrics_max: usize,
) -> Result<(Vec<metrics::MetricRecord>, bool), String> {
    let mut records: Vec<metrics::MetricRecord> = Vec::new();
    let mut metrics_truncated = false;

    for path in paths {
        let file = tokio::fs::File::open(path)
            .await
            .map_err(|err| format!("Failed to open metrics log {}: {}", path.display(), err))?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes = reader
                .read_line(&mut line)
                .await
                .map_err(|err| format!("Failed to read metrics log {}: {}", path.display(), err))?;
            if bytes == 0 {
                break;
            }

            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.split(',');
            let elapsed_ms = match parts.next().and_then(|value| value.parse::<u64>().ok()) {
                Some(value) => value,
                None => continue,
            };
            let latency_ms = match parts.next().and_then(|value| value.parse::<u64>().ok()) {
                Some(value) => value,
                None => continue,
            };
            let status_code = match parts.next().and_then(|value| value.parse::<u16>().ok()) {
                Some(value) => value,
                None => continue,
            };
            let timed_out = parts
                .next()
                .and_then(|value| value.parse::<u8>().ok())
                .is_some_and(|value| value != 0);
            let transport_error = parts
                .next()
                .and_then(|value| value.parse::<u8>().ok())
                .is_some_and(|value| value != 0);

            let seconds_elapsed = elapsed_ms / 1000;
            let in_range = match metrics_range {
                Some(metrics::MetricsRange(range)) => range.contains(&seconds_elapsed),
                None => true,
            };
            if !in_range {
                continue;
            }

            if metrics_max == 0 || records.len() < metrics_max {
                records.push(metrics::MetricRecord {
                    elapsed_ms,
                    latency_ms,
                    status_code,
                    timed_out,
                    transport_error,
                });
            } else {
                metrics_truncated = true;
                break;
            }
        }

        if metrics_truncated && metrics_max > 0 {
            break;
        }
    }

    if metrics_max > 0 && records.len() > metrics_max {
        records.truncate(metrics_max);
        metrics_truncated = true;
    }
    records.sort_by_key(|record| record.elapsed_ms);

    Ok((records, metrics_truncated))
}
