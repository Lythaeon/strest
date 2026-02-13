mod config;
mod worker;

use std::sync::Arc;
use std::time::Duration;

use reqwest::{
    Client, Proxy,
    header::{HeaderMap, HeaderName, HeaderValue},
    redirect,
};
use tokio::sync::mpsc;
use tracing::error;

use crate::{
    args::{DEFAULT_USER_AGENT, HttpMethod, HttpVersion, TesterArgs},
    error::{AppError, AppResult, HttpError, ValidationError},
    metrics::{LogSink, Metrics},
    shutdown::ShutdownSender,
};

use super::tls::apply_tls_settings;
use super::workload::{AuthConfig, BodySource, SingleRequestSpec, UrlSource, Workload};
use config::{
    apply_proxy_http_version, build_headers, parse_form_fields, resolve_addrs, resolve_auth,
    resolve_body_source, resolve_primary_host, resolve_url_source,
};
use worker::create_sender_task;

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
            .pool_idle_timeout(Some(Duration::from_secs(0)));
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
        #[cfg(unix)]
        {
            client_builder = client_builder.unix_socket(path.clone());
        }
        #[cfg(not(unix))]
        {
            drop(path);
            return Err(AppError::validation(
                ValidationError::UnixSocketUnsupportedOnPlatform,
            ));
        }
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
