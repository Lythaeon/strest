use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use arcshift::ArcShift;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time::{Instant, MissedTickBehavior};
use tracing::info;

use crate::args::TesterArgs;
use crate::config::types::ScenarioConfig;
use crate::error::{AppError, AppResult, DistributedError};

use super::super::control::ControlCommand;
use super::super::shared::{AgentEvent, resolve_heartbeat_check_interval};
use super::connections::{accept_control_connections, accept_manual_agents};
use super::loop_handlers::handle_active_run;
use super::loop_idle::handle_idle;
use super::state::{ManualAgent, ManualRunState, ScenarioState};

pub(in crate::distributed::controller) async fn run_controller_manual(
    args: &TesterArgs,
    scenarios: BTreeMap<String, ScenarioConfig>,
) -> AppResult<()> {
    let listen = args
        .controller_listen
        .as_deref()
        .ok_or_else(|| AppError::distributed(DistributedError::MissingControllerListen))?;
    let control_listen = args
        .control_listen
        .as_deref()
        .ok_or_else(|| AppError::distributed(DistributedError::MissingControlListen))?;

    let agent_listener = TcpListener::bind(listen).await.map_err(|err| {
        AppError::distributed(DistributedError::Bind {
            addr: listen.to_owned(),
            source: err,
        })
    })?;
    let control_listener = TcpListener::bind(control_listen).await.map_err(|err| {
        AppError::distributed(DistributedError::Bind {
            addr: control_listen.to_owned(),
            source: err,
        })
    })?;
    info!(
        "Controller listening on {} (manual mode, control plane {})",
        listen, control_listen
    );

    let mut agent_pool: ArcShift<HashMap<String, ManualAgent>> = ArcShift::new(HashMap::new());
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    let (control_tx, mut control_rx) = mpsc::unbounded_channel::<ControlCommand>();

    let heartbeat_timeout = Duration::from_millis(args.agent_heartbeat_timeout_ms.get());
    let mut heartbeat_interval =
        tokio::time::interval(resolve_heartbeat_check_interval(heartbeat_timeout));
    heartbeat_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_seen: ArcShift<HashMap<String, Instant>> = ArcShift::new(HashMap::new());

    tokio::spawn(accept_manual_agents(
        agent_listener,
        args.auth_token.clone(),
        agent_pool.clone(),
        event_tx,
        last_seen.clone(),
    ));
    tokio::spawn(accept_control_connections(
        control_listener,
        args.control_auth_token.clone(),
        control_tx,
    ));

    let mut scenario_state = ScenarioState {
        default: args.scenario.clone(),
        named: scenarios,
    };
    let mut run_state: Option<ManualRunState> = None;
    let mut disconnected_agents: HashSet<String> = HashSet::new();

    loop {
        if let Some(state) = run_state.as_mut() {
            if handle_active_run(
                args,
                state,
                &mut control_rx,
                &mut event_rx,
                &mut heartbeat_interval,
                heartbeat_timeout,
                &mut last_seen,
                &mut disconnected_agents,
                &mut agent_pool,
            )
            .await?
            {
                run_state = None;
            }
            continue;
        }

        handle_idle(
            args,
            &mut scenario_state,
            &mut control_rx,
            &mut event_rx,
            &mut heartbeat_interval,
            heartbeat_timeout,
            &mut last_seen,
            &mut disconnected_agents,
            &mut agent_pool,
            &mut run_state,
        )
        .await?;
    }
}
