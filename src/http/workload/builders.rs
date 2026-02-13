use std::collections::BTreeMap;

use reqwest::{Client, Request, Url};

use crate::{
    args::{ConnectToMapping, HttpMethod, Scenario, ScenarioStep},
    error::{AppError, AppResult, HttpError},
};

use super::builders_auth::apply_auth_headers;
use super::data::{AuthConfig, BodySource, FormFieldSpec, SingleRequestSpec};
use super::template::{render_template, resolve_step_url};

fn build_multipart(fields: &[FormFieldSpec]) -> AppResult<reqwest::multipart::Form> {
    let mut form = reqwest::multipart::Form::new();
    for field in fields {
        match field {
            FormFieldSpec::Text { name, value } => {
                form = form.text(name.clone(), value.clone());
            }
            FormFieldSpec::File { name, path } => {
                let bytes = std::fs::read(path).map_err(|err| {
                    AppError::http(HttpError::ReadFormFile {
                        path: path.clone(),
                        source: err,
                    })
                })?;
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

pub(super) fn build_request_from_spec(
    client: &Client,
    spec: &SingleRequestSpec,
) -> AppResult<Request> {
    let url_raw = spec.url.next_url()?;
    let url = Url::parse(&url_raw).map_err(|err| {
        AppError::http(HttpError::InvalidUrl {
            url: url_raw,
            source: err,
        })
    })?;
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
        BodySource::Lines(lines) => lines
            .next()
            .ok_or_else(|| AppError::http(HttpError::BodyLinesEmpty))?,
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
        .map_err(|err| AppError::http(HttpError::BuildRequestFailed { source: err }))
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
) -> AppResult<Request> {
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
        request_builder = request_builder.body(render_template(body, vars));
    }

    request_builder
        .build()
        .map_err(|err| AppError::http(HttpError::BuildRequestFailed { source: err }))
}

fn apply_connect_to(
    url: &Url,
    connect_to: &[ConnectToMapping],
) -> AppResult<(Url, Option<String>)> {
    let Some(host) = url.host_str() else {
        return Ok((url.clone(), None));
    };
    let port = url.port_or_known_default().unwrap_or(80);
    for mapping in connect_to {
        if mapping.source_host == host && mapping.source_port == port {
            let mut rewritten = url.clone();
            rewritten
                .set_host(Some(&mapping.target_host))
                .map_err(|err| AppError::http(HttpError::InvalidConnectToHost { source: err }))?;
            rewritten
                .set_port(Some(mapping.target_port))
                .map_err(|()| AppError::http(HttpError::InvalidConnectToPort))?;
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
