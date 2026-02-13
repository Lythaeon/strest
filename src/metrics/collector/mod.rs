mod helpers;
mod state;

use std::time::Duration;

use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::{Instant, MissedTickBehavior},
};

use crate::shutdown::ShutdownSender;
use crate::{
    args::TesterArgs,
    sinks::writers,
    ui::model::{DataUsage, StatusCounts, UiData},
};

use super::{Metrics, MetricsReport, MetricsSummary, StreamSnapshot};
use helpers::{
    build_sink_stats, build_stream_snapshot, compute_percentiles, process_metric_ui,
    prune_bytes_window, prune_latency_window, prune_rps_window, record_bytes_sample,
    record_rps_sample, resolve_sink_interval, resolve_stream_interval,
};
use state::UiAggregationState;

#[must_use]
pub fn setup_metrics_collector(
    args: &TesterArgs,
    run_start: Instant,
    shutdown_tx: &ShutdownSender,
    mut metrics_rx: mpsc::Receiver<Metrics>,
    ui_tx: &watch::Sender<UiData>,
    stream_tx: Option<mpsc::UnboundedSender<StreamSnapshot>>,
) -> JoinHandle<MetricsReport> {
    let shutdown_tx_main = shutdown_tx.clone();
    let ui_tx = ui_tx.clone();

    let ui_window_ms = args.ui_window_ms.get();
    let ui_fps = args.ui_fps.max(1);
    let target_duration = Duration::from_secs(args.target_duration.get());
    let expected_status_code = args.expected_status_code;
    let sinks_config = args.sinks.clone();
    let stream_summaries = args.distributed_stream_summaries;
    let no_color = args.no_color;
    let sink_interval_duration = resolve_sink_interval(&sinks_config);
    let stream_interval_duration =
        resolve_stream_interval(args.distributed_stream_interval_ms.as_ref());

    tokio::spawn(async move {
        let ui_window = Duration::from_millis(ui_window_ms);
        let mut state = UiAggregationState::new(ui_window);
        let start_time = run_start;
        let mut shutdown_rx_inner = shutdown_tx_main.subscribe();
        let ui_tx_clone = ui_tx.clone();
        let interval_ms = 1000u64.checked_div(u64::from(ui_fps)).unwrap_or(1).max(1);
        let mut ui_interval = tokio::time::interval(Duration::from_millis(interval_ms));
        ui_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut sink_interval = tokio::time::interval(sink_interval_duration);
        sink_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut stream_interval = tokio::time::interval(stream_interval_duration);
        stream_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut last_sink_error: Option<String> = None;
        let shutdown_timer = tokio::time::sleep(target_duration);
        tokio::pin!(shutdown_timer);
        let mut ui_enabled = ui_tx
            .send(UiData {
                elapsed_time: Duration::ZERO,
                target_duration,
                current_requests: 0,
                successful_requests: 0,
                timeout_requests: 0,
                transport_errors: 0,
                non_expected_status: 0,
                in_flight_ops: 0,
                ui_window_ms,
                no_color,
                latencies: vec![],
                rps_series: vec![],
                status_counts: Some(StatusCounts::default()),
                data_usage: Some(DataUsage {
                    total_bytes: 0,
                    bytes_per_sec: 0,
                    series: Vec::new(),
                }),
                p50: 0,
                p90: 0,
                p99: 0,
                p50_ok: 0,
                p90_ok: 0,
                p99_ok: 0,
                rps: 0,
                rpm: 0,
                replay: None,
                compare: None,
            })
            .is_ok();

        loop {
            tokio::select! {
                () = &mut shutdown_timer => {
                    drop(shutdown_tx_main.send(()));
                    break;
                },
                _ = shutdown_rx_inner.recv() => break,
                maybe_msg = metrics_rx.recv() => {
                    let msg = match maybe_msg {
                        Some(msg) => msg,
                        None => {
                            drop(shutdown_tx_main.send(()));
                            break;
                        }
                    };
                    process_metric_ui(msg, Instant::now(), expected_status_code, &mut state);
                },
                _ = ui_interval.tick() => {
                    let now = Instant::now();
                    prune_latency_window(&mut state.latency_window, now, state.ui_window);
                    prune_rps_window(&mut state.rps_window, now);
                    prune_bytes_window(&mut state.bytes_window, now);

                    let elapsed_time = start_time.elapsed();
                    let recent_latencies: Vec<(u64, u64)> = state
                        .latency_window
                        .iter()
                        .map(|&(ts, latency)| {
                            let ms_since_start =
                                u64::try_from(ts.duration_since(start_time).as_millis())
                                    .unwrap_or(u64::MAX);
                            (ms_since_start, latency)
                        })
                        .collect();

                    let (p50, p90, p99) = compute_percentiles(&state.latency_window);
                    let (p50_ok, p90_ok, p99_ok) = compute_percentiles(&state.latency_window_ok);
                    let rps: u64 = state
                        .rps_window
                        .iter()
                        .filter(|(ts, _)| now.duration_since(*ts) <= Duration::from_secs(1))
                        .map(|(_, count)| *count)
                        .sum::<u64>();

                    let rpm = rps.saturating_mul(60);
                    record_rps_sample(&mut state.rps_samples, now, rps, state.ui_window);
                    let recent_rps: Vec<(u64, u64)> = state
                        .rps_samples
                        .iter()
                        .map(|&(ts, sample_rps)| {
                            let ms_since_start =
                                u64::try_from(ts.duration_since(start_time).as_millis())
                                    .unwrap_or(u64::MAX);
                            (ms_since_start, sample_rps)
                        })
                        .collect();

                    let bytes_per_sec: u64 = state
                        .bytes_window
                        .iter()
                        .filter(|(ts, _)| now.duration_since(*ts) <= Duration::from_secs(1))
                        .map(|(_, bytes)| *bytes)
                        .sum::<u64>();
                    record_bytes_sample(&mut state.bytes_samples, now, bytes_per_sec, state.ui_window);
                    let recent_bytes: Vec<(u64, u64)> = state
                        .bytes_samples
                        .iter()
                        .map(|&(ts, bytes)| {
                            let ms_since_start =
                                u64::try_from(ts.duration_since(start_time).as_millis())
                                    .unwrap_or(u64::MAX);
                            (ms_since_start, bytes)
                        })
                        .collect();

                    if ui_enabled
                        && ui_tx_clone
                            .send(UiData {
                                elapsed_time,
                                target_duration,
                                current_requests: state.current_requests,
                                successful_requests: state.successful_requests,
                                timeout_requests: state.timeout_requests,
                                transport_errors: state.transport_errors,
                                non_expected_status: state.non_expected_status,
                                in_flight_ops: state.in_flight_ops,
                                ui_window_ms,
                                no_color,
                                latencies: recent_latencies,
                                rps_series: recent_rps,
                                status_counts: Some(state.status_counts.clone()),
                                data_usage: Some(DataUsage {
                                    total_bytes: state.total_bytes,
                                    bytes_per_sec,
                                    series: recent_bytes,
                                }),
                                p50,
                                p90,
                                p99,
                                p50_ok,
                                p90_ok,
                                p99_ok,
                                rps,
                                rpm,
                                replay: None,
                                compare: None,
                            })
                            .is_err()
                    {
                        ui_enabled = false;
                    }
                },
                _ = sink_interval.tick() => {
                    let duration = start_time.elapsed();

                    if !stream_summaries && let Some(sinks_config) = sinks_config.as_ref() {
                        let sink_stats = build_sink_stats(&state, duration);
                        match writers::write_sinks(sinks_config, &sink_stats).await {
                            Ok(()) => {
                                last_sink_error = None;
                            }
                            Err(err) => {
                                let err_message = err.to_string();
                                if last_sink_error.as_deref() != Some(err_message.as_str()) {
                                    tracing::warn!("Failed to write sinks: {}", err);
                                    last_sink_error = Some(err_message);
                                }
                            }
                        }
                    }
                },
                _ = stream_interval.tick(), if stream_tx.is_some() => {
                    let duration = start_time.elapsed();
                    if let Some(stream_tx) = stream_tx.as_ref()
                        && let Some(snapshot) = build_stream_snapshot(&state, duration)
                    {
                        drop(stream_tx.send(snapshot));
                    }
                }
            }
        }

        let drain_deadline = Instant::now()
            .checked_add(Duration::from_millis(200))
            .unwrap_or_else(Instant::now);
        loop {
            if Instant::now() > drain_deadline {
                break;
            }
            match metrics_rx.try_recv() {
                Ok(msg) => {
                    process_metric_ui(msg, Instant::now(), expected_status_code, &mut state);
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => break,
            }
        }

        let duration = start_time.elapsed();
        let avg_latency_ms = if state.current_requests > 0 {
            let avg = state
                .latency_sum_ms
                .checked_div(u128::from(state.current_requests))
                .unwrap_or(0);
            u64::try_from(avg).map_or(u64::MAX, |value| value)
        } else {
            0
        };
        let success_avg_latency_ms = if state.successful_requests > 0 {
            let avg = state
                .success_latency_sum_ms
                .checked_div(u128::from(state.successful_requests))
                .unwrap_or(0);
            u64::try_from(avg).map_or(u64::MAX, |value| value)
        } else {
            0
        };
        let min_latency_ms = if state.current_requests > 0 {
            state.min_latency_ms
        } else {
            0
        };
        let success_min_latency_ms = if state.successful_requests > 0 {
            state.success_min_latency_ms
        } else {
            0
        };
        let success_max_latency_ms = if state.successful_requests > 0 {
            state.success_max_latency_ms
        } else {
            0
        };
        let error_requests = state
            .current_requests
            .saturating_sub(state.successful_requests);

        MetricsReport {
            summary: MetricsSummary {
                duration,
                total_requests: state.current_requests,
                successful_requests: state.successful_requests,
                error_requests,
                timeout_requests: state.timeout_requests,
                transport_errors: state.transport_errors,
                non_expected_status: state.non_expected_status,
                min_latency_ms,
                max_latency_ms: state.max_latency_ms,
                avg_latency_ms,
                success_min_latency_ms,
                success_max_latency_ms,
                success_avg_latency_ms,
            },
        }
    })
}
