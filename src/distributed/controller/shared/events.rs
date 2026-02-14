use std::collections::{HashMap, HashSet};

use tracing::{debug, info, warn};

use crate::metrics::LatencyHistogram;

use super::super::super::protocol::{ReportMessage, StreamMessage, WireSummary};

pub(in crate::distributed::controller) struct AgentSnapshot {
    pub(in crate::distributed::controller) summary: WireSummary,
    pub(in crate::distributed::controller) histogram: LatencyHistogram,
    pub(in crate::distributed::controller) success_histogram: LatencyHistogram,
}

pub(in crate::distributed::controller) enum AgentEvent {
    Heartbeat {
        agent_id: String,
    },
    Stream {
        agent_id: String,
        message: StreamMessage,
    },
    Report {
        agent_id: String,
        message: ReportMessage,
    },
    Error {
        agent_id: String,
        message: String,
    },
    Disconnected {
        agent_id: String,
        message: String,
    },
}

pub(in crate::distributed::controller) fn handle_agent_event(
    event: AgentEvent,
    expected_run_id: &str,
    pending_agents: &mut HashSet<String>,
    agent_states: &mut HashMap<String, AgentSnapshot>,
    runtime_errors: &mut Vec<String>,
) {
    match event {
        AgentEvent::Stream { agent_id, message } => {
            debug!(
                "Stream snapshot from agent {} for run {}",
                agent_id, message.run_id
            );
            if message.run_id != expected_run_id {
                runtime_errors.push(format!("Agent {} returned mismatched run id.", agent_id));
                return;
            }
            if message.agent_id != agent_id {
                runtime_errors.push(format!(
                    "Agent {} reported unexpected id {}.",
                    agent_id, message.agent_id
                ));
                return;
            }
            match LatencyHistogram::decode_base64(&message.histogram_b64) {
                Ok(histogram) => {
                    let success_histogram = match message.success_histogram_b64.as_deref() {
                        Some(encoded) => match LatencyHistogram::decode_base64(encoded) {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram decode failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                        None => match LatencyHistogram::new() {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram init failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                    };
                    agent_states.insert(
                        agent_id,
                        AgentSnapshot {
                            summary: message.summary,
                            histogram,
                            success_histogram,
                        },
                    );
                }
                Err(err) => runtime_errors.push(format!(
                    "Agent {} histogram decode failed: {}",
                    agent_id, err
                )),
            }
        }
        AgentEvent::Report { agent_id, message } => {
            info!(
                "Report received from agent {} for run {}",
                agent_id, message.run_id
            );
            if message.run_id != expected_run_id {
                runtime_errors.push(format!("Agent {} returned mismatched run id.", agent_id));
                pending_agents.remove(&agent_id);
                return;
            }
            if message.agent_id != agent_id {
                runtime_errors.push(format!(
                    "Agent {} reported unexpected id {}.",
                    agent_id, message.agent_id
                ));
                pending_agents.remove(&agent_id);
                return;
            }
            if !message.runtime_errors.is_empty() {
                for err in message.runtime_errors {
                    runtime_errors.push(format!("Agent {}: {}", agent_id, err));
                }
            }
            match LatencyHistogram::decode_base64(&message.histogram_b64) {
                Ok(histogram) => {
                    let success_histogram = match message.success_histogram_b64.as_deref() {
                        Some(encoded) => match LatencyHistogram::decode_base64(encoded) {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram decode failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                        None => match LatencyHistogram::new() {
                            Ok(success_histogram) => success_histogram,
                            Err(err) => {
                                runtime_errors.push(format!(
                                    "Agent {} success histogram init failed: {}",
                                    agent_id, err
                                ));
                                return;
                            }
                        },
                    };
                    agent_states.insert(
                        agent_id.clone(),
                        AgentSnapshot {
                            summary: message.summary,
                            histogram,
                            success_histogram,
                        },
                    );
                }
                Err(err) => runtime_errors.push(format!(
                    "Agent {} histogram decode failed: {}",
                    agent_id, err
                )),
            }
            pending_agents.remove(&agent_id);
        }
        AgentEvent::Error { agent_id, message } => {
            warn!("Agent {} error: {}", agent_id, message);
            runtime_errors.push(format!("Agent {}: {}", agent_id, message));
            pending_agents.remove(&agent_id);
        }
        AgentEvent::Disconnected { agent_id, message } => {
            warn!("Agent {} disconnected: {}", agent_id, message);
            runtime_errors.push(format!("Agent {} disconnected: {}", agent_id, message));
            pending_agents.remove(&agent_id);
        }
        AgentEvent::Heartbeat { .. } => {}
    }
}

pub(in crate::distributed::controller) const fn event_agent_id(event: &AgentEvent) -> &str {
    match event {
        AgentEvent::Heartbeat { agent_id }
        | AgentEvent::Stream { agent_id, .. }
        | AgentEvent::Report { agent_id, .. }
        | AgentEvent::Error { agent_id, .. }
        | AgentEvent::Disconnected { agent_id, .. } => agent_id.as_str(),
    }
}
