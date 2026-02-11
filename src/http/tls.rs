use reqwest::ClientBuilder;

use crate::args::{HttpVersion, TesterArgs, TlsVersion};
use crate::error::{AppError, AppResult, ValidationError};

#[derive(Debug, Clone, Copy)]
pub(crate) enum AlpnChoice {
    Default,
    Http1Only,
    Http2Only,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AlpnSelection {
    pub(crate) choice: AlpnChoice,
    pub(crate) has_h3: bool,
}

pub(super) fn apply_tls_settings(
    mut builder: ClientBuilder,
    args: &TesterArgs,
) -> AppResult<ClientBuilder> {
    if let (Some(min), Some(max)) = (args.tls_min, args.tls_max)
        && tls_version_rank(min) > tls_version_rank(max)
    {
        return Err(AppError::validation(ValidationError::TlsMinGreaterThanMax));
    }

    if let Some(min) = args.tls_min {
        builder = builder.min_tls_version(to_reqwest_tls_version(min));
    }
    if let Some(max) = args.tls_max {
        builder = builder.max_tls_version(to_reqwest_tls_version(max));
    }

    let alpn = resolve_alpn(&args.alpn)?;

    if let Some(version) = args.http_version {
        #[cfg(feature = "http3")]
        {
            builder = apply_explicit_http_version(builder, version);
            return Ok(builder);
        }
        #[cfg(not(feature = "http3"))]
        {
            builder = apply_explicit_http_version(builder, version)?;
            return Ok(builder);
        }
    }

    if alpn.has_h3 && !args.http3 {
        return Err(AppError::validation(ValidationError::AlpnH3WithoutHttp3));
    }
    if args.http2 && args.http3 {
        return Err(AppError::validation(ValidationError::Http2Http3Conflict));
    }
    if args.http3 {
        if matches!(alpn.choice, AlpnChoice::Http1Only) {
            return Err(AppError::validation(
                ValidationError::Http3WithHttp1OnlyAlpn,
            ));
        }
        if matches!(alpn.choice, AlpnChoice::Http2Only) && !alpn.has_h3 {
            return Err(AppError::validation(ValidationError::Http3WithH2OnlyAlpn));
        }
        #[cfg(feature = "http3")]
        {
            builder = builder.http3_prior_knowledge();
            return Ok(builder);
        }
        #[cfg(not(feature = "http3"))]
        {
            return Err(AppError::validation(ValidationError::Http3NotEnabled));
        }
    }
    if args.http2 && matches!(alpn.choice, AlpnChoice::Http1Only) {
        return Err(AppError::validation(
            ValidationError::Http2WithHttp1OnlyAlpn,
        ));
    }

    builder = match alpn.choice {
        AlpnChoice::Http1Only => builder.http1_only(),
        AlpnChoice::Http2Only => builder.http2_prior_knowledge(),
        AlpnChoice::Default => {
            if args.http2 {
                builder.http2_prior_knowledge()
            } else {
                builder
            }
        }
    };

    Ok(builder)
}

#[cfg(feature = "http3")]
fn apply_explicit_http_version(mut builder: ClientBuilder, version: HttpVersion) -> ClientBuilder {
    match version {
        HttpVersion::V0_9 | HttpVersion::V1_0 | HttpVersion::V1_1 => {
            builder = builder.http1_only();
        }
        HttpVersion::V2 => {
            builder = builder.http2_prior_knowledge();
        }
        HttpVersion::V3 => {
            builder = builder.http3_prior_knowledge();
        }
    }
    builder
}

#[cfg(not(feature = "http3"))]
fn apply_explicit_http_version(
    mut builder: ClientBuilder,
    version: HttpVersion,
) -> AppResult<ClientBuilder> {
    match version {
        HttpVersion::V0_9 | HttpVersion::V1_0 | HttpVersion::V1_1 => {
            builder = builder.http1_only();
        }
        HttpVersion::V2 => {
            builder = builder.http2_prior_knowledge();
        }
        HttpVersion::V3 => {
            return Err(AppError::validation(ValidationError::Http3NotEnabled));
        }
    }
    Ok(builder)
}

const fn to_reqwest_tls_version(version: TlsVersion) -> reqwest::tls::Version {
    match version {
        TlsVersion::V1_0 => reqwest::tls::Version::TLS_1_0,
        TlsVersion::V1_1 => reqwest::tls::Version::TLS_1_1,
        TlsVersion::V1_2 => reqwest::tls::Version::TLS_1_2,
        TlsVersion::V1_3 => reqwest::tls::Version::TLS_1_3,
    }
}

const fn tls_version_rank(version: TlsVersion) -> u8 {
    match version {
        TlsVersion::V1_0 => 0,
        TlsVersion::V1_1 => 1,
        TlsVersion::V1_2 => 2,
        TlsVersion::V1_3 => 3,
    }
}

pub(crate) fn resolve_alpn(alpn: &[String]) -> AppResult<AlpnSelection> {
    if alpn.is_empty() {
        return Ok(AlpnSelection {
            choice: AlpnChoice::Default,
            has_h3: false,
        });
    }

    let mut has_h2 = false;
    let mut has_http1 = false;
    let mut has_h3 = false;
    for proto in alpn {
        let normalized = proto.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "h2" | "http2" | "http/2" => has_h2 = true,
            "http/1.1" | "http1" | "http/1" => has_http1 = true,
            "h3" | "http3" | "http/3" => has_h3 = true,
            other if other.starts_with("h3-") => {
                has_h3 = true;
            }
            "" => {}
            _ => {
                return Err(AppError::validation(
                    ValidationError::UnsupportedAlpnProtocol {
                        protocol: proto.to_owned(),
                    },
                ));
            }
        }
    }

    let choice = match (has_h2, has_http1) {
        (true, true) => AlpnChoice::Default,
        (true, false) => AlpnChoice::Http2Only,
        (false, true) => AlpnChoice::Http1Only,
        (false, false) => AlpnChoice::Default,
    };
    Ok(AlpnSelection { choice, has_h3 })
}
