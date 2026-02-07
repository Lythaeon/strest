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

#[cfg(any(test, feature = "fuzzing"))]
use std::path::Path;
#[cfg(any(test, feature = "fuzzing"))]
use tokio::io::{AsyncBufReadExt, BufReader};

use super::{LatencyHistogram, MetricRecord, Metrics, MetricsRange, MetricsSummary};

#[derive(Debug)]
pub struct LogSink {
    senders: Vec<mpsc::UnboundedSender<Metrics>>,
    next: AtomicUsize,
}

impl LogSink {
    #[must_use]
    pub const fn new(senders: Vec<mpsc::UnboundedSender<Metrics>>) -> Self {
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
            .is_some_and(|sender: &mpsc::UnboundedSender<Metrics>| sender.send(metric).is_ok())
    }
}

#[derive(Debug)]
pub struct LogResult {
    pub records: Vec<MetricRecord>,
    pub summary: MetricsSummary,
    pub metrics_truncated: bool,
    pub latency_sum_ms: u128,
    pub histogram: LatencyHistogram,
}

#[derive(Debug, Clone)]
pub struct MetricsLoggerConfig {
    pub run_start: tokio::time::Instant,
    pub warmup: Option<Duration>,
    pub expected_status_code: u16,
    pub metrics_range: Option<MetricsRange>,
    pub metrics_max: usize,
}

#[must_use]
pub fn setup_metrics_logger(
    log_path: PathBuf,
    config: MetricsLoggerConfig,
    mut log_rx: mpsc::UnboundedReceiver<Metrics>,
) -> JoinHandle<Result<LogResult, String>> {
    tokio::spawn(async move {
        let warmup_ms = config
            .warmup
            .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        let file = File::create(&log_path)
            .await
            .map_err(|err| format!("Failed to create metrics log: {}", err))?;
        let mut writer = BufWriter::new(file);
        let mut buffer = String::with_capacity(64 * 1024);
        let mut records = Vec::new();
        let mut metrics_truncated = false;
        let collect_records = config.metrics_max > 0;
        let mut histogram = LatencyHistogram::new()?;

        let mut total_requests: u64 = 0;
        let mut successful_requests: u64 = 0;
        let mut latency_sum_ms: u128 = 0;
        let mut min_latency_ms: u64 = u64::MAX;
        let mut max_latency_ms: u64 = 0;
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
                "{},{},{}",
                elapsed_ms, latency_ms, msg.status_code
            )
            .is_err()
            {
                return Err("Failed to format metrics log line".to_owned());
            }

            if buffer.len() >= 64 * 1024 {
                writer
                    .write_all(buffer.as_bytes())
                    .await
                    .map_err(|err| format!("Failed to write metrics log: {}", err))?;
                buffer.clear();
            }

            total_requests = total_requests.saturating_add(1);
            if msg.status_code == config.expected_status_code {
                successful_requests = successful_requests.saturating_add(1);
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
        let error_requests = total_requests.saturating_sub(successful_requests);

        Ok(LogResult {
            records,
            summary: MetricsSummary {
                duration,
                total_requests,
                successful_requests,
                error_requests,
                min_latency_ms,
                max_latency_ms,
                avg_latency_ms,
            },
            metrics_truncated,
            latency_sum_ms,
            histogram,
        })
    })
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

    let mut total_requests: u64 = 0;
    let mut successful_requests: u64 = 0;
    let mut latency_sum_ms: u128 = 0;
    let mut min_latency_ms: u64 = u64::MAX;
    let mut max_latency_ms: u64 = 0;
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

        total_requests = total_requests.saturating_add(1);
        if status_code == expected_status_code {
            successful_requests = successful_requests.saturating_add(1);
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
    let error_requests = total_requests.saturating_sub(successful_requests);

    Ok(LogResult {
        records,
        summary: MetricsSummary {
            duration,
            total_requests,
            successful_requests,
            error_requests,
            min_latency_ms,
            max_latency_ms,
            avg_latency_ms,
        },
        metrics_truncated,
        latency_sum_ms,
        histogram,
    })
}
