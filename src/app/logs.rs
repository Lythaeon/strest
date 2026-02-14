mod merge;
mod parsing;
mod records;
mod setup;
mod streaming;

use std::path::PathBuf;
use std::sync::Arc;

use tokio::time::Instant;

use crate::args::TesterArgs;
use crate::error::AppResult;
use crate::metrics;

pub(crate) struct LogSetup {
    pub(crate) log_sink: Option<Arc<metrics::LogSink>>,
    pub(crate) handles: Vec<tokio::task::JoinHandle<AppResult<metrics::LogResult>>>,
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
) -> AppResult<LogSetup> {
    setup::setup_log_sinks(args, run_start, charts_enabled, summary_enabled).await
}

pub(crate) fn merge_log_results(
    results: Vec<metrics::LogResult>,
    metrics_max: usize,
) -> AppResult<LogMergeResult> {
    merge::merge_log_results(results, metrics_max)
}

pub(crate) async fn load_chart_data_streaming(
    paths: &[PathBuf],
    expected_status_code: u16,
    metrics_range: &Option<metrics::MetricsRange>,
    latency_bucket_ms: u64,
) -> AppResult<metrics::StreamingChartData> {
    streaming::load_chart_data_streaming(
        paths,
        expected_status_code,
        metrics_range,
        latency_bucket_ms,
    )
    .await
}

pub(crate) async fn load_log_records(
    paths: &[PathBuf],
    metrics_range: &Option<metrics::MetricsRange>,
    metrics_max: usize,
) -> AppResult<(Vec<metrics::MetricRecord>, bool)> {
    records::load_log_records(paths, metrics_range, metrics_max).await
}
