use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use tokio::{sync::Semaphore, time::Instant};

use crate::{
    metrics::Metrics,
    shutdown::{ShutdownReceiver, ShutdownSender},
};

use super::data::{RequestLimiter, WorkerContext};

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

pub(super) async fn run_and_record(
    shutdown_rx: &mut ShutdownReceiver,
    worker: &WorkerContext<'_>,
    latency_start: Option<Instant>,
    run_request: impl std::future::Future<Output = (u16, bool, bool, u64)>,
) -> bool {
    let start = latency_start.unwrap_or_else(Instant::now);
    let in_flight_guard = InflightGuard::acquire(worker.in_flight_counter);
    let (status, timed_out, transport_error, response_bytes) = if worker.wait_ongoing {
        run_request.await
    } else {
        tokio::select! {
            _ = shutdown_rx.recv() => return true,
            result = run_request => result,
        }
    };
    drop(in_flight_guard);

    let in_flight_ops = worker.in_flight_counter.load(Ordering::Relaxed);
    let metric = Metrics::new(
        start,
        status,
        timed_out,
        transport_error,
        response_bytes,
        in_flight_ops,
    );
    if let Some(log_sink) = worker.log_sink
        && !log_sink.send(metric)
    {
        return true;
    }
    if worker.metrics_tx.try_send(metric).is_err() {
        // Ignore UI backpressure; summary and charts use log pipeline.
    }
    false
}

pub(super) async fn prepare_iteration(
    shutdown_rx: &mut ShutdownReceiver,
    shutdown_tx: &ShutdownSender,
    request_limiter: Option<&Arc<RequestLimiter>>,
    rate_limiter: Option<&Arc<Semaphore>>,
    wait_ongoing: bool,
    latency_correction: bool,
) -> Option<Option<Instant>> {
    if wait_ongoing && shutdown_rx.try_recv().is_ok() {
        return None;
    }
    if let Some(request_limiter) = request_limiter
        && !request_limiter.try_reserve(shutdown_tx)
    {
        return None;
    }
    let mut latency_start = if latency_correction {
        Some(Instant::now())
    } else {
        None
    };
    if let Some(rate_limiter) = rate_limiter {
        let denied = tokio::select! {
            _ = shutdown_rx.recv() => true,
            permit = rate_limiter.acquire() => permit.is_err(),
        };
        if denied {
            return None;
        }
        if !latency_correction {
            latency_start = None;
        }
    }

    Some(latency_start)
}
