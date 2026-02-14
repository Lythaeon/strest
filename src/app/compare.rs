mod compare_output;

use std::io::{self, IsTerminal};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use tokio::sync::watch;

use crate::args::CompareArgs;
use crate::error::{AppError, AppResult, MetricsError};
use crate::system::replay_compare::{
    PlaybackAction, PlaybackState, advance_playback, apply_playback_action,
    clamp_window_to_records, records_range, resolve_step_ms,
};
use crate::ui::model::{CompareOverlay, UiData};
use crate::ui::render::setup_render_ui;

use super::replay::{SnapshotMarkers, build_ui_data_with_config, read_records_from_path};
use compare_output::print_compare_summary;

/// Playback tick used when compare is in "playing" mode.
const COMPARE_TICK_MS: u64 = 1000;
/// Default step for manual seek when no flag is provided.
const DEFAULT_COMPARE_STEP: Duration = Duration::from_secs(1);
/// UI refresh poll cadence for compare mode.
const UI_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Non-blocking poll interval for keyboard events.
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(0);

pub(crate) async fn run_compare(args: &CompareArgs) -> AppResult<()> {
    let left_path = Path::new(&args.left);
    let right_path = Path::new(&args.right);
    let mut left_records = read_records_from_path(left_path).await?;
    let mut right_records = read_records_from_path(right_path).await?;
    if left_records.is_empty() || right_records.is_empty() {
        return Err(AppError::metrics(MetricsError::ReplayRecordsEmpty));
    }

    left_records.sort_by_key(|record| record.elapsed_ms);
    right_records.sort_by_key(|record| record.elapsed_ms);

    let (left_min, left_max) = records_range(&left_records)
        .ok_or_else(|| AppError::metrics(MetricsError::ReplayRecordsEmpty))?;
    let (right_min, right_max) = records_range(&right_records)
        .ok_or_else(|| AppError::metrics(MetricsError::ReplayRecordsEmpty))?;
    let start_ms = left_min.min(right_min);
    let end_ms = left_max.max(right_max);

    if !io::stdout().is_terminal() || args.no_ui {
        print_compare_summary(
            "left",
            &left_records,
            args.expected_status_code,
            left_min,
            left_max,
            args,
        )?;
        print_compare_summary(
            "right",
            &right_records,
            args.expected_status_code,
            right_min,
            right_max,
            args,
        )?;
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
    let step_ms = resolve_step_ms(args.replay_step, DEFAULT_COMPARE_STEP);
    let mut last_tick = tokio::time::Instant::now();
    let poll_interval = UI_POLL_INTERVAL;
    let mut dirty = true;
    let markers = SnapshotMarkers::default();
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
                if let Some(action) = resolve_playback_action(key.code)
                    && apply_playback_action(&mut state, action, step_ms)
                {
                    dirty = true;
                }
            }

            if state.playing {
                if advance_playback(&mut state, last_tick.elapsed(), COMPARE_TICK_MS) {
                    last_tick = tokio::time::Instant::now();
                    dirty = true;
                }
            } else {
                last_tick = tokio::time::Instant::now();
            }

            if dirty {
                let left_state = clamp_window_to_records(&state, left_min, left_max);
                let right_state = clamp_window_to_records(&state, right_min, right_max);
                let mut primary = build_ui_data_with_config(
                    &left_records,
                    args.expected_status_code,
                    args.ui_window_ms.get(),
                    args.no_color,
                    &left_state,
                    &markers,
                    None,
                )?;
                let overlay = build_ui_data_with_config(
                    &right_records,
                    args.expected_status_code,
                    args.ui_window_ms.get(),
                    args.no_color,
                    &right_state,
                    &markers,
                    None,
                )?;
                primary.elapsed_time =
                    Duration::from_millis(state.cursor_ms.saturating_sub(state.start_ms));
                primary.target_duration =
                    Duration::from_millis(state.end_ms.saturating_sub(state.start_ms));
                primary.replay = None;
                let right_label = resolve_label(&args.right, args.right_label.as_deref());
                primary.compare = Some(CompareOverlay::from_ui(right_label, &overlay));
                drop(ui_tx.send(primary));
                dirty = false;
            }

            tokio::time::sleep(poll_interval).await;
        }

        Ok::<(), AppError>(())
    }
    .await;

    drop(shutdown_tx.send(()));
    if let Err(err) = render_ui_handle.await {
        eprintln!("Compare UI task failed: {}", err);
    }
    result
}

fn resolve_label(path: &str, override_label: Option<&str>) -> String {
    if let Some(label) = override_label
        && !label.trim().is_empty()
    {
        return label.trim().to_owned();
    }
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|value| value.to_owned())
        .unwrap_or_else(|| "compare".to_owned())
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
