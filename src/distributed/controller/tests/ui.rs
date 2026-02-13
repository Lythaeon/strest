use std::collections::{HashMap, VecDeque};

use tokio::sync::watch;

use crate::error::{AppError, AppResult};
use crate::ui::model::UiData;

use super::{AgentSnapshot, WireSummary, base_args, build_hist, update_ui};

#[test]
fn update_ui_emits_aggregated_stats() -> AppResult<()> {
    let args = base_args()?;
    let (ui_tx, ui_rx) = watch::channel(UiData::default());

    let summary = WireSummary {
        duration_ms: 1000,
        total_requests: 10,
        successful_requests: 9,
        error_requests: 1,
        timeout_requests: 0,
        transport_errors: 0,
        non_expected_status: 1,
        success_min_latency_ms: 10,
        success_max_latency_ms: 50,
        success_latency_sum_ms: 900,
        min_latency_ms: 10,
        max_latency_ms: 50,
        latency_sum_ms: 1000,
    };
    let hist = build_hist(&[10, 20, 30])?;
    let success_hist = build_hist(&[10, 20, 30])?;

    let mut agent_states = HashMap::new();
    agent_states.insert(
        "a".to_owned(),
        AgentSnapshot {
            summary,
            histogram: hist,
            success_histogram: success_hist,
        },
    );

    let mut latency_window = VecDeque::new();
    let mut rps_window = VecDeque::new();
    update_ui(
        &ui_tx,
        &args,
        &agent_states,
        &mut latency_window,
        &mut rps_window,
    );

    let snapshot = ui_rx.borrow().clone();
    if snapshot.current_requests != 10 {
        return Err(AppError::distributed(format!(
            "Unexpected current_requests: {}",
            snapshot.current_requests
        )));
    }
    if snapshot.successful_requests != 9 {
        return Err(AppError::distributed(format!(
            "Unexpected successful_requests: {}",
            snapshot.successful_requests
        )));
    }
    if snapshot.p50 == 0 {
        return Err(AppError::distributed("Expected non-zero p50 latency"));
    }
    if snapshot.rps == 0 {
        return Err(AppError::distributed("Expected non-zero rps"));
    }
    Ok(())
}
