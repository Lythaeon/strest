use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Duration;

use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinHandle;
use tokio::time::{Instant, interval, sleep};
use tracing::{error, warn};

use crate::args::{Protocol, TesterArgs};
use crate::http::build_rate_limiter;
use crate::metrics::{LogSink, Metrics};
use crate::shutdown::{ShutdownReceiver, ShutdownSender};

use super::types::{
    InflightGuard, RequestLimiter, RequestOutcome, TransportRequestFn, TransportRunContext,
};

pub(super) fn spawn_transport_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
    request_fn: impl Fn(Duration, Duration) -> futures_util::future::BoxFuture<'static, RequestOutcome>
    + Send
    + Sync
    + 'static,
) -> JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    let metrics_tx = metrics_tx.clone();
    let log_sink = log_sink.cloned();
    let request_fn: Arc<TransportRequestFn> = Arc::new(request_fn);

    let skip_preflight = matches!(args.protocol, Protocol::GrpcUnary | Protocol::GrpcStreaming);

    let max_tasks = args.max_tasks.get();
    let spawn_rate = args.spawn_rate_per_tick.get();
    let tick_interval = args.tick_interval.get();
    let rate_limit = args.rate_limit.map(u64::from);
    let load_profile = args.load_profile.clone();
    let request_timeout = args.request_timeout;
    let connect_timeout = args.connect_timeout;
    let expected_status_code = args.expected_status_code;
    let request_limiter = RequestLimiter::new(args.requests.map(u64::from)).map(Arc::new);
    let burst_delay = args.burst_delay;
    let burst_rate = args.burst_rate.get();
    let wait_ongoing = args.wait_ongoing_requests_after_deadline;
    let latency_correction = if args.rate_limit.is_some() {
        args.latency_correction
    } else {
        if args.latency_correction {
            warn!("--latency-correction is ignored unless --rate is set.");
        }
        false
    };
    if args.rate_limit.is_some() && args.burst_delay.is_some() {
        warn!("--burst-delay/--burst-rate are ignored when --rate is set.");
    }
    if load_profile.is_some() && args.burst_delay.is_some() {
        warn!("--burst-delay/--burst-rate are ignored when a load profile is set.");
    }

    tokio::spawn(async move {
        if !skip_preflight {
            let preflight = request_fn(request_timeout, connect_timeout).await;
            if preflight.timed_out || preflight.transport_error {
                error!("Protocol preflight request failed");
                drop(shutdown_tx.send(()));
                return;
            }
        }

        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut spawn_interval = interval(Duration::from_millis(tick_interval));
        let mut total_spawned: usize = 0;
        let permits = Arc::new(Semaphore::new(0));
        let in_flight_counter = Arc::new(AtomicU64::new(0));
        let rate_limiter =
            build_rate_limiter(rate_limit, load_profile.as_ref(), burst_delay, burst_rate);
        let mut worker_handles = Vec::with_capacity(max_tasks);

        for _ in 0..max_tasks {
            let permits = Arc::clone(&permits);
            let shutdown_tx = shutdown_tx.clone();
            let metrics_tx = metrics_tx.clone();
            let log_sink = log_sink.clone();
            let rate_limiter = rate_limiter.clone();
            let request_limiter = request_limiter.clone();
            let in_flight_counter = in_flight_counter.clone();
            let request_fn = request_fn.clone();

            let handle = tokio::spawn(async move {
                let mut shutdown_rx_worker = shutdown_tx.subscribe();
                let startup_permit_result = tokio::select! {
                    _ = shutdown_rx_worker.recv() => return,
                    permit = permits.acquire_owned() => permit,
                };
                let _startup_permit = match startup_permit_result {
                    Ok(permit) => permit,
                    Err(_) => return,
                };

                loop {
                    let context = TransportRunContext {
                        shutdown_tx: &shutdown_tx,
                        rate_limiter: rate_limiter.as_ref(),
                        request_limiter: request_limiter.as_ref(),
                        in_flight_counter: &in_flight_counter,
                        metrics_tx: &metrics_tx,
                        log_sink: &log_sink,
                        wait_ongoing,
                        latency_correction,
                        expected_status_code,
                        request_timeout,
                        connect_timeout,
                        request_fn: request_fn.as_ref(),
                    };
                    let should_break =
                        run_transport_iteration(&mut shutdown_rx_worker, &context).await;

                    if should_break {
                        drop(shutdown_tx.send(()));
                        break;
                    }

                    if rate_limiter.is_none() {
                        sleep(Duration::from_millis(100)).await;
                    }
                }
            });

            worker_handles.push(handle);
        }

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = spawn_interval.tick() => {
                    if total_spawned >= max_tasks {
                        continue;
                    }
                    let available = max_tasks.saturating_sub(total_spawned);
                    let to_spawn = spawn_rate.min(available);
                    permits.add_permits(to_spawn);
                    total_spawned = total_spawned.saturating_add(to_spawn);
                }
            }
        }

        drop(permits);

        for handle in worker_handles {
            if handle.await.is_err() {
                break;
            }
        }
    })
}

async fn run_transport_iteration(
    shutdown_rx: &mut ShutdownReceiver,
    context: &TransportRunContext<'_>,
) -> bool {
    if context.wait_ongoing && shutdown_rx.try_recv().is_ok() {
        return true;
    }
    if let Some(limiter) = context.request_limiter
        && !limiter.try_reserve(context.shutdown_tx)
    {
        return true;
    }

    let mut latency_start = if context.latency_correction {
        Some(Instant::now())
    } else {
        None
    };

    if let Some(rate_limiter) = context.rate_limiter {
        let denied = tokio::select! {
            _ = shutdown_rx.recv() => true,
            permit = rate_limiter.acquire() => permit.is_err(),
        };
        if denied {
            return true;
        }
        if !context.latency_correction {
            latency_start = None;
        }
    }

    let run_request = async {
        let start = latency_start.unwrap_or_else(Instant::now);
        let in_flight_guard = InflightGuard::acquire(context.in_flight_counter);
        let outcome = (context.request_fn)(context.request_timeout, context.connect_timeout).await;
        drop(in_flight_guard);

        let in_flight_ops = context.in_flight_counter.load(Ordering::Relaxed);
        let status_code = if outcome.timed_out || outcome.transport_error {
            500
        } else {
            context.expected_status_code
        };
        let metric = Metrics::new(
            start,
            status_code,
            outcome.timed_out,
            outcome.transport_error,
            outcome.response_bytes,
            in_flight_ops,
        );
        if let Some(sink) = context.log_sink
            && !sink.send(metric)
        {
            return true;
        }
        if context.metrics_tx.try_send(metric).is_err() {
            // Ignore UI backpressure; summary/charts are log-based.
        }
        false
    };

    if context.wait_ongoing {
        run_request.await
    } else {
        tokio::select! {
            _ = shutdown_rx.recv() => true,
            result = run_request => result,
        }
    }
}
