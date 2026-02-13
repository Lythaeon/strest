use std::net::{SocketAddr, ToSocketAddrs};

use url::Url;

use crate::args::Protocol;
use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, HttpError, ValidationError};

pub(super) fn resolve_endpoint(
    args: &TesterArgs,
    allowed_schemes: &[(&'static str, u16)],
) -> AppResult<SocketAddr> {
    let raw_url = args
        .url
        .as_deref()
        .ok_or_else(|| AppError::validation(ValidationError::MissingUrl))?;
    let url = Url::parse(raw_url).map_err(|source| {
        AppError::validation(ValidationError::InvalidUrl {
            url: raw_url.to_owned(),
            source,
        })
    })?;

    let raw_scheme = url.scheme();
    let scheme = match (args.protocol, raw_scheme) {
        (Protocol::GrpcUnary | Protocol::GrpcStreaming, "grpc") => "http",
        (Protocol::GrpcUnary | Protocol::GrpcStreaming, "grpcs") => "https",
        _ => raw_scheme,
    };
    let default_port = allowed_schemes
        .iter()
        .find_map(|(allowed, default_port)| {
            if *allowed == scheme {
                Some(*default_port)
            } else {
                None
            }
        })
        .ok_or_else(|| {
            AppError::validation(ValidationError::UnsupportedProtocolUrlScheme {
                protocol: args.protocol.as_str().to_owned(),
                scheme: raw_scheme.to_owned(),
            })
        })?;

    let host = url
        .host_str()
        .ok_or_else(|| AppError::validation(ValidationError::UrlMissingHost))?;
    let port = url.port().unwrap_or(default_port);
    let endpoint = format!("{}:{}", host, port);
    let mut addrs = endpoint.to_socket_addrs().map_err(|source| {
        AppError::http(HttpError::ResolveHost {
            host: host.to_owned(),
            port,
            source,
        })
    })?;
    addrs.next().ok_or_else(|| {
        AppError::http(HttpError::NoAddressesResolved {
            host: endpoint.clone(),
        })
    })
}

pub(super) fn resolve_grpc_url(args: &TesterArgs) -> AppResult<(Url, bool)> {
    let raw_url = args
        .url
        .as_deref()
        .ok_or_else(|| AppError::validation(ValidationError::MissingUrl))?;
    let mut url = Url::parse(raw_url).map_err(|source| {
        AppError::validation(ValidationError::InvalidUrl {
            url: raw_url.to_owned(),
            source,
        })
    })?;

    let prior_knowledge = match url.scheme() {
        "http" => true,
        "https" => false,
        "grpc" => {
            if url.set_scheme("http").is_err() {
                url = replace_grpc_scheme(raw_url, &url, "http")?;
            }
            true
        }
        "grpcs" => {
            if url.set_scheme("https").is_err() {
                url = replace_grpc_scheme(raw_url, &url, "https")?;
            }
            false
        }
        other => {
            return Err(AppError::validation(
                ValidationError::UnsupportedProtocolUrlScheme {
                    protocol: args.protocol.as_str().to_owned(),
                    scheme: other.to_owned(),
                },
            ));
        }
    };

    if url.host_str().is_none() {
        return Err(AppError::validation(ValidationError::UrlMissingHost));
    }

    Ok((url, prior_knowledge))
}

fn replace_grpc_scheme(raw_url: &str, url: &Url, scheme: &str) -> AppResult<Url> {
    let rest_start = url.scheme().len().saturating_add(1);
    let rest = &url.as_str()[rest_start..];
    let normalized = format!("{scheme}:{}", rest);
    Url::parse(&normalized).map_err(|source| {
        AppError::validation(ValidationError::InvalidUrl {
            url: raw_url.to_owned(),
            source,
        })
    })
}

pub(super) fn resolve_websocket_url(args: &TesterArgs) -> AppResult<Url> {
    let raw_url = args
        .url
        .as_deref()
        .ok_or_else(|| AppError::validation(ValidationError::MissingUrl))?;
    let mut url = Url::parse(raw_url).map_err(|source| {
        AppError::validation(ValidationError::InvalidUrl {
            url: raw_url.to_owned(),
            source,
        })
    })?;

    match url.scheme() {
        "ws" | "wss" => Ok(url),
        "http" => {
            url.set_scheme("ws").map_err(|()| {
                AppError::validation(ValidationError::UnsupportedProtocolUrlScheme {
                    protocol: args.protocol.as_str().to_owned(),
                    scheme: "http".to_owned(),
                })
            })?;
            Ok(url)
        }
        "https" => {
            url.set_scheme("wss").map_err(|()| {
                AppError::validation(ValidationError::UnsupportedProtocolUrlScheme {
                    protocol: args.protocol.as_str().to_owned(),
                    scheme: "https".to_owned(),
                })
            })?;
            Ok(url)
        }
        other => Err(AppError::validation(
            ValidationError::UnsupportedProtocolUrlScheme {
                protocol: args.protocol.as_str().to_owned(),
                scheme: other.to_owned(),
            },
        )),
    }
}
