use std::time::Duration;

use crate::metrics::MetricRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PlaybackState {
    pub(crate) start_ms: u64,
    pub(crate) cursor_ms: u64,
    pub(crate) end_ms: u64,
    pub(crate) playing: bool,
}

impl PlaybackState {
    #[must_use]
    pub(crate) const fn new(start_ms: u64, end_ms: u64) -> Self {
        Self {
            start_ms,
            cursor_ms: start_ms,
            end_ms,
            playing: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlaybackAction {
    TogglePlayPause,
    SeekBackward,
    SeekForward,
    SeekStart,
    SeekEnd,
    Restart,
}

#[must_use]
pub(crate) fn resolve_step_ms(step: Option<Duration>, default_step: Duration) -> u64 {
    let millis = step.unwrap_or(default_step).as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX).max(1)
}

pub(crate) fn apply_playback_action(
    state: &mut PlaybackState,
    action: PlaybackAction,
    step_ms: u64,
) -> bool {
    let previous = *state;
    let step_ms = step_ms.max(1);
    match action {
        PlaybackAction::TogglePlayPause => {
            state.playing = !state.playing;
        }
        PlaybackAction::SeekBackward => {
            state.playing = false;
            state.cursor_ms = state.cursor_ms.saturating_sub(step_ms).max(state.start_ms);
        }
        PlaybackAction::SeekForward => {
            state.playing = false;
            state.cursor_ms = state.cursor_ms.saturating_add(step_ms).min(state.end_ms);
        }
        PlaybackAction::SeekStart | PlaybackAction::Restart => {
            state.playing = false;
            state.cursor_ms = state.start_ms;
        }
        PlaybackAction::SeekEnd => {
            state.playing = false;
            state.cursor_ms = state.end_ms;
        }
    }
    *state != previous
}

pub(crate) fn advance_playback(state: &mut PlaybackState, elapsed: Duration, tick_ms: u64) -> bool {
    if !state.playing || tick_ms == 0 {
        return false;
    }
    if elapsed < Duration::from_millis(tick_ms) {
        return false;
    }

    let tick_ms = u128::from(tick_ms);
    let steps = elapsed.as_millis().checked_div(tick_ms).unwrap_or(0);
    if steps == 0 {
        return false;
    }
    let advance_ms = u64::try_from(steps.saturating_mul(tick_ms)).unwrap_or(u64::MAX);
    state.cursor_ms = state.cursor_ms.saturating_add(advance_ms).min(state.end_ms);
    if state.cursor_ms >= state.end_ms {
        state.cursor_ms = state.end_ms;
        state.playing = false;
    }
    true
}

#[must_use]
pub(crate) fn records_range(records: &[MetricRecord]) -> Option<(u64, u64)> {
    let min = records.first().map(|record| record.elapsed_ms)?;
    let max = records.last().map(|record| record.elapsed_ms)?;
    Some((min, max))
}

#[must_use]
pub(crate) fn clamp_window_to_records(
    base: &PlaybackState,
    records_min: u64,
    records_max: u64,
) -> PlaybackState {
    let (min_ms, max_ms) = if records_min <= records_max {
        (records_min, records_max)
    } else {
        (records_max, records_min)
    };
    PlaybackState {
        start_ms: base.start_ms.max(min_ms).min(max_ms),
        cursor_ms: base.cursor_ms.clamp(min_ms, max_ms),
        end_ms: max_ms,
        playing: base.playing,
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::metrics::MetricRecord;

    use super::{
        PlaybackAction, PlaybackState, advance_playback, apply_playback_action,
        clamp_window_to_records, records_range, resolve_step_ms,
    };

    fn metric_record(elapsed_ms: u64) -> MetricRecord {
        MetricRecord {
            elapsed_ms,
            latency_ms: 1,
            status_code: 200,
            timed_out: false,
            transport_error: false,
            response_bytes: 0,
            in_flight_ops: 0,
        }
    }

    #[test]
    fn resolve_step_ms_defaults_and_clamps_zero() {
        assert_eq!(resolve_step_ms(None, Duration::from_secs(1)), 1000);
        assert_eq!(
            resolve_step_ms(Some(Duration::from_millis(0)), Duration::from_secs(1)),
            1
        );
    }

    #[test]
    fn apply_playback_action_handles_seek_and_restart() {
        let mut state = PlaybackState {
            start_ms: 1000,
            cursor_ms: 5000,
            end_ms: 9000,
            playing: true,
        };

        let changed_back = apply_playback_action(&mut state, PlaybackAction::SeekBackward, 2500);
        assert!(changed_back);
        assert_eq!(state.cursor_ms, 2500);
        assert!(!state.playing);

        let changed_forward =
            apply_playback_action(&mut state, PlaybackAction::SeekForward, 10_000);
        assert!(changed_forward);
        assert_eq!(state.cursor_ms, 9000);

        let changed_restart = apply_playback_action(&mut state, PlaybackAction::Restart, 2500);
        assert!(changed_restart);
        assert_eq!(state.cursor_ms, 1000);
    }

    #[test]
    fn advance_playback_moves_cursor_and_stops_at_end() {
        let mut state = PlaybackState {
            start_ms: 0,
            cursor_ms: 1000,
            end_ms: 2500,
            playing: true,
        };

        let changed_short_tick = advance_playback(&mut state, Duration::from_millis(500), 1000);
        assert!(!changed_short_tick);
        assert_eq!(state.cursor_ms, 1000);
        assert!(state.playing);

        let changed_full_tick = advance_playback(&mut state, Duration::from_millis(2100), 1000);
        assert!(changed_full_tick);
        assert_eq!(state.cursor_ms, 2500);
        assert!(!state.playing);
    }

    #[test]
    fn records_range_returns_min_and_max_when_present() {
        let records = vec![metric_record(100), metric_record(500)];
        assert_eq!(records_range(&records), Some((100, 500)));
        assert_eq!(records_range(&[]), None);
    }

    #[test]
    fn clamp_window_to_records_limits_state() {
        let base = PlaybackState {
            start_ms: 0,
            cursor_ms: 15_000,
            end_ms: 20_000,
            playing: true,
        };
        let clamped = clamp_window_to_records(&base, 1000, 10_000);
        assert_eq!(clamped.start_ms, 1000);
        assert_eq!(clamped.cursor_ms, 10_000);
        assert_eq!(clamped.end_ms, 10_000);
        assert!(clamped.playing);
    }
}
