use std::net::ToSocketAddrs;
use std::path::PathBuf;
use std::sync::Arc;

use reqwest::Url;
use tracing::warn;

use crate::{
    args::{HttpVersion, TesterArgs},
    error::{AppError, AppResult, HttpError, ValidationError},
};

use super::super::workload::{AuthConfig, BodySource, FormFieldSpec, UrlSource};

pub(super) fn build_headers(args: &TesterArgs) -> Vec<(String, String)> {
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

pub(super) fn resolve_body_source(args: &TesterArgs) -> AppResult<BodySource> {
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

pub(super) fn resolve_url_source(args: &TesterArgs) -> AppResult<UrlSource> {
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

pub(super) fn parse_form_fields(args: &TesterArgs) -> AppResult<Option<Vec<FormFieldSpec>>> {
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

pub(super) fn resolve_http2_parallel(args: &TesterArgs) -> usize {
    let http2_enabled = args.http2 || matches!(args.http_version, Some(HttpVersion::V2));
    if !http2_enabled && args.http2_parallel.get() > 1 {
        warn!("--http2-parallel is ignored unless HTTP/2 is enabled.");
        return 1;
    }
    args.http2_parallel.get()
}

pub(super) fn apply_proxy_http_version(
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

pub(super) fn resolve_primary_host(args: &TesterArgs) -> AppResult<Option<(String, u16)>> {
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

pub(super) fn resolve_addrs(
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

pub(super) fn resolve_auth(args: &TesterArgs) -> AppResult<Option<AuthConfig>> {
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
