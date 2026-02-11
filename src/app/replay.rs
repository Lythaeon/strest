use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{broadcast, watch};

use super::{export, summary};
use crate::args::TesterArgs;
use crate::error::{AppError, AppResult};
use crate::metrics::{LatencyHistogram, MetricRecord, MetricsSummary};
use crate::ui::model::{ReplayUi, UiData};
use crate::ui::render::setup_render_ui;

const REPLAY_TICK_MS: u64 = 1000;

pub(crate) async fn run_replay(args: &TesterArgs) -> AppResult<()> {
    let records = load_replay_records(args).await?;
    if records.is_empty() {
        return Err("No metrics records found for replay.".into());
    }

    let mut records = records;
    records.sort_by_key(|record| record.elapsed_ms);
    let min_ms = records.first().map(|record| record.elapsed_ms).unwrap_or(0);
    let max_ms = records.last().map(|record| record.elapsed_ms).unwrap_or(0);

    let start_ms = resolve_bound(
        args.replay_start.as_deref(),
        min_ms,
        max_ms,
        BoundDefault::Min,
    )?;
    let end_ms = resolve_bound(
        args.replay_end.as_deref(),
        min_ms,
        max_ms,
        BoundDefault::Max,
    )?;
    if start_ms > end_ms {
        return Err("Replay start must be <= replay end.".into());
    }

    let snapshot_format = parse_snapshot_format(&args.replay_snapshot_format)?;
    let snapshot_window = resolve_snapshot_window(args, min_ms, max_ms, start_ms, end_ms)?;
    let snapshot_default_range =
        if args.replay_snapshot_start.is_some() || args.replay_snapshot_end.is_some() {
            Some(snapshot_window)
        } else {
            None
        };
    let snapshot_requested = args.replay_snapshot_interval.is_some()
        || args.replay_snapshot_start.is_some()
        || args.replay_snapshot_end.is_some()
        || args.replay_snapshot_out.is_some();

    let step_ms = args
        .replay_step
        .unwrap_or_else(|| Duration::from_secs(1))
        .as_millis()
        .try_into()
        .unwrap_or(1);

    if !io::stdout().is_terminal() || args.no_ui {
        if snapshot_requested {
            if let Some(interval) = args.replay_snapshot_interval {
                let mut interval_state =
                    SnapshotIntervalState::new(interval, snapshot_window.0, snapshot_window.1)?;
                emit_interval_snapshots(
                    &records,
                    args,
                    snapshot_format,
                    &mut interval_state,
                    snapshot_window.1,
                    true,
                )
                .await?;
            } else {
                let (snapshot_start, snapshot_end) = snapshot_window;
                write_snapshot(
                    &records,
                    args,
                    snapshot_format,
                    snapshot_start,
                    snapshot_end,
                    false,
                )
                .await?;
            }
        }
        render_once(&records, args, start_ms, end_ms)?;
        return Ok(());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_handle = stop.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            stop_handle.store(true, Ordering::SeqCst);
        }
    });

    let (shutdown_tx, _) = broadcast::channel::<u16>(1);
    let initial_ui = UiData {
        target_duration: Duration::from_millis(end_ms.saturating_sub(start_ms)),
        ui_window_ms: args.ui_window_ms.get(),
        no_color: args.no_color,
        ..UiData::default()
    };
    let (ui_tx, _) = watch::channel(initial_ui);
    let render_ui_handle = setup_render_ui(args, &shutdown_tx, &ui_tx);

    let mut state = ReplayWindow {
        start_ms,
        cursor_ms: start_ms,
        end_ms,
        playing: true,
    };
    let mut last_tick = tokio::time::Instant::now();
    let poll_interval = Duration::from_millis(100);
    let mut dirty = true;
    let mut snapshot_markers = SnapshotMarkers::default();
    let mut snapshot_interval_state = args
        .replay_snapshot_interval
        .map(|interval| SnapshotIntervalState::new(interval, snapshot_window.0, snapshot_window.1))
        .transpose()?;
    let result = async {
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }

            if event::poll(Duration::from_millis(0))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc) {
                    break;
                }
                if matches!(key.code, KeyCode::Char(' ')) {
                    state.playing = !state.playing;
                    dirty = true;
                } else if matches!(key.code, KeyCode::Left | KeyCode::Char('h')) {
                    state.playing = false;
                    state.cursor_ms = state.cursor_ms.saturating_sub(step_ms).max(state.start_ms);
                    dirty = true;
                } else if matches!(key.code, KeyCode::Right | KeyCode::Char('l')) {
                    state.playing = false;
                    state.cursor_ms = state.cursor_ms.saturating_add(step_ms).min(state.end_ms);
                    dirty = true;
                } else if matches!(key.code, KeyCode::Home) {
                    state.playing = false;
                    state.cursor_ms = state.start_ms;
                    dirty = true;
                } else if matches!(key.code, KeyCode::End) {
                    state.playing = false;
                    state.cursor_ms = state.end_ms;
                    dirty = true;
                } else if matches!(key.code, KeyCode::Char('r')) {
                    state.playing = false;
                    state.cursor_ms = state.start_ms;
                    dirty = true;
                } else if matches!(key.code, KeyCode::Char('s')) {
                    snapshot_markers.start = Some(state.cursor_ms);
                    dirty = true;
                } else if matches!(key.code, KeyCode::Char('e')) {
                    snapshot_markers.end = Some(state.cursor_ms);
                    dirty = true;
                } else if matches!(key.code, KeyCode::Char('w')) {
                    let (snapshot_start, snapshot_end) =
                        resolve_snapshot_range(&snapshot_markers, &state, snapshot_default_range);
                    write_snapshot(
                        &records,
                        args,
                        snapshot_format,
                        snapshot_start,
                        snapshot_end,
                        true,
                    )
                    .await?;
                    snapshot_markers.clear();
                    dirty = true;
                }
            }

            if state.playing {
                let elapsed = last_tick.elapsed();
                if elapsed >= Duration::from_millis(REPLAY_TICK_MS) {
                    let tick_ms = u128::from(REPLAY_TICK_MS);
                    let steps = elapsed.as_millis().checked_div(tick_ms).unwrap_or(0);
                    let advance_ms =
                        u64::try_from(steps.saturating_mul(tick_ms)).unwrap_or(REPLAY_TICK_MS);
                    state.cursor_ms = state.cursor_ms.saturating_add(advance_ms).min(state.end_ms);
                    last_tick = tokio::time::Instant::now();
                    if state.cursor_ms >= state.end_ms {
                        state.cursor_ms = state.end_ms;
                        state.playing = false;
                    }
                    dirty = true;
                }
            } else {
                last_tick = tokio::time::Instant::now();
            }

            if let Some(snapshot_interval) = snapshot_interval_state.as_mut() {
                emit_interval_snapshots(
                    &records,
                    args,
                    snapshot_format,
                    snapshot_interval,
                    state.cursor_ms,
                    false,
                )
                .await?;
            }

            if dirty {
                let ui_data = build_ui_data(
                    &records,
                    args,
                    &state,
                    &snapshot_markers,
                    snapshot_default_range,
                )?;
                drop(ui_tx.send(ui_data));
                dirty = false;
            }

            tokio::time::sleep(poll_interval).await;
        }

        if let Some(snapshot_interval) = snapshot_interval_state.as_mut() {
            emit_interval_snapshots(
                &records,
                args,
                snapshot_format,
                snapshot_interval,
                state.cursor_ms,
                true,
            )
            .await?;
        }

        Ok::<(), AppError>(())
    }
    .await;

    drop(shutdown_tx.send(1));
    if let Err(err) = render_ui_handle.await {
        eprintln!("Replay UI task failed: {}", err);
    }
    result
}

fn render_once(
    records: &[MetricRecord],
    args: &TesterArgs,
    start_ms: u64,
    end_ms: u64,
) -> AppResult<()> {
    let slice = window_slice(records, start_ms, end_ms);
    let summary_output = summarize(slice, args.expected_status_code, start_ms, end_ms)?;
    let stats = summary::compute_summary_stats(&summary_output.summary);
    let (p50, p90, p99, success_p50, success_p90, success_p99) =
        compute_replay_percentiles(&summary_output, slice, args.expected_status_code);
    let extras = summary::SummaryExtras {
        metrics_truncated: false,
        charts_enabled: false,
        p50,
        p90,
        p99,
        success_p50,
        success_p90,
        success_p99,
    };
    for line in summary::summary_lines(&summary_output.summary, &extras, &stats, args) {
        println!("{line}");
    }
    Ok(())
}

fn window_slice(records: &[MetricRecord], start_ms: u64, end_ms: u64) -> &[MetricRecord] {
    if records.is_empty() {
        return records;
    }
    let start_idx = records.partition_point(|record| record.elapsed_ms < start_ms);
    let end_idx = records.partition_point(|record| record.elapsed_ms <= end_ms);
    records.get(start_idx..end_idx).unwrap_or(&[])
}

fn summarize(
    records: &[MetricRecord],
    expected_status_code: u16,
    window_start_ms: u64,
    window_end_ms: u64,
) -> AppResult<SummaryOutput> {
    let mut histogram = LatencyHistogram::new().map_err(AppError::from)?;
    let mut success_histogram = LatencyHistogram::new().map_err(AppError::from)?;

    let mut total_requests: u64 = 0;
    let mut successful_requests: u64 = 0;
    let mut timeout_requests: u64 = 0;
    let mut transport_errors: u64 = 0;
    let mut non_expected_status: u64 = 0;
    let mut latency_sum_ms: u128 = 0;
    let mut success_latency_sum_ms: u128 = 0;
    let mut min_latency_ms: u64 = u64::MAX;
    let mut max_latency_ms: u64 = 0;
    let mut success_min_latency_ms: u64 = u64::MAX;
    let mut success_max_latency_ms: u64 = 0;

    for record in records {
        total_requests = total_requests.saturating_add(1);
        latency_sum_ms = latency_sum_ms.saturating_add(u128::from(record.latency_ms));
        if record.latency_ms < min_latency_ms {
            min_latency_ms = record.latency_ms;
        }
        if record.latency_ms > max_latency_ms {
            max_latency_ms = record.latency_ms;
        }
        if record.status_code == expected_status_code
            && !record.timed_out
            && !record.transport_error
        {
            successful_requests = successful_requests.saturating_add(1);
            success_latency_sum_ms =
                success_latency_sum_ms.saturating_add(u128::from(record.latency_ms));
            if record.latency_ms < success_min_latency_ms {
                success_min_latency_ms = record.latency_ms;
            }
            if record.latency_ms > success_max_latency_ms {
                success_max_latency_ms = record.latency_ms;
            }
            success_histogram
                .record(record.latency_ms)
                .map_err(AppError::from)?;
        }
        if record.timed_out {
            timeout_requests = timeout_requests.saturating_add(1);
        } else if record.transport_error {
            transport_errors = transport_errors.saturating_add(1);
        } else if record.status_code != expected_status_code {
            non_expected_status = non_expected_status.saturating_add(1);
        }
        histogram
            .record(record.latency_ms)
            .map_err(AppError::from)?;
    }

    let duration_ms = window_end_ms.saturating_sub(window_start_ms);
    let duration = Duration::from_millis(duration_ms);
    let avg_latency_ms = if total_requests > 0 {
        let avg = latency_sum_ms
            .checked_div(u128::from(total_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };
    let success_avg_latency_ms = if successful_requests > 0 {
        let avg = success_latency_sum_ms
            .checked_div(u128::from(successful_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };
    let min_latency_ms = if total_requests > 0 {
        min_latency_ms
    } else {
        0
    };
    let max_latency_ms = if total_requests > 0 {
        max_latency_ms
    } else {
        0
    };
    let success_min_latency_ms = if successful_requests > 0 {
        success_min_latency_ms
    } else {
        0
    };
    let success_max_latency_ms = if successful_requests > 0 {
        success_max_latency_ms
    } else {
        0
    };
    let error_requests = total_requests.saturating_sub(successful_requests);

    Ok(SummaryOutput {
        summary: MetricsSummary {
            duration,
            total_requests,
            successful_requests,
            error_requests,
            timeout_requests,
            transport_errors,
            non_expected_status,
            min_latency_ms,
            max_latency_ms,
            avg_latency_ms,
            success_min_latency_ms,
            success_max_latency_ms,
            success_avg_latency_ms,
        },
        histogram,
        success_histogram,
    })
}

struct SummaryOutput {
    summary: MetricsSummary,
    histogram: LatencyHistogram,
    success_histogram: LatencyHistogram,
}

struct ReplayWindow {
    start_ms: u64,
    cursor_ms: u64,
    end_ms: u64,
    playing: bool,
}

#[derive(Default)]
struct SnapshotMarkers {
    start: Option<u64>,
    end: Option<u64>,
}

impl SnapshotMarkers {
    const fn clear(&mut self) {
        self.start = None;
        self.end = None;
    }
}

#[derive(Debug, Clone, Copy)]
enum SnapshotFormat {
    Json,
    Jsonl,
    Csv,
}

struct SnapshotIntervalState {
    interval_ms: u64,
    next_start_ms: u64,
    end_ms: u64,
}

impl SnapshotIntervalState {
    fn new(interval: Duration, start_ms: u64, end_ms: u64) -> AppResult<Self> {
        let interval_ms = u64::try_from(interval.as_millis()).unwrap_or(0);
        if interval_ms == 0 {
            return Err("Replay snapshot interval must be >= 1ms.".into());
        }
        Ok(Self {
            interval_ms,
            next_start_ms: start_ms,
            end_ms,
        })
    }
}

#[derive(Debug, Clone, Copy)]
enum BoundDefault {
    Min,
    Max,
}

fn resolve_bound(
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

fn parse_bound(value: &str) -> AppResult<ReplayBound> {
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

enum ReplayBound {
    Min,
    Max,
    Duration(Duration),
}

fn parse_snapshot_format(value: &str) -> AppResult<SnapshotFormat> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "json" => Ok(SnapshotFormat::Json),
        "jsonl" | "ndjson" => Ok(SnapshotFormat::Jsonl),
        "csv" => Ok(SnapshotFormat::Csv),
        _ => Err(format!(
            "Unsupported snapshot format: {} (expected json, jsonl, or csv).",
            value
        )
        .into()),
    }
}

const fn snapshot_extension(format: SnapshotFormat) -> &'static str {
    match format {
        SnapshotFormat::Json => "json",
        SnapshotFormat::Jsonl => "jsonl",
        SnapshotFormat::Csv => "csv",
    }
}

fn resolve_snapshot_window(
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
        return Err("Replay snapshot start must be <= replay snapshot end.".into());
    }
    Ok((start, end))
}

fn resolve_snapshot_range(
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

async fn emit_interval_snapshots(
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

async fn write_snapshot(
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
            let summary_output = summarize(slice, args.expected_status_code, start_ms, end_ms)?;
            export::export_json(&path.to_string_lossy(), &summary_output.summary, slice).await?;
        }
        SnapshotFormat::Jsonl => {
            let summary_output = summarize(slice, args.expected_status_code, start_ms, end_ms)?;
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

async fn load_replay_records(args: &TesterArgs) -> AppResult<Vec<MetricRecord>> {
    let export_sources = [
        args.export_csv.as_ref(),
        args.export_json.as_ref(),
        args.export_jsonl.as_ref(),
    ]
    .into_iter()
    .filter(|value| value.is_some())
    .count();
    if export_sources > 1 {
        return Err(
            "Provide only one of --export-csv, --export-json, or --export-jsonl for replay.".into(),
        );
    }
    if let Some(path) = args.export_csv.as_deref() {
        return read_csv_records(Path::new(path)).await.map_err(Into::into);
    }
    if let Some(path) = args.export_json.as_deref() {
        return read_json_records(Path::new(path)).await.map_err(Into::into);
    }
    if let Some(path) = args.export_jsonl.as_deref() {
        return read_jsonl_records(Path::new(path))
            .await
            .map_err(Into::into);
    }
    read_tmp_records(Path::new(&args.tmp_path))
        .await
        .map_err(Into::into)
}

async fn read_tmp_records(path: &Path) -> Result<Vec<MetricRecord>, String> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|err| format!("Failed to stat tmp path {}: {}", path.display(), err))?;
    if metadata.is_file() {
        return read_csv_records(path).await;
    }
    if !metadata.is_dir() {
        return Err("Tmp path is not a file or directory.".to_owned());
    }

    let mut entries = tokio::fs::read_dir(path)
        .await
        .map_err(|err| format!("Failed to read tmp directory: {}", err))?;
    let mut records = Vec::new();
    let mut found = false;
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|err| format!("Failed to read tmp entry: {}", err))?
    {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let entry_path = entry.path();
        if !file_name.starts_with("metrics-") || !file_name.ends_with(".log") {
            continue;
        }
        found = true;
        let mut file_records = read_csv_records(&entry_path).await?;
        records.append(&mut file_records);
    }
    if !found {
        return Err("No metrics logs found in tmp directory.".to_owned());
    }
    Ok(records)
}

async fn read_csv_records(path: &Path) -> Result<Vec<MetricRecord>, String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|err| format!("Failed to open {}: {}", path.display(), err))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut records = Vec::new();
    let mut saw_header = false;

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|err| format!("Failed to read {}: {}", path.display(), err))?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !saw_header && trimmed.starts_with("elapsed_ms") {
            saw_header = true;
            continue;
        }
        saw_header = true;
        let mut parts = trimmed.split(',');
        let elapsed_ms = match parts.next().and_then(|value| value.parse::<u64>().ok()) {
            Some(value) => value,
            None => continue,
        };
        let latency_ms = match parts.next().and_then(|value| value.parse::<u64>().ok()) {
            Some(value) => value,
            None => continue,
        };
        let status_code = match parts.next().and_then(|value| value.parse::<u16>().ok()) {
            Some(value) => value,
            None => continue,
        };
        let timed_out = parts.next().map(parse_bool).unwrap_or(false);
        let transport_error = parts.next().map(parse_bool).unwrap_or(false);
        records.push(MetricRecord {
            elapsed_ms,
            latency_ms,
            status_code,
            timed_out,
            transport_error,
        });
    }

    Ok(records)
}

async fn read_json_records(path: &Path) -> Result<Vec<MetricRecord>, String> {
    let bytes = tokio::fs::read(path)
        .await
        .map_err(|err| format!("Failed to read {}: {}", path.display(), err))?;
    let payload: ExportJson = serde_json::from_slice(&bytes)
        .map_err(|err| format!("Failed to parse JSON {}: {}", path.display(), err))?;
    Ok(payload
        .records
        .into_iter()
        .map(|record| MetricRecord {
            elapsed_ms: record.elapsed_ms,
            latency_ms: record.latency_ms,
            status_code: record.status_code,
            timed_out: record.timed_out,
            transport_error: record.transport_error,
        })
        .collect())
}

async fn read_jsonl_records(path: &Path) -> Result<Vec<MetricRecord>, String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|err| format!("Failed to open {}: {}", path.display(), err))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut records = Vec::new();

    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .await
            .map_err(|err| format!("Failed to read {}: {}", path.display(), err))?;
        if bytes == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let parsed: ExportJsonlLine = serde_json::from_str(trimmed)
            .map_err(|err| format!("Failed to parse JSONL {}: {}", path.display(), err))?;
        if let Some(kind) = parsed.kind.as_deref()
            && kind != "record"
        {
            continue;
        }
        let Some(elapsed_ms) = parsed.elapsed_ms else {
            continue;
        };
        let Some(latency_ms) = parsed.latency_ms else {
            continue;
        };
        let Some(status_code) = parsed.status_code else {
            continue;
        };
        records.push(MetricRecord {
            elapsed_ms,
            latency_ms,
            status_code,
            timed_out: parsed.timed_out.unwrap_or(false),
            transport_error: parsed.transport_error.unwrap_or(false),
        });
    }

    Ok(records)
}

fn parse_bool(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed == "1" || trimmed.eq_ignore_ascii_case("true")
}

fn build_ui_data(
    records: &[MetricRecord],
    args: &TesterArgs,
    state: &ReplayWindow,
    markers: &SnapshotMarkers,
    default_range: Option<(u64, u64)>,
) -> AppResult<UiData> {
    let slice = window_slice(records, state.start_ms, state.cursor_ms);
    let summary_output = summarize(
        slice,
        args.expected_status_code,
        state.start_ms,
        state.cursor_ms,
    )?;
    let (p50, p90, p99, p50_ok, p90_ok, p99_ok) =
        compute_replay_percentiles(&summary_output, slice, args.expected_status_code);

    let ui_window_ms = args.ui_window_ms.get();
    let chart_start = state.cursor_ms.saturating_sub(ui_window_ms);
    let chart_slice = window_slice(records, chart_start, state.cursor_ms);
    let latencies = chart_slice
        .iter()
        .map(|record| (record.elapsed_ms, record.latency_ms))
        .collect();

    let rps_start = state.cursor_ms.saturating_sub(1000);
    let rps_slice = window_slice(records, rps_start, state.cursor_ms);
    let rps = u64::try_from(rps_slice.len()).unwrap_or(u64::MAX);
    let rpm = rps.saturating_mul(60);

    let (snapshot_start_ms, snapshot_end_ms) = if markers.start.is_some() || markers.end.is_some() {
        (markers.start, markers.end)
    } else if let Some((start, end)) = default_range {
        (Some(start), Some(end))
    } else {
        (None, None)
    };

    Ok(UiData {
        elapsed_time: Duration::from_millis(state.cursor_ms.saturating_sub(state.start_ms)),
        target_duration: Duration::from_millis(state.end_ms.saturating_sub(state.start_ms)),
        current_requests: summary_output.summary.total_requests,
        successful_requests: summary_output.summary.successful_requests,
        timeout_requests: summary_output.summary.timeout_requests,
        transport_errors: summary_output.summary.transport_errors,
        non_expected_status: summary_output.summary.non_expected_status,
        ui_window_ms,
        no_color: args.no_color,
        latencies,
        p50,
        p90,
        p99,
        p50_ok,
        p90_ok,
        p99_ok,
        rps,
        rpm,
        replay: Some(ReplayUi {
            playing: state.playing,
            window_start_ms: state.start_ms,
            window_end_ms: state.end_ms,
            cursor_ms: state.cursor_ms,
            snapshot_start_ms,
            snapshot_end_ms,
        }),
    })
}

fn compute_replay_percentiles(
    summary_output: &SummaryOutput,
    slice: &[MetricRecord],
    expected_status_code: u16,
) -> (u64, u64, u64, u64, u64, u64) {
    let (mut p50, mut p90, mut p99) = summary_output.histogram.percentiles();
    let (mut success_p50, mut success_p90, mut success_p99) =
        summary_output.success_histogram.percentiles();
    if summary_output.histogram.count() == 0 {
        let (fallback_p50, fallback_p90, fallback_p99) = summary::compute_percentiles(slice);
        p50 = fallback_p50;
        p90 = fallback_p90;
        p99 = fallback_p99;
    }
    if summary_output.success_histogram.count() == 0 {
        let success_records: Vec<MetricRecord> = slice
            .iter()
            .copied()
            .filter(|record| {
                record.status_code == expected_status_code
                    && !record.timed_out
                    && !record.transport_error
            })
            .collect();
        let (fallback_p50, fallback_p90, fallback_p99) =
            summary::compute_percentiles(&success_records);
        success_p50 = fallback_p50;
        success_p90 = fallback_p90;
        success_p99 = fallback_p99;
    }
    (p50, p90, p99, success_p50, success_p90, success_p99)
}

#[derive(Debug, Deserialize)]
struct ExportJson {
    records: Vec<ExportRecord>,
}

#[derive(Debug, Deserialize)]
struct ExportRecord {
    elapsed_ms: u64,
    latency_ms: u64,
    status_code: u16,
    timed_out: bool,
    transport_error: bool,
}

#[derive(Debug, Deserialize)]
struct ExportJsonlLine {
    #[serde(rename = "type")]
    kind: Option<String>,
    elapsed_ms: Option<u64>,
    latency_ms: Option<u64>,
    status_code: Option<u16>,
    timed_out: Option<bool>,
    transport_error: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn run_async_test<F>(future: F) -> Result<(), String>
    where
        F: std::future::Future<Output = Result<(), String>>,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("Failed to build runtime: {}", err))?;
        runtime.block_on(future)
    }

    #[test]
    fn parse_bound_accepts_min_max_and_duration() -> Result<(), String> {
        match parse_bound("min").map_err(|err| err.to_string())? {
            ReplayBound::Min => {}
            ReplayBound::Max | ReplayBound::Duration(_) => {
                return Err("Expected min bound".to_owned());
            }
        }
        match parse_bound("max").map_err(|err| err.to_string())? {
            ReplayBound::Max => {}
            ReplayBound::Min | ReplayBound::Duration(_) => {
                return Err("Expected max bound".to_owned());
            }
        }
        match parse_bound("10s").map_err(|err| err.to_string())? {
            ReplayBound::Duration(duration) => {
                if duration != Duration::from_secs(10) {
                    return Err("Unexpected duration value".to_owned());
                }
            }
            ReplayBound::Min | ReplayBound::Max => {
                return Err("Expected duration bound".to_owned());
            }
        }
        Ok(())
    }

    #[test]
    fn resolve_bound_clamps_to_range() -> Result<(), String> {
        let min = 100;
        let max = 200;
        let resolved_max = resolve_bound(Some("1s"), min, max, BoundDefault::Min)
            .map_err(|err| err.to_string())?;
        if resolved_max != max {
            return Err(format!("Expected clamp to {}, got {}", max, resolved_max));
        }
        let resolved_min = resolve_bound(Some("min"), min, max, BoundDefault::Max)
            .map_err(|err| err.to_string())?;
        if resolved_min != min {
            return Err(format!("Expected min {} got {}", min, resolved_min));
        }
        Ok(())
    }

    #[test]
    fn window_slice_handles_bounds() -> Result<(), String> {
        let records = vec![
            MetricRecord {
                elapsed_ms: 0,
                latency_ms: 10,
                status_code: 200,
                timed_out: false,
                transport_error: false,
            },
            MetricRecord {
                elapsed_ms: 1000,
                latency_ms: 20,
                status_code: 200,
                timed_out: false,
                transport_error: false,
            },
        ];
        let first_slice = window_slice(&records, 0, 500);
        if first_slice.len() != 1 {
            return Err(format!("Expected 1 record, got {}", first_slice.len()));
        }
        let empty_slice = window_slice(&records, 2000, 3000);
        if !empty_slice.is_empty() {
            return Err("Expected empty slice for out-of-range window".to_owned());
        }
        Ok(())
    }

    #[test]
    fn read_csv_records_parses_header_and_values() -> Result<(), String> {
        run_async_test(async {
            let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
            let path = dir.path().join("metrics.csv");
            tokio::fs::write(
                &path,
                "elapsed_ms,latency_ms,status_code,timed_out,transport_error\n1,10,200,0,1\n",
            )
            .await
            .map_err(|err| format!("write failed: {}", err))?;

            let records = read_csv_records(&path).await?;
            if records.len() != 1 {
                return Err(format!("Expected 1 record, got {}", records.len()));
            }
            let record = records
                .first()
                .ok_or_else(|| "Missing parsed record".to_owned())?;
            if record.elapsed_ms != 1 || record.latency_ms != 10 {
                return Err("Unexpected record values".to_owned());
            }
            if !record.transport_error || record.timed_out {
                return Err("Unexpected flags in record".to_owned());
            }
            Ok(())
        })
    }

    #[test]
    fn read_json_records_parses_payload() -> Result<(), String> {
        run_async_test(async {
            let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
            let path = dir.path().join("metrics.json");
            let payload = r#"{
                "summary": { "duration_ms": 10 },
                "records": [
                    { "elapsed_ms": 5, "latency_ms": 20, "status_code": 200, "timed_out": false, "transport_error": false }
                ]
            }"#;
            tokio::fs::write(&path, payload)
                .await
                .map_err(|err| format!("write failed: {}", err))?;
            let records = read_json_records(&path).await?;
            if records.len() != 1 {
                return Err(format!("Expected 1 record, got {}", records.len()));
            }
            let record = records
                .first()
                .ok_or_else(|| "Missing parsed record".to_owned())?;
            if record.elapsed_ms != 5 || record.latency_ms != 20 {
                return Err("Unexpected record values".to_owned());
            }
            Ok(())
        })
    }

    #[test]
    fn read_jsonl_records_parses_payload() -> Result<(), String> {
        run_async_test(async {
            let dir = tempdir().map_err(|err| format!("tempdir failed: {}", err))?;
            let path = dir.path().join("metrics.jsonl");
            let payload = r#"{"type":"summary","duration_ms":10}
{"type":"record","elapsed_ms":5,"latency_ms":20,"status_code":200,"timed_out":false,"transport_error":false}
"#;
            tokio::fs::write(&path, payload)
                .await
                .map_err(|err| format!("write failed: {}", err))?;
            let records = read_jsonl_records(&path).await?;
            if records.len() != 1 {
                return Err(format!("Expected 1 record, got {}", records.len()));
            }
            let record = records
                .first()
                .ok_or_else(|| "Missing parsed record".to_owned())?;
            if record.elapsed_ms != 5 || record.latency_ms != 20 {
                return Err("Unexpected record values".to_owned());
            }
            Ok(())
        })
    }
}
