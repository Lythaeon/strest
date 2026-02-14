use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use arcshift::ArcShift;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::args::{Scenario, TesterArgs};
use crate::config::types::ScenarioConfig;

use super::super::control::{ControlError, ControlStartRequest};
use super::super::output::DistributedOutputState;
use super::super::shared::AgentEvent;
use crate::distributed::protocol::WireMessage;

#[derive(Clone)]
pub(super) struct ManualAgent {
    pub(super) agent_id: String,
    pub(super) weight: u64,
    pub(super) sender: mpsc::UnboundedSender<WireMessage>,
}

pub(super) struct ManualRunState {
    pub(super) run_id: String,
    pub(super) pending_agents: HashSet<String>,
    pub(super) agent_states: HashMap<String, super::super::shared::AgentSnapshot>,
    pub(super) runtime_errors: Vec<String>,
    pub(super) sink_interval: tokio::time::Interval,
    pub(super) output_state: DistributedOutputState,
    pub(super) deadline: Instant,
}

pub(super) struct ScenarioState {
    pub(super) default: Option<Scenario>,
    pub(super) named: BTreeMap<String, ScenarioConfig>,
}

pub(super) fn resolve_manual_wait_timeout(
    args: &TesterArgs,
    request: &ControlStartRequest,
) -> Option<Duration> {
    let request_ms = request.agent_wait_timeout_ms.filter(|value| *value > 0);
    let base_ms = args.agent_wait_timeout_ms.map(|value| value.get());
    request_ms.or(base_ms).map(Duration::from_millis)
}

pub(super) async fn wait_for_min_agents(
    min_agents: usize,
    timeout: Duration,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
    event_rx: &mut mpsc::UnboundedReceiver<AgentEvent>,
    last_seen: &mut ArcShift<HashMap<String, Instant>>,
    disconnected_agents: &mut HashSet<String>,
) -> Result<(), ControlError> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now);
    loop {
        let available = agent_pool.shared_get().len();
        if available >= min_agents {
            return Ok(());
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(ControlError::new(
                409,
                format!(
                    "Timed out waiting for {} agents (got {}).",
                    min_agents, available
                ),
            ));
        }
        let remaining = deadline.duration_since(now);
        let tick = tokio::time::sleep(remaining.min(Duration::from_millis(200)));
        tokio::select! {
            () = tick => {},
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return Err(ControlError::new(503, "Agent event channel closed."));
                };
                let agent_id = super::super::shared::event_agent_id(&event).to_owned();
                if disconnected_agents.contains(agent_id.as_str()) {
                    continue;
                }
                last_seen.rcu(|current| {
                    let mut next = current.clone();
                    next.insert(agent_id.clone(), Instant::now());
                    next
                });
                if matches!(event, AgentEvent::Heartbeat { .. }) {
                    continue;
                }
                if let AgentEvent::Disconnected { .. } = event {
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
        }
    }
}
