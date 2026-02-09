use std::time::Duration;

use super::types::{ConnectToMapping, PositiveU64, PositiveUsize, TlsVersion};

pub(crate) fn parse_header(s: &str) -> Result<(String, String), String> {
    match s.split_once(':') {
        Some((key, value)) => Ok((key.trim().to_owned(), value.trim().to_owned())),
        None => Err(format!(
            "Invalid header format: '{}'. Expected 'Key: Value'",
            s
        )),
    }
}

pub(super) fn parse_positive_u64(s: &str) -> Result<PositiveU64, String> {
    s.parse::<PositiveU64>()
}

pub(super) fn parse_positive_usize(s: &str) -> Result<PositiveUsize, String> {
    s.parse::<PositiveUsize>()
}

pub(super) fn parse_tls_version(s: &str) -> Result<TlsVersion, String> {
    s.parse::<TlsVersion>()
}

pub(crate) fn parse_connect_to(s: &str) -> Result<ConnectToMapping, String> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 4 {
        return Err(format!(
            "Invalid connect-to '{}'. Expected 'source_host:source_port:target_host:target_port'.",
            s
        ));
    }
    let source_host = parts
        .first()
        .ok_or_else(|| format!("Invalid connect-to '{}'.", s))?
        .trim();
    let source_port: u16 = parts
        .get(1)
        .ok_or_else(|| format!("Invalid connect-to '{}'.", s))?
        .trim()
        .parse()
        .map_err(|err| format!("Invalid source port in '{}': {}", s, err))?;
    let target_host = parts
        .get(2)
        .ok_or_else(|| format!("Invalid connect-to '{}'.", s))?
        .trim();
    let target_port: u16 = parts
        .get(3)
        .ok_or_else(|| format!("Invalid connect-to '{}'.", s))?
        .trim()
        .parse()
        .map_err(|err| format!("Invalid target port in '{}': {}", s, err))?;
    if source_host.is_empty() || target_host.is_empty() {
        return Err(format!(
            "Invalid connect-to '{}'. Host must not be empty.",
            s
        ));
    }
    Ok(ConnectToMapping {
        source_host: source_host.to_owned(),
        source_port,
        target_host: target_host.to_owned(),
        target_port,
    })
}

pub(crate) fn parse_duration_arg(s: &str) -> Result<Duration, String> {
    let value = s.trim();
    if value.is_empty() {
        return Err("Duration must not be empty.".to_owned());
    }

    let mut digits_len = 0usize;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            digits_len = digits_len.saturating_add(1);
        } else {
            break;
        }
    }
    if digits_len == 0 {
        return Err(format!("Invalid duration '{}'.", value));
    }
    let (num_part, unit_part) = value.split_at(digits_len);
    let number: u64 = num_part
        .parse()
        .map_err(|err| format!("Invalid duration '{}': {}", value, err))?;

    let unit = if unit_part.is_empty() { "s" } else { unit_part };
    let duration = match unit {
        "ms" => Duration::from_millis(number),
        "s" => Duration::from_secs(number),
        "m" => {
            let secs = number
                .checked_mul(60)
                .ok_or_else(|| "Duration overflow.".to_owned())?;
            Duration::from_secs(secs)
        }
        "h" => {
            let secs = number
                .checked_mul(60)
                .and_then(|seconds| seconds.checked_mul(60))
                .ok_or_else(|| "Duration overflow.".to_owned())?;
            Duration::from_secs(secs)
        }
        _ => return Err(format!("Invalid duration unit '{}'.", unit)),
    };

    if duration.as_millis() == 0 {
        return Err("Duration must be > 0.".to_owned());
    }

    Ok(duration)
}
