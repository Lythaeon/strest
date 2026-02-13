use std::collections::HashMap;

use arcshift::ArcShift;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio::time::Instant;
use tracing::info;

use super::super::agent::accept_agent;
use super::super::control::ControlCommand;
use super::super::shared::AgentEvent;
use super::control_http::handle_control_connection;
use super::state::ManualAgent;
use crate::distributed::protocol::{WireMessage, read_message, send_message};

pub(super) async fn accept_manual_agents(
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

pub(super) async fn register_manual_agent(
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
                    // Controller loop is gone; nothing left to notify.
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
                        // Controller loop is gone; nothing left to notify.
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

pub(super) async fn accept_control_connections(
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
