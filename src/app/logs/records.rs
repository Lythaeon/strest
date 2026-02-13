use std::path::PathBuf;

use tokio::io::{AsyncBufReadExt, BufReader};

use crate::error::{AppError, AppResult, MetricsError};
use crate::metrics;

pub(super) async fn load_log_records(
    paths: &[PathBuf],
    metrics_range: &Option<metrics::MetricsRange>,
    metrics_max: usize,
) -> AppResult<(Vec<metrics::MetricRecord>, bool)> {
    let mut records: Vec<metrics::MetricRecord> = Vec::new();
    let mut metrics_truncated = false;

    for path in paths {
        let file = tokio::fs::File::open(path).await.map_err(|err| {
            AppError::metrics(MetricsError::Io {
                context: "open metrics log",
                source: err,
            })
        })?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        loop {
            line.clear();
            let bytes = reader.read_line(&mut line).await.map_err(|err| {
                AppError::metrics(MetricsError::Io {
                    context: "read metrics log",
                    source: err,
                })
            })?;
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
            let response_bytes = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);
            let in_flight_ops = parts
                .next()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(0);

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
                    response_bytes,
                    in_flight_ops,
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
