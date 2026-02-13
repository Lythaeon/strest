mod reader;
mod writer;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;

use super::{LatencyHistogram, MetricRecord, Metrics, MetricsRange, MetricsSummary};

#[cfg(any(test, feature = "fuzzing"))]
pub use reader::read_metrics_log;
pub use writer::setup_metrics_logger;

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
