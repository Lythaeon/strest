use std::sync::{Arc, atomic::AtomicU64};
use std::time::Duration;

use reqwest::Client;
use tokio::sync::{Semaphore, mpsc};
use tokio::time::{interval, sleep};
use tracing::{error, warn};

use crate::{
    args::TesterArgs,
    metrics::{LogSink, Metrics},
    shutdown::ShutdownSender,
};

use super::super::rate::build_rate_limiter;
use super::super::workload::{
    RequestLimiter, ScenarioRunContext, WorkerContext, Workload, preflight_request,
    run_scenario_iteration, run_single_dynamic_iteration, run_single_iteration,
};
use super::config::resolve_http2_parallel;

pub(super) fn create_sender_task(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<Arc<LogSink>>,
    client: Client,
    workload: Workload,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    let metrics_tx = metrics_tx.clone();
    let log_sink = log_sink;

    let http2_parallel = resolve_http2_parallel(args);
    let max_tasks = args.max_tasks.get().saturating_mul(http2_parallel);
    let spawn_rate = args
        .spawn_rate_per_tick
        .get()
        .saturating_mul(http2_parallel);
    let tick_interval = args.tick_interval.get();
    let rate_limit = args.rate_limit.map(u64::from);
    let load_profile = args.load_profile.clone();
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
        if let Err(err) = preflight_request(&client, &workload).await {
            error!("Test request failed: {}", err);
            drop(shutdown_tx.send(()));
            return;
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
            let client = client.clone();
            let workload = workload.clone();
            let rate_limiter = rate_limiter.clone();
            let request_limiter = request_limiter.clone();
            let in_flight_counter = in_flight_counter.clone();

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

                let mut request_seq: u64 = 0;
                loop {
                    let worker = WorkerContext {
                        shutdown_tx: &shutdown_tx,
                        rate_limiter: rate_limiter.as_ref(),
                        request_limiter: request_limiter.as_ref(),
                        in_flight_counter: &in_flight_counter,
                        client: &client,
                        log_sink: &log_sink,
                        metrics_tx: &metrics_tx,
                        wait_ongoing,
                        latency_correction,
                    };
                    let should_break = match &workload {
                        Workload::Single(request_template) => {
                            run_single_iteration(&mut shutdown_rx_worker, &worker, request_template)
                                .await
                        }
                        Workload::SingleDynamic(spec) => {
                            run_single_dynamic_iteration(&mut shutdown_rx_worker, &worker, spec)
                                .await
                        }
                        Workload::Scenario(scenario, connect_to, host_header, auth) => {
                            let mut context = ScenarioRunContext {
                                client: &client,
                                scenario,
                                connect_to,
                                host_header: host_header.as_deref(),
                                auth: auth.as_ref(),
                                expected_status_code,
                                log_sink: &log_sink,
                                metrics_tx: &metrics_tx,
                                request_seq: &mut request_seq,
                            };
                            run_scenario_iteration(&mut shutdown_rx_worker, &worker, &mut context)
                                .await
                        }
                    };

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
