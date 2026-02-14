use std::time::Duration;

use tokio::time::Instant;
use tracing::{debug, info};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};

use super::super::agent::{AgentConn, accept_agent};
use super::super::load::apply_load_share;
use super::super::output::{DistributedOutputState, setup_output_state};
use super::super::shared::{DEFAULT_START_AFTER_MS, REPORT_GRACE_SECS, resolve_agent_wait_timeout};
use crate::distributed::protocol::{ConfigMessage, StartMessage, WireMessage, send_message};
use crate::distributed::utils::build_run_id;
use crate::distributed::wire::build_wire_args;

pub(super) struct AutoRunSetup {
    pub(super) run_id: String,
    pub(super) agents: Vec<AgentConn>,
    pub(super) output_state: DistributedOutputState,
    pub(super) heartbeat_timeout: Duration,
    pub(super) report_deadline: Instant,
}

pub(super) async fn prepare_auto_run(args: &TesterArgs) -> AppResult<AutoRunSetup> {
    let listener = bind_listener(args).await?;
    let mut agents = accept_agents(args, listener).await?;

    let run_id = build_run_id();
    let weights = compute_weights(&agents);
    let base_args = build_wire_args(args);
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

    for agent in &mut agents {
        debug!(
            "Sending start to agent {} for run {}",
            agent.agent_id, run_id
        );
        send_message(
            &mut agent.writer,
            &WireMessage::Start(StartMessage {
                run_id: run_id.clone(),
                start_after_ms: DEFAULT_START_AFTER_MS,
            }),
        )
        .await?;
    }

    let output_state = setup_output_state(args);
    let heartbeat_timeout = Duration::from_millis(args.agent_heartbeat_timeout_ms.get());
    let report_deadline = Instant::now()
        .checked_add(
            Duration::from_secs(args.target_duration.get())
                .saturating_add(Duration::from_secs(REPORT_GRACE_SECS)),
        )
        .unwrap_or_else(Instant::now);

    Ok(AutoRunSetup {
        run_id,
        agents,
        output_state,
        heartbeat_timeout,
        report_deadline,
    })
}

async fn bind_listener(args: &TesterArgs) -> AppResult<tokio::net::TcpListener> {
    let listen = args
        .controller_listen
        .as_deref()
        .ok_or_else(|| AppError::distributed(DistributedError::MissingControllerListen))?;

    let listener = tokio::net::TcpListener::bind(listen).await.map_err(|err| {
        AppError::distributed(DistributedError::Bind {
            addr: listen.to_owned(),
            source: err,
        })
    })?;
    info!(
        "Controller listening on {} (auto mode, min_agents={})",
        listen,
        args.min_agents.get()
    );
    Ok(listener)
}

async fn accept_agents(
    args: &TesterArgs,
    listener: tokio::net::TcpListener,
) -> AppResult<Vec<AgentConn>> {
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
                    return Err(AppError::distributed(DistributedError::AgentWaitTimeout {
                        expected: args.min_agents.get(),
                        actual: agents.len(),
                    }));
                }
                let remaining = deadline.duration_since(now);
                match tokio::time::timeout(remaining, listener.accept()).await {
                    Ok(result) => {
                        let (stream, _) = result.map_err(|err| {
                            AppError::distributed(DistributedError::Io {
                                context: "accept agent",
                                source: err,
                            })
                        })?;
                        stream
                    }
                    Err(_) => {
                        return Err(AppError::distributed(DistributedError::AgentWaitTimeout {
                            expected: args.min_agents.get(),
                            actual: agents.len(),
                        }));
                    }
                }
            }
            None => {
                let (stream, _) = listener.accept().await.map_err(|err| {
                    AppError::distributed(DistributedError::Io {
                        context: "accept agent",
                        source: err,
                    })
                })?;
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
    Ok(agents)
}

fn compute_weights(agents: &[AgentConn]) -> Vec<u64> {
    agents.iter().map(|agent| agent.weight).collect()
}
