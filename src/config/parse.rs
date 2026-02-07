use std::time::Duration;

pub(crate) fn parse_duration_value(value: &str) -> Result<Duration, String> {
    let value = value.trim();
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
