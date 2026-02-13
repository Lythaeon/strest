use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use tokio::sync::{Semaphore, mpsc};

use crate::metrics::{LogSink, Metrics};
use crate::shutdown::ShutdownSender;

#[derive(Clone, Copy)]
pub(super) struct RequestOutcome {
    pub(super) timed_out: bool,
    pub(super) transport_error: bool,
    pub(super) response_bytes: u64,
}

impl RequestOutcome {
    pub(super) const fn success(response_bytes: u64) -> Self {
        Self {
            timed_out: false,
            transport_error: false,
            response_bytes,
        }
    }

    pub(super) const fn timeout() -> Self {
        Self {
            timed_out: true,
            transport_error: false,
            response_bytes: 0,
        }
    }

    pub(super) const fn transport_error() -> Self {
        Self {
            timed_out: false,
            transport_error: true,
            response_bytes: 0,
        }
    }
}

#[derive(Debug)]
pub(super) struct RequestLimiter {
    limit: Option<u64>,
    counter: AtomicU64,
}

impl RequestLimiter {
    pub(super) fn new(limit: Option<u64>) -> Option<Self> {
        limit.map(|value| Self {
            limit: Some(value),
            counter: AtomicU64::new(0),
        })
    }

    pub(super) fn try_reserve(&self, shutdown_tx: &ShutdownSender) -> bool {
        let Some(limit) = self.limit else {
            return true;
        };
        loop {
            let current = self.counter.load(Ordering::Relaxed);
            if current >= limit {
                drop(shutdown_tx.send(()));
                return false;
            }
            let Some(next) = current.checked_add(1) else {
                drop(shutdown_tx.send(()));
                return false;
            };
            if self
                .counter
                .compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }
}

pub(super) struct InflightGuard<'counter> {
    counter: &'counter AtomicU64,
}

impl<'counter> InflightGuard<'counter> {
    pub(super) fn acquire(counter: &'counter Arc<AtomicU64>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self {
            counter: counter.as_ref(),
        }
    }
}

impl Drop for InflightGuard<'_> {
    fn drop(&mut self) {
        loop {
            let current = self.counter.load(Ordering::Relaxed);
            let Some(next) = current.checked_sub(1) else {
                break;
            };
            if self
                .counter
                .compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }
}

pub(super) type TransportRequestFn = dyn Fn(Duration, Duration) -> futures_util::future::BoxFuture<'static, RequestOutcome>
    + Send
    + Sync;

pub(super) struct TransportRunContext<'ctx> {
    pub(super) shutdown_tx: &'ctx ShutdownSender,
    pub(super) rate_limiter: Option<&'ctx Arc<Semaphore>>,
    pub(super) request_limiter: Option<&'ctx Arc<RequestLimiter>>,
    pub(super) in_flight_counter: &'ctx Arc<AtomicU64>,
    pub(super) metrics_tx: &'ctx mpsc::Sender<Metrics>,
    pub(super) log_sink: &'ctx Option<Arc<LogSink>>,
    pub(super) wait_ongoing: bool,
    pub(super) latency_correction: bool,
    pub(super) expected_status_code: u16,
    pub(super) request_timeout: Duration,
    pub(super) connect_timeout: Duration,
    pub(super) request_fn: &'ctx TransportRequestFn,
}
