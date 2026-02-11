use super::model::{DataUsage, StatusCounts, UiData, UiRenderData};
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
        in_flight_ops: 2,
        ui_window_ms: 10_000,
        no_color: false,
        latencies: vec![(0, 10), (500, 20), (900, 15)],
        rps_series: vec![(0, 1), (500, 2), (900, 3)],
        status_counts: Some(StatusCounts {
            status_2xx: 3,
            status_3xx: 0,
            status_4xx: 1,
            status_5xx: 0,
            status_other: 0,
        }),
        data_usage: Some(DataUsage {
            total_bytes: 2048,
            bytes_per_sec: 1024,
            series: vec![(0, 512), (500, 1024), (900, 512)],
        }),
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
        in_flight_ops: 3,
        ui_window_ms: 10_000,
        no_color: false,
        latencies: vec![(0, 5), (100, 7)],
        rps_series: vec![(0, 1), (100, 2)],
        status_counts: Some(StatusCounts {
            status_2xx: 2,
            status_3xx: 0,
            status_4xx: 0,
            status_5xx: 0,
            status_other: 0,
        }),
        data_usage: Some(DataUsage {
            total_bytes: 1024,
            bytes_per_sec: 512,
            series: vec![(0, 256), (100, 256)],
        }),
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
    if render_data.rps_series != ui_data.rps_series {
        return Err(AppError::validation("rps_series mismatch"));
    }
    if render_data.status_counts.is_none() != ui_data.status_counts.is_none() {
        return Err(AppError::validation("status_counts mismatch"));
    }
    if render_data.data_usage.is_none() != ui_data.data_usage.is_none() {
        return Err(AppError::validation("data_usage mismatch"));
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
    if render_data.in_flight_ops != ui_data.in_flight_ops {
        return Err(AppError::validation("in_flight_ops mismatch"));
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
