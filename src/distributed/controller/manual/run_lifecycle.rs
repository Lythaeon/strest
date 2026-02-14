use std::collections::{HashMap, HashSet};
use std::time::Duration;

use arcshift::ArcShift;
use tokio::time::{Instant, MissedTickBehavior};

use crate::args::{Scenario, TesterArgs};
use crate::config::apply::scenario::{ScenarioDefaults, parse_scenario};

use super::super::control::{ControlError, ControlStartRequest};
use super::super::load::apply_load_share;
use super::super::output::setup_output_state;
use super::super::shared::{DEFAULT_START_AFTER_MS, REPORT_GRACE_SECS, resolve_sink_interval};
use super::state::{ManualAgent, ManualRunState, ScenarioState};
use crate::distributed::protocol::{ConfigMessage, StartMessage, StopMessage, WireMessage};
use crate::distributed::utils::build_run_id;
use crate::distributed::wire::build_wire_args;

pub(super) async fn start_manual_run(
    args: &TesterArgs,
    request: &ControlStartRequest,
    scenario_state: &mut ScenarioState,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
) -> Result<ManualRunState, ControlError> {
    let scenario = resolve_scenario_for_run(args, request, scenario_state)?;
    let mut run_args = args.clone();
    if let Some(scenario) = scenario {
        run_args.scenario = Some(scenario);
    }
    if run_args.scenario.is_none() && run_args.url.is_none() {
        return Err(ControlError::new(400, "Missing scenario or url for run."));
    }

    let start_after_ms = request.start_after_ms.unwrap_or(DEFAULT_START_AFTER_MS);
    let run_id = build_run_id();
    let base_args = build_wire_args(&run_args);

    let agents = agent_pool
        .shared_get()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    if agents.len() < args.min_agents.get() {
        return Err(ControlError::new(
            409,
            format!(
                "Need at least {} agents before starting.",
                args.min_agents.get()
            ),
        ));
    }

    let weights: Vec<u64> = agents.iter().map(|agent| agent.weight).collect();
    let mut pending_agents = HashSet::new();
    let mut failed_agents = Vec::new();

    for (idx, agent) in agents.iter().enumerate() {
        let mut agent_args = base_args.clone();
        apply_load_share(&mut agent_args, args, &weights, idx);
        if agent
            .sender
            .send(WireMessage::Config(Box::new(ConfigMessage {
                run_id: run_id.clone(),
                args: agent_args,
            })))
            .is_err()
            || agent
                .sender
                .send(WireMessage::Start(StartMessage {
                    run_id: run_id.clone(),
                    start_after_ms,
                }))
                .is_err()
        {
            failed_agents.push(agent.agent_id.clone());
            continue;
        }
        pending_agents.insert(agent.agent_id.clone());
    }

    if !failed_agents.is_empty() {
        agent_pool.rcu(|current| {
            let mut next = current.clone();
            for agent_id in &failed_agents {
                next.remove(agent_id);
            }
            next
        });
    }

    if pending_agents.len() < args.min_agents.get() {
        for agent in &agents {
            if pending_agents.contains(&agent.agent_id)
                && agent
                    .sender
                    .send(WireMessage::Stop(StopMessage {
                        run_id: run_id.clone(),
                    }))
                    .is_err()
            {
                // Agent channel already closed.
            }
        }
        return Err(ControlError::new(
            409,
            "Not enough agents available to start run.",
        ));
    }

    let output_state = setup_output_state(args);
    let mut sink_interval = tokio::time::interval(resolve_sink_interval(args.sinks.as_ref()));
    sink_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

    let report_deadline = Instant::now()
        .checked_add(
            Duration::from_secs(args.target_duration.get())
                .saturating_add(Duration::from_secs(REPORT_GRACE_SECS)),
        )
        .unwrap_or_else(Instant::now);

    Ok(ManualRunState {
        run_id,
        pending_agents,
        agent_states: HashMap::new(),
        runtime_errors: Vec::new(),
        sink_interval,
        output_state,
        deadline: report_deadline,
    })
}

pub(super) fn resolve_scenario_for_run(
    args: &TesterArgs,
    request: &ControlStartRequest,
    scenario_state: &mut ScenarioState,
) -> Result<Option<Scenario>, ControlError> {
    let scenario_defaults = ScenarioDefaults::new(
        args.url.clone(),
        args.method,
        args.data.clone(),
        args.headers.clone(),
    );
    if let Some(config) = request.scenario.as_ref() {
        let scenario = parse_scenario(config, &scenario_defaults)
            .map_err(|err| ControlError::new(400, err.to_string()))?;
        if let Some(name) = request.scenario_name.as_ref() {
            scenario_state.named.insert(name.clone(), config.clone());
        }
        return Ok(Some(scenario));
    }

    if let Some(name) = request.scenario_name.as_ref() {
        let config = scenario_state
            .named
            .get(name)
            .ok_or_else(|| ControlError::new(404, "Scenario not found."))?;
        let scenario = parse_scenario(config, &scenario_defaults)
            .map_err(|err| ControlError::new(400, err.to_string()))?;
        return Ok(Some(scenario));
    }

    Ok(scenario_state.default.clone())
}

pub(super) async fn request_stop(
    state: &mut ManualRunState,
    agent_pool: &mut ArcShift<HashMap<String, ManualAgent>>,
) {
    let run_id = state.run_id.clone();
    let agents = agent_pool
        .shared_get()
        .values()
        .cloned()
        .collect::<Vec<_>>();
    let mut failed_agents = Vec::new();
    for agent in agents {
        if agent
            .sender
            .send(WireMessage::Stop(StopMessage {
                run_id: run_id.clone(),
            }))
            .is_err()
        {
            failed_agents.push(agent.agent_id);
        }
    }
    if !failed_agents.is_empty() {
        agent_pool.rcu(|current| {
            let mut next = current.clone();
            for agent_id in &failed_agents {
                next.remove(agent_id);
            }
            next
        });
    }

    state.deadline = Instant::now()
        .checked_add(Duration::from_secs(REPORT_GRACE_SECS))
        .unwrap_or_else(Instant::now);
}
