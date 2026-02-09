use super::model::{UiData, UiRenderData};
use super::render::{Ui, UiActions};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::time::Duration;

#[test]
fn ui_render_does_not_panic() -> Result<(), String> {
    let backend = TestBackend::new(80, 24);
    let mut terminal = match Terminal::new(backend) {
        Ok(term) => term,
        Err(err) => {
            return Err(format!("Failed to create TestBackend terminal: {}", err));
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
fn ui_render_data_from_ui_data() -> Result<(), String> {
    let ui_data = UiData {
        elapsed_time: Duration::from_secs(3),
        target_duration: Duration::from_secs(9),
        current_requests: 12,
        successful_requests: 10,
        timeout_requests: 1,
        transport_errors: 0,
        non_expected_status: 1,
        ui_window_ms: 10_000,
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
        return Err("elapsed_time mismatch".to_owned());
    }
    if render_data.target_duration != ui_data.target_duration {
        return Err("target_duration mismatch".to_owned());
    }
    if render_data.current_request != ui_data.current_requests {
        return Err("current_request mismatch".to_owned());
    }
    if render_data.successful_requests != ui_data.successful_requests {
        return Err("successful_requests mismatch".to_owned());
    }
    if render_data.latencies != ui_data.latencies {
        return Err("latencies mismatch".to_owned());
    }
    if render_data.p50 != ui_data.p50 {
        return Err("p50 mismatch".to_owned());
    }
    if render_data.p90 != ui_data.p90 {
        return Err("p90 mismatch".to_owned());
    }
    if render_data.p99 != ui_data.p99 {
        return Err("p99 mismatch".to_owned());
    }
    if render_data.rps != ui_data.rps {
        return Err("rps mismatch".to_owned());
    }
    if render_data.rpm != ui_data.rpm {
        return Err("rpm mismatch".to_owned());
    }
    if render_data.timeout_requests != ui_data.timeout_requests {
        return Err("timeout_requests mismatch".to_owned());
    }
    if render_data.transport_errors != ui_data.transport_errors {
        return Err("transport_errors mismatch".to_owned());
    }
    if render_data.non_expected_status != ui_data.non_expected_status {
        return Err("non_expected_status mismatch".to_owned());
    }
    if render_data.ui_window_ms != ui_data.ui_window_ms {
        return Err("ui_window_ms mismatch".to_owned());
    }
    if render_data.p50_ok != ui_data.p50_ok {
        return Err("p50_ok mismatch".to_owned());
    }
    if render_data.p90_ok != ui_data.p90_ok {
        return Err("p90_ok mismatch".to_owned());
    }
    if render_data.p99_ok != ui_data.p99_ok {
        return Err("p99_ok mismatch".to_owned());
    }
    if render_data.replay.is_some() {
        return Err("replay mismatch".to_owned());
    }

    Ok(())
}
