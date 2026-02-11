use std::io::{self, IsTerminal};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use tokio::sync::watch;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, MetricsError, ValidationError};
use crate::metrics::MetricRecord;
use crate::ui::model::UiData;
use crate::ui::render::setup_render_ui;

use super::bounds::{BoundDefault, resolve_bound};
use super::records::load_replay_records;
use super::snapshots::{
    SnapshotIntervalState, parse_snapshot_format, resolve_snapshot_range, resolve_snapshot_window,
};
use super::state::{ReplayWindow, SnapshotMarkers};
use super::ui::render_once;
use super::{snapshots, ui};

/// Playback tick used when replay is in "playing" mode.
const REPLAY_TICK_MS: u64 = 1000;
/// Default step for manual seek when no flag is provided.
const DEFAULT_REPLAY_STEP: Duration = Duration::from_secs(1);
/// UI refresh poll cadence for replay mode.
const UI_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Non-blocking poll interval for keyboard events.
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(0);

pub(crate) async fn run_replay(args: &TesterArgs) -> AppResult<()> {
    let records = load_replay_records(args).await?;
    if records.is_empty() {
        return Err(AppError::metrics(MetricsError::ReplayRecordsEmpty));
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
        return Err(AppError::validation(ValidationError::ReplayStartAfterEnd));
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
        .unwrap_or(DEFAULT_REPLAY_STEP)
        .as_millis()
        .try_into()
        .unwrap_or(1);

    if !io::stdout().is_terminal() || args.no_ui {
        if snapshot_requested {
            if let Some(interval) = args.replay_snapshot_interval {
                let mut interval_state =
                    SnapshotIntervalState::new(interval, snapshot_window.0, snapshot_window.1)?;
                snapshots::emit_interval_snapshots(
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
                snapshots::write_snapshot(
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

    let (shutdown_tx, _) = crate::shutdown_handlers::shutdown_channel();
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
    let poll_interval = UI_POLL_INTERVAL;
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

            if event::poll(EVENT_POLL_INTERVAL)?
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
                    snapshots::write_snapshot(
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
                snapshots::emit_interval_snapshots(
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
                let ui_data = ui::build_ui_data(
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
            snapshots::emit_interval_snapshots(
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

    drop(shutdown_tx.send(()));
    if let Err(err) = render_ui_handle.await {
        eprintln!("Replay UI task failed: {}", err);
    }
    result
}

pub(super) fn window_slice(
    records: &[MetricRecord],
    start_ms: u64,
    end_ms: u64,
) -> &[MetricRecord] {
    if records.is_empty() {
        return records;
    }
    let start_idx = records.partition_point(|record| record.elapsed_ms < start_ms);
    let end_idx = records.partition_point(|record| record.elapsed_ms <= end_ms);
    records.get(start_idx..end_idx).unwrap_or(&[])
}
