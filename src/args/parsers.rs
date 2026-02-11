use std::time::Duration;

use super::types::{ConnectToMapping, PositiveU64, PositiveUsize, TlsVersion};
use crate::error::{AppError, AppResult, ConnectToPortKind, ValidationError};

pub(crate) fn parse_header(s: &str) -> Result<(String, String), ValidationError> {
    match s.split_once(':') {
        Some((key, value)) => Ok((key.trim().to_owned(), value.trim().to_owned())),
        None => Err(ValidationError::InvalidHeaderFormat {
            value: s.to_owned(),
        }),
    }
}

pub(super) fn parse_positive_u64(s: &str) -> AppResult<PositiveU64> {
    s.parse::<PositiveU64>().map_err(AppError::from)
}

pub(super) fn parse_positive_usize(s: &str) -> AppResult<PositiveUsize> {
    s.parse::<PositiveUsize>().map_err(AppError::from)
}

pub(super) fn parse_tls_version(s: &str) -> AppResult<TlsVersion> {
    s.parse::<TlsVersion>()
}

pub(crate) fn parse_bool_env(s: &str) -> AppResult<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" | "on" => Ok(true),
        "0" | "false" | "no" | "n" | "off" => Ok(false),
        _ => Err(AppError::validation(ValidationError::InvalidBoolean {
            value: s.to_owned(),
        })),
    }
}

pub(crate) fn parse_connect_to(s: &str) -> Result<ConnectToMapping, ValidationError> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 4 {
        return Err(ValidationError::InvalidConnectToFormat {
            value: s.to_owned(),
        });
    }
    let source_host = parts
        .first()
        .ok_or_else(|| ValidationError::InvalidConnectTo {
            value: s.to_owned(),
        })?
        .trim();
    let source_port: u16 = parts
        .get(1)
        .ok_or_else(|| ValidationError::InvalidConnectTo {
            value: s.to_owned(),
        })?
        .trim()
        .parse()
        .map_err(|err| ValidationError::InvalidConnectToPort {
            value: s.to_owned(),
            kind: ConnectToPortKind::Source,
            source: err,
        })?;
    let target_host = parts
        .get(2)
        .ok_or_else(|| ValidationError::InvalidConnectTo {
            value: s.to_owned(),
        })?
        .trim();
    let target_port: u16 = parts
        .get(3)
        .ok_or_else(|| ValidationError::InvalidConnectTo {
            value: s.to_owned(),
        })?
        .trim()
        .parse()
        .map_err(|err| ValidationError::InvalidConnectToPort {
            value: s.to_owned(),
            kind: ConnectToPortKind::Target,
            source: err,
        })?;
    if source_host.is_empty() || target_host.is_empty() {
        return Err(ValidationError::ConnectToHostEmpty {
            value: s.to_owned(),
        });
    }
    Ok(ConnectToMapping {
        source_host: source_host.to_owned(),
        source_port,
        target_host: target_host.to_owned(),
        target_port,
    })
}

pub(crate) fn parse_duration_arg(s: &str) -> AppResult<Duration> {
    let value = s.trim();
    if value.is_empty() {
        return Err(AppError::validation(ValidationError::DurationEmpty));
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
        return Err(AppError::validation(
            ValidationError::InvalidDurationFormat {
                value: value.to_owned(),
            },
        ));
    }
    let (num_part, unit_part) = value.split_at(digits_len);
    let number: u64 = num_part.parse().map_err(|err| {
        AppError::validation(ValidationError::InvalidDurationNumber {
            value: value.to_owned(),
            source: err,
        })
    })?;

    let unit = if unit_part.is_empty() { "s" } else { unit_part };
    let duration = match unit {
        "ms" => Duration::from_millis(number),
        "s" => Duration::from_secs(number),
        "m" => {
            let secs = number
                .checked_mul(60)
                .ok_or_else(|| AppError::validation(ValidationError::DurationOverflow))?;
            Duration::from_secs(secs)
        }
        "h" => {
            let secs = number
                .checked_mul(60)
                .and_then(|seconds| seconds.checked_mul(60))
                .ok_or_else(|| AppError::validation(ValidationError::DurationOverflow))?;
            Duration::from_secs(secs)
        }
        _ => {
            return Err(AppError::validation(ValidationError::InvalidDurationUnit {
                unit: unit.to_owned(),
            }));
        }
    };

    if duration.as_millis() == 0 {
        return Err(AppError::validation(ValidationError::DurationZero));
    }

    Ok(duration)
}
