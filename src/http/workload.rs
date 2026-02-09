use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use rand::distributions::Distribution;
use rand::thread_rng;
use rand_regex::Regex as RandRegex;
use reqwest::{Client, Request, RequestBuilder, Url};
use tokio::sync::Semaphore;
use tokio::{
    sync::{broadcast, mpsc},
    time::{Instant, sleep},
};
use tracing::error;

use aws_credential_types::Credentials;
use aws_sigv4::http_request::{SignableBody, SignableRequest, SigningSettings, sign};
use aws_sigv4::sign::v4;
use aws_smithy_runtime_api::client::identity::Identity;
use base64::Engine as _;

use crate::{
    args::{ConnectToMapping, HttpMethod, Scenario, ScenarioStep},
    metrics::{LogSink, Metrics},
};

const ASSERT_FAILED_STATUS: u16 = 0;

#[derive(Clone)]
pub(super) enum Workload {
    Single(Arc<Request>),
    SingleDynamic(Arc<SingleRequestSpec>),
    Scenario(
        Arc<Scenario>,
        Arc<Vec<ConnectToMapping>>,
        Option<String>,
        Option<AuthConfig>,
    ),
}

#[derive(Debug, Clone)]
pub(crate) enum AuthConfig {
    Basic {
        username: String,
        password: String,
    },
    SigV4 {
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        region: String,
        service: String,
    },
}

#[derive(Debug)]
pub(super) struct RequestLimiter {
    limit: Option<u64>,
    counter: Arc<AtomicU64>,
}

impl RequestLimiter {
    pub(super) fn new(limit: Option<u64>) -> Option<Self> {
        limit.map(|limit| RequestLimiter {
            limit: Some(limit),
            counter: Arc::new(AtomicU64::new(0)),
        })
    }

    pub(super) fn try_reserve(&self, shutdown_tx: &broadcast::Sender<u16>) -> bool {
        let Some(limit) = self.limit else {
            return true;
        };
        loop {
            let current = self.counter.load(Ordering::Relaxed);
            if current >= limit {
                drop(shutdown_tx.send(1));
                return false;
            }
            let Some(next) = current.checked_add(1) else {
                drop(shutdown_tx.send(1));
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

#[derive(Clone)]
pub(super) enum BodySource {
    Static(String),
    Lines(Arc<Vec<String>>, Arc<AtomicUsize>),
}

#[derive(Clone)]
pub(super) enum UrlSource {
    Static(String),
    List(Arc<Vec<String>>, Arc<AtomicUsize>),
    Regex(Arc<RandRegex>),
}

impl UrlSource {
    pub(super) fn next_url(&self) -> Result<String, String> {
        match self {
            UrlSource::Static(url) => Ok(url.clone()),
            UrlSource::List(list, cursor) => {
                if list.is_empty() {
                    return Err("URL list was empty.".to_owned());
                }
                let idx = cursor.fetch_add(1, Ordering::Relaxed);
                let len = list.len();
                let selected = idx.rem_euclid(len);
                list.get(selected)
                    .cloned()
                    .ok_or_else(|| "URL list was empty.".to_owned())
            }
            UrlSource::Regex(regex) => {
                let mut rng = thread_rng();
                Ok(regex.sample(&mut rng))
            }
        }
    }
}

#[derive(Clone)]
pub(super) enum FormFieldSpec {
    Text { name: String, value: String },
    File { name: String, path: String },
}

#[derive(Clone)]
pub(super) struct SingleRequestSpec {
    pub(super) method: HttpMethod,
    pub(super) url: UrlSource,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: BodySource,
    pub(super) form: Option<Arc<Vec<FormFieldSpec>>>,
    pub(super) connect_to: Arc<Vec<ConnectToMapping>>,
    pub(super) auth: Option<AuthConfig>,
}

pub(super) struct WorkerContext<'ctx> {
    pub(super) shutdown_tx: &'ctx broadcast::Sender<u16>,
    pub(super) rate_limiter: Option<&'ctx Arc<Semaphore>>,
    pub(super) request_limiter: Option<&'ctx Arc<RequestLimiter>>,
    pub(super) wait_ongoing: bool,
    pub(super) latency_correction: bool,
    pub(super) client: &'ctx Client,
    pub(super) log_sink: &'ctx Option<Arc<LogSink>>,
    pub(super) metrics_tx: &'ctx mpsc::Sender<Metrics>,
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
        Workload::SingleDynamic(spec) => {
            let request = build_request_from_spec(client, spec)?;
            execute_request(client, request, true)
                .await
                .map_err(|err| format!("Test request failed: {}", err))?;
            Ok(())
        }
        Workload::Scenario(scenario, connect_to, host_header, auth) => {
            let step = scenario
                .steps
                .first()
                .ok_or_else(|| "Scenario has no steps.".to_owned())?;
            let vars = build_template_vars(scenario, step, 0, 0);
            let request = build_step_request(
                client,
                scenario,
                step,
                &vars,
                &StepRequestContext {
                    connect_to,
                    host_header: host_header.as_deref(),
                    auth: auth.as_ref(),
                },
            )?;
            execute_request(client, request, true)
                .await
                .map_err(|err| format!("Scenario preflight failed: {}", err))?;
            Ok(())
        }
    }
}

pub(super) async fn run_single_iteration(
    shutdown_rx: &mut broadcast::Receiver<u16>,
    context: &WorkerContext<'_>,
    request_template: &Arc<Request>,
) -> bool {
    if context.wait_ongoing && shutdown_rx.try_recv().is_ok() {
        return true;
    }
    if let Some(request_limiter) = context.request_limiter
        && !request_limiter.try_reserve(context.shutdown_tx)
    {
        return true;
    }
    let mut latency_start = if context.latency_correction {
        Some(Instant::now())
    } else {
        None
    };
    if let Some(rate_limiter) = context.rate_limiter {
        let rate_permit_result = tokio::select! {
            _ = shutdown_rx.recv() => return true,
            permit = rate_limiter.acquire() => permit,
        };
        if rate_permit_result.is_err() {
            return true;
        }
        if !context.latency_correction {
            latency_start = None;
        }
    }

    let run_request = async {
        let start = latency_start.unwrap_or_else(Instant::now);
        let (status, timed_out, transport_error) = match request_template.try_clone() {
            Some(req_clone) => execute_request_status(context.client, req_clone).await,
            None => {
                error!("Failed to clone request template.");
                (500, false, true)
            }
        };
        let metric = Metrics::new(start, status, timed_out, transport_error);
        if let Some(log_sink) = context.log_sink
            && !log_sink.send(metric)
        {
            return true;
        }
        if context.metrics_tx.try_send(metric).is_err() {
            // Ignore UI backpressure; summary and charts use log pipeline.
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

pub(super) async fn run_single_dynamic_iteration(
    shutdown_rx: &mut broadcast::Receiver<u16>,
    context: &WorkerContext<'_>,
    spec: &Arc<SingleRequestSpec>,
) -> bool {
    if context.wait_ongoing && shutdown_rx.try_recv().is_ok() {
        return true;
    }
    if let Some(request_limiter) = context.request_limiter
        && !request_limiter.try_reserve(context.shutdown_tx)
    {
        return true;
    }
    let mut latency_start = if context.latency_correction {
        Some(Instant::now())
    } else {
        None
    };
    if let Some(rate_limiter) = context.rate_limiter {
        let rate_permit_result = tokio::select! {
            _ = shutdown_rx.recv() => return true,
            permit = rate_limiter.acquire() => permit,
        };
        if rate_permit_result.is_err() {
            return true;
        }
        if !context.latency_correction {
            latency_start = None;
        }
    }

    let run_request = async {
        let request = match build_request_from_spec(context.client, spec) {
            Ok(request) => request,
            Err(err) => {
                error!("Failed to build request: {}", err);
                return true;
            }
        };
        let start = latency_start.unwrap_or_else(Instant::now);
        let (status, timed_out, transport_error) =
            execute_request_status(context.client, request).await;
        let metric = Metrics::new(start, status, timed_out, transport_error);
        if let Some(log_sink) = context.log_sink
            && !log_sink.send(metric)
        {
            return true;
        }
        if context.metrics_tx.try_send(metric).is_err() {
            // Ignore UI backpressure; summary and charts use log pipeline.
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

pub(super) struct ScenarioRunContext<'ctx> {
    pub(super) client: &'ctx Client,
    pub(super) scenario: &'ctx Scenario,
    pub(super) connect_to: &'ctx [ConnectToMapping],
    pub(super) host_header: Option<&'ctx str>,
    pub(super) auth: Option<&'ctx AuthConfig>,
    pub(super) expected_status_code: u16,
    pub(super) log_sink: &'ctx Option<Arc<LogSink>>,
    pub(super) metrics_tx: &'ctx mpsc::Sender<Metrics>,
    pub(super) request_seq: &'ctx mut u64,
}

pub(super) async fn run_scenario_iteration(
    shutdown_rx: &mut broadcast::Receiver<u16>,
    worker: &WorkerContext<'_>,
    context: &mut ScenarioRunContext<'_>,
) -> bool {
    for (step_index, step) in context.scenario.steps.iter().enumerate() {
        if worker.wait_ongoing && shutdown_rx.try_recv().is_ok() {
            return true;
        }
        if let Some(request_limiter) = worker.request_limiter
            && !request_limiter.try_reserve(worker.shutdown_tx)
        {
            return true;
        }
        let mut latency_start = if worker.latency_correction {
            Some(Instant::now())
        } else {
            None
        };
        if let Some(rate_limiter) = worker.rate_limiter {
            let rate_permit_result = tokio::select! {
                _ = shutdown_rx.recv() => return true,
                permit = rate_limiter.acquire() => permit,
            };
            if rate_permit_result.is_err() {
                return true;
            }
            if !worker.latency_correction {
                latency_start = None;
            }
        }

        let vars = build_template_vars(context.scenario, step, *context.request_seq, step_index);
        let request = match build_step_request(
            context.client,
            context.scenario,
            step,
            &vars,
            &StepRequestContext {
                connect_to: context.connect_to,
                host_header: context.host_header,
                auth: context.auth,
            },
        ) {
            Ok(request) => request,
            Err(err) => {
                error!("Failed to build scenario request: {}", err);
                return true;
            }
        };

        let expected = step.assert_status.unwrap_or(context.expected_status_code);
        let start = latency_start.unwrap_or_else(Instant::now);
        let run_request = async {
            execute_request_with_asserts(
                context.client,
                request,
                context.expected_status_code,
                step.assert_status,
                step.assert_body_contains.as_deref(),
            )
            .await
        };
        let outcome = if worker.wait_ongoing {
            run_request.await
        } else {
            tokio::select! {
                _ = shutdown_rx.recv() => return true,
                result = run_request => result,
            }
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
        let metric = Metrics::new(
            start,
            metric_status,
            outcome.timed_out,
            outcome.transport_error,
        );
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
    timed_out: bool,
    transport_error: bool,
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

            let body_result = match assert_body_contains {
                Some(fragment) => drain_body_contains(response, fragment).await,
                None => drain_response_body(response).await.map(|()| false),
            };
            let mut timed_out = false;
            let mut transport_error = false;
            let body_ok = match (assert_body_contains, body_result) {
                (Some(_), Ok(found)) => found,
                (Some(_), Err(err)) => {
                    timed_out = err.is_timeout();
                    transport_error = !timed_out;
                    error!("Failed to read response body: {}", err);
                    false
                }
                (None, Ok(_)) => true,
                (None, Err(err)) => {
                    timed_out = err.is_timeout();
                    transport_error = !timed_out;
                    error!("Failed to read response body: {}", err);
                    false
                }
            };

            RequestOutcome {
                status,
                success: status_ok && body_ok,
                timed_out,
                transport_error,
            }
        }
        Err(err) => {
            error!("Request failed: {}", err);
            let timed_out = err.is_timeout();
            RequestOutcome {
                status: 500,
                success: false,
                timed_out,
                transport_error: !timed_out,
            }
        }
    }
}

fn build_request_from_spec(client: &Client, spec: &SingleRequestSpec) -> Result<Request, String> {
    let url_raw = spec.url.next_url()?;
    let url = Url::parse(&url_raw).map_err(|err| format!("Invalid URL '{}': {}", url_raw, err))?;
    let (url, host_override) = apply_connect_to(&url, &spec.connect_to)?;

    let mut request_builder = match spec.method {
        HttpMethod::Get => client.get(url.clone()),
        HttpMethod::Post => client.post(url.clone()),
        HttpMethod::Patch => client.patch(url.clone()),
        HttpMethod::Put => client.put(url.clone()),
        HttpMethod::Delete => client.delete(url.clone()),
    };

    for (key, value) in &spec.headers {
        request_builder = request_builder.header(key, value);
    }
    if let Some(host) = host_override.as_ref()
        && !has_host_header(&spec.headers)
    {
        request_builder = request_builder.header("Host", host);
    }

    let body = match &spec.body {
        BodySource::Static(body) => body.clone(),
        BodySource::Lines(lines, cursor) => {
            if lines.is_empty() {
                return Err("Body lines file was empty.".to_owned());
            }
            let idx = cursor.fetch_add(1, Ordering::Relaxed);
            let len = lines.len();
            let selected = idx.rem_euclid(len);
            lines
                .get(selected)
                .cloned()
                .ok_or_else(|| "Body lines file was empty.".to_owned())?
        }
    };

    if let Some(auth) = spec.auth.as_ref() {
        let mut headers_for_sign = spec.headers.clone();
        if let Some(host) = host_override.as_ref()
            && !has_host_header(&headers_for_sign)
        {
            headers_for_sign.push(("Host".to_owned(), host.clone()));
        }
        request_builder = apply_auth_headers(
            request_builder,
            spec.method,
            &url,
            &headers_for_sign,
            &body,
            auth,
        )?;
    }

    if let Some(form) = spec.form.as_ref() {
        let multipart = build_multipart(form)?;
        request_builder = request_builder.multipart(multipart);
    } else {
        request_builder = request_builder.body(body);
    }

    request_builder
        .build()
        .map_err(|err| format!("Failed to build request: {}", err))
}

fn build_multipart(fields: &[FormFieldSpec]) -> Result<reqwest::multipart::Form, String> {
    let mut form = reqwest::multipart::Form::new();
    for field in fields {
        match field {
            FormFieldSpec::Text { name, value } => {
                form = form.text(name.clone(), value.clone());
            }
            FormFieldSpec::File { name, path } => {
                let bytes = std::fs::read(path)
                    .map_err(|err| format!("Failed to read form file '{}': {}", path, err))?;
                let part = reqwest::multipart::Part::bytes(bytes).file_name(
                    std::path::Path::new(path)
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("file")
                        .to_owned(),
                );
                form = form.part(name.clone(), part);
            }
        }
    }
    Ok(form)
}

pub(crate) struct StepRequestContext<'ctx> {
    pub connect_to: &'ctx [ConnectToMapping],
    pub host_header: Option<&'ctx str>,
    pub auth: Option<&'ctx AuthConfig>,
}

pub(crate) fn build_step_request(
    client: &Client,
    scenario: &Scenario,
    step: &ScenarioStep,
    vars: &BTreeMap<String, String>,
    context: &StepRequestContext<'_>,
) -> Result<Request, String> {
    let url = resolve_step_url(scenario, step, vars)?;
    let (url, host_override) = apply_connect_to(&url, context.connect_to)?;
    let mut request_builder = match step.method {
        HttpMethod::Get => client.get(url.clone()),
        HttpMethod::Post => client.post(url.clone()),
        HttpMethod::Patch => client.patch(url.clone()),
        HttpMethod::Put => client.put(url.clone()),
        HttpMethod::Delete => client.delete(url.clone()),
    };

    let mut rendered_headers = Vec::with_capacity(step.headers.len());
    for (key, value) in &step.headers {
        let key_rendered = render_template(key, vars);
        let value_rendered = render_template(value, vars);
        request_builder = request_builder.header(&key_rendered, &value_rendered);
        rendered_headers.push((key_rendered, value_rendered));
    }
    if !has_host_header(&rendered_headers) {
        if let Some(host) = context.host_header {
            request_builder = request_builder.header("Host", host);
        } else if let Some(host) = host_override.as_ref() {
            request_builder = request_builder.header("Host", host);
        }
    }

    let body_rendered = step
        .body
        .as_ref()
        .map(|body| render_template(body, vars))
        .unwrap_or_default();
    if let Some(auth) = context.auth {
        let mut headers_for_sign = rendered_headers.clone();
        if !has_host_header(&headers_for_sign) {
            if let Some(host) = context.host_header {
                headers_for_sign.push(("Host".to_owned(), host.to_owned()));
            } else if let Some(host) = host_override.as_ref() {
                headers_for_sign.push(("Host".to_owned(), host.clone()));
            }
        }
        request_builder = apply_auth_headers(
            request_builder,
            step.method,
            &url,
            &headers_for_sign,
            &body_rendered,
            auth,
        )?;
    }

    if let Some(body) = step.body.as_ref() {
        let body_rendered_step = render_template(body, vars);
        request_builder = request_builder.body(body_rendered_step);
    }

    request_builder
        .build()
        .map_err(|err| format!("Failed to build request: {}", err))
}

fn apply_connect_to(
    url: &Url,
    connect_to: &[ConnectToMapping],
) -> Result<(Url, Option<String>), String> {
    let Some(host) = url.host_str() else {
        return Ok((url.clone(), None));
    };
    let port = url.port_or_known_default().unwrap_or(80);
    for mapping in connect_to {
        if mapping.source_host == host && mapping.source_port == port {
            let mut rewritten = url.clone();
            rewritten
                .set_host(Some(&mapping.target_host))
                .map_err(|err| format!("Invalid connect-to host: {}", err))?;
            rewritten
                .set_port(Some(mapping.target_port))
                .map_err(|()| "Invalid connect-to port.".to_owned())?;
            let host_header = if port == 80 || port == 443 {
                host.to_owned()
            } else {
                format!("{}:{}", host, port)
            };
            return Ok((rewritten, Some(host_header)));
        }
    }
    Ok((url.clone(), None))
}

fn has_host_header(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case("host"))
}

const fn http_method_str(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
    }
}

fn apply_auth_headers(
    mut builder: RequestBuilder,
    method: HttpMethod,
    url: &Url,
    headers: &[(String, String)],
    body: &str,
    auth: &AuthConfig,
) -> Result<RequestBuilder, String> {
    match auth {
        AuthConfig::Basic { username, password } => {
            let token = format!("{}:{}", username, password);
            let encoded = base64::engine::general_purpose::STANDARD.encode(token.as_bytes());
            builder = builder.header("Authorization", format!("Basic {}", encoded));
            Ok(builder)
        }
        AuthConfig::SigV4 {
            access_key,
            secret_key,
            session_token,
            region,
            service,
        } => {
            let identity: Identity = Credentials::new(
                access_key,
                secret_key,
                session_token.clone(),
                None,
                "strest",
            )
            .into();
            let signing_settings = SigningSettings::default();
            let signing_params = v4::SigningParams::builder()
                .identity(&identity)
                .region(region)
                .name(service)
                .time(std::time::SystemTime::now())
                .settings(signing_settings)
                .build()
                .map_err(|err| format!("Failed to build sigv4 params: {}", err))?
                .into();

            let method_str = http_method_str(method);
            let signable = SignableRequest::new(
                method_str,
                url.as_str(),
                headers.iter().map(|(k, v)| (k.as_str(), v.as_str())),
                SignableBody::Bytes(body.as_bytes()),
            )
            .map_err(|err| format!("Failed to build sigv4 request: {}", err))?;

            let (instructions, _signature) = sign(signable, &signing_params)
                .map_err(|err| format!("Failed to sign request: {}", err))?
                .into_parts();

            let mut http_req = http::Request::builder()
                .method(method_str)
                .uri(url.as_str());
            for (key, value) in headers {
                http_req = http_req.header(key, value);
            }
            let mut http_req = http_req
                .body(())
                .map_err(|err| format!("Failed to build sign request: {}", err))?;
            instructions.apply_to_request_http1x(&mut http_req);

            for (name, value) in http_req.headers().iter() {
                builder = builder.header(name, value);
            }
            Ok(builder)
        }
    }
}

fn resolve_step_url(
    scenario: &Scenario,
    step: &ScenarioStep,
    vars: &BTreeMap<String, String>,
) -> Result<Url, String> {
    if let Some(url) = step.url.as_ref() {
        let rendered = render_template(url, vars);
        return Url::parse(&rendered)
            .map_err(|err| format!("Invalid scenario url '{}': {}", rendered, err));
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
    Ok(joined)
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
        drain_response_body(response).await?;
    }
    Ok(status)
}

async fn execute_request_status(client: &Client, request: Request) -> (u16, bool, bool) {
    match client.execute(request).await {
        Ok(response) => {
            let status = response.status().as_u16();
            match drain_response_body(response).await {
                Ok(()) => (status, false, false),
                Err(err) => {
                    let timed_out = err.is_timeout();
                    (500, timed_out, !timed_out)
                }
            }
        }
        Err(err) => {
            let timed_out = err.is_timeout();
            (500, timed_out, !timed_out)
        }
    }
}

async fn drain_response_body(response: reqwest::Response) -> Result<(), reqwest::Error> {
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        chunk?;
    }
    Ok(())
}

async fn drain_body_contains(
    response: reqwest::Response,
    fragment: &str,
) -> Result<bool, reqwest::Error> {
    let needle = fragment.as_bytes();
    if needle.is_empty() {
        drain_response_body(response).await?;
        return Ok(true);
    }
    let mut found = false;
    let mut carry: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let bytes = chunk?;
        if !found {
            let mut window = std::mem::take(&mut carry);
            window.extend_from_slice(&bytes);
            if window.windows(needle.len()).any(|slice| slice == needle) {
                found = true;
            }
            let keep = needle.len().saturating_sub(1);
            if keep > 0 {
                let len = window.len();
                let start = len.saturating_sub(keep);
                if let Some(tail) = window.get(start..) {
                    carry = tail.to_vec();
                } else {
                    carry.clear();
                }
            } else {
                carry.clear();
            }
        }
    }
    Ok(found)
}
