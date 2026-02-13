use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{AppError, AppResult, MetricsError};

pub(super) struct LogCursor {
    pub(super) reader: BufReader<tokio::fs::File>,
    pub(super) line: String,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct LogRecord {
    pub(super) elapsed_ms: u64,
    pub(super) latency_ms: u64,
    pub(super) status_code: u16,
    pub(super) timed_out: bool,
    pub(super) transport_error: bool,
}

pub(super) fn parse_log_line(line: &str) -> Option<LogRecord> {
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

pub(super) async fn read_next_record(cursor: &mut LogCursor) -> AppResult<Option<LogRecord>> {
    loop {
        cursor.line.clear();
        let bytes = cursor
            .reader
            .read_line(&mut cursor.line)
            .await
            .map_err(|err| {
                AppError::metrics(MetricsError::Io {
                    context: "read metrics log",
                    source: err,
                })
            })?;
        if bytes == 0 {
            return Ok(None);
        }
        if let Some(record) = parse_log_line(&cursor.line) {
            return Ok(Some(record));
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct HeapItem {
    pub(super) elapsed_ms: u64,
    pub(super) idx: usize,
    pub(super) record: LogRecord,
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

pub(super) fn percentile(values: &mut [u64], percentile: u64) -> u64 {
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

pub(super) struct PercentileSeries<'series> {
    pub(super) latency_seconds: &'series mut Vec<u64>,
    pub(super) p50: &'series mut Vec<u64>,
    pub(super) p90: &'series mut Vec<u64>,
    pub(super) p99: &'series mut Vec<u64>,
    pub(super) p50_ok: &'series mut Vec<u64>,
    pub(super) p90_ok: &'series mut Vec<u64>,
    pub(super) p99_ok: &'series mut Vec<u64>,
}

impl<'series> PercentileSeries<'series> {
    pub(super) fn push_percentiles_for_sec(
        &mut self,
        sec: u64,
        values: &mut Vec<u64>,
        values_ok: &mut Vec<u64>,
    ) {
        let mut values = std::mem::take(values);
        let mut values_ok = std::mem::take(values_ok);
        self.latency_seconds.push(sec);
        self.p50.push(percentile(&mut values, 50));
        self.p90.push(percentile(&mut values, 90));
        self.p99.push(percentile(&mut values, 99));
        self.p50_ok.push(percentile(&mut values_ok, 50));
        self.p90_ok.push(percentile(&mut values_ok, 90));
        self.p99_ok.push(percentile(&mut values_ok, 99));
    }
}

pub(super) fn ensure_len(vec: &mut Vec<u32>, len: usize) {
    if vec.len() < len {
        vec.resize(len, 0);
    }
}

pub(super) fn inc_slot(vec: &mut [u32], idx: usize) {
    if let Some(slot) = vec.get_mut(idx) {
        *slot = slot.saturating_add(1);
    }
}
