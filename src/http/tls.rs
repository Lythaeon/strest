use reqwest::ClientBuilder;

use crate::args::{TesterArgs, TlsVersion};

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
) -> Result<ClientBuilder, String> {
    if let (Some(min), Some(max)) = (args.tls_min, args.tls_max)
        && tls_version_rank(min) > tls_version_rank(max)
    {
        return Err("tls-min must be <= tls-max.".to_owned());
    }

    if let Some(min) = args.tls_min {
        builder = builder.min_tls_version(to_reqwest_tls_version(min));
    }
    if let Some(max) = args.tls_max {
        builder = builder.max_tls_version(to_reqwest_tls_version(max));
    }

    let alpn = resolve_alpn(&args.alpn)?;
    if alpn.has_h3 && !args.http3 {
        return Err("ALPN includes h3, but http3 is not enabled.".to_owned());
    }
    if args.http2 && args.http3 {
        return Err("Cannot enable http2 and http3 at the same time.".to_owned());
    }
    if args.http3 {
        if matches!(alpn.choice, AlpnChoice::Http1Only) {
            return Err("Cannot enable http3 while ALPN is restricted to http/1.1.".to_owned());
        }
        if matches!(alpn.choice, AlpnChoice::Http2Only) && !alpn.has_h3 {
            return Err("Cannot enable http3 while ALPN is restricted to h2.".to_owned());
        }
        #[cfg(feature = "http3")]
        {
            builder = builder.http3_prior_knowledge();
            return Ok(builder);
        }
        #[cfg(not(feature = "http3"))]
        {
            return Err(
                "HTTP/3 support is not enabled in this build. Rebuild with --features http3 \
and set RUSTFLAGS=\"--cfg reqwest_unstable\"."
                    .to_owned(),
            );
        }
    }
    if args.http2 && matches!(alpn.choice, AlpnChoice::Http1Only) {
        return Err("Cannot enable http2 while ALPN is set to http/1.1 only.".to_owned());
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

pub(crate) fn resolve_alpn(alpn: &[String]) -> Result<AlpnSelection, String> {
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
                return Err(format!(
                    "Unsupported ALPN protocol '{}'. Use h2, http/1.1, or h3.",
                    proto
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
