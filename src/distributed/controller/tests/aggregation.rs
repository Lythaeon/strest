use std::collections::HashMap;

use crate::error::{AppError, AppResult};

use super::{AgentSnapshot, WireSummary, aggregate_snapshots, build_hist};

#[test]
fn aggregate_snapshots_merges_summary() -> AppResult<()> {
    let summary_a = WireSummary {
        duration_ms: 1000,
        total_requests: 10,
        successful_requests: 9,
        error_requests: 1,
        timeout_requests: 1,
        transport_errors: 0,
        non_expected_status: 0,
        success_min_latency_ms: 10,
        success_max_latency_ms: 50,
        success_latency_sum_ms: 900,
        min_latency_ms: 10,
        max_latency_ms: 50,
        latency_sum_ms: 1000,
    };
    let summary_b = WireSummary {
        duration_ms: 1500,
        total_requests: 20,
        successful_requests: 19,
        error_requests: 1,
        timeout_requests: 2,
        transport_errors: 1,
        non_expected_status: 0,
        success_min_latency_ms: 5,
        success_max_latency_ms: 40,
        success_latency_sum_ms: 1900,
        min_latency_ms: 5,
        max_latency_ms: 40,
        latency_sum_ms: 600,
    };

    let hist_a = build_hist(&[10, 20])?;
    let hist_b = build_hist(&[30, 40])?;
    let success_hist_a = build_hist(&[10, 20])?;
    let success_hist_b = build_hist(&[30, 40])?;

    let mut agent_states = HashMap::new();
    agent_states.insert(
        "a".to_owned(),
        AgentSnapshot {
            summary: summary_a,
            histogram: hist_a,
            success_histogram: success_hist_a,
        },
    );
    agent_states.insert(
        "b".to_owned(),
        AgentSnapshot {
            summary: summary_b,
            histogram: hist_b,
            success_histogram: success_hist_b,
        },
    );

    let (summary, merged_hist, _success_hist) = aggregate_snapshots(&agent_states)?;
    if summary.total_requests != 30 {
        return Err(AppError::distributed(format!(
            "Unexpected total_requests: {}",
            summary.total_requests
        )));
    }
    if summary.successful_requests != 28 {
        return Err(AppError::distributed(format!(
            "Unexpected successful_requests: {}",
            summary.successful_requests
        )));
    }
    if summary.error_requests != 2 {
        return Err(AppError::distributed(format!(
            "Unexpected error_requests: {}",
            summary.error_requests
        )));
    }
    if summary.timeout_requests != 3 {
        return Err(AppError::distributed(format!(
            "Unexpected timeout_requests: {}",
            summary.timeout_requests
        )));
    }
    if summary.success_avg_latency_ms != 100 {
        return Err(AppError::distributed(format!(
            "Unexpected success_avg_latency_ms: {}",
            summary.success_avg_latency_ms
        )));
    }
    if summary.min_latency_ms != 5 {
        return Err(AppError::distributed(format!(
            "Unexpected min_latency_ms: {}",
            summary.min_latency_ms
        )));
    }
    if summary.max_latency_ms != 50 {
        return Err(AppError::distributed(format!(
            "Unexpected max_latency_ms: {}",
            summary.max_latency_ms
        )));
    }
    if summary.avg_latency_ms != 53 {
        return Err(AppError::distributed(format!(
            "Unexpected avg_latency_ms: {}",
            summary.avg_latency_ms
        )));
    }
    if merged_hist.count() != 4 {
        return Err(AppError::distributed(format!(
            "Unexpected merged histogram count: {}",
            merged_hist.count()
        )));
    }
    Ok(())
}
