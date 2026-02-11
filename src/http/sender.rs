use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::sync::{Arc, atomic::AtomicU64};
use std::time::Duration;

use reqwest::{
    Client, Proxy, Url,
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect,
};
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::time::{interval, sleep};
use tracing::{error, warn};

use crate::{
    args::{DEFAULT_USER_AGENT, HttpMethod, HttpVersion, TesterArgs},
    error::{AppError, AppResult, HttpError, ValidationError},
    metrics::{LogSink, Metrics},
    shutdown::ShutdownSender,
};

use super::rate::build_rate_limiter;
use super::tls::apply_tls_settings;
use super::workload::{
    AuthConfig, BodySource, FormFieldSpec, RequestLimiter, ScenarioRunContext, SingleRequestSpec,
    UrlSource, WorkerContext, Workload, preflight_request, run_scenario_iteration,
    run_single_dynamic_iteration, run_single_iteration,
};

/// Creates the request sender task and validates the HTTP client/config.
///
/// # Errors
///
/// Returns an error when the HTTP client or request template cannot be built.
pub fn setup_request_sender(
    args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    metrics_tx: &mpsc::Sender<Metrics>,
    log_sink: Option<&Arc<LogSink>>,
) -> AppResult<tokio::task::JoinHandle<()>> {
    let shutdown_tx = shutdown_tx.clone();
    let metrics_tx = metrics_tx.clone();

    if args.ipv4_only && args.ipv6_only {
        return Err(AppError::validation(ValidationError::Ipv4Ipv6Conflict));
    }
    if args.proxy_http2
        && args.proxy_http_version.is_some()
        && args.proxy_http_version != Some(HttpVersion::V2)
    {
        return Err(AppError::validation(ValidationError::ProxyHttp2Conflict));
    }

    let auth_config = resolve_auth(args)?;

    if args.scenario.is_some()
        && (args.urls_from_file || args.rand_regex_url || args.dump_urls.is_some())
    {
        return Err(AppError::validation(
            ValidationError::ScenarioUrlGenerationConflict,
        ));
    }
    if args.dump_urls.is_some() && !args.rand_regex_url {
        return Err(AppError::validation(
            ValidationError::DumpUrlsRequiresRandRegex,
        ));
    }
    if args.urls_from_file && args.rand_regex_url {
        return Err(AppError::validation(
            ValidationError::UrlsFromFileAndRandRegexConflict,
        ));
    }
    if matches!(auth_config, Some(AuthConfig::SigV4 { .. })) && !args.form.is_empty() {
        return Err(AppError::validation(ValidationError::SigV4FormUnsupported));
    }

    let mut client_builder = Client::builder()
        .timeout(args.request_timeout)
        .connect_timeout(args.connect_timeout);

    if !args.no_ua {
        client_builder = client_builder.user_agent(DEFAULT_USER_AGENT);
    }

    if let Some((host, port)) = resolve_primary_host(args)? {
        if args.ipv4_only || args.ipv6_only {
            let addrs = resolve_addrs(&host, port, args.ipv4_only, args.ipv6_only)?;
            if addrs.is_empty() {
                return Err(AppError::http(HttpError::NoAddressesResolved { host }));
            }
            client_builder = client_builder.resolve_to_addrs(&host, &addrs);
        } else if !args.no_pre_lookup {
            let _ = resolve_addrs(&host, port, false, false)?;
        }
    }

    if args.redirect_limit == 0 {
        client_builder = client_builder.redirect(redirect::Policy::none());
    } else {
        client_builder = client_builder.redirect(redirect::Policy::limited(
            usize::try_from(args.redirect_limit).unwrap_or(10),
        ));
    }

    if args.disable_keepalive {
        client_builder = client_builder
            .pool_max_idle_per_host(0)
            .pool_idle_timeout(Some(std::time::Duration::from_secs(0)));
    }

    if let Some(max_idle) = args.pool_max_idle_per_host.as_ref() {
        client_builder = client_builder.pool_max_idle_per_host(max_idle.get());
    }

    if let Some(idle_timeout_ms) = args.pool_idle_timeout_ms.as_ref() {
        client_builder =
            client_builder.pool_idle_timeout(Some(Duration::from_millis(idle_timeout_ms.get())));
    }

    if args.disable_compression {
        client_builder = client_builder.no_gzip().no_brotli().no_deflate();
    }

    client_builder = apply_tls_settings(client_builder, args)?;

    if let Some(path) = args.cacert.as_ref() {
        let bytes = std::fs::read(path).map_err(|err| {
            AppError::http(HttpError::ReadCacert {
                path: path.clone().into(),
                source: err,
            })
        })?;
        let cert = reqwest::Certificate::from_pem(&bytes).map_err(|err| {
            AppError::http(HttpError::InvalidCacert {
                path: path.clone().into(),
                source: err,
            })
        })?;
        client_builder = client_builder.add_root_certificate(cert);
    }

    if args.cert.is_some() || args.key.is_some() {
        let cert_path = args
            .cert
            .as_ref()
            .ok_or_else(|| AppError::validation(ValidationError::CertRequiresKey))?;
        let key_path = args
            .key
            .as_ref()
            .ok_or_else(|| AppError::validation(ValidationError::KeyRequiresCert))?;
        let cert_bytes = std::fs::read(cert_path).map_err(|err| {
            AppError::http(HttpError::ReadCert {
                path: cert_path.clone().into(),
                source: err,
            })
        })?;
        let key_bytes = std::fs::read(key_path).map_err(|err| {
            AppError::http(HttpError::ReadKey {
                path: key_path.clone().into(),
                source: err,
            })
        })?;
        let identity = reqwest::Identity::from_pkcs8_pem(&cert_bytes, &key_bytes)
            .map_err(|err| AppError::http(HttpError::InvalidIdentity { source: err }))?;
        client_builder = client_builder.identity(identity);
    }

    if args.insecure {
        client_builder = client_builder
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true);
    }

    if let Some(ref proxy_url) = args.proxy_url {
        match Proxy::all(proxy_url) {
            Ok(mut proxy) => {
                if !args.proxy_headers.is_empty() {
                    let mut headers = HeaderMap::new();
                    for (key, value) in &args.proxy_headers {
                        let name = HeaderName::from_bytes(key.as_bytes()).map_err(|err| {
                            AppError::validation(ValidationError::InvalidProxyHeaderName {
                                header: key.clone(),
                                source: err,
                            })
                        })?;
                        let val = HeaderValue::from_str(value).map_err(|err| {
                            AppError::validation(ValidationError::InvalidProxyHeaderValue {
                                header: key.clone(),
                                source: err,
                            })
                        })?;
                        headers.insert(name, val);
                    }
                    proxy = proxy.headers(headers);
                }
                client_builder = client_builder.proxy(proxy);
            }
            Err(e) => {
                error!("Invalid proxy URL '{}': {}", proxy_url, e);
                return Err(AppError::validation(ValidationError::InvalidProxyUrl {
                    url: proxy_url.clone(),
                    source: e,
                }));
            }
        }
    }

    if args.proxy_http2 {
        client_builder = client_builder.http2_prior_knowledge();
    }
    if let Some(version) = args.proxy_http_version {
        client_builder = apply_proxy_http_version(client_builder, version)?;
    }

    if let Some(path) = args.unix_socket.as_ref() {
        client_builder = client_builder.unix_socket(path.clone());
    }

    let client = match client_builder.build() {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to build HTTP client: {}", e);
            return Err(AppError::http(HttpError::BuildClientFailed { source: e }));
        }
    };

    let workload = if let Some(scenario) = args.scenario.clone() {
        Workload::Scenario(
            Arc::new(scenario),
            Arc::new(args.connect_to.clone()),
            args.host_header.clone(),
            auth_config,
        )
    } else {
        let url_source = resolve_url_source(args)?;
        let body_source = resolve_body_source(args)?;
        let form_fields = parse_form_fields(args)?;
        let headers = build_headers(args);

        let requires_dynamic = matches!(body_source, BodySource::Lines(_))
            || matches!(url_source, UrlSource::List(_) | UrlSource::Regex(_))
            || form_fields.is_some()
            || !args.connect_to.is_empty()
            || auth_config.is_some();

        if requires_dynamic {
            Workload::SingleDynamic(Arc::new(SingleRequestSpec {
                method: args.method,
                url: url_source,
                headers,
                body: body_source,
                form: form_fields,
                connect_to: args.connect_to.clone(),
                auth: auth_config,
            }))
        } else {
            let UrlSource::Static(url) = url_source else {
                return Err(AppError::http(HttpError::InvalidUrlSourceForStaticWorkload));
            };
            let BodySource::Static(body) = body_source else {
                return Err(AppError::http(
                    HttpError::InvalidBodySourceForStaticWorkload,
                ));
            };

            drop(auth_config);
            let mut request_builder = match args.method {
                HttpMethod::Get => client.get(&url),
                HttpMethod::Post => client.post(&url),
                HttpMethod::Patch => client.patch(&url),
                HttpMethod::Put => client.put(&url),
                HttpMethod::Delete => client.delete(&url),
            };

            for (key, value) in &headers {
                request_builder = request_builder.header(key, value);
            }

            let request = match request_builder.body(body).build() {
                Ok(req) => req,
                Err(e) => {
                    error!("Failed to build request: {}", e);
                    return Err(AppError::http(HttpError::BuildRequestFailed { source: e }));
                }
            };

            Workload::Single(Arc::new(request))
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

fn build_headers(args: &TesterArgs) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if let Some(host) = args.host_header.as_ref()
        && !has_host_header(&args.headers)
    {
        headers.push(("Host".to_owned(), host.clone()));
    }
    if let Some(accept) = args.accept_header.as_ref() {
        headers.push(("Accept".to_owned(), accept.clone()));
    }
    if let Some(content_type) = args.content_type.as_ref() {
        headers.push(("Content-Type".to_owned(), content_type.clone()));
    }
    headers.extend(args.headers.clone());
    headers
}

fn has_host_header(headers: &[(String, String)]) -> bool {
    headers
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case("host"))
}

fn resolve_body_source(args: &TesterArgs) -> AppResult<BodySource> {
    if let Some(path) = args.data_lines.as_ref() {
        let content = std::fs::read_to_string(path).map_err(|err| {
            AppError::http(HttpError::ReadFile {
                path: path.clone().into(),
                source: err,
            })
        })?;
        let lines: Vec<String> = content.lines().map(|line| line.to_owned()).collect();
        if lines.is_empty() {
            return Err(AppError::http(HttpError::BodyLinesEmpty));
        }
        return Ok(BodySource::from_lines(lines));
    }

    if let Some(path) = args.data_file.as_ref() {
        let content = std::fs::read_to_string(path).map_err(|err| {
            AppError::http(HttpError::ReadFile {
                path: path.clone().into(),
                source: err,
            })
        })?;
        return Ok(BodySource::Static(content));
    }

    Ok(BodySource::Static(args.data.clone()))
}

fn resolve_url_source(args: &TesterArgs) -> AppResult<UrlSource> {
    let value = args
        .url
        .as_deref()
        .ok_or_else(|| AppError::validation(ValidationError::MissingUrl))?;

    if args.urls_from_file {
        let content = std::fs::read_to_string(value).map_err(|err| {
            AppError::http(HttpError::ReadUrlFile {
                path: PathBuf::from(value),
                source: err,
            })
        })?;
        let urls: Vec<String> = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .map(|line| line.to_owned())
            .collect();
        if urls.is_empty() {
            return Err(AppError::http(HttpError::UrlFileEmpty {
                path: PathBuf::from(value),
            }));
        }
        return Ok(UrlSource::from_list(urls));
    }

    if args.rand_regex_url {
        let max_repeat = u32::try_from(args.max_repeat.get()).unwrap_or(u32::MAX);
        let regex = rand_regex::Regex::compile(value, max_repeat).map_err(|err| {
            AppError::validation(ValidationError::InvalidRandRegex {
                pattern: value.to_owned(),
                source: err,
            })
        })?;
        return Ok(UrlSource::Regex(Arc::new(regex)));
    }

    Ok(UrlSource::Static(value.to_owned()))
}

fn parse_form_fields(args: &TesterArgs) -> AppResult<Option<Vec<FormFieldSpec>>> {
    if args.form.is_empty() {
        return Ok(None);
    }
    let mut fields = Vec::with_capacity(args.form.len());
    for entry in &args.form {
        let (name, value) = entry.split_once('=').ok_or_else(|| {
            AppError::validation(ValidationError::InvalidFormEntryFormat {
                entry: entry.to_owned(),
            })
        })?;
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::validation(ValidationError::FormEntryNameEmpty {
                entry: entry.to_owned(),
            }));
        }
        let value = value.trim();
        if let Some(path) = value.strip_prefix('@') {
            if path.is_empty() {
                return Err(AppError::validation(ValidationError::FormEntryPathEmpty {
                    entry: entry.to_owned(),
                }));
            }
            fields.push(FormFieldSpec::File {
                name: name.to_owned(),
                path: path.to_owned(),
            });
        } else {
            fields.push(FormFieldSpec::Text {
                name: name.to_owned(),
                value: value.to_owned(),
            });
        }
    }
    Ok(Some(fields))
}

fn resolve_http2_parallel(args: &TesterArgs) -> usize {
    let http2_enabled = args.http2 || matches!(args.http_version, Some(HttpVersion::V2));
    if !http2_enabled && args.http2_parallel.get() > 1 {
        warn!("--http2-parallel is ignored unless HTTP/2 is enabled.");
        return 1;
    }
    args.http2_parallel.get()
}

fn apply_proxy_http_version(
    mut builder: reqwest::ClientBuilder,
    version: HttpVersion,
) -> AppResult<reqwest::ClientBuilder> {
    match version {
        HttpVersion::V0_9 | HttpVersion::V1_0 | HttpVersion::V1_1 => {
            builder = builder.http1_only();
        }
        HttpVersion::V2 => {
            builder = builder.http2_prior_knowledge();
        }
        HttpVersion::V3 => {
            return Err(AppError::validation(
                ValidationError::ProxyHttpVersionUnsupported,
            ));
        }
    }
    Ok(builder)
}

fn resolve_primary_host(args: &TesterArgs) -> AppResult<Option<(String, u16)>> {
    if args.urls_from_file || args.rand_regex_url {
        return Ok(None);
    }
    if let Some(url) = args.url.as_deref() {
        let parsed = Url::parse(url).map_err(|err| {
            AppError::validation(ValidationError::InvalidUrl {
                url: url.to_owned(),
                source: err,
            })
        })?;
        let host = parsed
            .host_str()
            .ok_or_else(|| AppError::validation(ValidationError::UrlMissingHost))?;
        let port = parsed.port_or_known_default().unwrap_or(80);
        return Ok(Some((host.to_owned(), port)));
    }
    if let Some(scenario) = args.scenario.as_ref() {
        if let Some(base_url) = scenario.base_url.as_ref() {
            let parsed = Url::parse(base_url).map_err(|err| {
                AppError::validation(ValidationError::InvalidBaseUrl {
                    url: base_url.to_owned(),
                    source: err,
                })
            })?;
            let host = parsed
                .host_str()
                .ok_or_else(|| AppError::validation(ValidationError::ScenarioBaseUrlMissingHost))?;
            let port = parsed.port_or_known_default().unwrap_or(80);
            return Ok(Some((host.to_owned(), port)));
        }
        if let Some(step) = scenario.steps.first()
            && let Some(url) = step.url.as_ref()
        {
            let parsed = Url::parse(url).map_err(|err| {
                AppError::validation(ValidationError::InvalidScenarioUrl {
                    url: url.to_owned(),
                    source: err,
                })
            })?;
            let host = parsed
                .host_str()
                .ok_or_else(|| AppError::validation(ValidationError::ScenarioUrlMissingHost))?;
            let port = parsed.port_or_known_default().unwrap_or(80);
            return Ok(Some((host.to_owned(), port)));
        }
    }
    Ok(None)
}

fn resolve_addrs(
    host: &str,
    port: u16,
    ipv4_only: bool,
    ipv6_only: bool,
) -> AppResult<Vec<std::net::SocketAddr>> {
    let mut addrs: Vec<std::net::SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|err| {
            AppError::http(HttpError::ResolveHost {
                host: host.to_owned(),
                port,
                source: err,
            })
        })?
        .collect();
    if ipv4_only {
        addrs.retain(|addr| addr.is_ipv4());
    }
    if ipv6_only {
        addrs.retain(|addr| addr.is_ipv6());
    }
    Ok(addrs)
}

fn resolve_auth(args: &TesterArgs) -> AppResult<Option<AuthConfig>> {
    if let Some(sigv4) = args.aws_sigv4.as_ref() {
        let basic = args
            .basic_auth
            .as_ref()
            .ok_or_else(|| AppError::validation(ValidationError::AwsSigv4RequiresBasicAuth))?;
        let (access_key, secret_key) = parse_auth_pair(basic)?;
        let (region, service) = parse_aws_sigv4(sigv4)?;
        return Ok(Some(AuthConfig::SigV4 {
            access_key,
            secret_key,
            session_token: args.aws_session.clone(),
            region,
            service,
        }));
    }
    if args.aws_session.is_some() {
        return Err(AppError::validation(
            ValidationError::AwsSessionRequiresSigv4,
        ));
    }
    if let Some(basic) = args.basic_auth.as_ref() {
        let (username, password) = parse_auth_pair(basic)?;
        return Ok(Some(AuthConfig::Basic { username, password }));
    }
    Ok(None)
}

fn parse_auth_pair(value: &str) -> AppResult<(String, String)> {
    let (left, right) = value
        .split_once(':')
        .ok_or_else(|| AppError::validation(ValidationError::AuthPairInvalidFormat))?;
    if left.is_empty() {
        return Err(AppError::validation(ValidationError::AuthUsernameEmpty));
    }
    Ok((left.to_owned(), right.to_owned()))
}

fn parse_aws_sigv4(value: &str) -> AppResult<(String, String)> {
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 4 {
        return Err(AppError::validation(ValidationError::AwsSigv4InvalidFormat));
    }
    let region = parts
        .get(2)
        .ok_or_else(|| AppError::validation(ValidationError::AwsSigv4InvalidFormat))?
        .trim();
    let service = parts
        .get(3)
        .ok_or_else(|| AppError::validation(ValidationError::AwsSigv4InvalidFormat))?
        .trim();
    if region.is_empty() || service.is_empty() {
        return Err(AppError::validation(
            ValidationError::AwsSigv4EmptyRegionOrService,
        ));
    }
    Ok((region.to_owned(), service.to_owned()))
}
