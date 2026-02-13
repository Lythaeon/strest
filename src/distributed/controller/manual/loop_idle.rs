use std::collections::{HashMap, HashSet};
use std::time::Duration;

use arcshift::ArcShift;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::warn;

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};

use super::super::control::{ControlCommand, ControlError, ControlResponse};
use super::super::shared::{AgentEvent, event_agent_id};
use super::run_lifecycle::start_manual_run;
use super::state::{
    ManualAgent, ManualRunState, ScenarioState, resolve_manual_wait_timeout, wait_for_min_agents,
};

#[expect(
    clippy::too_many_arguments,
    reason = "Idle loop uses shared controller state"
)]
pub(super) async fn handle_idle(
    args: &TesterArgs,
    scenario_state: &mut ScenarioState,
    control_rx: &mut mpsc::UnboundedReceiver<ControlCommand>,
    event_rx: &mut mpsc::UnboundedReceiver<AgentEvent>,
    heartbeat_interval: &mut tokio::time::Interval,
    heartbeat_timeout: Duration,
    last_seen: &mut ArcShift<HashMap<String, Instant>>,
    disconnected_agents: &mut HashSet<String>,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
    run_state: &mut Option<ManualRunState>,
) -> AppResult<()> {
    tokio::select! {
        command = control_rx.recv() => {
            let Some(command) = command else {
                return Err(AppError::distributed(DistributedError::ControlChannelClosed));
            };
            match command {
                ControlCommand::Start { request, respond_to } => {
                    let min_agents = args.min_agents.get();
                    if let Some(timeout) = resolve_manual_wait_timeout(args, &request) {
                        if let Err(err) = wait_for_min_agents(min_agents, timeout, agent_pool, event_rx, last_seen, disconnected_agents).await {
                            if respond_to.send(Err(err)).is_err() {
                                // Requester dropped the response channel.
                            }
                            return Ok(());
                        }
                    } else if agent_pool.shared_get().len() < min_agents {
                        if respond_to
                            .send(Err(ControlError::new(
                                409,
                                format!("Need at least {} agents before starting.", min_agents),
                            )))
                            .is_err()
                        {
                            // Requester dropped the response channel.
                        }
                        return Ok(());
                    }

                    match start_manual_run(args, &request, scenario_state, agent_pool).await {
                        Ok(state) => {
                            if respond_to
                                .send(Ok(ControlResponse {
                                    status: "started".to_owned(),
                                    run_id: Some(state.run_id.clone()),
                                }))
                                .is_err()
                            {
                                // Requester dropped the response channel.
                            }
                            *run_state = Some(state);
                        }
                        Err(err) => {
                            if respond_to.send(Err(err)).is_err() {
                                // Requester dropped the response channel.
                            }
                        }
                    }
                }
                ControlCommand::Stop { respond_to } => {
                    if respond_to
                        .send(Ok(ControlResponse {
                            status: "idle".to_owned(),
                            run_id: None,
                        }))
                        .is_err()
                    {
                        // Requester dropped the response channel.
                    }
                }
            }
        }
        event = event_rx.recv() => {
            let Some(event) = event else {
                return Err(AppError::distributed(DistributedError::AgentEventChannelClosed));
            };
            on_idle_event(&event, disconnected_agents, last_seen, agent_pool);
        }
        _ = heartbeat_interval.tick() => {
            prune_idle_heartbeat(heartbeat_timeout, last_seen, disconnected_agents, agent_pool);
        }
    }

    Ok(())
}

fn on_idle_event(
    event: &AgentEvent,
    disconnected_agents: &mut HashSet<String>,
    last_seen: &mut ArcShift<HashMap<String, Instant>>,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
) {
    let agent_id = event_agent_id(event).to_owned();
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
    if matches!(event, AgentEvent::Disconnected { .. }) {
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
}

fn prune_idle_heartbeat(
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
            last_seen.rcu(|current| {
                let mut next = current.clone();
                next.remove(&agent_id);
                next
            });
            warn!(
                "Agent {} heartbeat timed out after {}ms.",
                agent_id,
                heartbeat_timeout.as_millis()
            );
        }
    }
}
