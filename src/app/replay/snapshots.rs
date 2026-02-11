use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, ValidationError};
use crate::metrics::MetricRecord;

use super::super::export;
use super::bounds::{BoundDefault, resolve_bound};
use super::state::{ReplayWindow, SnapshotMarkers};
use super::{summary, window_slice};

#[derive(Debug, Clone, Copy)]
pub(super) enum SnapshotFormat {
    Json,
    Jsonl,
    Csv,
}

pub(super) struct SnapshotIntervalState {
    interval_ms: u64,
    next_start_ms: u64,
    end_ms: u64,
}

impl SnapshotIntervalState {
    pub(super) fn new(interval: Duration, start_ms: u64, end_ms: u64) -> AppResult<Self> {
        let interval_ms = u64::try_from(interval.as_millis()).unwrap_or(0);
        if interval_ms == 0 {
            return Err(AppError::validation(
                ValidationError::ReplaySnapshotIntervalTooSmall,
            ));
        }
        Ok(Self {
            interval_ms,
            next_start_ms: start_ms,
            end_ms,
        })
    }
}

pub(super) fn parse_snapshot_format(value: &str) -> AppResult<SnapshotFormat> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "json" => Ok(SnapshotFormat::Json),
        "jsonl" | "ndjson" => Ok(SnapshotFormat::Jsonl),
        "csv" => Ok(SnapshotFormat::Csv),
        _ => Err(AppError::validation(
            ValidationError::InvalidSnapshotFormat {
                value: value.to_owned(),
            },
        )),
    }
}

const fn snapshot_extension(format: SnapshotFormat) -> &'static str {
    match format {
        SnapshotFormat::Json => "json",
        SnapshotFormat::Jsonl => "jsonl",
        SnapshotFormat::Csv => "csv",
    }
}

pub(super) fn resolve_snapshot_window(
    args: &TesterArgs,
    min_ms: u64,
    max_ms: u64,
    replay_start: u64,
    replay_end: u64,
) -> AppResult<(u64, u64)> {
    let start = if args.replay_snapshot_start.is_some() {
        resolve_bound(
            args.replay_snapshot_start.as_deref(),
            min_ms,
            max_ms,
            BoundDefault::Min,
        )?
    } else {
        replay_start
    };
    let end = if args.replay_snapshot_end.is_some() {
        resolve_bound(
            args.replay_snapshot_end.as_deref(),
            min_ms,
            max_ms,
            BoundDefault::Max,
        )?
    } else {
        replay_end
    };
    let start = start.clamp(replay_start, replay_end);
    let end = end.clamp(replay_start, replay_end);
    if start > end {
        return Err(AppError::validation(
            ValidationError::ReplaySnapshotStartAfterEnd,
        ));
    }
    Ok((start, end))
}

pub(super) fn resolve_snapshot_range(
    markers: &SnapshotMarkers,
    window: &ReplayWindow,
    default_range: Option<(u64, u64)>,
) -> (u64, u64) {
    let (mut start, mut end) = match (markers.start, markers.end) {
        (Some(start), Some(end)) => (start, end),
        (Some(start), None) => (start, window.cursor_ms),
        (None, Some(end)) => (window.cursor_ms, end),
        (None, None) => default_range.unwrap_or((window.start_ms, window.cursor_ms)),
    };
    start = start.clamp(window.start_ms, window.end_ms);
    end = end.clamp(window.start_ms, window.end_ms);
    if start > end {
        (end, start)
    } else {
        (start, end)
    }
}

pub(super) async fn emit_interval_snapshots(
    records: &[MetricRecord],
    args: &TesterArgs,
    format: SnapshotFormat,
    state: &mut SnapshotIntervalState,
    current_ms: u64,
    finalize: bool,
) -> AppResult<Option<PathBuf>> {
    let mut last_path = None;
    let current_ms = current_ms.min(state.end_ms);
    if current_ms < state.next_start_ms {
        return Ok(None);
    }

    while state.next_start_ms < state.end_ms {
        let next_end = state
            .next_start_ms
            .saturating_add(state.interval_ms)
            .min(state.end_ms);
        if current_ms >= next_end {
            last_path = Some(
                write_snapshot(records, args, format, state.next_start_ms, next_end, true).await?,
            );
            state.next_start_ms = next_end;
            continue;
        }
        if finalize && current_ms > state.next_start_ms {
            last_path = Some(
                write_snapshot(records, args, format, state.next_start_ms, current_ms, true)
                    .await?,
            );
            state.next_start_ms = current_ms;
        }
        break;
    }

    Ok(last_path)
}

pub(super) async fn write_snapshot(
    records: &[MetricRecord],
    args: &TesterArgs,
    format: SnapshotFormat,
    start_ms: u64,
    end_ms: u64,
    multi: bool,
) -> AppResult<PathBuf> {
    let path = snapshot_path(args, format, start_ms, end_ms, multi).await?;
    let slice = window_slice(records, start_ms, end_ms);
    match format {
        SnapshotFormat::Csv => {
            export::export_csv(&path.to_string_lossy(), slice).await?;
        }
        SnapshotFormat::Json => {
            let summary_output =
                summary::summarize(slice, args.expected_status_code, start_ms, end_ms)?;
            export::export_json(&path.to_string_lossy(), &summary_output.summary, slice).await?;
        }
        SnapshotFormat::Jsonl => {
            let summary_output =
                summary::summarize(slice, args.expected_status_code, start_ms, end_ms)?;
            export::export_jsonl(&path.to_string_lossy(), &summary_output.summary, slice).await?;
        }
    }
    Ok(path)
}

async fn snapshot_path(
    args: &TesterArgs,
    format: SnapshotFormat,
    start_ms: u64,
    end_ms: u64,
    multi: bool,
) -> AppResult<PathBuf> {
    let ext = snapshot_extension(format);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis();
    let file_name = format!("snapshot-{}ms-{}ms-{}.{}", start_ms, end_ms, stamp, ext);

    let base = args
        .replay_snapshot_out
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default_snapshots_path()));

    if base.exists() && base.is_dir() {
        tokio::fs::create_dir_all(&base).await?;
        return Ok(base.join(file_name));
    }

    if args.replay_snapshot_out.is_none() || base.extension().is_none() {
        tokio::fs::create_dir_all(&base).await?;
        return Ok(base.join(file_name));
    }

    if multi {
        let parent = base.parent().unwrap_or_else(|| Path::new("."));
        tokio::fs::create_dir_all(parent).await?;
        let stem = base
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("snapshot");
        let name = format!("{stem}-{}ms-{}ms-{}.{}", start_ms, end_ms, stamp, ext);
        return Ok(parent.join(name));
    }

    if let Some(parent) = base.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    Ok(base)
}

fn default_snapshots_path() -> String {
    default_base_dir()
        .join("snapshots")
        .to_string_lossy()
        .into_owned()
}

fn default_base_dir() -> PathBuf {
    if let Some(home) = user_home_dir() {
        return home.join(".strest");
    }

    PathBuf::from(".strest")
}

fn user_home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        if let Some(value) = std::env::var_os("USERPROFILE") {
            return Some(PathBuf::from(value));
        }
        let drive = std::env::var_os("HOMEDRIVE");
        let path = std::env::var_os("HOMEPATH");
        match (drive, path) {
            (Some(drive), Some(path)) => {
                let mut full = PathBuf::from(drive);
                full.push(path);
                return Some(full);
            }
            _ => {}
        }
    }

    if let Some(value) = std::env::var_os("HOME") {
        return Some(PathBuf::from(value));
    }

    None
}
