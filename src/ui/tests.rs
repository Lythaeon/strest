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
        latencies: vec![(0, 10), (500, 20), (900, 15)],
        p50: 15,
        p90: 20,
        p99: 20,
        rps: 2,
        rpm: 120,
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
        latencies: vec![(0, 5), (100, 7)],
        p50: 6,
        p90: 7,
        p99: 7,
        rps: 4,
        rpm: 240,
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

    Ok(())
}
