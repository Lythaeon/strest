use std::time::Duration;

use tracing::info;

use crate::shutdown::ShutdownSender;

pub(in crate::app::runner) fn setup_rss_log_task(
    shutdown_tx: &ShutdownSender,
    no_ui: bool,
    interval_ms: Option<&crate::args::PositiveU64>,
) -> tokio::task::JoinHandle<()> {
    if !no_ui {
        return tokio::spawn(async {});
    }
    let Some(interval_ms) = interval_ms.map(|value| value.get()) else {
        return tokio::spawn(async {});
    };
    let shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = interval.tick() => {
                    if let Some(rss_bytes) = read_rss_bytes() {
                        let rss_mb_x100 = u128::from(rss_bytes)
                            .saturating_mul(100)
                            .checked_div(1024 * 1024)
                            .unwrap_or(0);
                        let whole = rss_mb_x100 / 100;
                        let frac = rss_mb_x100 % 100;
                        info!("rss_mb={}.{:02}", whole, frac);
                    } else {
                        break;
                    }
                }
            }
        }
    })
}

fn read_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
        let mut parts = statm.split_whitespace();
        let _size = parts.next()?;
        let resident = parts.next()?.parse::<u64>().ok()?;
        // Safety: sysconf is safe to call; we only read the page size.
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        if page_size <= 0 {
            return None;
        }
        let page_size = u64::try_from(page_size).ok()?;
        Some(resident.saturating_mul(page_size))
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
