use std::collections::{HashMap, HashSet};
use std::time::Duration;

use arcshift::ArcShift;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};

use super::super::control::{ControlCommand, ControlError, ControlResponse};
use super::super::shared::{
    AgentEvent, event_agent_id, handle_agent_event, record_aggregated_sample, update_ui,
    write_streaming_sinks,
};
use super::run_finalize::finalize_manual_run;
use super::run_lifecycle::request_stop;
use super::state::{ManualAgent, ManualRunState};

#[expect(
    clippy::too_many_arguments,
    reason = "Controller orchestration state bundle"
)]
pub(super) async fn handle_active_run(
    args: &TesterArgs,
    state: &mut ManualRunState,
    control_rx: &mut mpsc::UnboundedReceiver<ControlCommand>,
    event_rx: &mut mpsc::UnboundedReceiver<AgentEvent>,
    heartbeat_interval: &mut tokio::time::Interval,
    heartbeat_timeout: Duration,
    last_seen: &mut ArcShift<HashMap<String, Instant>>,
    disconnected_agents: &mut HashSet<String>,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
) -> AppResult<bool> {
    let mut finish_run = false;
    let mut finish_error: Option<AppError> = None;
    let deadline = state.deadline;

    tokio::select! {
        command = control_rx.recv() => {
            let Some(command) = command else {
                return Err(AppError::distributed(DistributedError::ControlChannelClosed));
            };
            match command {
                ControlCommand::Start { respond_to, .. } => {
                    if respond_to
                        .send(Err(ControlError::new(409, "Run already in progress.")))
                        .is_err()
                    {
                        // Requester dropped the response channel.
                    }
                }
                ControlCommand::Stop { respond_to } => {
                    request_stop(state, agent_pool).await;
                    let response = ControlResponse { status: "stopping".to_owned(), run_id: Some(state.run_id.clone()) };
                    if respond_to.send(Ok(response)).is_err() {
                        // Requester dropped the response channel.
                    }
                }
            }
        }
        event = event_rx.recv() => {
            let Some(event) = event else {
                return Err(AppError::distributed(DistributedError::AgentEventChannelClosed));
            };
            on_event(args, state, event, disconnected_agents, last_seen, agent_pool);
            if state.pending_agents.is_empty() {
                finish_run = true;
                finish_error = finalize_manual_run(args, state).await.err();
            }
        }
        _ = state.sink_interval.tick() => {
            if state.sink_updates_enabled && state.sink_dirty {
                if let Err(err) = write_streaming_sinks(args, &state.agent_states).await {
                    state.runtime_errors.push(err.to_string());
                } else {
                    state.sink_dirty = false;
                }
            }
        }
        _ = heartbeat_interval.tick() => {
            process_heartbeat_timeouts(state, heartbeat_timeout, last_seen, disconnected_agents, agent_pool);
            if state.pending_agents.is_empty() {
                finish_run = true;
                finish_error = finalize_manual_run(args, state).await.err();
            }
        }
        () = tokio::time::sleep_until(deadline) => {
            if !state.pending_agents.is_empty() {
                for agent_id in &state.pending_agents {
                    state.runtime_errors.push(format!("Timed out waiting for report from agent {}.", agent_id));
                }
            }
            finish_run = true;
            finish_error = finalize_manual_run(args, state).await.err();
        }
    }

    if finish_run && let Some(err) = finish_error {
        eprintln!("Distributed run completed with errors: {}", err);
    }
    Ok(finish_run)
}

fn on_event(
    args: &TesterArgs,
    state: &mut ManualRunState,
    event: AgentEvent,
    disconnected_agents: &mut HashSet<String>,
    last_seen: &mut ArcShift<HashMap<String, Instant>>,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
) {
    let agent_id = event_agent_id(&event).to_owned();
    if disconnected_agents.contains(agent_id.as_str()) {
        return;
    }
    last_seen.rcu(|current| {
        let mut next = current.clone();
        next.insert(agent_id.clone(), Instant::now());
        next
    });
    if matches!(event, AgentEvent::Heartbeat { .. }) {
        return;
    }
    let is_disconnected = matches!(event, AgentEvent::Disconnected { .. });
    handle_agent_event(
        event,
        &state.run_id,
        &mut state.pending_agents,
        &mut state.agent_states,
        &mut state.runtime_errors,
        &mut state.sink_dirty,
    );
    if is_disconnected {
        disconnected_agents.insert(agent_id.clone());
        agent_pool.rcu(|current| {
            let mut next = current.clone();
            next.remove(agent_id.as_str());
            next
        });
        last_seen.rcu(|current| {
            let mut next = current.clone();
            next.remove(agent_id.as_str());
            next
        });
    }
    if state.charts_enabled {
        record_aggregated_sample(&mut state.aggregated_samples, &state.agent_states);
    }
    if let Some(ui_tx) = state.ui_tx.as_ref() {
        update_ui(
            ui_tx,
            args,
            &state.agent_states,
            &mut state.ui_latency_window,
            &mut state.ui_rps_window,
        );
    }
}

fn process_heartbeat_timeouts(
    state: &mut ManualRunState,
    heartbeat_timeout: Duration,
    last_seen: &mut ArcShift<HashMap<String, Instant>>,
    disconnected_agents: &mut HashSet<String>,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
) {
    let now = Instant::now();
    let timed_out: Vec<String> = {
        let seen = last_seen.shared_get();
        seen.iter()
            .filter(|(_, last)| now.duration_since(**last) > heartbeat_timeout)
            .map(|(agent_id, _)| agent_id.clone())
            .collect()
    };
    for agent_id in timed_out {
        if disconnected_agents.insert(agent_id.clone()) {
            agent_pool.rcu(|current| {
                let mut next = current.clone();
                next.remove(&agent_id);
                next
            });
            handle_agent_event(
                AgentEvent::Disconnected {
                    agent_id: agent_id.clone(),
                    message: format!(
                        "Heartbeat timed out after {}ms.",
                        heartbeat_timeout.as_millis()
                    ),
                },
                &state.run_id,
                &mut state.pending_agents,
                &mut state.agent_states,
                &mut state.runtime_errors,
                &mut state.sink_dirty,
            );
            last_seen.rcu(|current| {
                let mut next = current.clone();
                next.remove(&agent_id);
                next
            });
        }
    }
}
