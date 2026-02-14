use std::time::Duration;

use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;
use tracing::{debug, info};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};

use super::command::AgentCommand;
use super::run_exec::{AgentLocalRunPort, run_agent_run};
use super::wire::{build_agent_id, build_hello, send_wire};
use crate::distributed::protocol::{HeartbeatMessage, WireMessage, read_message, send_message};
use crate::distributed::utils::current_time_ms;

pub(super) async fn run_agent_session<TLocalRunPort>(
    base_args: &TesterArgs,
    local_run_port: &TLocalRunPort,
) -> AppResult<()>
where
    TLocalRunPort: AgentLocalRunPort + Sync,
{
    let join = base_args.agent_join.as_deref().ok_or_else(|| {
        AppError::distributed(DistributedError::MissingOption {
            option: "--agent-join",
        })
    })?;

    info!("Connecting to controller {}", join);
    let stream = TcpStream::connect(join).await.map_err(|err| {
        AppError::distributed(DistributedError::Connection {
            addr: join.to_owned(),
            source: err,
        })
    })?;
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
                WireMessage::Error(message) => {
                    AgentCommand::Error(AppError::distributed(DistributedError::Remote {
                        message: message.message,
                    }))
                }
                WireMessage::Heartbeat(_) => continue,
                WireMessage::Hello(_) | WireMessage::Stream(_) | WireMessage::Report(_) => {
                    AgentCommand::Error(AppError::distributed(
                        DistributedError::UnexpectedMessageFromController,
                    ))
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
            break Err(AppError::distributed(DistributedError::RunIdMismatch {
                expected: config.run_id.clone(),
                actual: start.run_id.clone(),
            }));
        }

        if start.start_after_ms > 0 {
            tokio::time::sleep(Duration::from_millis(start.start_after_ms)).await;
        }

        let run_result = run_agent_run(
            base_args,
            config,
            agent_id.clone(),
            &out_tx,
            &mut cmd_rx,
            local_run_port,
        )
        .await;

        if let Err(err) = run_result {
            if !base_args.agent_standby {
                break Err(err);
            }
            tracing::warn!("Agent run error: {}", err);
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
) -> AppResult<crate::distributed::protocol::ConfigMessage> {
    while let Some(command) = cmd_rx.recv().await {
        match command {
            AgentCommand::Config(config) => return Ok(*config),
            AgentCommand::Disconnected(err) => return Err(err),
            AgentCommand::Error(err) => return Err(err),
            AgentCommand::Stop(_) => continue,
            AgentCommand::Start(_) => {
                return Err(AppError::distributed(DistributedError::StartBeforeConfig));
            }
        }
    }
    Err(AppError::distributed(
        DistributedError::ControllerConnectionClosed,
    ))
}

async fn wait_for_start(
    cmd_rx: &mut mpsc::UnboundedReceiver<AgentCommand>,
    run_id: &str,
) -> AppResult<crate::distributed::protocol::StartMessage> {
    while let Some(command) = cmd_rx.recv().await {
        match command {
            AgentCommand::Start(start) => {
                if start.run_id != run_id {
                    return Err(AppError::distributed(DistributedError::RunIdMismatch {
                        expected: run_id.to_owned(),
                        actual: start.run_id,
                    }));
                }
                return Ok(start);
            }
            AgentCommand::Stop(stop) => {
                if stop.run_id == run_id {
                    return Err(AppError::distributed(DistributedError::StopBeforeStart));
                }
            }
            AgentCommand::Config(_) => {
                return Err(AppError::distributed(
                    DistributedError::ConfigWhileWaitingForStart,
                ));
            }
            AgentCommand::Disconnected(err) => return Err(err),
            AgentCommand::Error(err) => return Err(err),
        }
    }
    Err(AppError::distributed(
        DistributedError::ControllerConnectionClosed,
    ))
}
