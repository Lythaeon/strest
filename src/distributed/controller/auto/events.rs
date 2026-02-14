use std::collections::{HashMap, HashSet};

use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use crate::args::TesterArgs;

use super::super::output::{DistributedOutputState, OutputEvent, handle_output_event};
use super::super::shared::{
    AgentEvent, AgentSnapshot, event_agent_id, handle_agent_event,
    resolve_heartbeat_check_interval, resolve_sink_interval,
};
use super::setup::AutoRunSetup;
use crate::distributed::protocol::{WireMessage, read_message};

pub(super) struct AutoRunOutcome {
    pub(super) run_id: String,
    pub(super) output_state: DistributedOutputState,
    pub(super) agent_states: HashMap<String, AgentSnapshot>,
    pub(super) runtime_errors: Vec<String>,
    pub(super) channel_closed: bool,
    pub(super) pending_agents: HashSet<String>,
}

pub(super) async fn collect_auto_run_events(
    args: &TesterArgs,
    setup: AutoRunSetup,
) -> AutoRunOutcome {
    let AutoRunSetup {
        run_id,
        agents,
        mut output_state,
        heartbeat_timeout,
        report_deadline,
    } = setup;

    let mut runtime_errors: Vec<String> = Vec::new();
    let mut agent_states: HashMap<String, AgentSnapshot> = HashMap::new();
    let mut pending_agents: HashSet<String> =
        agents.iter().map(|agent| agent.agent_id.clone()).collect();
    let mut sink_interval = tokio::time::interval(resolve_sink_interval(args.sinks.as_ref()));
    sink_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut channel_closed = false;
    let mut heartbeat_interval =
        tokio::time::interval(resolve_heartbeat_check_interval(heartbeat_timeout));
    heartbeat_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_seen: HashMap<String, tokio::time::Instant> = agents
        .iter()
        .map(|agent| (agent.agent_id.clone(), tokio::time::Instant::now()))
        .collect();
    let mut disconnected_agents: HashSet<String> = HashSet::new();
    let deadline_sleep = tokio::time::sleep_until(report_deadline);
    tokio::pin!(deadline_sleep);

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    let mut _writer_guards = Vec::with_capacity(agents.len());
    for agent in agents {
        let agent_id = agent.agent_id.clone();
        let mut reader = agent.reader;
        _writer_guards.push(agent.writer);
        let event_tx = event_tx.clone();
        tokio::spawn(async move {
            loop {
                let message = match read_message(&mut reader).await {
                    Ok(message) => message,
                    Err(err) => {
                        if event_tx
                            .send(AgentEvent::Disconnected {
                                agent_id: agent_id.clone(),
                                message: err.to_string(),
                            })
                            .is_err()
                        {
                            break;
                        }
                        break;
                    }
                };

                match message {
                    WireMessage::Heartbeat(_) => {
                        if event_tx
                            .send(AgentEvent::Heartbeat {
                                agent_id: agent_id.clone(),
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    WireMessage::Stream(message) => {
                        if event_tx
                            .send(AgentEvent::Stream {
                                agent_id: agent_id.clone(),
                                message: *message,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    WireMessage::Report(message) => {
                        if event_tx
                            .send(AgentEvent::Report {
                                agent_id: agent_id.clone(),
                                message: *message,
                            })
                            .is_err()
                        {
                            break;
                        }
                        break;
                    }
                    WireMessage::Error(message) => {
                        if event_tx
                            .send(AgentEvent::Error {
                                agent_id: agent_id.clone(),
                                message: message.message,
                            })
                            .is_err()
                        {
                            break;
                        }
                        break;
                    }
                    WireMessage::Hello(_)
                    | WireMessage::Config(_)
                    | WireMessage::Start(_)
                    | WireMessage::Stop(_) => {
                        if event_tx
                            .send(AgentEvent::Error {
                                agent_id: agent_id.clone(),
                                message: "Unexpected message from agent.".to_owned(),
                            })
                            .is_err()
                        {
                            break;
                        }
                        break;
                    }
                }
            }
        });
    }
    drop(event_tx);

    loop {
        tokio::select! {
            () = &mut deadline_sleep => {
                if !pending_agents.is_empty() {
                    for agent_id in &pending_agents {
                        runtime_errors.push(format!(
                            "Timed out waiting for report from agent {}.",
                            agent_id
                        ));
                    }
                }
                break;
            }
            maybe_event = event_rx.recv() => {
                let Some(event) = maybe_event else {
                    channel_closed = true;
                    break;
                };
                let agent_id = event_agent_id(&event).to_owned();
                if disconnected_agents.contains(agent_id.as_str()) {
                    continue;
                }
                last_seen.insert(agent_id.clone(), tokio::time::Instant::now());
                let is_heartbeat = matches!(event, AgentEvent::Heartbeat { .. });
                let is_disconnected = matches!(event, AgentEvent::Disconnected { .. });
                if is_heartbeat {
                    continue;
                }
                handle_agent_event(
                    event,
                    &run_id,
                    &mut pending_agents,
                    &mut agent_states,
                    &mut runtime_errors,
                );
                if is_disconnected {
                    disconnected_agents.insert(agent_id.clone());
                    last_seen.remove(agent_id.as_str());
                }
                handle_output_event(
                    args,
                    &mut output_state,
                    &agent_states,
                    &mut runtime_errors,
                    OutputEvent::AgentStateUpdated,
                )
                .await;
                if pending_agents.is_empty() {
                    break;
                }
            }
            _ = sink_interval.tick() => {
                handle_output_event(
                    args,
                    &mut output_state,
                    &agent_states,
                    &mut runtime_errors,
                    OutputEvent::SinkTick,
                )
                .await;
            }
            _ = heartbeat_interval.tick() => {
                let now = tokio::time::Instant::now();
                let mut timed_out: Vec<String> = Vec::new();
                for (agent_id, last) in &last_seen {
                    if now.duration_since(*last) > heartbeat_timeout {
                        timed_out.push(agent_id.clone());
                    }
                }
                for agent_id in timed_out {
                    if disconnected_agents.insert(agent_id.clone()) {
                        let message = format!(
                            "Heartbeat timed out after {}ms.",
                            heartbeat_timeout.as_millis()
                        );
                        handle_agent_event(
                            AgentEvent::Disconnected { agent_id: agent_id.clone(), message },
                            &run_id,
                            &mut pending_agents,
                            &mut agent_states,
                            &mut runtime_errors,
                        );
                        last_seen.remove(&agent_id);
                    }
                }
                if pending_agents.is_empty() {
                    break;
                }
            }
        }
    }

    AutoRunOutcome {
        run_id,
        output_state,
        agent_states,
        runtime_errors,
        channel_closed,
        pending_agents,
    }
}
