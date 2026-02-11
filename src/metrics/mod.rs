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
pub use logging::read_metrics_log;
