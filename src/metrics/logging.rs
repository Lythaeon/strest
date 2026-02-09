use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::{
    fs::File,
    io::{AsyncWriteExt, BufWriter},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_rusqlite::Connection;

#[cfg(any(test, feature = "fuzzing"))]
use std::path::Path;
#[cfg(any(test, feature = "fuzzing"))]
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{LatencyHistogram, MetricRecord, Metrics, MetricsRange, MetricsSummary};

#[derive(Debug)]
pub struct LogSink {
    senders: Vec<mpsc::Sender<Metrics>>,
    next: AtomicUsize,
}

impl LogSink {
    #[must_use]
    pub const fn new(senders: Vec<mpsc::Sender<Metrics>>) -> Self {
        Self {
            senders,
            next: AtomicUsize::new(0),
        }
    }

    pub fn send(&self, metric: Metrics) -> bool {
        if self.senders.is_empty() {
            return false;
        }
        let len = self.senders.len();
        let idx = self
            .next
            .fetch_add(1, Ordering::Relaxed)
            .checked_rem(len)
            .unwrap_or(0);
        self.senders
            .get(idx)
            .is_some_and(|sender| match sender.try_send(metric) {
                Ok(()) => true,
                Err(mpsc::error::TrySendError::Full(_)) => true,
                Err(mpsc::error::TrySendError::Closed(_)) => false,
            })
    }
}

#[derive(Debug)]
pub struct LogResult {
    pub records: Vec<MetricRecord>,
    pub summary: MetricsSummary,
    pub metrics_truncated: bool,
    pub latency_sum_ms: u128,
    pub success_latency_sum_ms: u128,
    pub histogram: LatencyHistogram,
    pub success_histogram: LatencyHistogram,
}

#[derive(Debug, Clone)]
pub struct MetricsLoggerConfig {
    pub run_start: tokio::time::Instant,
    pub warmup: Option<Duration>,
    pub expected_status_code: u16,
    pub metrics_range: Option<MetricsRange>,
    pub metrics_max: usize,
    pub db_url: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct DbRecord {
    elapsed_ms: u64,
    latency_ms: u64,
    status_code: u16,
    timed_out: bool,
    transport_error: bool,
}

const DB_FLUSH_SIZE: usize = 500;

#[must_use]
pub fn setup_metrics_logger(
    log_path: PathBuf,
    config: MetricsLoggerConfig,
    mut log_rx: mpsc::Receiver<Metrics>,
) -> JoinHandle<Result<LogResult, String>> {
    tokio::spawn(async move {
        let warmup_ms = config
            .warmup
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let file = File::create(&log_path)
            .await
            .map_err(|err| format!("Failed to create metrics log: {}", err))?;
        const LOG_BUFFER_SIZE: usize = 256 * 1024;
        let mut writer = BufWriter::with_capacity(LOG_BUFFER_SIZE, file);
        let mut buffer = String::with_capacity(LOG_BUFFER_SIZE);
        let mut records = Vec::new();
        let mut metrics_truncated = false;
        let collect_records = config.metrics_max > 0;
        let mut histogram = LatencyHistogram::new()?;
        let mut success_histogram = LatencyHistogram::new()?;
        let db_conn = if let Some(db_url) = config.db_url.as_deref() {
            let conn = Connection::open(db_url)
                .await
                .map_err(|err| format!("Failed to open sqlite db {}: {}", db_url, err))?;
            conn.call(|conn| {
                conn.execute_batch(
                    "CREATE TABLE IF NOT EXISTS metrics (
                        id INTEGER PRIMARY KEY AUTOINCREMENT,
                        elapsed_ms INTEGER NOT NULL,
                        latency_ms INTEGER NOT NULL,
                        status_code INTEGER NOT NULL,
                        timed_out INTEGER NOT NULL,
                        transport_error INTEGER NOT NULL
                    );
                    CREATE INDEX IF NOT EXISTS idx_metrics_elapsed_ms ON metrics(elapsed_ms);",
                )?;
                Ok(())
            })
            .await
            .map_err(|err| format!("Failed to initialize sqlite db: {}", err))?;
            Some(conn)
        } else {
            None
        };
        let mut db_buffer: Vec<DbRecord> = Vec::new();

        let mut total_requests: u64 = 0;
        let mut successful_requests: u64 = 0;
        let mut timeout_requests: u64 = 0;
        let mut latency_sum_ms: u128 = 0;
        let mut success_latency_sum_ms: u128 = 0;
        let mut min_latency_ms: u64 = u64::MAX;
        let mut max_latency_ms: u64 = 0;
        let mut success_min_latency_ms: u64 = u64::MAX;
        let mut success_max_latency_ms: u64 = 0;
        let mut transport_errors: u64 = 0;
        let mut non_expected_status: u64 = 0;
        let mut max_elapsed_ms: u64 = 0;

        while let Some(msg) = log_rx.recv().await {
            let elapsed_ms_raw = u64::try_from(
                msg.start
                    .saturating_duration_since(config.run_start)
                    .as_millis(),
            )
            .unwrap_or(u64::MAX);
            if elapsed_ms_raw < warmup_ms {
                continue;
            }
            let elapsed_ms = elapsed_ms_raw.saturating_sub(warmup_ms);
            let latency_ms = u64::try_from(msg.response_time.as_millis()).unwrap_or(u64::MAX);

            if writeln!(
                &mut buffer,
                "{},{},{},{},{}",
                elapsed_ms,
                latency_ms,
                msg.status_code,
                u8::from(msg.timed_out),
                u8::from(msg.transport_error)
            )
            .is_err()
            {
                return Err("Failed to format metrics log line".to_owned());
            }

            if buffer.len() >= LOG_BUFFER_SIZE {
                writer
                    .write_all(buffer.as_bytes())
                    .await
                    .map_err(|err| format!("Failed to write metrics log: {}", err))?;
                buffer.clear();
            }

            total_requests = total_requests.saturating_add(1);
            if msg.status_code == config.expected_status_code
                && !msg.timed_out
                && !msg.transport_error
            {
                successful_requests = successful_requests.saturating_add(1);
                success_latency_sum_ms =
                    success_latency_sum_ms.saturating_add(u128::from(latency_ms));
                if latency_ms < success_min_latency_ms {
                    success_min_latency_ms = latency_ms;
                }
                if latency_ms > success_max_latency_ms {
                    success_max_latency_ms = latency_ms;
                }
                success_histogram.record(latency_ms)?;
            }
            if msg.timed_out {
                timeout_requests = timeout_requests.saturating_add(1);
            } else if msg.transport_error {
                transport_errors = transport_errors.saturating_add(1);
            } else if msg.status_code != config.expected_status_code {
                non_expected_status = non_expected_status.saturating_add(1);
            }
            latency_sum_ms = latency_sum_ms.saturating_add(u128::from(latency_ms));
            if latency_ms < min_latency_ms {
                min_latency_ms = latency_ms;
            }
            if latency_ms > max_latency_ms {
                max_latency_ms = latency_ms;
            }
            if elapsed_ms > max_elapsed_ms {
                max_elapsed_ms = elapsed_ms;
            }
            histogram.record(latency_ms)?;

            if let Some(conn) = db_conn.as_ref() {
                db_buffer.push(DbRecord {
                    elapsed_ms,
                    latency_ms,
                    status_code: msg.status_code,
                    timed_out: msg.timed_out,
                    transport_error: msg.transport_error,
                });
                if db_buffer.len() >= DB_FLUSH_SIZE {
                    flush_db_records(conn, &mut db_buffer).await?;
                }
            }

            if collect_records {
                let seconds_elapsed = elapsed_ms / 1000;
                let in_range = match &config.metrics_range {
                    Some(MetricsRange(range)) => range.contains(&seconds_elapsed),
                    None => true,
                };
                if in_range {
                    if records.len() < config.metrics_max {
                        records.push(MetricRecord {
                            elapsed_ms,
                            latency_ms,
                            status_code: msg.status_code,
                            timed_out: msg.timed_out,
                            transport_error: msg.transport_error,
                        });
                    } else {
                        metrics_truncated = true;
                    }
                }
            }
        }

        if !buffer.is_empty() {
            writer
                .write_all(buffer.as_bytes())
                .await
                .map_err(|err| format!("Failed to write metrics log: {}", err))?;
        }
        writer
            .flush()
            .await
            .map_err(|err| format!("Failed to flush metrics log: {}", err))?;
        if let Some(conn) = db_conn.as_ref() {
            flush_db_records(conn, &mut db_buffer).await?;
        }
        let duration = Duration::from_millis(max_elapsed_ms);
        let avg_latency_ms = if total_requests > 0 {
            let avg = latency_sum_ms
                .checked_div(u128::from(total_requests))
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
        let success_avg_latency_ms = if successful_requests > 0 {
            let avg = success_latency_sum_ms
                .checked_div(u128::from(successful_requests))
                .unwrap_or(0);
            u64::try_from(avg).map_or(u64::MAX, |value| value)
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

        Ok(LogResult {
            records,
            summary: MetricsSummary {
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
            metrics_truncated,
            latency_sum_ms,
            success_latency_sum_ms,
            histogram,
            success_histogram,
        })
    })
}

async fn flush_db_records(conn: &Connection, buffer: &mut Vec<DbRecord>) -> Result<(), String> {
    if buffer.is_empty() {
        return Ok(());
    }

    let records = std::mem::take(buffer);
    conn.call(move |conn| {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO metrics (elapsed_ms, latency_ms, status_code, timed_out, transport_error)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for record in records {
                stmt.execute(rusqlite::params![
                    clamp_i64(record.elapsed_ms),
                    clamp_i64(record.latency_ms),
                    i64::from(record.status_code),
                    i64::from(u8::from(record.timed_out)),
                    i64::from(u8::from(record.transport_error))
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
    .await
    .map_err(|err| format!("Failed to write sqlite metrics: {}", err))?;

    Ok(())
}

fn clamp_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(any(test, feature = "fuzzing"))]
/// Read metrics from a log file and summarize them.
///
/// # Errors
///
/// Returns an error if the log cannot be read or parsed, or if histogram
/// operations fail.
pub async fn read_metrics_log(
    log_path: &Path,
    expected_status_code: u16,
    metrics_range: &Option<MetricsRange>,
    metrics_max: usize,
    warmup: Option<Duration>,
) -> Result<LogResult, String> {
    let warmup_ms = warmup
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0);
    let file = File::open(log_path)
        .await
        .map_err(|err| format!("Failed to open metrics log: {}", err))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut records = Vec::new();
    let mut metrics_truncated = false;
    let collect_records = metrics_max > 0;
    let mut histogram = LatencyHistogram::new()?;
    let mut success_histogram = LatencyHistogram::new()?;

    let mut total_requests: u64 = 0;
    let mut successful_requests: u64 = 0;
    let mut timeout_requests: u64 = 0;
    let mut latency_sum_ms: u128 = 0;
    let mut success_latency_sum_ms: u128 = 0;
    let mut min_latency_ms: u64 = u64::MAX;
    let mut max_latency_ms: u64 = 0;
    let mut success_min_latency_ms: u64 = u64::MAX;
    let mut success_max_latency_ms: u64 = 0;
    let mut transport_errors: u64 = 0;
    let mut non_expected_status: u64 = 0;
    let mut max_elapsed_ms: u64 = 0;

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|err| format!("Failed to read metrics log: {}", err))?;
        if bytes == 0 {
            break;
        }

        let trimmed = line.trim_end();
        let mut parts = trimmed.split(',');
        let elapsed_ms_raw = match parts.next().and_then(|value| value.parse::<u64>().ok()) {
            Some(value) => value,
            None => continue,
        };
        if elapsed_ms_raw < warmup_ms {
            continue;
        }
        let elapsed_ms = elapsed_ms_raw.saturating_sub(warmup_ms);
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

        total_requests = total_requests.saturating_add(1);
        if status_code == expected_status_code && !timed_out && !transport_error {
            successful_requests = successful_requests.saturating_add(1);
            success_latency_sum_ms = success_latency_sum_ms.saturating_add(u128::from(latency_ms));
            if latency_ms < success_min_latency_ms {
                success_min_latency_ms = latency_ms;
            }
            if latency_ms > success_max_latency_ms {
                success_max_latency_ms = latency_ms;
            }
            success_histogram.record(latency_ms)?;
        }
        if timed_out {
            timeout_requests = timeout_requests.saturating_add(1);
        } else if transport_error {
            transport_errors = transport_errors.saturating_add(1);
        } else if status_code != expected_status_code {
            non_expected_status = non_expected_status.saturating_add(1);
        }
        latency_sum_ms = latency_sum_ms.saturating_add(u128::from(latency_ms));
        if latency_ms < min_latency_ms {
            min_latency_ms = latency_ms;
        }
        if latency_ms > max_latency_ms {
            max_latency_ms = latency_ms;
        }
        if elapsed_ms > max_elapsed_ms {
            max_elapsed_ms = elapsed_ms;
        }
        histogram.record(latency_ms)?;

        if collect_records {
            let seconds_elapsed = elapsed_ms / 1000;
            let in_range = match metrics_range {
                Some(MetricsRange(range)) => range.contains(&seconds_elapsed),
                None => true,
            };

            if in_range {
                if records.len() < metrics_max {
                    records.push(MetricRecord {
                        elapsed_ms,
                        latency_ms,
                        status_code,
                        timed_out,
                        transport_error,
                    });
                } else {
                    metrics_truncated = true;
                }
            }
        }
    }

    let duration = Duration::from_millis(max_elapsed_ms);
    let avg_latency_ms = if total_requests > 0 {
        let avg = latency_sum_ms
            .checked_div(u128::from(total_requests))
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
    let success_avg_latency_ms = if successful_requests > 0 {
        let avg = success_latency_sum_ms
            .checked_div(u128::from(successful_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
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

    Ok(LogResult {
        records,
        summary: MetricsSummary {
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
        metrics_truncated,
        latency_sum_ms,
        success_latency_sum_ms,
        histogram,
        success_histogram,
    })
}
