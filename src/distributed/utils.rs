use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(super) fn build_run_id() -> String {
    let now = current_time_ms();
    format!("{}-{}", now, std::process::id())
}

pub(super) fn current_time_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

pub(super) fn duration_to_ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}
