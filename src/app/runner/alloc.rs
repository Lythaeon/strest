use tracing::warn;

use crate::shutdown::ShutdownSender;

#[cfg(feature = "alloc-profiler")]
use std::time::Duration;

#[cfg(feature = "alloc-profiler")]
use tracing::info;

#[cfg(feature = "alloc-profiler")]
use crate::error::{AppError, AppResult, MetricsError};

#[cfg(feature = "alloc-profiler")]
#[derive(Debug)]
struct JemallocCtlError(jemalloc_ctl::Error);

#[cfg(feature = "alloc-profiler")]
impl std::fmt::Display for JemallocCtlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(feature = "alloc-profiler")]
impl std::error::Error for JemallocCtlError {}

#[cfg(feature = "alloc-profiler")]
fn boxed_jemalloc_error(err: jemalloc_ctl::Error) -> Box<dyn std::error::Error + Send + Sync> {
    Box::new(JemallocCtlError(err))
}

pub(in crate::app::runner) fn setup_alloc_profiler_task(
    shutdown_tx: &ShutdownSender,
    interval_ms: Option<&crate::args::PositiveU64>,
) -> tokio::task::JoinHandle<()> {
    let Some(interval_ms) = interval_ms.map(|value| value.get()) else {
        return tokio::spawn(async {});
    };
    setup_alloc_profiler_task_inner(shutdown_tx, interval_ms)
}

#[cfg(not(feature = "alloc-profiler"))]
fn setup_alloc_profiler_task_inner(
    _shutdown_tx: &ShutdownSender,
    _interval_ms: u64,
) -> tokio::task::JoinHandle<()> {
    warn!("alloc-profiler-ms set but alloc-profiler feature is disabled.");
    tokio::spawn(async {})
}

#[cfg(feature = "alloc-profiler")]
fn setup_alloc_profiler_task_inner(
    shutdown_tx: &ShutdownSender,
    interval_ms: u64,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = interval.tick() => {
                    if let Err(err) = log_alloc_stats() {
                        warn!("alloc-profiler failed: {}", err);
                        break;
                    }
                }
            }
        }
    })
}

pub(in crate::app::runner) fn setup_alloc_profiler_dump_task(
    shutdown_tx: &ShutdownSender,
    interval_ms: Option<&crate::args::PositiveU64>,
    dump_path: &str,
) -> tokio::task::JoinHandle<()> {
    let Some(interval_ms) = interval_ms.map(|value| value.get()) else {
        return tokio::spawn(async {});
    };
    setup_alloc_profiler_dump_task_inner(shutdown_tx, interval_ms, dump_path)
}

#[cfg(not(feature = "alloc-profiler"))]
fn setup_alloc_profiler_dump_task_inner(
    _shutdown_tx: &ShutdownSender,
    _interval_ms: u64,
    _dump_path: &str,
) -> tokio::task::JoinHandle<()> {
    warn!("alloc-profiler-dump-ms set but alloc-profiler feature is disabled.");
    tokio::spawn(async {})
}

#[cfg(feature = "alloc-profiler")]
fn setup_alloc_profiler_dump_task_inner(
    shutdown_tx: &ShutdownSender,
    interval_ms: u64,
    dump_path: &str,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    let dump_path = dump_path.to_owned();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(interval_ms.max(1)));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        if let Err(err) = tokio::fs::create_dir_all(&dump_path).await {
            warn!(
                "alloc-profiler failed to create dump dir {}: {}",
                dump_path, err
            );
            return;
        }
        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                _ = interval.tick() => {
                    if let Err(err) = dump_alloc_profile(&dump_path) {
                        warn!("alloc-profiler dump failed: {}", err);
                        break;
                    }
                }
            }
        }
    })
}

#[cfg(feature = "alloc-profiler")]
fn log_alloc_stats() -> AppResult<()> {
    jemalloc_ctl::epoch::advance().map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "epoch advance failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    let allocated = jemalloc_ctl::stats::allocated::read().map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "allocated read failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    let active = jemalloc_ctl::stats::active::read().map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "active read failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    let resident = jemalloc_ctl::stats::resident::read().map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "resident read failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    let mapped = jemalloc_ctl::stats::mapped::read().map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "mapped read failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    let metadata = jemalloc_ctl::stats::metadata::read().map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "metadata read failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    info!(
        "alloc_bytes={},active_bytes={},resident_bytes={},mapped_bytes={},metadata_bytes={}",
        allocated, active, resident, mapped, metadata
    );
    Ok(())
}

#[cfg(feature = "alloc-profiler")]
fn dump_alloc_profile(dir: &str) -> AppResult<()> {
    use std::ffi::CString;
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| {
            AppError::metrics(MetricsError::External {
                context: "timestamp error",
                source: Box::new(err),
            })
        })?
        .as_millis();
    let path = std::path::Path::new(dir).join(format!("heap-{}.prof", stamp));
    let path_cstr = CString::new(path.to_string_lossy().as_bytes()).map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "invalid dump path",
            source: Box::new(err),
        })
    })?;
    ensure_prof_enabled()?;
    jemalloc_ctl::epoch::advance().map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "epoch advance failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    // Safety: prof.dump expects a C string pointing to the output file path.
    unsafe {
        jemalloc_ctl::raw::write(b"prof.dump\0", path_cstr.as_ptr()).map_err(|err| {
            AppError::metrics(MetricsError::External {
                context: "prof dump failed",
                source: boxed_jemalloc_error(err),
            })
        })?;
    }
    info!("alloc_profiler_dump={}", path.display());
    Ok(())
}

#[cfg(feature = "alloc-profiler")]
fn ensure_prof_enabled() -> AppResult<()> {
    // Safety: config.prof is a valid NUL-terminated key for jemalloc boolean config.
    let config_prof =
        unsafe { jemalloc_ctl::raw::read::<bool>(b"config.prof\0") }.map_err(|err| {
            AppError::metrics(MetricsError::External {
                context: "prof config read failed",
                source: boxed_jemalloc_error(err),
            })
        })?;
    if !config_prof {
        return Err(AppError::metrics(MetricsError::ProfilerNotCompiled));
    }
    // Safety: opt.prof is a valid NUL-terminated key for jemalloc boolean config.
    let opt_prof = unsafe { jemalloc_ctl::raw::read::<bool>(b"opt.prof\0") }.map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "opt.prof read failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    if !opt_prof {
        return Err(AppError::metrics(MetricsError::ProfilerDisabled));
    }
    // Safety: prof.active is a valid NUL-terminated key for jemalloc boolean config.
    let active = unsafe { jemalloc_ctl::raw::read::<bool>(b"prof.active\0") }.map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "prof.active read failed",
            source: boxed_jemalloc_error(err),
        })
    })?;
    if !active {
        // Safety: prof.active expects a boolean value.
        unsafe {
            jemalloc_ctl::raw::write(b"prof.active\0", true).map_err(|err| {
                AppError::metrics(MetricsError::External {
                    context: "prof.active write failed",
                    source: boxed_jemalloc_error(err),
                })
            })?;
        }
    }
    Ok(())
}
