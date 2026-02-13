use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::args::TesterArgs;
use crate::error::AppResult;
use crate::metrics;

use super::LogSetup;

pub(super) async fn setup_log_sinks(
    args: &TesterArgs,
    run_start: Instant,
    charts_enabled: bool,
    summary_enabled: bool,
) -> AppResult<LogSetup> {
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
