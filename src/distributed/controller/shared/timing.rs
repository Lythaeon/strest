use std::time::Duration;

use tracing::warn;

use crate::args::TesterArgs;
use crate::sinks::config::SinksConfig;

pub(in crate::distributed::controller) const REPORT_GRACE_SECS: u64 = 30;
pub(in crate::distributed::controller) const DEFAULT_SINK_INTERVAL: Duration =
    Duration::from_secs(1);
pub(in crate::distributed::controller) const DEFAULT_START_AFTER_MS: u64 = 3000;

pub(in crate::distributed::controller) fn resolve_sink_interval(
    config: Option<&SinksConfig>,
) -> Duration {
    match config.and_then(|value| value.update_interval_ms) {
        Some(0) => {
            warn!(
                "sinks.update_interval_ms must be > 0; using default {}ms",
                DEFAULT_SINK_INTERVAL.as_millis()
            );
            DEFAULT_SINK_INTERVAL
        }
        Some(ms) => Duration::from_millis(ms),
        None => DEFAULT_SINK_INTERVAL,
    }
}

pub(in crate::distributed::controller) fn resolve_agent_wait_timeout(
    args: &TesterArgs,
) -> Option<Duration> {
    args.agent_wait_timeout_ms
        .map(|value| Duration::from_millis(value.get()))
}

pub(in crate::distributed::controller) fn resolve_heartbeat_check_interval(
    timeout: Duration,
) -> Duration {
    let timeout_ms = timeout.as_millis();
    let mut interval_ms = timeout_ms.saturating_div(2);
    if interval_ms < 200 {
        interval_ms = timeout_ms.max(1);
    }
    Duration::from_millis(u64::try_from(interval_ms).unwrap_or(1))
}
