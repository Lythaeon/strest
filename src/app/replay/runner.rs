use std::io::{self, IsTerminal};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use tokio::sync::watch;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, MetricsError, ValidationError};
use crate::metrics::MetricRecord;
use crate::system::replay_compare::{
    PlaybackAction, PlaybackState, advance_playback, apply_playback_action, resolve_step_ms,
};
use crate::ui::model::UiData;
use crate::ui::render::setup_render_ui;

use super::bounds::{BoundDefault, resolve_bound};
use super::records::load_replay_records;
use super::snapshots::{
    SnapshotIntervalState, parse_snapshot_format, resolve_snapshot_range, resolve_snapshot_window,
};
use super::state::SnapshotMarkers;
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

    let step_ms = resolve_step_ms(args.replay_step, DEFAULT_REPLAY_STEP);

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

    let (shutdown_tx, _) = crate::system::shutdown_handlers::shutdown_channel();
    let initial_ui = UiData {
        target_duration: Duration::from_millis(end_ms.saturating_sub(start_ms)),
        ui_window_ms: args.ui_window_ms.get(),
        no_color: args.no_color,
        ..UiData::default()
    };
    let (ui_tx, _) = watch::channel(initial_ui);
    let render_ui_handle = setup_render_ui(&shutdown_tx, &ui_tx);

    let mut state = PlaybackState::new(start_ms, end_ms);
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
                // Handle Ctrl+C and q/Esc for quitting
                if matches!(key.code, KeyCode::Char('q') | KeyCode::Esc)
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(event::KeyModifiers::CONTROL))
                {
                    break;
                }
                if let Some(action) = resolve_playback_action(key.code) {
                    if apply_playback_action(&mut state, action, step_ms) {
                        dirty = true;
                    }
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
                if advance_playback(&mut state, last_tick.elapsed(), REPLAY_TICK_MS) {
                    last_tick = tokio::time::Instant::now();
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

const fn resolve_playback_action(key_code: KeyCode) -> Option<PlaybackAction> {
    if matches!(key_code, KeyCode::Char(' ')) {
        return Some(PlaybackAction::TogglePlayPause);
    }
    if matches!(key_code, KeyCode::Left | KeyCode::Char('h')) {
        return Some(PlaybackAction::SeekBackward);
    }
    if matches!(key_code, KeyCode::Right | KeyCode::Char('l')) {
        return Some(PlaybackAction::SeekForward);
    }
    if matches!(key_code, KeyCode::Home) {
        return Some(PlaybackAction::SeekStart);
    }
    if matches!(key_code, KeyCode::End) {
        return Some(PlaybackAction::SeekEnd);
    }
    if matches!(key_code, KeyCode::Char('r')) {
        return Some(PlaybackAction::Restart);
    }
    None
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
