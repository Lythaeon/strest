//! Metrics collection, aggregation, and histogram utilities.
mod collector;
mod histogram;
mod logging;
mod types;

#[cfg(test)]
mod tests;

pub use collector::setup_metrics_collector;
pub use histogram::LatencyHistogram;
pub use logging::{LogResult, LogSink, MetricsLoggerConfig, setup_metrics_logger};
pub use types::{
    AggregatedMetricSample, MetricRecord, Metrics, MetricsRange, MetricsReport, MetricsSummary,
    StreamSnapshot, StreamingChartData,
};

#[cfg(any(test, feature = "fuzzing"))]
/// Read metrics from a log file and summarize them.
///
/// # Errors
///
/// Returns an error if the log cannot be read or parsed, or if histogram
/// operations fail.
pub async fn read_metrics_log(
    log_path: &std::path::Path,
    expected_status_code: u16,
    metrics_range: &Option<MetricsRange>,
    metrics_max: usize,
    warmup: Option<std::time::Duration>,
) -> Result<LogResult, String> {
    logging::read_metrics_log(
        log_path,
        expected_status_code,
        metrics_range,
        metrics_max,
        warmup,
    )
    .await
}

#[cfg(any(test, feature = "fuzzing"))]
const _: () = {
    let _ = read_metrics_log;
};
