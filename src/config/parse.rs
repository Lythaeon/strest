use std::time::Duration;

use crate::error::{AppError, AppResult, ConfigError};

pub(crate) fn parse_duration_value(value: &str) -> AppResult<Duration> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::config(ConfigError::DurationEmpty));
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
        return Err(AppError::config(ConfigError::InvalidDurationFormat {
            value: value.to_owned(),
        }));
    }
    let (num_part, unit_part) = value.split_at(digits_len);
    let number: u64 = num_part.parse().map_err(|err| {
        AppError::config(ConfigError::InvalidDurationNumber {
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
                .ok_or_else(|| AppError::config(ConfigError::DurationOverflow))?;
            Duration::from_secs(secs)
        }
        "h" => {
            let secs = number
                .checked_mul(60)
                .and_then(|seconds| seconds.checked_mul(60))
                .ok_or_else(|| AppError::config(ConfigError::DurationOverflow))?;
            Duration::from_secs(secs)
        }
        _ => {
            return Err(AppError::config(ConfigError::InvalidDurationUnit {
                unit: unit.to_owned(),
            }));
        }
    };

    if duration.as_millis() == 0 {
        return Err(AppError::config(ConfigError::DurationZero));
    }

    Ok(duration)
}
