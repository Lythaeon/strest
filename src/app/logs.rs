use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
        || args.export_jsonl.is_some();

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
    let collect_records = charts_enabled
        || summary_enabled
        || args.export_csv.is_some()
        || args.export_json.is_some()
        || args.export_jsonl.is_some();
    let metrics_max = if collect_records {
        args.metrics_max.get()
    } else {
        0
    };
    let metrics_max_per_shard = if metrics_max == 0 {
        0
    } else {
        metrics_max
            .saturating_add(shards)
            .saturating_sub(1)
            .checked_div(shards)
            .unwrap_or(0)
    };

    let mut senders = Vec::with_capacity(shards);
    let mut handles = Vec::with_capacity(shards);
    let mut paths = Vec::with_capacity(shards);

    for shard in 0..shards {
        let file_name = format!("metrics-{}-{}-{}.log", std::process::id(), stamp, shard);
        let log_path = tmp_dir.join(file_name);
        let (log_tx, log_rx) = mpsc::unbounded_channel();
        senders.push(log_tx);
        paths.push(log_path.clone());
        let logger_config = metrics::MetricsLoggerConfig {
            run_start,
            warmup: args.warmup,
            expected_status_code: args.expected_status_code,
            metrics_range: args.metrics_range.clone(),
            metrics_max: metrics_max_per_shard,
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
