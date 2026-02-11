use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::IsTerminal;
use std::time::Duration;

use arcshift::ArcShift;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{Instant, MissedTickBehavior};
use tracing::{info, warn};

use crate::args::{Scenario, TesterArgs};
use crate::config::apply::parse_scenario;
use crate::config::types::ScenarioConfig;
use crate::error::{AppError, AppResult, DistributedError};
use crate::metrics::AggregatedMetricSample;
use crate::shutdown::ShutdownSender;
use crate::sinks::config::SinkStats;
use crate::ui::{model::UiData, render::setup_render_ui};

use super::super::protocol::{
    ConfigMessage, StartMessage, StopMessage, WireMessage, read_message, send_message,
};
use super::super::summary::{
    Percentiles, SummaryPercentiles, compute_summary_stats, print_summary,
};
use super::super::utils::build_run_id;
use super::super::wire::build_wire_args;
use super::agent::accept_agent;
use super::control::{ControlCommand, ControlError, ControlResponse, ControlStartRequest};
use super::http::{read_http_request, write_error_response, write_json_response};
use super::load::apply_load_share;
use super::shared::{
    AgentEvent, AgentSnapshot, DEFAULT_START_AFTER_MS, REPORT_GRACE_SECS, aggregate_snapshots,
    event_agent_id, handle_agent_event, record_aggregated_sample, resolve_heartbeat_check_interval,
    resolve_sink_interval, update_ui, write_aggregated_charts, write_streaming_sinks,
};

#[derive(Clone)]
struct ManualAgent {
    agent_id: String,
    weight: u64,
    sender: mpsc::UnboundedSender<WireMessage>,
}

struct ManualRunState {
    run_id: String,
    pending_agents: HashSet<String>,
    agent_states: HashMap<String, AgentSnapshot>,
    aggregated_samples: Vec<AggregatedMetricSample>,
    runtime_errors: Vec<String>,
    sink_dirty: bool,
    sink_updates_enabled: bool,
    sink_interval: tokio::time::Interval,
    ui_tx: Option<watch::Sender<UiData>>,
    shutdown_tx: Option<ShutdownSender>,
    ui_latency_window: VecDeque<(u64, u64)>,
    ui_rps_window: VecDeque<(u64, u64)>,
    deadline: Instant,
    charts_enabled: bool,
}

struct ScenarioState {
    default: Option<Scenario>,
    named: BTreeMap<String, ScenarioConfig>,
}

pub(super) async fn run_controller_manual(
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

    let auth_token = args.auth_token.clone();
    let pool_clone = agent_pool.clone();
    let event_tx_clone = event_tx.clone();
    let last_seen_clone = last_seen.clone();
    tokio::spawn(async move {
        accept_manual_agents(
            agent_listener,
            auth_token,
            pool_clone,
            event_tx_clone,
            last_seen_clone,
        )
        .await;
    });

    let control_auth_token = args.control_auth_token.clone();
    tokio::spawn(async move {
        accept_control_connections(control_listener, control_auth_token, control_tx).await;
    });

    let mut scenario_state = ScenarioState {
        default: args.scenario.clone(),
        named: scenarios,
    };
    let mut run_state: Option<ManualRunState> = None;
    let mut disconnected_agents: HashSet<String> = HashSet::new();

    loop {
        if let Some(state) = run_state.as_mut() {
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
                                .send(Err(ControlError::new(
                                    409,
                                    "Run already in progress.",
                                )))
                                .is_err()
                            {
                                eprintln!("Failed to send control response.");
                            }
                        }
                        ControlCommand::Stop { respond_to } => {
                            request_stop(state, &mut agent_pool).await;
                            let response = ControlResponse {
                                status: "stopping".to_owned(),
                                run_id: Some(state.run_id.clone()),
                            };
                            if respond_to.send(Ok(response)).is_err() {
                                eprintln!("Failed to send control response.");
                            }
                        }
                    }
                }
                event = event_rx.recv() => {
                    let Some(event) = event else {
                        return Err(AppError::distributed(DistributedError::AgentEventChannelClosed));
                    };
                    let agent_id = event_agent_id(&event).to_owned();
                    if disconnected_agents.contains(agent_id.as_str()) {
                        continue;
                    }
                    last_seen.rcu(|current| {
                        let mut next = current.clone();
                        next.insert(agent_id.clone(), Instant::now());
                        next
                    });
                    let is_heartbeat = matches!(event, AgentEvent::Heartbeat { .. });
                    let is_disconnected = matches!(event, AgentEvent::Disconnected { .. });
                    if is_heartbeat {
                        continue;
                    }
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
                        record_aggregated_sample(
                            &mut state.aggregated_samples,
                            &state.agent_states,
                        );
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
                    let now = Instant::now();
                    let timed_out: Vec<String> = {
                        let seen = last_seen.shared_get();
                        seen
                            .iter()
                            .filter(|(_, last)| now.duration_since(**last) > heartbeat_timeout)
                            .map(|(agent_id, _)| agent_id.clone())
                            .collect()
                    };
                    if !timed_out.is_empty() {
                        for agent_id in timed_out {
                            if disconnected_agents.insert(agent_id.clone()) {
                                agent_pool.rcu(|current| {
                                    let mut next = current.clone();
                                    next.remove(&agent_id);
                                    next
                                });
                                let message = format!(
                                    "Heartbeat timed out after {}ms.",
                                    heartbeat_timeout.as_millis()
                                );
                                handle_agent_event(
                                    AgentEvent::Disconnected { agent_id: agent_id.clone(), message },
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
                        if state.pending_agents.is_empty() {
                            finish_run = true;
                            finish_error = finalize_manual_run(args, state).await.err();
                        }
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    if !state.pending_agents.is_empty() {
                        for agent_id in &state.pending_agents {
                            state.runtime_errors.push(format!(
                                "Timed out waiting for report from agent {}.",
                                agent_id
                            ));
                        }
                    }
                    finish_run = true;
                    finish_error = finalize_manual_run(args, state).await.err();
                }
            }

            if finish_run {
                if let Some(err) = finish_error {
                    eprintln!("Distributed run completed with errors: {}", err);
                }
                run_state = None;
            }
            continue;
        }

        tokio::select! {
            command = control_rx.recv() => {
                let Some(command) = command else {
                    return Err(AppError::distributed(DistributedError::ControlChannelClosed));
                };
                match command {
                    ControlCommand::Start { request, respond_to } => {
                        let min_agents = args.min_agents.get();
                        let wait_timeout = resolve_manual_wait_timeout(args, &request);
                        if let Some(timeout) = wait_timeout {
                            info!(
                                "Waiting up to {}ms for {} agent(s) before starting",
                                timeout.as_millis(),
                                min_agents
                            );
                            if let Err(err) = wait_for_min_agents(
                                min_agents,
                                timeout,
                                &mut agent_pool,
                                &mut event_rx,
                                &mut last_seen,
                                &mut disconnected_agents,
                            )
                            .await
                            {
                                if respond_to.send(Err(err)).is_err() {
                                    eprintln!("Failed to send control response.");
                                }
                                continue;
                            }
                        } else {
                            let available = agent_pool.shared_get().len();
                            if available < min_agents {
                                let err = ControlError::new(
                                    409,
                                    format!(
                                        "Need at least {} agents before starting.",
                                        min_agents
                                    ),
                                );
                                if respond_to.send(Err(err)).is_err() {
                                    eprintln!("Failed to send control response.");
                                }
                                continue;
                            }
                        }
                        let result = start_manual_run(
                            args,
                            &request,
                            &mut scenario_state,
                            &mut agent_pool,
                        )
                        .await;
                        match result {
                            Ok(state) => {
                                let response = ControlResponse {
                                    status: "started".to_owned(),
                                    run_id: Some(state.run_id.clone()),
                                };
                                if respond_to.send(Ok(response)).is_err() {
                                    eprintln!("Failed to send control response.");
                                }
                                run_state = Some(state);
                            }
                            Err(err) => {
                                if respond_to.send(Err(err)).is_err() {
                                    eprintln!("Failed to send control response.");
                                }
                            }
                        }
                    }
                    ControlCommand::Stop { respond_to } => {
                        let response = ControlResponse {
                            status: "idle".to_owned(),
                            run_id: None,
                        };
                        if respond_to.send(Ok(response)).is_err() {
                            eprintln!("Failed to send control response.");
                        }
                    }
                }
            }
            event = event_rx.recv() => {
                let Some(event) = event else {
                    return Err(AppError::distributed(DistributedError::AgentEventChannelClosed));
                };
                let agent_id = event_agent_id(&event).to_owned();
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
            _ = heartbeat_interval.tick() => {
                let now = Instant::now();
                let timed_out: Vec<String> = {
                    let seen = last_seen.shared_get();
                    seen
                        .iter()
                        .filter(|(_, last)| now.duration_since(**last) > heartbeat_timeout)
                        .map(|(agent_id, _)| agent_id.clone())
                        .collect()
                };
                if !timed_out.is_empty() {
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
            }
        }
    }
}

async fn accept_manual_agents(
    listener: TcpListener,
    auth_token: Option<String>,
    agent_pool: ArcShift<HashMap<String, ManualAgent>>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    last_seen: ArcShift<HashMap<String, Instant>>,
) {
    info!("Controller listening for agents (manual mode)");
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(result) => result,
            Err(err) => {
                eprintln!("Failed to accept agent: {}", err);
                continue;
            }
        };
        let pool = agent_pool.clone();
        let event_tx = event_tx.clone();
        let auth_token = auth_token.clone();
        let last_seen = last_seen.clone();
        tokio::spawn(async move {
            register_manual_agent(stream, auth_token.as_deref(), pool, event_tx, last_seen).await;
        });
    }
}

async fn register_manual_agent(
    stream: TcpStream,
    auth_token: Option<&str>,
    mut agent_pool: ArcShift<HashMap<String, ManualAgent>>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    mut last_seen: ArcShift<HashMap<String, Instant>>,
) {
    if let Ok(peer) = stream.peer_addr() {
        info!("Manual agent connection from {}", peer);
    }
    let agent = match accept_agent(stream, auth_token).await {
        Ok(agent) => agent,
        Err(err) => {
            eprintln!("Agent rejected: {}", err);
            return;
        }
    };
    info!(
        "Manual agent {} registered (weight={})",
        agent.agent_id, agent.weight
    );

    last_seen.rcu(|current| {
        let mut next = current.clone();
        next.insert(agent.agent_id.clone(), Instant::now());
        next
    });

    let agent_id = agent.agent_id.clone();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<WireMessage>();

    let mut writer = agent.writer;
    let event_tx_writer = event_tx.clone();
    let agent_id_writer = agent_id.clone();
    tokio::spawn(async move {
        while let Some(message) = out_rx.recv().await {
            if let Err(err) = send_message(&mut writer, &message).await {
                if event_tx_writer
                    .send(AgentEvent::Disconnected {
                        agent_id: agent_id_writer.clone(),
                        message: err.to_string(),
                    })
                    .is_err()
                {
                    eprintln!("Agent event channel closed.");
                }
                break;
            }
        }
    });

    let mut reader = agent.reader;
    let event_tx_reader = event_tx.clone();
    let agent_id_reader = agent_id.clone();
    tokio::spawn(async move {
        loop {
            let message = match read_message(&mut reader).await {
                Ok(message) => message,
                Err(err) => {
                    if event_tx_reader
                        .send(AgentEvent::Disconnected {
                            agent_id: agent_id_reader.clone(),
                            message: err.to_string(),
                        })
                        .is_err()
                    {
                        eprintln!("Agent event channel closed.");
                    }
                    break;
                }
            };

            let event = match message {
                WireMessage::Heartbeat(_) => AgentEvent::Heartbeat {
                    agent_id: agent_id_reader.clone(),
                },
                WireMessage::Stream(message) => AgentEvent::Stream {
                    agent_id: agent_id_reader.clone(),
                    message: *message,
                },
                WireMessage::Report(message) => AgentEvent::Report {
                    agent_id: agent_id_reader.clone(),
                    message: *message,
                },
                WireMessage::Error(message) => AgentEvent::Error {
                    agent_id: agent_id_reader.clone(),
                    message: message.message,
                },
                WireMessage::Hello(_)
                | WireMessage::Config(_)
                | WireMessage::Start(_)
                | WireMessage::Stop(_) => AgentEvent::Error {
                    agent_id: agent_id_reader.clone(),
                    message: "Unexpected message from agent.".to_owned(),
                },
            };

            if event_tx_reader.send(event).is_err() {
                break;
            }
        }
    });

    let handle = ManualAgent {
        agent_id: agent_id.clone(),
        weight: agent.weight,
        sender: out_tx,
    };
    agent_pool.rcu(|current| {
        let mut next = current.clone();
        next.insert(agent_id.clone(), handle.clone());
        next
    });
}

async fn accept_control_connections(
    listener: TcpListener,
    auth_token: Option<String>,
    control_tx: mpsc::UnboundedSender<ControlCommand>,
) {
    loop {
        let (socket, _) = match listener.accept().await {
            Ok(result) => result,
            Err(err) => {
                eprintln!("Failed to accept control connection: {}", err);
                continue;
            }
        };
        let control_tx = control_tx.clone();
        let auth_token = auth_token.clone();
        tokio::spawn(async move {
            handle_control_connection(socket, auth_token.as_deref(), control_tx).await;
        });
    }
}

async fn handle_control_connection(
    mut socket: TcpStream,
    auth_token: Option<&str>,
    control_tx: mpsc::UnboundedSender<ControlCommand>,
) {
    let request = match read_http_request(&mut socket).await {
        Ok(request) => request,
        Err(err) => {
            if let Err(write_err) =
                write_error_response(&mut socket, err.status, &err.message).await
            {
                eprintln!("Failed to write control response: {}", write_err);
            }
            return;
        }
    };

    if let Some(token) = auth_token {
        let header = request
            .headers
            .get("authorization")
            .map(|value| value.trim().to_owned());
        let expected = format!("Bearer {}", token);
        if header.as_deref() != Some(expected.as_str()) {
            if let Err(write_err) = write_error_response(&mut socket, 401, "Unauthorized").await {
                eprintln!("Failed to write control response: {}", write_err);
            }
            return;
        }
    }

    match (request.method.as_str(), request.path.as_str()) {
        ("POST", "/start") => {
            let start_request = if request.body.is_empty() {
                ControlStartRequest::default()
            } else {
                match serde_json::from_slice::<ControlStartRequest>(&request.body) {
                    Ok(value) => value,
                    Err(err) => {
                        let message = format!("Invalid JSON: {}", err);
                        if let Err(write_err) =
                            write_error_response(&mut socket, 400, &message).await
                        {
                            eprintln!("Failed to write control response: {}", write_err);
                        }
                        return;
                    }
                }
            };

            let (respond_to, response_rx) = oneshot::channel();
            if control_tx
                .send(ControlCommand::Start {
                    request: start_request,
                    respond_to,
                })
                .is_err()
            {
                if let Err(write_err) =
                    write_error_response(&mut socket, 503, "Controller unavailable").await
                {
                    eprintln!("Failed to write control response: {}", write_err);
                }
                return;
            }

            let response = tokio::time::timeout(Duration::from_secs(5), response_rx)
                .await
                .map_or_else(
                    |_| Err(ControlError::new(504, "Controller response timed out")),
                    |result| {
                        result.unwrap_or_else(|_| {
                            Err(ControlError::new(503, "Controller unavailable"))
                        })
                    },
                );

            match response {
                Ok(response) => {
                    if let Err(write_err) = write_json_response(&mut socket, 200, &response).await {
                        eprintln!("Failed to write control response: {}", write_err);
                    }
                }
                Err(err) => {
                    if let Err(write_err) =
                        write_error_response(&mut socket, err.status, &err.message).await
                    {
                        eprintln!("Failed to write control response: {}", write_err);
                    }
                }
            }
        }
        ("POST", "/stop") => {
            let (respond_to, response_rx) = oneshot::channel();
            if control_tx
                .send(ControlCommand::Stop { respond_to })
                .is_err()
            {
                if let Err(write_err) =
                    write_error_response(&mut socket, 503, "Controller unavailable").await
                {
                    eprintln!("Failed to write control response: {}", write_err);
                }
                return;
            }

            let response = tokio::time::timeout(Duration::from_secs(5), response_rx)
                .await
                .map_or_else(
                    |_| Err(ControlError::new(504, "Controller response timed out")),
                    |result| {
                        result.unwrap_or_else(|_| {
                            Err(ControlError::new(503, "Controller unavailable"))
                        })
                    },
                );

            match response {
                Ok(response) => {
                    if let Err(write_err) = write_json_response(&mut socket, 200, &response).await {
                        eprintln!("Failed to write control response: {}", write_err);
                    }
                }
                Err(err) => {
                    if let Err(write_err) =
                        write_error_response(&mut socket, err.status, &err.message).await
                    {
                        eprintln!("Failed to write control response: {}", write_err);
                    }
                }
            }
        }
        _ => {
            if let Err(write_err) = write_error_response(&mut socket, 404, "Not found").await {
                eprintln!("Failed to write control response: {}", write_err);
            }
        }
    }
}

async fn start_manual_run(
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
        {
            failed_agents.push(agent.agent_id.clone());
            continue;
        }
        if agent
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
                eprintln!("Failed to send stop to agent {}.", agent.agent_id);
            }
        }
        return Err(ControlError::new(
            409,
            "Not enough agents available to start run.",
        ));
    }

    let ui_enabled =
        args.distributed_stream_summaries && !args.no_ui && std::io::stdout().is_terminal();
    let (ui_tx, shutdown_tx, _ui_handle) = if ui_enabled {
        let target_duration = Duration::from_secs(args.target_duration.get());
        let (shutdown_tx, _) = crate::shutdown_handlers::shutdown_channel();
        let (ui_tx, _) = watch::channel(UiData {
            target_duration,
            ui_window_ms: args.ui_window_ms.get(),
            no_color: args.no_color,
            ..UiData::default()
        });
        let handle = setup_render_ui(args, &shutdown_tx, &ui_tx);
        (Some(ui_tx), Some(shutdown_tx), Some(handle))
    } else {
        (None, None, None)
    };

    let sink_updates_enabled = args.distributed_stream_summaries && args.sinks.is_some();
    let charts_enabled = !args.no_charts && args.distributed_stream_summaries;
    let mut sink_interval = tokio::time::interval(resolve_sink_interval(args.sinks.as_ref()));
    sink_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

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
        aggregated_samples: Vec::new(),
        runtime_errors: Vec::new(),
        sink_dirty: false,
        sink_updates_enabled,
        sink_interval,
        ui_tx,
        shutdown_tx,
        ui_latency_window: VecDeque::new(),
        ui_rps_window: VecDeque::new(),
        deadline: report_deadline,
        charts_enabled,
    })
}

fn resolve_scenario_for_run(
    args: &TesterArgs,
    request: &ControlStartRequest,
    scenario_state: &mut ScenarioState,
) -> Result<Option<Scenario>, ControlError> {
    if let Some(config) = request.scenario.as_ref() {
        let scenario =
            parse_scenario(config, args).map_err(|err| ControlError::new(400, err.to_string()))?;
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
        let scenario =
            parse_scenario(config, args).map_err(|err| ControlError::new(400, err.to_string()))?;
        return Ok(Some(scenario));
    }

    Ok(scenario_state.default.clone())
}

async fn request_stop(
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

async fn finalize_manual_run(args: &TesterArgs, state: &mut ManualRunState) -> AppResult<()> {
    if state.agent_states.is_empty() {
        state
            .runtime_errors
            .push("No successful agent reports received.".to_owned());
    } else if let Ok((summary, merged_hist, success_hist)) =
        aggregate_snapshots(&state.agent_states)
    {
        let (p50, p90, p99) = merged_hist.percentiles();
        let (success_p50, success_p90, success_p99) = success_hist.percentiles();
        let stats = compute_summary_stats(&summary);
        let mut charts_written = false;
        if state.charts_enabled {
            match write_aggregated_charts(&state.aggregated_samples, args).await {
                Ok(written) => charts_written = written,
                Err(err) => state.runtime_errors.push(err.to_string()),
            }
        }

        let percentiles = SummaryPercentiles {
            all: Percentiles { p50, p90, p99 },
            ok: Percentiles {
                p50: success_p50,
                p90: success_p90,
                p99: success_p99,
            },
        };

        print_summary(&summary, percentiles, args, charts_written);

        if let Some(sinks) = args.sinks.as_ref() {
            let sink_stats = SinkStats {
                duration: summary.duration,
                total_requests: summary.total_requests,
                successful_requests: summary.successful_requests,
                error_requests: summary.error_requests,
                timeout_requests: summary.timeout_requests,
                min_latency_ms: summary.min_latency_ms,
                max_latency_ms: summary.max_latency_ms,
                avg_latency_ms: summary.avg_latency_ms,
                p50_latency_ms: p50,
                p90_latency_ms: p90,
                p99_latency_ms: p99,
                success_rate_x100: stats.success_rate_x100,
                avg_rps_x100: stats.avg_rps_x100,
                avg_rpm_x100: stats.avg_rpm_x100,
            };
            if let Err(err) = crate::sinks::writers::write_sinks(sinks, &sink_stats).await {
                state.runtime_errors.push(format!("Sinks: {}", err));
            }
        }
    } else {
        state
            .runtime_errors
            .push("Failed to aggregate agent summaries.".to_owned());
    }

    if let Some(shutdown_tx) = state.shutdown_tx.as_ref() {
        drop(shutdown_tx.send(()));
    }

    if !state.runtime_errors.is_empty() {
        eprintln!("Runtime errors:");
        for err in &state.runtime_errors {
            eprintln!("- {}", err);
        }
        return Err(AppError::distributed(
            DistributedError::RunCompletedWithErrors,
        ));
    }

    Ok(())
}

fn resolve_manual_wait_timeout(
    args: &TesterArgs,
    request: &ControlStartRequest,
) -> Option<Duration> {
    let request_ms = request.agent_wait_timeout_ms.filter(|value| *value > 0);
    let base_ms = args.agent_wait_timeout_ms.map(|value| value.get());
    request_ms.or(base_ms).map(Duration::from_millis)
}

async fn wait_for_min_agents(
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
                let agent_id = event_agent_id(&event).to_owned();
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
