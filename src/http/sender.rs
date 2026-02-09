use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client, Proxy};
use tokio::sync::Semaphore;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, sleep};
use tracing::error;

use crate::{
    args::{DEFAULT_USER_AGENT, HttpMethod, TesterArgs},
    metrics::{LogSink, Metrics},
};

use super::rate::build_rate_limiter;
use super::tls::apply_tls_settings;
use super::workload::{
    ScenarioRunContext, Workload, preflight_request, run_scenario_iteration, run_single_iteration,
};

/// Creates the request sender task and validates the HTTP client/config.
///
/// # Errors
///
/// Returns an error when the HTTP client or request template cannot be built.
pub fn setup_request_sender(
    args: &TesterArgs,
    shutdown_tx: &broadcast::Sender<u16>,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> Result<tokio::task::JoinHandle<()>, String> {
    let shutdown_tx = shutdown_tx.clone();
    let metrics_tx = metrics_tx.clone();

    let mut client_builder = Client::builder().timeout(args.request_timeout);

    if !args.no_ua {
        client_builder = client_builder.user_agent(DEFAULT_USER_AGENT);
    }

    client_builder = apply_tls_settings(client_builder, args)?;

    if let Some(ref proxy_url) = args.proxy_url {
        match Proxy::all(proxy_url) {
            Ok(proxy) => {
                client_builder = client_builder.proxy(proxy);
            }
            Err(e) => {
                error!("Invalid proxy URL '{}': {}", proxy_url, e);
                return Err(format!("Invalid proxy URL '{}': {}", proxy_url, e));
            }
        }
    }

    let client = match client_builder.build() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to build HTTP client: {}", e);
            return Err(format!("Failed to build HTTP client: {}", e));
        }
    };

    let workload = if let Some(scenario) = args.scenario.clone() {
        Workload::Scenario(Arc::new(scenario))
    } else {
        let url = args
            .url
            .as_deref()
            .ok_or_else(|| "Missing URL (set --url or provide in config).".to_owned())?;

        let mut request_builder = match args.method {
            HttpMethod::Get => client.get(url),
            HttpMethod::Post => client.post(url),
            HttpMethod::Patch => client.patch(url),
            HttpMethod::Put => client.put(url),
            HttpMethod::Delete => client.delete(url),
        };

        for (key, value) in &args.headers {
            request_builder = request_builder.header(key, value);
        }

        let request = match request_builder.body(args.data.clone()).build() {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to build request: {}", e);
                return Err(format!("Failed to build request: {}", e));
            }
        };

        Workload::Single(Arc::new(request))
    };

    Ok(create_sender_task(
        args,
        &shutdown_tx,
        &metrics_tx,
        log_sink.cloned(),
        client,
        workload,
    ))
}

fn create_sender_task(
    args: &TesterArgs,
    shutdown_tx: &broadcast::Sender<u16>,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<Arc<LogSink>>,
    client: Client,
    workload: Workload,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    let metrics_tx = metrics_tx.clone();
    let log_sink = log_sink;

    let max_tasks = args.max_tasks.get();
    let spawn_rate = args.spawn_rate_per_tick.get();
    let tick_interval = args.tick_interval.get();
    let rate_limit = args.rate_limit.map(u64::from);
    let load_profile = args.load_profile.clone();
    let expected_status_code = args.expected_status_code;

    tokio::spawn(async move {
        if let Err(err) = preflight_request(&client, &workload).await {
            error!("Test request failed: {}", err);
            drop(shutdown_tx.send(1));
            return;
        }

        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut spawn_interval = interval(Duration::from_millis(tick_interval));
        let mut total_spawned: usize = 0;
        let permits = Arc::new(Semaphore::new(0));
        let rate_limiter = build_rate_limiter(rate_limit, load_profile.as_ref());
        let mut worker_handles = Vec::with_capacity(max_tasks);

        for _ in 0..max_tasks {
            let permits = Arc::clone(&permits);
            let shutdown_tx = shutdown_tx.clone();
            let metrics_tx = metrics_tx.clone();
            let log_sink = log_sink.clone();
            let client = client.clone();
            let workload = workload.clone();
            let rate_limiter = rate_limiter.clone();

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
                    let should_break = match &workload {
                        Workload::Single(request_template) => {
                            run_single_iteration(
                                &mut shutdown_rx_worker,
                                rate_limiter.as_ref(),
                                &client,
                                request_template,
                                &log_sink,
                                &metrics_tx,
                            )
                            .await
                        }
                        Workload::Scenario(scenario) => {
                            let mut context = ScenarioRunContext {
                                client: &client,
                                scenario,
                                expected_status_code,
                                log_sink: &log_sink,
                                metrics_tx: &metrics_tx,
                                request_seq: &mut request_seq,
                            };
                            run_scenario_iteration(
                                &mut shutdown_rx_worker,
                                rate_limiter.as_ref(),
                                &mut context,
                            )
                            .await
                        }
                    };

                    if should_break {
                        drop(shutdown_tx.send(1));
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
