use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::io::IsTerminal;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::time::{Instant, MissedTickBehavior, timeout};

use crate::arcshift::ArcShift;
use crate::args::{ControllerMode, LoadProfile, Scenario, TesterArgs};
use crate::charts;
use crate::config::apply::parse_scenario;
use crate::config::types::ScenarioConfig;
use crate::metrics::{AggregatedMetricSample, LatencyHistogram};
use crate::sinks::config::{SinkStats, SinksConfig};
use crate::ui::{model::UiData, render::setup_render_ui};
use tracing::{debug, info, warn};

use super::protocol::{
    ConfigMessage, ErrorMessage, ReportMessage, StartMessage, StopMessage, StreamMessage,
    WireMessage, WireSummary, read_message, send_message,
};
use super::summary::{compute_summary_stats, merge_summaries, print_summary};
use super::utils::build_run_id;
use super::wire::build_wire_args;

struct AgentConn {
    agent_id: String,
    weight: u64,
    reader: BufReader<tokio::net::tcp::OwnedReadHalf>,
    writer: tokio::net::tcp::OwnedWriteHalf,
}

const AGENT_HELLO_TIMEOUT: Duration = Duration::from_secs(10);
const REPORT_GRACE_SECS: u64 = 30;
const DEFAULT_SINK_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_START_AFTER_MS: u64 = 3000;

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
    shutdown_tx: Option<broadcast::Sender<u16>>,
    ui_latency_window: VecDeque<(u64, u64)>,
    deadline: Instant,
    charts_enabled: bool,
}

struct ScenarioState {
    default: Option<Scenario>,
    named: BTreeMap<String, ScenarioConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct ControlStartRequest {
    scenario_name: Option<String>,
    scenario: Option<ScenarioConfig>,
    start_after_ms: Option<u64>,
    agent_wait_timeout_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ControlResponse {
    status: String,
    run_id: Option<String>,
}

#[derive(Debug)]
struct ControlError {
    status: u16,
    message: String,
}

impl ControlError {
    fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

enum ControlCommand {
    Start {
        request: ControlStartRequest,
        respond_to: oneshot::Sender<Result<ControlResponse, ControlError>>,
    },
    Stop {
        respond_to: oneshot::Sender<Result<ControlResponse, ControlError>>,
    },
}

struct AgentSnapshot {
    summary: WireSummary,
    histogram: LatencyHistogram,
}

enum AgentEvent {
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

/// Runs the distributed controller in auto or manual mode.
///
/// # Errors
///
/// Returns an error if the controller cannot bind, validate configuration,
/// or complete the distributed run.
pub async fn run_controller(
    args: &TesterArgs,
    scenarios: Option<BTreeMap<String, ScenarioConfig>>,
) -> Result<(), String> {
    match args.controller_mode {
        ControllerMode::Auto => run_controller_auto(args).await,
        ControllerMode::Manual => run_controller_manual(args, scenarios.unwrap_or_default()).await,
    }
}

async fn run_controller_auto(args: &TesterArgs) -> Result<(), String> {
    let listen = args
        .controller_listen
        .as_deref()
        .ok_or_else(|| "Missing --controller-listen.".to_owned())?;

    let listener = TcpListener::bind(listen)
        .await
        .map_err(|err| format!("Failed to bind controller {}: {}", listen, err))?;
    info!(
        "Controller listening on {} (auto mode, min_agents={})",
        listen,
        args.min_agents.get()
    );

    let wait_timeout = resolve_agent_wait_timeout(args);
    let wait_deadline = wait_timeout.and_then(|timeout| Instant::now().checked_add(timeout));
    if let Some(timeout) = wait_timeout {
        info!(
            "Waiting up to {}ms for {} agent(s)",
            timeout.as_millis(),
            args.min_agents.get()
        );
    }

    let mut agents: Vec<AgentConn> = Vec::new();
    while agents.len() < args.min_agents.get() {
        let stream = match wait_deadline {
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    return Err(format!(
                        "Timed out waiting for {} agents (got {}).",
                        args.min_agents.get(),
                        agents.len()
                    ));
                }
                let remaining = deadline.duration_since(now);
                match tokio::time::timeout(remaining, listener.accept()).await {
                    Ok(result) => {
                        let (stream, _) =
                            result.map_err(|err| format!("Failed to accept agent: {}", err))?;
                        stream
                    }
                    Err(_) => {
                        return Err(format!(
                            "Timed out waiting for {} agents (got {}).",
                            args.min_agents.get(),
                            agents.len()
                        ));
                    }
                }
            }
            None => {
                let (stream, _) = listener
                    .accept()
                    .await
                    .map_err(|err| format!("Failed to accept agent: {}", err))?;
                stream
            }
        };
        match accept_agent(stream, args.auth_token.as_deref()).await {
            Ok(agent) => {
                info!(
                    "Agent {} registered (weight={})",
                    agent.agent_id, agent.weight
                );
                agents.push(agent);
            }
            Err(err) => {
                eprintln!("Agent rejected: {}", err);
            }
        }
    }

    let run_id = build_run_id();
    let weights = compute_weights(&agents);
    let base_args = build_wire_args(args);
    let start_after_ms = DEFAULT_START_AFTER_MS;
    info!(
        "Starting distributed run {} with {} agent(s)",
        run_id,
        agents.len()
    );

    for (idx, agent) in agents.iter_mut().enumerate() {
        let mut agent_args = base_args.clone();
        apply_load_share(&mut agent_args, args, &weights, idx);
        debug!(
            "Sending config to agent {} for run {}",
            agent.agent_id, run_id
        );
        send_message(
            &mut agent.writer,
            &WireMessage::Config(Box::new(ConfigMessage {
                run_id: run_id.clone(),
                args: agent_args,
            })),
        )
        .await?;
    }

    for agent in agents.iter_mut() {
        debug!(
            "Sending start to agent {} for run {}",
            agent.agent_id, run_id
        );
        send_message(
            &mut agent.writer,
            &WireMessage::Start(StartMessage {
                run_id: run_id.clone(),
                start_after_ms,
            }),
        )
        .await?;
    }

    let ui_enabled =
        args.distributed_stream_summaries && !args.no_ui && std::io::stdout().is_terminal();
    let (ui_tx, shutdown_tx, _ui_handle) = if ui_enabled {
        let target_duration = Duration::from_secs(args.target_duration.get());
        let (shutdown_tx, _) = broadcast::channel::<u16>(1);
        let (ui_tx, _) = watch::channel(UiData {
            target_duration,
            ..UiData::default()
        });
        let handle = setup_render_ui(args, &shutdown_tx, &ui_tx);
        (Some(ui_tx), Some(shutdown_tx), Some(handle))
    } else {
        (None, None, None)
    };
    let mut runtime_errors: Vec<String> = Vec::new();
    let mut agent_states: HashMap<String, AgentSnapshot> = HashMap::new();
    let mut pending_agents: HashSet<String> =
        agents.iter().map(|agent| agent.agent_id.clone()).collect();
    let mut ui_latency_window: VecDeque<(u64, u64)> = VecDeque::new();
    let charts_enabled = !args.no_charts && args.distributed_stream_summaries;
    let mut aggregated_samples: Vec<AggregatedMetricSample> = Vec::new();
    let sink_updates_enabled = args.distributed_stream_summaries && args.sinks.is_some();
    let mut sink_interval = tokio::time::interval(resolve_sink_interval(args.sinks.as_ref()));
    sink_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut sink_dirty = false;
    let mut channel_closed = false;
    let heartbeat_timeout = Duration::from_millis(args.agent_heartbeat_timeout_ms.get());
    let mut heartbeat_interval =
        tokio::time::interval(resolve_heartbeat_check_interval(heartbeat_timeout));
    heartbeat_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut last_seen: HashMap<String, Instant> = agents
        .iter()
        .map(|agent| (agent.agent_id.clone(), Instant::now()))
        .collect();
    let mut disconnected_agents: HashSet<String> = HashSet::new();

    let report_deadline = Instant::now()
        .checked_add(
            Duration::from_secs(args.target_duration.get())
                .saturating_add(Duration::from_secs(REPORT_GRACE_SECS)),
        )
        .unwrap_or_else(Instant::now);
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
                                message: err,
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
                last_seen.insert(agent_id.clone(), Instant::now());
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
                    &mut sink_dirty,
                );
                if is_disconnected {
                    disconnected_agents.insert(agent_id.clone());
                    last_seen.remove(agent_id.as_str());
                }
                if charts_enabled {
                    record_aggregated_sample(&mut aggregated_samples, &agent_states);
                }
                if let Some(ui_tx) = ui_tx.as_ref() {
                    update_ui(ui_tx, args, &agent_states, &mut ui_latency_window);
                }
                if pending_agents.is_empty() {
                    break;
                }
            }
            _ = sink_interval.tick() => {
                if sink_updates_enabled && sink_dirty {
                    if let Err(err) = write_streaming_sinks(args, &agent_states).await {
                        runtime_errors.push(err);
                    } else {
                        sink_dirty = false;
                    }
                }
            }
            _ = heartbeat_interval.tick() => {
                let now = Instant::now();
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
                            &mut sink_dirty,
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

    if channel_closed && !pending_agents.is_empty() {
        for agent_id in &pending_agents {
            runtime_errors.push(format!(
                "Agent {} disconnected before sending a report.",
                agent_id
            ));
        }
    }

    if agent_states.is_empty() {
        runtime_errors.push("No successful agent reports received.".to_owned());
    } else if let Ok((summary, merged_hist)) = aggregate_snapshots(&agent_states) {
        let (p50, p90, p99) = merged_hist.percentiles();
        let stats = compute_summary_stats(&summary);
        let mut charts_written = false;
        if charts_enabled {
            match write_aggregated_charts(&aggregated_samples, args).await {
                Ok(written) => charts_written = written,
                Err(err) => runtime_errors.push(err),
            }
        }

        print_summary(&summary, p50, p90, p99, args, charts_written);

        if let Some(sinks) = args.sinks.as_ref() {
            let sink_stats = SinkStats {
                duration: summary.duration,
                total_requests: summary.total_requests,
                successful_requests: summary.successful_requests,
                error_requests: summary.error_requests,
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
                runtime_errors.push(format!("Sinks: {}", err));
            }
        }
    } else {
        runtime_errors.push("Failed to aggregate agent summaries.".to_owned());
    }

    if let Some(shutdown_tx) = shutdown_tx.as_ref() {
        drop(shutdown_tx.send(1));
    }

    if !runtime_errors.is_empty() {
        eprintln!("Runtime errors:");
        for err in runtime_errors {
            eprintln!("- {}", err);
        }
        return Err("Distributed run completed with errors.".to_owned());
    }

    Ok(())
}

async fn run_controller_manual(
    args: &TesterArgs,
    scenarios: BTreeMap<String, ScenarioConfig>,
) -> Result<(), String> {
    let listen = args
        .controller_listen
        .as_deref()
        .ok_or_else(|| "Missing --controller-listen.".to_owned())?;
    let control_listen = args
        .control_listen
        .as_deref()
        .ok_or_else(|| "Missing --control-listen for manual controller.".to_owned())?;

    let agent_listener = TcpListener::bind(listen)
        .await
        .map_err(|err| format!("Failed to bind controller {}: {}", listen, err))?;
    let control_listener = TcpListener::bind(control_listen)
        .await
        .map_err(|err| format!("Failed to bind control server {}: {}", control_listen, err))?;
    info!(
        "Controller listening on {} (manual mode, control plane {})",
        listen, control_listen
    );

    let agent_pool: Arc<ArcShift<HashMap<String, ManualAgent>>> =
        Arc::new(ArcShift::new(HashMap::new()));
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AgentEvent>();
    let (control_tx, mut control_rx) = mpsc::unbounded_channel::<ControlCommand>();

    let heartbeat_timeout = Duration::from_millis(args.agent_heartbeat_timeout_ms.get());
    let mut heartbeat_interval =
        tokio::time::interval(resolve_heartbeat_check_interval(heartbeat_timeout));
    heartbeat_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let last_seen: Arc<ArcShift<HashMap<String, Instant>>> =
        Arc::new(ArcShift::new(HashMap::new()));

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
        if run_state.is_some() {
            let mut finish_run = false;
            let mut finish_error: Option<String> = None;
            {
                let state = run_state
                    .as_mut()
                    .ok_or_else(|| "Missing run state.".to_owned())?;
                let deadline = state.deadline;

                tokio::select! {
                    command = control_rx.recv() => {
                        let Some(command) = command else {
                            return Err("Control channel closed.".to_owned());
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
                                request_stop(state, &agent_pool).await;
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
                            return Err("Agent event channel closed.".to_owned());
                        };
                        let agent_id = event_agent_id(&event).to_owned();
                        if disconnected_agents.contains(agent_id.as_str()) {
                            continue;
                        }
                        last_seen.update(|current| {
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
                            agent_pool.update(|current| {
                                let mut next = current.clone();
                                next.remove(agent_id.as_str());
                                next
                            });
                            last_seen.update(|current| {
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
                            update_ui(ui_tx, args, &state.agent_states, &mut state.ui_latency_window);
                        }
                        if state.pending_agents.is_empty() {
                            finish_run = true;
                            finish_error = finalize_manual_run(args, state).await.err();
                        }
                    }
                    _ = state.sink_interval.tick() => {
                        if state.sink_updates_enabled && state.sink_dirty {
                            if let Err(err) = write_streaming_sinks(args, &state.agent_states).await {
                                state.runtime_errors.push(err);
                            } else {
                                state.sink_dirty = false;
                            }
                        }
                    }
                    _ = heartbeat_interval.tick() => {
                        let now = Instant::now();
                        let seen = last_seen.load();
                        let timed_out: Vec<String> = seen
                            .iter()
                            .filter(|(_, last)| now.duration_since(**last) > heartbeat_timeout)
                            .map(|(agent_id, _)| agent_id.clone())
                            .collect();
                        if !timed_out.is_empty() {
                            for agent_id in timed_out {
                                if disconnected_agents.insert(agent_id.clone()) {
                                    agent_pool.update(|current| {
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
                                    last_seen.update(|current| {
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
                    return Err("Control channel closed.".to_owned());
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
                                &agent_pool,
                                &mut event_rx,
                                &last_seen,
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
                            let available = agent_pool.load().len();
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
                            &agent_pool,
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
                    return Err("Agent event channel closed.".to_owned());
                };
                let agent_id = event_agent_id(&event).to_owned();
                if disconnected_agents.contains(agent_id.as_str()) {
                    continue;
                }
                last_seen.update(|current| {
                    let mut next = current.clone();
                    next.insert(agent_id.clone(), Instant::now());
                    next
                });
                if matches!(event, AgentEvent::Heartbeat { .. }) {
                    continue;
                }
                if matches!(event, AgentEvent::Disconnected { .. }) {
                    disconnected_agents.insert(agent_id.clone());
                    agent_pool.update(|current| {
                        let mut next = current.clone();
                        next.remove(agent_id.as_str());
                        next
                    });
                    last_seen.update(|current| {
                        let mut next = current.clone();
                        next.remove(agent_id.as_str());
                        next
                    });
                }
            }
            _ = heartbeat_interval.tick() => {
                let now = Instant::now();
                let seen = last_seen.load();
                let timed_out: Vec<String> = seen
                    .iter()
                    .filter(|(_, last)| now.duration_since(**last) > heartbeat_timeout)
                    .map(|(agent_id, _)| agent_id.clone())
                    .collect();
                if !timed_out.is_empty() {
                    for agent_id in timed_out {
                        if disconnected_agents.insert(agent_id.clone()) {
                            agent_pool.update(|current| {
                                let mut next = current.clone();
                                next.remove(&agent_id);
                                next
                            });
                            last_seen.update(|current| {
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
    agent_pool: Arc<ArcShift<HashMap<String, ManualAgent>>>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    last_seen: Arc<ArcShift<HashMap<String, Instant>>>,
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
    agent_pool: Arc<ArcShift<HashMap<String, ManualAgent>>>,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    last_seen: Arc<ArcShift<HashMap<String, Instant>>>,
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

    last_seen.update(|current| {
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
                        message: err,
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
                            message: err,
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
    agent_pool.update(|current| {
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

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

async fn read_http_request(socket: &mut TcpStream) -> Result<HttpRequest, ControlError> {
    const MAX_REQUEST_BYTES: usize = 1024 * 1024;
    let mut buffer: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let header_end;

    loop {
        let bytes = socket
            .read(&mut chunk)
            .await
            .map_err(|err| ControlError::new(400, format!("Failed to read request: {}", err)))?;
        if bytes == 0 {
            return Err(ControlError::new(400, "Empty request"));
        }
        let read_slice = chunk
            .get(..bytes)
            .ok_or_else(|| ControlError::new(400, "Invalid read length"))?;
        buffer.extend_from_slice(read_slice);
        if buffer.len() > MAX_REQUEST_BYTES {
            return Err(ControlError::new(413, "Request too large"));
        }
        if let Some(pos) = find_header_end(&buffer) {
            header_end = pos;
            break;
        }
    }

    let header_bytes = buffer
        .get(..header_end)
        .ok_or_else(|| ControlError::new(400, "Malformed request headers"))?;
    let header_text = std::str::from_utf8(header_bytes)
        .map_err(|err| ControlError::new(400, format!("Invalid request encoding: {}", err)))?;
    let mut lines = header_text.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| ControlError::new(400, "Missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| ControlError::new(400, "Missing HTTP method"))?;
    let path = parts
        .next()
        .ok_or_else(|| ControlError::new(400, "Missing request path"))?;

    let mut headers = HashMap::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            return Err(ControlError::new(400, "Malformed header"));
        };
        headers.insert(key.trim().to_ascii_lowercase(), value.trim().to_owned());
    }

    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let body_start = header_end
        .checked_add(4)
        .ok_or_else(|| ControlError::new(400, "Malformed request headers"))?;
    let mut body = buffer.get(body_start..).unwrap_or_default().to_vec();
    while body.len() < content_length {
        let bytes = socket
            .read(&mut chunk)
            .await
            .map_err(|err| ControlError::new(400, format!("Failed to read body: {}", err)))?;
        if bytes == 0 {
            break;
        }
        let read_slice = chunk
            .get(..bytes)
            .ok_or_else(|| ControlError::new(400, "Invalid read length"))?;
        body.extend_from_slice(read_slice);
        if body.len() > MAX_REQUEST_BYTES {
            return Err(ControlError::new(413, "Request body too large"));
        }
    }
    body.truncate(content_length);

    Ok(HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

const fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        409 => "Conflict",
        413 => "Payload Too Large",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "OK",
    }
}

async fn write_json_response(
    socket: &mut TcpStream,
    status: u16,
    response: &ControlResponse,
) -> Result<(), String> {
    let body = serde_json::to_vec(response)
        .map_err(|err| format!("Failed to encode response: {}", err))?;
    write_response(socket, status, &body).await
}

async fn write_error_response(
    socket: &mut TcpStream,
    status: u16,
    message: &str,
) -> Result<(), String> {
    #[derive(Serialize)]
    struct ErrorResponse<'msg> {
        error: &'msg str,
    }
    let body = serde_json::to_vec(&ErrorResponse { error: message })
        .map_err(|err| format!("Failed to encode error: {}", err))?;
    write_response(socket, status, &body).await
}

async fn write_response(socket: &mut TcpStream, status: u16, body: &[u8]) -> Result<(), String> {
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        status_text(status),
        body.len()
    );
    socket
        .write_all(response.as_bytes())
        .await
        .map_err(|err| format!("Failed to write response: {}", err))?;
    socket
        .write_all(body)
        .await
        .map_err(|err| format!("Failed to write response body: {}", err))
}

async fn start_manual_run(
    args: &TesterArgs,
    request: &ControlStartRequest,
    scenario_state: &mut ScenarioState,
    agent_pool: &Arc<ArcShift<HashMap<String, ManualAgent>>>,
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

    let agents = agent_pool.load().values().cloned().collect::<Vec<_>>();
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
        agent_pool.update(|current| {
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
        let (shutdown_tx, _) = broadcast::channel::<u16>(1);
        let (ui_tx, _) = watch::channel(UiData {
            target_duration,
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
        let scenario = parse_scenario(config, args).map_err(|err| ControlError::new(400, err))?;
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
        let scenario = parse_scenario(config, args).map_err(|err| ControlError::new(400, err))?;
        return Ok(Some(scenario));
    }

    Ok(scenario_state.default.clone())
}

async fn request_stop(
    state: &mut ManualRunState,
    agent_pool: &Arc<ArcShift<HashMap<String, ManualAgent>>>,
) {
    let run_id = state.run_id.clone();
    let agents = agent_pool.load().values().cloned().collect::<Vec<_>>();
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
        agent_pool.update(|current| {
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

async fn finalize_manual_run(args: &TesterArgs, state: &mut ManualRunState) -> Result<(), String> {
    if state.agent_states.is_empty() {
        state
            .runtime_errors
            .push("No successful agent reports received.".to_owned());
    } else if let Ok((summary, merged_hist)) = aggregate_snapshots(&state.agent_states) {
        let (p50, p90, p99) = merged_hist.percentiles();
        let stats = compute_summary_stats(&summary);
        let mut charts_written = false;
        if state.charts_enabled {
            match write_aggregated_charts(&state.aggregated_samples, args).await {
                Ok(written) => charts_written = written,
                Err(err) => state.runtime_errors.push(err),
            }
        }

        print_summary(&summary, p50, p90, p99, args, charts_written);

        if let Some(sinks) = args.sinks.as_ref() {
            let sink_stats = SinkStats {
                duration: summary.duration,
                total_requests: summary.total_requests,
                successful_requests: summary.successful_requests,
                error_requests: summary.error_requests,
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
        drop(shutdown_tx.send(1));
    }

    if !state.runtime_errors.is_empty() {
        eprintln!("Runtime errors:");
        for err in &state.runtime_errors {
            eprintln!("- {}", err);
        }
        return Err("Distributed run completed with errors.".to_owned());
    }

    Ok(())
}

fn handle_agent_event(
    event: AgentEvent,
    expected_run_id: &str,
    pending_agents: &mut HashSet<String>,
    agent_states: &mut HashMap<String, AgentSnapshot>,
    runtime_errors: &mut Vec<String>,
    sink_dirty: &mut bool,
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
                    agent_states.insert(
                        agent_id,
                        AgentSnapshot {
                            summary: message.summary,
                            histogram,
                        },
                    );
                    *sink_dirty = true;
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
                    agent_states.insert(
                        agent_id.clone(),
                        AgentSnapshot {
                            summary: message.summary,
                            histogram,
                        },
                    );
                    *sink_dirty = true;
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

const fn event_agent_id(event: &AgentEvent) -> &str {
    match event {
        AgentEvent::Heartbeat { agent_id }
        | AgentEvent::Stream { agent_id, .. }
        | AgentEvent::Report { agent_id, .. }
        | AgentEvent::Error { agent_id, .. }
        | AgentEvent::Disconnected { agent_id, .. } => agent_id.as_str(),
    }
}

fn update_ui(
    ui_tx: &watch::Sender<UiData>,
    args: &TesterArgs,
    agent_states: &HashMap<String, AgentSnapshot>,
    latency_window: &mut VecDeque<(u64, u64)>,
) {
    let Ok((summary, merged_hist)) = aggregate_snapshots(agent_states) else {
        return;
    };
    let (p50, p90, p99) = merged_hist.percentiles();
    let stats = compute_summary_stats(&summary);
    let elapsed_ms = summary.duration.as_millis().min(u128::from(u64::MAX)) as u64;
    let window_start = elapsed_ms.saturating_sub(10_000);
    latency_window.push_back((elapsed_ms, summary.avg_latency_ms));
    while latency_window
        .front()
        .is_some_and(|(ts, _)| *ts < window_start)
    {
        latency_window.pop_front();
    }
    let latencies: Vec<(u64, u64)> = latency_window.iter().copied().collect();

    drop(ui_tx.send(UiData {
        elapsed_time: summary.duration,
        target_duration: Duration::from_secs(args.target_duration.get()),
        current_requests: summary.total_requests,
        successful_requests: summary.successful_requests,
        latencies,
        p50,
        p90,
        p99,
        rps: stats.avg_rps_x100 / 100,
        rpm: stats.avg_rpm_x100 / 100,
    }));
}

async fn write_streaming_sinks(
    args: &TesterArgs,
    agent_states: &HashMap<String, AgentSnapshot>,
) -> Result<(), String> {
    if agent_states.is_empty() {
        return Ok(());
    }
    let (summary, merged_hist) = aggregate_snapshots(agent_states)?;
    let (p50, p90, p99) = merged_hist.percentiles();
    let stats = compute_summary_stats(&summary);
    if let Some(sinks) = args.sinks.as_ref() {
        let sink_stats = SinkStats {
            duration: summary.duration,
            total_requests: summary.total_requests,
            successful_requests: summary.successful_requests,
            error_requests: summary.error_requests,
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
        crate::sinks::writers::write_sinks(sinks, &sink_stats)
            .await
            .map_err(|err| format!("Sinks: {}", err))?;
    }
    Ok(())
}

fn aggregate_snapshots(
    agent_states: &HashMap<String, AgentSnapshot>,
) -> Result<(crate::metrics::MetricsSummary, LatencyHistogram), String> {
    let mut summaries = Vec::with_capacity(agent_states.len());
    let mut merged_hist = LatencyHistogram::new()?;
    for snapshot in agent_states.values() {
        summaries.push(snapshot.summary.clone());
        merged_hist.merge(&snapshot.histogram)?;
    }
    Ok((merge_summaries(&summaries), merged_hist))
}

fn record_aggregated_sample(
    samples: &mut Vec<AggregatedMetricSample>,
    agent_states: &HashMap<String, AgentSnapshot>,
) {
    let Ok((summary, merged_hist)) = aggregate_snapshots(agent_states) else {
        return;
    };
    let (p50, p90, p99) = merged_hist.percentiles();
    let elapsed_ms = u64::try_from(summary.duration.as_millis()).unwrap_or(u64::MAX);
    let sample = AggregatedMetricSample {
        elapsed_ms,
        total_requests: summary.total_requests,
        successful_requests: summary.successful_requests,
        error_requests: summary.error_requests,
        avg_latency_ms: summary.avg_latency_ms,
        p50_latency_ms: p50,
        p90_latency_ms: p90,
        p99_latency_ms: p99,
    };

    if let Some(last) = samples.last()
        && last.elapsed_ms == sample.elapsed_ms
        && last.total_requests == sample.total_requests
        && last.successful_requests == sample.successful_requests
        && last.error_requests == sample.error_requests
        && last.avg_latency_ms == sample.avg_latency_ms
        && last.p50_latency_ms == sample.p50_latency_ms
        && last.p90_latency_ms == sample.p90_latency_ms
        && last.p99_latency_ms == sample.p99_latency_ms
    {
        return;
    }

    samples.push(sample);
}

async fn write_aggregated_charts(
    samples: &[AggregatedMetricSample],
    args: &TesterArgs,
) -> Result<bool, String> {
    if args.no_charts {
        return Ok(false);
    }
    if samples.len() < 2 {
        return Ok(false);
    }
    charts::plot_aggregated_metrics(samples, args)
        .await
        .map_err(|err| format!("Charts: {}", err))?;
    Ok(true)
}

fn resolve_sink_interval(config: Option<&SinksConfig>) -> Duration {
    match config.and_then(|value| value.update_interval_ms) {
        Some(0) => {
            warn!(
                "sinks.update_interval_ms must be > 0; using default {}ms",
                DEFAULT_SINK_INTERVAL.as_millis()
            );
            DEFAULT_SINK_INTERVAL
        }
        Some(ms) => Duration::from_millis(ms),
        None => DEFAULT_SINK_INTERVAL,
    }
}

fn resolve_agent_wait_timeout(args: &TesterArgs) -> Option<Duration> {
    args.agent_wait_timeout_ms
        .map(|value| Duration::from_millis(value.get()))
}

fn resolve_heartbeat_check_interval(timeout: Duration) -> Duration {
    let timeout_ms = timeout.as_millis();
    let mut interval_ms = timeout_ms.saturating_div(2);
    if interval_ms < 200 {
        interval_ms = timeout_ms.max(1);
    }
    Duration::from_millis(u64::try_from(interval_ms).unwrap_or(1))
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
    agent_pool: &Arc<ArcShift<HashMap<String, ManualAgent>>>,
    event_rx: &mut mpsc::UnboundedReceiver<AgentEvent>,
    last_seen: &Arc<ArcShift<HashMap<String, Instant>>>,
    disconnected_agents: &mut HashSet<String>,
) -> Result<(), ControlError> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .unwrap_or_else(Instant::now);
    loop {
        let available = agent_pool.load().len();
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
                last_seen.update(|current| {
                    let mut next = current.clone();
                    next.insert(agent_id.clone(), Instant::now());
                    next
                });
                if matches!(event, AgentEvent::Heartbeat { .. }) {
                    continue;
                }
                if let AgentEvent::Disconnected { .. } = event {
                    disconnected_agents.insert(agent_id.clone());
                    agent_pool.update(|current| {
                        let mut next = current.clone();
                        next.remove(agent_id.as_str());
                        next
                    });
                    last_seen.update(|current| {
                        let mut next = current.clone();
                        next.remove(agent_id.as_str());
                        next
                    });
                }
            }
        }
    }
}

async fn accept_agent(stream: TcpStream, auth_token: Option<&str>) -> Result<AgentConn, String> {
    let peer = stream
        .peer_addr()
        .map(|addr| addr.to_string())
        .unwrap_or_else(|_| "<unknown>".to_owned());
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let hello = match timeout(AGENT_HELLO_TIMEOUT, read_message(&mut reader)).await {
        Ok(result) => match result? {
            WireMessage::Hello(message) => message,
            WireMessage::Error(message) => {
                return Err(format!("Agent error: {}", message.message));
            }
            WireMessage::Config(_)
            | WireMessage::Start(_)
            | WireMessage::Stop(_)
            | WireMessage::Heartbeat(_)
            | WireMessage::Report(_)
            | WireMessage::Stream(_) => return Err("Expected hello from agent.".to_owned()),
        },
        Err(_) => {
            return Err("Timed out waiting for agent hello.".to_owned());
        }
    };

    if let Some(expected) = auth_token {
        let provided = hello.auth_token.as_deref().unwrap_or("");
        if provided != expected {
            let mut writer = write_half;
            send_message(
                &mut writer,
                &WireMessage::Error(ErrorMessage {
                    message: "Invalid auth token.".to_owned(),
                }),
            )
            .await?;
            return Err("Invalid auth token.".to_owned());
        }
    }

    info!(
        "Accepted agent {} from {} (weight={})",
        hello.agent_id, peer, hello.weight
    );
    Ok(AgentConn {
        agent_id: hello.agent_id,
        weight: hello.weight.max(1),
        reader,
        writer: write_half,
    })
}

fn compute_weights(agents: &[AgentConn]) -> Vec<u64> {
    agents.iter().map(|agent| agent.weight).collect()
}

fn apply_load_share(
    agent_args: &mut super::protocol::WireArgs,
    args: &TesterArgs,
    weights: &[u64],
    idx: usize,
) {
    let use_weights = weights.iter().any(|value| *value != 1);
    let share_weights: Vec<u64> = if use_weights {
        weights.to_vec()
    } else {
        vec![1; weights.len()]
    };

    if let Some(profile) = args.load_profile.as_ref() {
        let split = split_load_profile(profile, &share_weights);
        if let Some(agent_profile) = split.get(idx) {
            agent_args.load_profile = Some(agent_profile.clone());
            agent_args.rate_limit = None;
        }
        return;
    }

    if let Some(rate) = args.rate_limit.map(u64::from) {
        let shares = split_total(rate, &share_weights);
        if let Some(share) = shares.get(idx) {
            agent_args.rate_limit = Some(*share);
        }
    }
}

fn split_load_profile(
    profile: &LoadProfile,
    weights: &[u64],
) -> Vec<super::protocol::WireLoadProfile> {
    let initial_shares = split_total(profile.initial_rpm, weights);
    let mut stage_shares: Vec<Vec<u64>> = Vec::new();
    for stage in &profile.stages {
        stage_shares.push(split_total(stage.target_rpm, weights));
    }

    let mut per_agent = Vec::with_capacity(weights.len());
    for idx in 0..weights.len() {
        let mut stages = Vec::with_capacity(profile.stages.len());
        for (stage_idx, stage) in profile.stages.iter().enumerate() {
            let share = stage_shares
                .get(stage_idx)
                .and_then(|values| values.get(idx))
                .copied()
                .unwrap_or(0);
            stages.push(super::protocol::WireLoadStage {
                duration_secs: stage.duration.as_secs(),
                target_rpm: share,
            });
        }
        let initial_rpm = initial_shares.get(idx).copied().unwrap_or(0);
        per_agent.push(super::protocol::WireLoadProfile {
            initial_rpm,
            stages,
        });
    }

    per_agent
}

fn split_total(total: u64, weights: &[u64]) -> Vec<u64> {
    if weights.is_empty() {
        return Vec::new();
    }
    let total_weight: u128 = weights.iter().map(|value| u128::from(*value)).sum();
    if total_weight == 0 {
        return vec![0; weights.len()];
    }

    let mut shares = vec![0u64; weights.len()];
    let mut remainder = u128::from(total);
    for (idx, weight) in weights.iter().enumerate() {
        let share = u128::from(total)
            .saturating_mul(u128::from(*weight))
            .checked_div(total_weight)
            .unwrap_or(0);
        if let Some(slot) = shares.get_mut(idx) {
            *slot = u64::try_from(share).unwrap_or(u64::MAX);
        }
        remainder = remainder.saturating_sub(share);
    }

    let mut idx = 0usize;
    while remainder > 0 {
        if let Some(value) = shares.get_mut(idx) {
            *value = value.saturating_add(1);
        }
        remainder = remainder.saturating_sub(1);
        idx = idx.saturating_add(1);
        if idx >= shares.len() {
            idx = 0;
        }
    }

    shares
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_hist(values: &[u64]) -> Result<LatencyHistogram, String> {
        let mut hist = LatencyHistogram::new()?;
        for value in values {
            hist.record(*value)?;
        }
        Ok(hist)
    }

    fn base_args() -> Result<TesterArgs, String> {
        Ok(TesterArgs {
            method: crate::args::HttpMethod::Get,
            url: Some("http://localhost".to_owned()),
            headers: vec![],
            data: String::new(),
            target_duration: crate::args::PositiveU64::try_from(1)?,
            expected_status_code: 200,
            request_timeout: Duration::from_secs(2),
            charts_path: "./charts".to_owned(),
            no_charts: true,
            verbose: false,
            config: None,
            tmp_path: "./tmp".to_owned(),
            load_profile: None,
            controller_listen: None,
            controller_mode: crate::args::ControllerMode::Auto,
            control_listen: None,
            control_auth_token: None,
            agent_join: None,
            auth_token: None,
            agent_id: None,
            agent_weight: crate::args::PositiveU64::try_from(1)?,
            min_agents: crate::args::PositiveUsize::try_from(1)?,
            agent_wait_timeout_ms: None,
            agent_standby: false,
            agent_reconnect_ms: crate::args::PositiveU64::try_from(1000)?,
            agent_heartbeat_interval_ms: crate::args::PositiveU64::try_from(1000)?,
            agent_heartbeat_timeout_ms: crate::args::PositiveU64::try_from(3000)?,
            keep_tmp: false,
            warmup: None,
            export_csv: None,
            export_json: None,
            log_shards: crate::args::PositiveUsize::try_from(1)?,
            no_ui: true,
            summary: false,
            tls_min: None,
            tls_max: None,
            http2: false,
            http3: false,
            alpn: vec![],
            proxy_url: None,
            max_tasks: crate::args::PositiveUsize::try_from(1)?,
            spawn_rate_per_tick: crate::args::PositiveUsize::try_from(1)?,
            tick_interval: crate::args::PositiveU64::try_from(100)?,
            rate_limit: None,
            metrics_range: None,
            metrics_max: crate::args::PositiveUsize::try_from(1_000)?,
            scenario: None,
            script: None,
            install_service: false,
            uninstall_service: false,
            service_name: None,
            sinks: None,
            distributed_silent: false,
            distributed_stream_summaries: false,
            distributed_stream_interval_ms: None,
        })
    }

    #[test]
    fn aggregate_snapshots_merges_summary() -> Result<(), String> {
        let summary_a = WireSummary {
            duration_ms: 1000,
            total_requests: 10,
            successful_requests: 9,
            error_requests: 1,
            min_latency_ms: 10,
            max_latency_ms: 50,
            latency_sum_ms: 1000,
        };
        let summary_b = WireSummary {
            duration_ms: 1500,
            total_requests: 20,
            successful_requests: 19,
            error_requests: 1,
            min_latency_ms: 5,
            max_latency_ms: 40,
            latency_sum_ms: 600,
        };

        let hist_a = build_hist(&[10, 20])?;
        let hist_b = build_hist(&[30, 40])?;

        let mut agent_states = HashMap::new();
        agent_states.insert(
            "a".to_owned(),
            AgentSnapshot {
                summary: summary_a,
                histogram: hist_a,
            },
        );
        agent_states.insert(
            "b".to_owned(),
            AgentSnapshot {
                summary: summary_b,
                histogram: hist_b,
            },
        );

        let (summary, merged_hist) = aggregate_snapshots(&agent_states)?;
        if summary.total_requests != 30 {
            return Err(format!(
                "Unexpected total_requests: {}",
                summary.total_requests
            ));
        }
        if summary.successful_requests != 28 {
            return Err(format!(
                "Unexpected successful_requests: {}",
                summary.successful_requests
            ));
        }
        if summary.error_requests != 2 {
            return Err(format!(
                "Unexpected error_requests: {}",
                summary.error_requests
            ));
        }
        if summary.min_latency_ms != 5 {
            return Err(format!(
                "Unexpected min_latency_ms: {}",
                summary.min_latency_ms
            ));
        }
        if summary.max_latency_ms != 50 {
            return Err(format!(
                "Unexpected max_latency_ms: {}",
                summary.max_latency_ms
            ));
        }
        if summary.avg_latency_ms != 53 {
            return Err(format!(
                "Unexpected avg_latency_ms: {}",
                summary.avg_latency_ms
            ));
        }
        if merged_hist.count() != 4 {
            return Err(format!(
                "Unexpected merged histogram count: {}",
                merged_hist.count()
            ));
        }
        Ok(())
    }

    #[test]
    fn update_ui_emits_aggregated_stats() -> Result<(), String> {
        let args = base_args()?;
        let (ui_tx, ui_rx) = watch::channel(UiData::default());

        let summary = WireSummary {
            duration_ms: 1000,
            total_requests: 10,
            successful_requests: 9,
            error_requests: 1,
            min_latency_ms: 10,
            max_latency_ms: 50,
            latency_sum_ms: 1000,
        };
        let hist = build_hist(&[10, 20, 30])?;

        let mut agent_states = HashMap::new();
        agent_states.insert(
            "a".to_owned(),
            AgentSnapshot {
                summary,
                histogram: hist,
            },
        );

        let mut latency_window = VecDeque::new();
        update_ui(&ui_tx, &args, &agent_states, &mut latency_window);

        let snapshot = ui_rx.borrow().clone();
        if snapshot.current_requests != 10 {
            return Err(format!(
                "Unexpected current_requests: {}",
                snapshot.current_requests
            ));
        }
        if snapshot.successful_requests != 9 {
            return Err(format!(
                "Unexpected successful_requests: {}",
                snapshot.successful_requests
            ));
        }
        if snapshot.p50 == 0 {
            return Err("Expected non-zero p50 latency".to_owned());
        }
        if snapshot.rps == 0 {
            return Err("Expected non-zero rps".to_owned());
        }
        Ok(())
    }
}
