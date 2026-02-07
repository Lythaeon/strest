use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::{Client, Request, Url};
use tokio::sync::Semaphore;
use tokio::{
    sync::{broadcast, mpsc},
    time::{Instant, sleep},
};
use tracing::error;

use crate::{
    args::{HttpMethod, Scenario, ScenarioStep},
    metrics::{LogSink, Metrics},
};

const ASSERT_FAILED_STATUS: u16 = 0;

#[derive(Clone)]
pub(super) enum Workload {
    Single(Arc<Request>),
    Scenario(Arc<Scenario>),
}

pub(super) async fn preflight_request(client: &Client, workload: &Workload) -> Result<(), String> {
    match workload {
        Workload::Single(request_template) => {
            let request = request_template
                .try_clone()
                .ok_or_else(|| "Failed to clone request for initial test.".to_owned())?;
            execute_request(client, request, true)
                .await
                .map_err(|err| format!("Test request failed: {}", err))?;
            Ok(())
        }
        Workload::Scenario(scenario) => {
            let step = scenario
                .steps
                .first()
                .ok_or_else(|| "Scenario has no steps.".to_owned())?;
            let vars = build_template_vars(scenario, step, 0, 0);
            let request = build_step_request(client, scenario, step, &vars)?;
            execute_request(client, request, true)
                .await
                .map_err(|err| format!("Scenario preflight failed: {}", err))?;
            Ok(())
        }
    }
}

pub(super) async fn run_single_iteration(
    shutdown_rx: &mut broadcast::Receiver<u16>,
    rate_limiter: Option<&Arc<Semaphore>>,
    client: &Client,
    request_template: &Arc<Request>,
    log_sink: &Option<Arc<LogSink>>,
    metrics_tx: &mpsc::Sender<Metrics>,
) -> bool {
    if let Some(rate_limiter) = rate_limiter {
        let rate_permit_result = tokio::select! {
            _ = shutdown_rx.recv() => return true,
            permit = rate_limiter.acquire() => permit,
        };
        if rate_permit_result.is_err() {
            return true;
        }
    }

    let should_stop = tokio::select! {
        _ = shutdown_rx.recv() => true,
        result = async {
            let start = Instant::now();
            let status = match request_template.try_clone() {
                Some(req_clone) => execute_request_status(client, req_clone).await,
                None => {
                    error!("Failed to clone request template.");
                    500
                }
            };
            let metric = Metrics::new(start, status);
            if let Some(log_sink) = log_sink && !log_sink.send(metric) {
                return true;
            }
            if metrics_tx.try_send(metric).is_err() {
                // Ignore UI backpressure; summary and charts use log pipeline.
            }
            false
        } => result,
    };

    should_stop
}

pub(super) struct ScenarioRunContext<'ctx> {
    pub(super) client: &'ctx Client,
    pub(super) scenario: &'ctx Scenario,
    pub(super) expected_status_code: u16,
    pub(super) log_sink: &'ctx Option<Arc<LogSink>>,
    pub(super) metrics_tx: &'ctx mpsc::Sender<Metrics>,
    pub(super) request_seq: &'ctx mut u64,
}

pub(super) async fn run_scenario_iteration(
    shutdown_rx: &mut broadcast::Receiver<u16>,
    rate_limiter: Option<&Arc<Semaphore>>,
    context: &mut ScenarioRunContext<'_>,
) -> bool {
    for (step_index, step) in context.scenario.steps.iter().enumerate() {
        if let Some(rate_limiter) = rate_limiter {
            let rate_permit_result = tokio::select! {
                _ = shutdown_rx.recv() => return true,
                permit = rate_limiter.acquire() => permit,
            };
            if rate_permit_result.is_err() {
                return true;
            }
        }

        let vars = build_template_vars(context.scenario, step, *context.request_seq, step_index);
        let request = match build_step_request(context.client, context.scenario, step, &vars) {
            Ok(request) => request,
            Err(err) => {
                error!("Failed to build scenario request: {}", err);
                return true;
            }
        };

        let expected = step.assert_status.unwrap_or(context.expected_status_code);
        let start = Instant::now();
        let outcome = tokio::select! {
            _ = shutdown_rx.recv() => return true,
            result = execute_request_with_asserts(
                context.client,
                request,
                context.expected_status_code,
                step.assert_status,
                step.assert_body_contains.as_deref(),
            ) => result,
        };

        if !outcome.success {
            let label = step_label(step, step_index);
            if let Some(fragment) = step.assert_body_contains.as_deref() {
                error!(
                    "Scenario step {} failed: status {} (expected {}) or body missing '{}'.",
                    label, outcome.status, expected, fragment
                );
            } else {
                error!(
                    "Scenario step {} failed: status {} (expected {}).",
                    label, outcome.status, expected
                );
            }
        }

        let metric_status = if outcome.success {
            context.expected_status_code
        } else {
            ASSERT_FAILED_STATUS
        };
        let metric = Metrics::new(start, metric_status);
        if let Some(log_sink) = context.log_sink
            && !log_sink.send(metric)
        {
            return true;
        }
        if context.metrics_tx.try_send(metric).is_err() {
            // Ignore UI backpressure; summary and charts use log pipeline.
        }

        *context.request_seq = context.request_seq.saturating_add(1);

        if let Some(think_time) = step.think_time {
            tokio::select! {
                _ = shutdown_rx.recv() => return true,
                () = sleep(think_time) => {},
            };
        }
    }

    false
}

#[derive(Debug)]
struct RequestOutcome {
    status: u16,
    success: bool,
}

async fn execute_request_with_asserts(
    client: &Client,
    request: Request,
    expected_status_code: u16,
    assert_status: Option<u16>,
    assert_body_contains: Option<&str>,
) -> RequestOutcome {
    match client.execute(request).await {
        Ok(response) => {
            let status = response.status().as_u16();
            let expected = assert_status.unwrap_or(expected_status_code);
            let status_ok = status == expected;

            let body_result = response.bytes().await;
            let body_ok = match (assert_body_contains, body_result) {
                (Some(fragment), Ok(bytes)) => {
                    let body = String::from_utf8_lossy(&bytes);
                    body.contains(fragment)
                }
                (Some(_), Err(err)) => {
                    error!("Failed to read response body: {}", err);
                    false
                }
                (None, Ok(_)) => true,
                (None, Err(err)) => {
                    error!("Failed to read response body: {}", err);
                    false
                }
            };

            RequestOutcome {
                status,
                success: status_ok && body_ok,
            }
        }
        Err(err) => {
            error!("Request failed: {}", err);
            RequestOutcome {
                status: 500,
                success: false,
            }
        }
    }
}

pub(crate) fn build_step_request(
    client: &Client,
    scenario: &Scenario,
    step: &ScenarioStep,
    vars: &BTreeMap<String, String>,
) -> Result<Request, String> {
    let url = resolve_step_url(scenario, step, vars)?;
    let mut request_builder = match step.method {
        HttpMethod::Get => client.get(url),
        HttpMethod::Post => client.post(url),
        HttpMethod::Patch => client.patch(url),
        HttpMethod::Put => client.put(url),
        HttpMethod::Delete => client.delete(url),
    };

    for (key, value) in &step.headers {
        let key_rendered = render_template(key, vars);
        let value_rendered = render_template(value, vars);
        request_builder = request_builder.header(key_rendered, value_rendered);
    }

    if let Some(body) = step.body.as_ref() {
        let body_rendered = render_template(body, vars);
        request_builder = request_builder.body(body_rendered);
    }

    request_builder
        .build()
        .map_err(|err| format!("Failed to build request: {}", err))
}

fn resolve_step_url(
    scenario: &Scenario,
    step: &ScenarioStep,
    vars: &BTreeMap<String, String>,
) -> Result<String, String> {
    if let Some(url) = step.url.as_ref() {
        return Ok(render_template(url, vars));
    }

    let path = step
        .path
        .as_ref()
        .ok_or_else(|| "Scenario step missing url/path.".to_owned())?;
    let base_url = scenario
        .base_url
        .as_ref()
        .ok_or_else(|| "Scenario base_url is required for relative paths.".to_owned())?;
    let rendered_path = render_template(path, vars);
    let base = Url::parse(base_url)
        .map_err(|err| format!("Invalid scenario base_url '{}': {}", base_url, err))?;
    let joined = base
        .join(&rendered_path)
        .map_err(|err| format!("Failed to join URL '{}': {}", rendered_path, err))?;
    Ok(joined.to_string())
}

pub(crate) fn build_template_vars(
    scenario: &Scenario,
    step: &ScenarioStep,
    seq: u64,
    step_index: usize,
) -> BTreeMap<String, String> {
    let mut vars = scenario.vars.clone();
    for (key, value) in &step.vars {
        vars.insert(key.clone(), value.clone());
    }

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis());
    vars.insert("seq".to_owned(), seq.to_string());
    vars.insert("step".to_owned(), step_index.saturating_add(1).to_string());
    vars.insert("timestamp_ms".to_owned(), now_ms.to_string());
    vars.insert("timestamp_s".to_owned(), (now_ms / 1000).to_string());

    vars
}

pub(crate) fn render_template(input: &str, vars: &BTreeMap<String, String>) -> String {
    let mut rest = input;
    let mut output = String::with_capacity(input.len());

    loop {
        let start = match rest.find("{{") {
            Some(start) => start,
            None => {
                output.push_str(rest);
                break;
            }
        };
        let (before, after_start) = rest.split_at(start);
        output.push_str(before);
        let after = match after_start.strip_prefix("{{") {
            Some(after) => after,
            None => {
                output.push_str(after_start);
                break;
            }
        };
        let end = match after.find("}}") {
            Some(end) => end,
            None => {
                output.push_str("{{");
                output.push_str(after);
                break;
            }
        };
        let (key_part, after_end) = after.split_at(end);
        let key = key_part.trim();
        if let Some(value) = vars.get(key) {
            output.push_str(value);
        } else {
            output.push_str("{{");
            output.push_str(key);
            output.push_str("}}");
        }
        rest = match after_end.strip_prefix("}}") {
            Some(remaining) => remaining,
            None => {
                output.push_str(after_end);
                break;
            }
        };
    }

    output
}

fn step_label(step: &ScenarioStep, step_index: usize) -> String {
    step.name
        .clone()
        .unwrap_or_else(|| format!("step {}", step_index.saturating_add(1)))
}

async fn execute_request(
    client: &Client,
    request: Request,
    drain_body: bool,
) -> Result<u16, reqwest::Error> {
    let response = client.execute(request).await?;
    let status = response.status().as_u16();
    if drain_body {
        response.bytes().await?;
    }
    Ok(status)
}

async fn execute_request_status(client: &Client, request: Request) -> u16 {
    match client.execute(request).await {
        Ok(response) => {
            let status = response.status().as_u16();
            match response.bytes().await {
                Ok(_) => status,
                Err(_) => 500,
            }
        }
        Err(_) => 500,
    }
}
