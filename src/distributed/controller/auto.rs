use std::collections::{HashMap, HashSet, VecDeque};
use std::io::IsTerminal;
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior};
use tracing::{debug, info};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};
use crate::metrics::AggregatedMetricSample;
use crate::ui::{model::UiData, render::setup_render_ui};

use super::super::protocol::{
    ConfigMessage, StartMessage, WireMessage, read_message, send_message,
};
use super::super::summary::{
    Percentiles, SummaryPercentiles, compute_summary_stats, print_summary,
};
use super::super::utils::build_run_id;
use super::super::wire::build_wire_args;
use super::agent::AgentConn;
use super::load::apply_load_share;
use super::shared::{
    AgentEvent, AgentSnapshot, DEFAULT_START_AFTER_MS, REPORT_GRACE_SECS, aggregate_snapshots,
    event_agent_id, handle_agent_event, record_aggregated_sample, resolve_agent_wait_timeout,
    resolve_heartbeat_check_interval, resolve_sink_interval, update_ui, write_aggregated_charts,
    write_streaming_sinks,
};

pub(super) async fn run_controller_auto(args: &TesterArgs) -> AppResult<()> {
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
        match super::agent::accept_agent(stream, args.auth_token.as_deref()).await {
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
    let mut runtime_errors: Vec<String> = Vec::new();
    let mut agent_states: HashMap<String, AgentSnapshot> = HashMap::new();
    let mut pending_agents: HashSet<String> =
        agents.iter().map(|agent| agent.agent_id.clone()).collect();
    let mut ui_latency_window: VecDeque<(u64, u64)> = VecDeque::new();
    let mut ui_rps_window: VecDeque<(u64, u64)> = VecDeque::new();
    let charts_enabled = !args.no_charts && args.distributed_stream_summaries;
    let mut aggregated_samples: Vec<AggregatedMetricSample> = Vec::new();
    let sink_updates_enabled = args.distributed_stream_summaries && args.sinks.is_some();
    let mut sink_interval = tokio::time::interval(resolve_sink_interval(args.sinks.as_ref()));
    sink_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
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
                    update_ui(
                        ui_tx,
                        args,
                        &agent_states,
                        &mut ui_latency_window,
                        &mut ui_rps_window,
                    );
                }
                if pending_agents.is_empty() {
                    break;
                }
            }
            _ = sink_interval.tick() => {
                if sink_updates_enabled && sink_dirty {
                    if let Err(err) = write_streaming_sinks(args, &agent_states).await {
                        runtime_errors.push(err.to_string());
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
    } else if let Ok((summary, merged_hist, success_hist)) = aggregate_snapshots(&agent_states) {
        let (p50, p90, p99) = merged_hist.percentiles();
        let (success_p50, success_p90, success_p99) = success_hist.percentiles();
        let stats = compute_summary_stats(&summary);
        let mut charts_written = false;
        if charts_enabled {
            match write_aggregated_charts(&aggregated_samples, args).await {
                Ok(written) => charts_written = written,
                Err(err) => runtime_errors.push(err.to_string()),
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
            let sink_stats = crate::sinks::config::SinkStats {
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
                runtime_errors.push(format!("Sinks: {}", err));
            }
        }
    } else {
        runtime_errors.push("Failed to aggregate agent summaries.".to_owned());
    }

    if let Some(shutdown_tx) = shutdown_tx.as_ref() {
        drop(shutdown_tx.send(()));
    }

    if !runtime_errors.is_empty() {
        eprintln!("Runtime errors:");
        for err in runtime_errors {
            eprintln!("- {}", err);
        }
        return Err(AppError::distributed(
            DistributedError::RunCompletedWithErrors,
        ));
    }

    Ok(())
}

fn compute_weights(agents: &[AgentConn]) -> Vec<u64> {
    agents.iter().map(|agent| agent.weight).collect()
}
