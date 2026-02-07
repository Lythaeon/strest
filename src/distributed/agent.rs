use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, watch};
use tokio::time::MissedTickBehavior;
use tracing::{debug, info, warn};

use crate::args::TesterArgs;
use crate::metrics::StreamSnapshot;

use super::protocol::{
    ConfigMessage, ErrorMessage, HeartbeatMessage, HelloMessage, ReportMessage, StartMessage,
    StopMessage, StreamMessage, WireMessage, WireSummary, read_message, send_message,
};
use super::utils::{current_time_ms, duration_to_ms};
use super::wire::apply_wire_args;

enum AgentCommand {
    Config(Box<ConfigMessage>),
    Start(StartMessage),
    Stop(StopMessage),
    Error(String),
    Disconnected(String),
}

/// Runs the distributed agent loop.
///
/// # Errors
///
/// Returns an error if the agent cannot connect, negotiate, or execute a run.
pub async fn run_agent(args: TesterArgs) -> Result<(), String> {
    let standby = args.agent_standby;
    let reconnect_delay = Duration::from_millis(args.agent_reconnect_ms.get());
    info!(
        "Agent starting (standby={}, reconnect={}ms)",
        standby,
        reconnect_delay.as_millis()
    );

    loop {
        let result = run_agent_session(&args).await;
        match result {
            Ok(()) => {
                if !standby {
                    return Ok(());
                }
            }
            Err(err) => {
                if !standby {
                    return Err(err);
                }
                warn!("Agent session error: {}", err);
            }
        }
        tokio::time::sleep(reconnect_delay).await;
    }
}

async fn run_agent_session(base_args: &TesterArgs) -> Result<(), String> {
    let join = base_args
        .agent_join
        .as_deref()
        .ok_or_else(|| "Missing --agent-join.".to_owned())?;

    info!("Connecting to controller {}", join);
    let stream = TcpStream::connect(join)
        .await
        .map_err(|err| format!("Failed to connect to controller {}: {}", join, err))?;
    info!("Connected to controller {}", join);
    let (read_half, mut write_half) = stream.into_split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<WireMessage>();
    let writer_handle = tokio::spawn(async move {
        while let Some(message) = out_rx.recv().await {
            if send_message(&mut write_half, &message).await.is_err() {
                break;
            }
        }
    });
    let mut reader = BufReader::new(read_half);

    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<AgentCommand>();
    let reader_handle = tokio::spawn(async move {
        loop {
            let message = match read_message(&mut reader).await {
                Ok(message) => message,
                Err(err) => {
                    if cmd_tx.send(AgentCommand::Disconnected(err)).is_err() {
                        break;
                    }
                    break;
                }
            };

            let command = match message {
                WireMessage::Config(message) => AgentCommand::Config(message),
                WireMessage::Start(message) => AgentCommand::Start(message),
                WireMessage::Stop(message) => AgentCommand::Stop(message),
                WireMessage::Error(message) => AgentCommand::Error(message.message),
                WireMessage::Heartbeat(_) => continue,
                WireMessage::Hello(_) | WireMessage::Stream(_) | WireMessage::Report(_) => {
                    AgentCommand::Error("Unexpected message from controller.".to_owned())
                }
            };

            if cmd_tx.send(command).is_err() {
                break;
            }
        }
    });

    let agent_id = build_agent_id(base_args);
    let hello = build_hello(base_args, &agent_id);
    send_wire(&out_tx, WireMessage::Hello(hello))?;
    debug!("Sent hello as {}", agent_id);

    let heartbeat_interval = Duration::from_millis(base_args.agent_heartbeat_interval_ms.get());
    let heartbeat_tx = out_tx.clone();
    let heartbeat_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(heartbeat_interval);
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let sent_at_ms = u64::try_from(current_time_ms()).unwrap_or(u64::MAX);
            let message = WireMessage::Heartbeat(HeartbeatMessage { sent_at_ms });
            if send_wire(&heartbeat_tx, message).is_err() {
                break;
            }
        }
    });

    let session_result = loop {
        let config = match wait_for_config(&mut cmd_rx).await {
            Ok(config) => config,
            Err(err) => break Err(err),
        };
        info!("Received config for run {}", config.run_id);
        let start = match wait_for_start(&mut cmd_rx, &config.run_id).await {
            Ok(start) => start,
            Err(err) => break Err(err),
        };
        info!(
            "Received start for run {} (start_after_ms={})",
            start.run_id, start.start_after_ms
        );

        if start.run_id != config.run_id {
            break Err("Start run_id did not match config.".to_owned());
        }

        if start.start_after_ms > 0 {
            tokio::time::sleep(Duration::from_millis(start.start_after_ms)).await;
        }

        let run_result =
            run_agent_run(base_args, config, agent_id.clone(), &out_tx, &mut cmd_rx).await;

        if let Err(err) = run_result {
            if !base_args.agent_standby {
                break Err(err);
            }
            warn!("Agent run error: {}", err);
        }

        if !base_args.agent_standby {
            break Ok(());
        }
    };

    heartbeat_handle.abort();
    drop(out_tx);
    if writer_handle.await.is_err() {
        // ignore
    }
    reader_handle.abort();
    session_result
}

async fn wait_for_config(
    cmd_rx: &mut mpsc::UnboundedReceiver<AgentCommand>,
) -> Result<ConfigMessage, String> {
    while let Some(command) = cmd_rx.recv().await {
        match command {
            AgentCommand::Config(config) => return Ok(*config),
            AgentCommand::Disconnected(err) => return Err(err),
            AgentCommand::Error(err) => return Err(err),
            AgentCommand::Stop(_) => continue,
            AgentCommand::Start(_) => {
                return Err("Received start before config.".to_owned());
            }
        }
    }
    Err("Controller connection closed.".to_owned())
}

async fn wait_for_start(
    cmd_rx: &mut mpsc::UnboundedReceiver<AgentCommand>,
    run_id: &str,
) -> Result<StartMessage, String> {
    while let Some(command) = cmd_rx.recv().await {
        match command {
            AgentCommand::Start(start) => {
                if start.run_id != run_id {
                    return Err("Start run_id did not match config.".to_owned());
                }
                return Ok(start);
            }
            AgentCommand::Stop(stop) => {
                if stop.run_id == run_id {
                    return Err("Received stop before start.".to_owned());
                }
            }
            AgentCommand::Config(_) => {
                return Err("Received config while waiting for start.".to_owned());
            }
            AgentCommand::Disconnected(err) => return Err(err),
            AgentCommand::Error(err) => return Err(err),
        }
    }
    Err("Controller connection closed.".to_owned())
}

async fn run_agent_run(
    base_args: &TesterArgs,
    config: ConfigMessage,
    agent_id: String,
    out_tx: &mpsc::UnboundedSender<WireMessage>,
    cmd_rx: &mut mpsc::UnboundedReceiver<AgentCommand>,
) -> Result<(), String> {
    let ConfigMessage { run_id, args } = config;
    let mut run_args = base_args.clone();
    if let Err(err) = apply_wire_args(&mut run_args, args) {
        let message = format!("Run {} config error: {}", run_id, err);
        if send_wire(out_tx, WireMessage::Error(ErrorMessage { message })).is_err() {
            // ignore
        }
        return Err(err);
    }
    info!("Starting run {} on agent {}", run_id, agent_id);
    run_args.distributed_silent = true;
    let streaming_enabled = run_args.distributed_stream_summaries;

    let (stop_tx, stop_rx) = watch::channel(false);

    let (stream_tx, mut stream_rx) = if streaming_enabled {
        let (stream_tx, stream_rx) = mpsc::unbounded_channel::<StreamSnapshot>();
        (Some(stream_tx), Some(stream_rx))
    } else {
        (None, None)
    };

    let mut run_future = Box::pin(crate::app::run_local(run_args, stream_tx, Some(stop_rx)));
    let mut abort_reason: Option<String> = None;

    let run_outcome = loop {
        tokio::select! {
            result = &mut run_future => {
                break result.map_err(|err| err.to_string());
            }
            command = cmd_rx.recv() => {
                match command {
                    Some(command) => match command {
                        AgentCommand::Stop(stop) => {
                            if stop.run_id == run_id {
                                match stop_tx.send(true) {
                                    Ok(()) | Err(_) => {}
                                }
                            }
                        }
                        AgentCommand::Disconnected(err) => {
                            abort_reason = Some(err);
                            match stop_tx.send(true) {
                                Ok(()) | Err(_) => {}
                            }
                        }
                        AgentCommand::Error(err) => {
                            abort_reason = Some(err);
                            match stop_tx.send(true) {
                                Ok(()) | Err(_) => {}
                            }
                        }
                        AgentCommand::Config(_) | AgentCommand::Start(_) => {
                            abort_reason = Some("Unexpected controller message while running.".to_owned());
                            match stop_tx.send(true) {
                                Ok(()) | Err(_) => {}
                            }
                        }
                    },
                    None => {
                        abort_reason = Some("Controller connection closed.".to_owned());
                        match stop_tx.send(true) {
                            Ok(()) | Err(_) => {}
                        }
                    }
                }
            }
            snapshot = async {
                if let Some(rx) = stream_rx.as_mut() {
                    rx.recv().await
                } else {
                    None
                }
            }, if streaming_enabled => {
                if let Some(snapshot) = snapshot {
                    let message = WireMessage::Stream(Box::new(StreamMessage {
                        run_id: run_id.clone(),
                        agent_id: agent_id.clone(),
                        summary: snapshot_to_wire_summary(&snapshot),
                        histogram_b64: snapshot.histogram_b64,
                    }));
                    if send_wire(out_tx, message).is_err() {
                        return Err("Controller connection closed.".to_owned());
                    }
                }
            }
        }

        if let Some(reason) = abort_reason.take() {
            match run_future.await {
                Ok(_) | Err(_) => {}
            }
            return Err(reason);
        }
    };

    let run_outcome = match run_outcome {
        Ok(outcome) => outcome,
        Err(err) => {
            let message = format!("Run {} failed: {}", run_id, err);
            if send_wire(out_tx, WireMessage::Error(ErrorMessage { message })).is_err() {
                // ignore
            }
            return Err(err);
        }
    };

    info!(
        "Run {} completed on agent {} (runtime_errors={})",
        run_id,
        agent_id,
        run_outcome.runtime_errors.len()
    );
    let histogram_b64 = run_outcome.histogram.encode_base64()?;
    let summary = WireSummary {
        duration_ms: duration_to_ms(run_outcome.summary.duration),
        total_requests: run_outcome.summary.total_requests,
        successful_requests: run_outcome.summary.successful_requests,
        error_requests: run_outcome.summary.error_requests,
        min_latency_ms: run_outcome.summary.min_latency_ms,
        max_latency_ms: run_outcome.summary.max_latency_ms,
        latency_sum_ms: run_outcome.latency_sum_ms,
    };

    let report = ReportMessage {
        run_id,
        agent_id,
        summary,
        histogram_b64,
        runtime_errors: run_outcome.runtime_errors,
    };
    debug!("Sending report for run {}", report.run_id);
    if send_wire(out_tx, WireMessage::Report(Box::new(report))).is_err() {
        return Err("Controller connection closed.".to_owned());
    }

    Ok(())
}

fn send_wire(tx: &mpsc::UnboundedSender<WireMessage>, message: WireMessage) -> Result<(), String> {
    tx.send(message)
        .map_err(|err| format!("Controller connection closed: {}", err))
}

fn snapshot_to_wire_summary(snapshot: &StreamSnapshot) -> WireSummary {
    WireSummary {
        duration_ms: duration_to_ms(snapshot.duration),
        total_requests: snapshot.total_requests,
        successful_requests: snapshot.successful_requests,
        error_requests: snapshot.error_requests,
        min_latency_ms: snapshot.min_latency_ms,
        max_latency_ms: snapshot.max_latency_ms,
        latency_sum_ms: snapshot.latency_sum_ms,
    }
}

fn build_hello(args: &TesterArgs, agent_id: &str) -> HelloMessage {
    HelloMessage {
        agent_id: agent_id.to_owned(),
        hostname: std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".to_owned()),
        cpu_cores: std::thread::available_parallelism()
            .map(|value| value.get())
            .unwrap_or(1),
        weight: args.agent_weight.get(),
        auth_token: args.auth_token.clone(),
    }
}

fn build_agent_id(args: &TesterArgs) -> String {
    if let Some(id) = args.agent_id.as_ref() {
        return id.clone();
    }
    let host = std::env::var("HOSTNAME").unwrap_or_else(|_| "agent".to_owned());
    format!("{}-{}", host, std::process::id())
}
