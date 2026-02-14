use std::collections::{HashMap, HashSet};

use crate::error::{AppError, AppResult};
use crate::metrics::LatencyHistogram;

use super::super::shared::AgentEvent;
use super::handle_agent_event;
use crate::distributed::protocol::{ReportMessage, StreamMessage, WireSummary};

fn summary_fixture() -> WireSummary {
    WireSummary {
        duration_ms: 1000,
        total_requests: 10,
        successful_requests: 9,
        error_requests: 1,
        timeout_requests: 0,
        transport_errors: 0,
        non_expected_status: 0,
        success_min_latency_ms: 10,
        success_max_latency_ms: 30,
        success_latency_sum_ms: 160,
        min_latency_ms: 10,
        max_latency_ms: 30,
        latency_sum_ms: 200,
    }
}

fn histogram_b64_fixture() -> AppResult<String> {
    let mut histogram = LatencyHistogram::new()?;
    histogram.record(10)?;
    histogram.record(30)?;
    histogram.encode_base64()
}

#[test]
fn disconnected_event_marks_agent_as_failed() -> AppResult<()> {
    let mut pending_agents = HashSet::from(["agent-1".to_owned()]);
    let mut agent_states = HashMap::new();
    let mut runtime_errors = Vec::new();

    handle_agent_event(
        AgentEvent::Disconnected {
            agent_id: "agent-1".to_owned(),
            message: "socket closed".to_owned(),
        },
        "run-1",
        &mut pending_agents,
        &mut agent_states,
        &mut runtime_errors,
    );

    if pending_agents.contains("agent-1") {
        return Err(AppError::distributed(
            "Expected disconnected agent to be removed from pending set",
        ));
    }
    if !runtime_errors
        .iter()
        .any(|error| error.contains("disconnected"))
    {
        return Err(AppError::distributed(
            "Expected disconnected event to be reported as runtime error",
        ));
    }
    Ok(())
}

#[test]
fn report_with_mismatched_run_id_is_rejected() -> AppResult<()> {
    let mut pending_agents = HashSet::from(["agent-1".to_owned()]);
    let mut agent_states = HashMap::new();
    let mut runtime_errors = Vec::new();
    let report = ReportMessage {
        run_id: "wrong-run".to_owned(),
        agent_id: "agent-1".to_owned(),
        summary: summary_fixture(),
        histogram_b64: histogram_b64_fixture()?,
        success_histogram_b64: None,
        runtime_errors: vec![],
    };

    handle_agent_event(
        AgentEvent::Report {
            agent_id: "agent-1".to_owned(),
            message: report,
        },
        "run-1",
        &mut pending_agents,
        &mut agent_states,
        &mut runtime_errors,
    );

    if pending_agents.contains("agent-1") {
        return Err(AppError::distributed(
            "Expected mismatched run id report to clear pending agent",
        ));
    }
    if !runtime_errors
        .iter()
        .any(|error| error.contains("mismatched run id"))
    {
        return Err(AppError::distributed(
            "Expected mismatched run id to be reported as runtime error",
        ));
    }
    if !agent_states.is_empty() {
        return Err(AppError::distributed(
            "Expected mismatched run id report not to update agent state",
        ));
    }
    Ok(())
}

#[test]
fn report_with_mismatched_agent_id_is_rejected() -> AppResult<()> {
    let mut pending_agents = HashSet::from(["agent-1".to_owned()]);
    let mut agent_states = HashMap::new();
    let mut runtime_errors = Vec::new();
    let report = ReportMessage {
        run_id: "run-1".to_owned(),
        agent_id: "agent-2".to_owned(),
        summary: summary_fixture(),
        histogram_b64: histogram_b64_fixture()?,
        success_histogram_b64: None,
        runtime_errors: vec![],
    };

    handle_agent_event(
        AgentEvent::Report {
            agent_id: "agent-1".to_owned(),
            message: report,
        },
        "run-1",
        &mut pending_agents,
        &mut agent_states,
        &mut runtime_errors,
    );

    if pending_agents.contains("agent-1") {
        return Err(AppError::distributed(
            "Expected mismatched agent id report to clear pending agent",
        ));
    }
    if !runtime_errors
        .iter()
        .any(|error| error.contains("unexpected id"))
    {
        return Err(AppError::distributed(
            "Expected mismatched agent id to be reported as runtime error",
        ));
    }
    if !agent_states.is_empty() {
        return Err(AppError::distributed(
            "Expected mismatched agent id report not to update agent state",
        ));
    }
    Ok(())
}

#[test]
fn stream_with_invalid_histogram_reports_decode_error() -> AppResult<()> {
    let mut pending_agents = HashSet::from(["agent-1".to_owned()]);
    let mut agent_states = HashMap::new();
    let mut runtime_errors = Vec::new();
    let stream = StreamMessage {
        run_id: "run-1".to_owned(),
        agent_id: "agent-1".to_owned(),
        summary: summary_fixture(),
        histogram_b64: "not-a-valid-histogram".to_owned(),
        success_histogram_b64: None,
    };

    handle_agent_event(
        AgentEvent::Stream {
            agent_id: "agent-1".to_owned(),
            message: stream,
        },
        "run-1",
        &mut pending_agents,
        &mut agent_states,
        &mut runtime_errors,
    );

    if !pending_agents.contains("agent-1") {
        return Err(AppError::distributed(
            "Expected stream decode failure not to remove pending agent",
        ));
    }
    if !runtime_errors
        .iter()
        .any(|error| error.contains("histogram decode failed"))
    {
        return Err(AppError::distributed(
            "Expected stream decode failure to be reported as runtime error",
        ));
    }
    if !agent_states.is_empty() {
        return Err(AppError::distributed(
            "Expected stream decode failure not to update agent state",
        ));
    }
    Ok(())
}
