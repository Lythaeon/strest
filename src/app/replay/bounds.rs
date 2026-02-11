use std::time::Duration;

use crate::error::AppResult;

#[derive(Debug, Clone, Copy)]
pub(super) enum BoundDefault {
    Min,
    Max,
}

pub(super) fn resolve_bound(
    value: Option<&str>,
    min_ms: u64,
    max_ms: u64,
    default: BoundDefault,
) -> AppResult<u64> {
    let bound = match value {
        Some(value) => parse_bound(value)?,
        None => match default {
            BoundDefault::Min => return Ok(min_ms),
            BoundDefault::Max => return Ok(max_ms),
        },
    };
    let resolved = match bound {
        ReplayBound::Min => min_ms,
        ReplayBound::Max => max_ms,
        ReplayBound::Duration(duration) => u64::try_from(duration.as_millis()).unwrap_or(max_ms),
    };
    Ok(resolved.clamp(min_ms, max_ms))
}

pub(super) fn parse_bound(value: &str) -> AppResult<ReplayBound> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("min") {
        return Ok(ReplayBound::Min);
    }
    if trimmed.eq_ignore_ascii_case("max") {
        return Ok(ReplayBound::Max);
    }
    let duration = crate::args::parsers::parse_duration_arg(trimmed)?;
    Ok(ReplayBound::Duration(duration))
}

pub(super) enum ReplayBound {
    Min,
    Max,
    Duration(Duration),
}
