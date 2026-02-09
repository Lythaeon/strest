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
    BodySource, RequestLimiter, ScenarioRunContext, SingleRequestSpec, WorkerContext, Workload,
    preflight_request, run_scenario_iteration, run_single_dynamic_iteration, run_single_iteration,
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

    let mut client_builder = Client::builder()
        .timeout(args.request_timeout)
        .connect_timeout(args.connect_timeout);

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

        let body_source = resolve_body_source(args)?;
        let headers = build_headers(args);

        match body_source {
            BodySource::Lines(_, _) => Workload::SingleDynamic(Arc::new(SingleRequestSpec {
                method: args.method,
                url: url.to_owned(),
                headers,
                body: body_source,
            })),
            BodySource::Static(body) => {
                let mut request_builder = match args.method {
                    HttpMethod::Get => client.get(url),
                    HttpMethod::Post => client.post(url),
                    HttpMethod::Patch => client.patch(url),
                    HttpMethod::Put => client.put(url),
                    HttpMethod::Delete => client.delete(url),
                };

                for (key, value) in &headers {
                    request_builder = request_builder.header(key, value);
                }

                let request = match request_builder.body(body).build() {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Failed to build request: {}", e);
                        return Err(format!("Failed to build request: {}", e));
                    }
                };

                Workload::Single(Arc::new(request))
            }
        }
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
    let request_limiter = RequestLimiter::new(args.requests.map(u64::from)).map(Arc::new);

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
            let request_limiter = request_limiter.clone();

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
                        client: &client,
                        log_sink: &log_sink,
                        metrics_tx: &metrics_tx,
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
                        Workload::Scenario(scenario) => {
                            let mut context = ScenarioRunContext {
                                client: &client,
                                scenario,
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

fn build_headers(args: &TesterArgs) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if let Some(accept) = args.accept_header.as_ref() {
        headers.push(("Accept".to_owned(), accept.clone()));
    }
    if let Some(content_type) = args.content_type.as_ref() {
        headers.push(("Content-Type".to_owned(), content_type.clone()));
    }
    headers.extend(args.headers.clone());
    headers
}

fn resolve_body_source(args: &TesterArgs) -> Result<BodySource, String> {
    if let Some(path) = args.data_lines.as_ref() {
        let content = std::fs::read_to_string(path)
            .map_err(|err| format!("Failed to read {}: {}", path, err))?;
        let lines: Vec<String> = content.lines().map(|line| line.to_owned()).collect();
        if lines.is_empty() {
            return Err(format!("Body lines file '{}' was empty.", path));
        }
        return Ok(BodySource::Lines(
            Arc::new(lines),
            Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        ));
    }

    if let Some(path) = args.data_file.as_ref() {
        let content = std::fs::read_to_string(path)
            .map_err(|err| format!("Failed to read {}: {}", path, err))?;
        return Ok(BodySource::Static(content));
    }

    Ok(BodySource::Static(args.data.clone()))
}
