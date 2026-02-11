use super::model::{UiData, UiRenderData};
use super::render::{Ui, UiActions};
use crate::error::{AppError, AppResult};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::time::Duration;

#[test]
fn ui_render_does_not_panic() -> AppResult<()> {
    let backend = TestBackend::new(80, 24);
    let mut terminal = match Terminal::new(backend) {
        Ok(term) => term,
        Err(err) => {
            return Err(AppError::validation(format!(
                "Failed to create TestBackend terminal: {}",
                err
            )));
        }
    };

    let data = UiRenderData {
        elapsed_time: Duration::from_secs(1),
        target_duration: Duration::from_secs(10),
        current_request: 5,
        successful_requests: 4,
        timeout_requests: 1,
        transport_errors: 0,
        non_expected_status: 0,
        ui_window_ms: 10_000,
        no_color: false,
        latencies: vec![(0, 10), (500, 20), (900, 15)],
        p50: 15,
        p90: 20,
        p99: 20,
        p50_ok: 15,
        p90_ok: 20,
        p99_ok: 20,
        rps: 2,
        rpm: 120,
        replay: None,
    };

    Ui::render(&mut terminal, &data);
    Ok(())
}

#[test]
fn ui_render_data_from_ui_data() -> AppResult<()> {
    let ui_data = UiData {
        elapsed_time: Duration::from_secs(3),
        target_duration: Duration::from_secs(9),
        current_requests: 12,
        successful_requests: 10,
        timeout_requests: 1,
        transport_errors: 0,
        non_expected_status: 1,
        ui_window_ms: 10_000,
        no_color: false,
        latencies: vec![(0, 5), (100, 7)],
        p50: 6,
        p90: 7,
        p99: 7,
        p50_ok: 6,
        p90_ok: 7,
        p99_ok: 7,
        rps: 4,
        rpm: 240,
        replay: None,
    };

    let render_data = UiRenderData::from(&ui_data);
    if render_data.elapsed_time != ui_data.elapsed_time {
        return Err(AppError::validation("elapsed_time mismatch"));
    }
    if render_data.target_duration != ui_data.target_duration {
        return Err(AppError::validation("target_duration mismatch"));
    }
    if render_data.current_request != ui_data.current_requests {
        return Err(AppError::validation("current_request mismatch"));
    }
    if render_data.successful_requests != ui_data.successful_requests {
        return Err(AppError::validation("successful_requests mismatch"));
    }
    if render_data.latencies != ui_data.latencies {
        return Err(AppError::validation("latencies mismatch"));
    }
    if render_data.p50 != ui_data.p50 {
        return Err(AppError::validation("p50 mismatch"));
    }
    if render_data.p90 != ui_data.p90 {
        return Err(AppError::validation("p90 mismatch"));
    }
    if render_data.p99 != ui_data.p99 {
        return Err(AppError::validation("p99 mismatch"));
    }
    if render_data.rps != ui_data.rps {
        return Err(AppError::validation("rps mismatch"));
    }
    if render_data.rpm != ui_data.rpm {
        return Err(AppError::validation("rpm mismatch"));
    }
    if render_data.timeout_requests != ui_data.timeout_requests {
        return Err(AppError::validation("timeout_requests mismatch"));
    }
    if render_data.transport_errors != ui_data.transport_errors {
        return Err(AppError::validation("transport_errors mismatch"));
    }
    if render_data.non_expected_status != ui_data.non_expected_status {
        return Err(AppError::validation("non_expected_status mismatch"));
    }
    if render_data.ui_window_ms != ui_data.ui_window_ms {
        return Err(AppError::validation("ui_window_ms mismatch"));
    }
    if render_data.p50_ok != ui_data.p50_ok {
        return Err(AppError::validation("p50_ok mismatch"));
    }
    if render_data.p90_ok != ui_data.p90_ok {
        return Err(AppError::validation("p90_ok mismatch"));
    }
    if render_data.p99_ok != ui_data.p99_ok {
        return Err(AppError::validation("p99_ok mismatch"));
    }
    if render_data.replay.is_some() {
        return Err(AppError::validation("replay mismatch"));
    }

    Ok(())
}
