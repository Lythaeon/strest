use std::collections::VecDeque;
use std::time::Duration;

use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::{Instant, MissedTickBehavior},
};

use crate::shutdown::ShutdownSender;
use crate::{
    args::{PositiveU64, TesterArgs},
    sinks::{
        config::{SinkStats, SinksConfig},
        writers,
    },
    ui::model::{DataUsage, StatusCounts, UiData},
};

use super::{LatencyHistogram, Metrics, MetricsReport, MetricsSummary, StreamSnapshot};

const SINK_UPDATE_INTERVAL: Duration = Duration::from_secs(1);
const STREAM_UPDATE_INTERVAL: Duration = Duration::from_secs(1);

struct UiAggregationState {
    current_requests: u64,
    successful_requests: u64,
    timeout_requests: u64,
    transport_errors: u64,
    non_expected_status: u64,
    in_flight_ops: u64,
    ui_window: Duration,
    latency_sum_ms: u128,
    success_latency_sum_ms: u128,
    min_latency_ms: u64,
    max_latency_ms: u64,
    success_min_latency_ms: u64,
    success_max_latency_ms: u64,
    latency_window: VecDeque<(Instant, u64)>,
    latency_window_ok: VecDeque<(Instant, u64)>,
    rps_window: VecDeque<(Instant, u64)>,
    rps_samples: VecDeque<(Instant, u64)>,
    status_window: VecDeque<(Instant, StatusBucket)>,
    bytes_window: VecDeque<(Instant, u64)>,
    bytes_samples: VecDeque<(Instant, u64)>,
    total_bytes: u128,
    histogram: Option<LatencyHistogram>,
    success_histogram: Option<LatencyHistogram>,
}

impl UiAggregationState {
    fn new(ui_window: Duration) -> Self {
        let histogram = match LatencyHistogram::new() {
            Ok(histogram) => Some(histogram),
            Err(err) => {
                tracing::warn!("Failed to initialize latency histogram: {}", err);
                None
            }
        };
        let success_histogram = match LatencyHistogram::new() {
            Ok(success_histogram) => Some(success_histogram),
            Err(err) => {
                tracing::warn!("Failed to initialize success latency histogram: {}", err);
                None
            }
        };

        Self {
            current_requests: 0,
            successful_requests: 0,
            timeout_requests: 0,
            transport_errors: 0,
            non_expected_status: 0,
            in_flight_ops: 0,
            ui_window,
            latency_sum_ms: 0,
            success_latency_sum_ms: 0,
            min_latency_ms: u64::MAX,
            max_latency_ms: 0,
            success_min_latency_ms: u64::MAX,
            success_max_latency_ms: 0,
            latency_window: VecDeque::new(),
            latency_window_ok: VecDeque::new(),
            rps_window: VecDeque::new(),
            rps_samples: VecDeque::new(),
            status_window: VecDeque::new(),
            bytes_window: VecDeque::new(),
            bytes_samples: VecDeque::new(),
            total_bytes: 0,
            histogram,
            success_histogram,
        }
    }
}

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
                    process_metric_ui(
                        msg,
                        Instant::now(),
                        expected_status_code,
                        &mut state,
                    );
                },
                _ = ui_interval.tick() => {
                    let now = Instant::now();
                    prune_latency_window(&mut state.latency_window, now, state.ui_window);
                    prune_rps_window(&mut state.rps_window, now);
                    prune_status_window(&mut state.status_window, now, state.ui_window);
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
                    let status_counts = compute_status_counts(&state.status_window);

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
                    record_bytes_sample(
                        &mut state.bytes_samples,
                        now,
                        bytes_per_sec,
                        state.ui_window,
                    );
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
                                status_counts: Some(status_counts),
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

fn prune_latency_window(
    window: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    window_span: Duration,
) {
    while window
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > window_span)
    {
        window.pop_front();
    }
}

fn process_metric_ui(
    msg: Metrics,
    now: Instant,
    expected_status_code: u16,
    state: &mut UiAggregationState,
) {
    let status_code = msg.status_code;
    let latency_ms = u64::try_from(msg.response_time.as_millis()).unwrap_or(u64::MAX);
    state.in_flight_ops = msg.in_flight_ops;

    state.current_requests = state.current_requests.saturating_add(1);

    let is_success = status_code == expected_status_code && !msg.timed_out && !msg.transport_error;
    if is_success {
        state.successful_requests = state.successful_requests.saturating_add(1);
        state.success_latency_sum_ms = state
            .success_latency_sum_ms
            .saturating_add(u128::from(latency_ms));
        if latency_ms < state.success_min_latency_ms {
            state.success_min_latency_ms = latency_ms;
        }
        if latency_ms > state.success_max_latency_ms {
            state.success_max_latency_ms = latency_ms;
        }
        state.latency_window_ok.push_back((now, latency_ms));
        prune_latency_window(&mut state.latency_window_ok, now, state.ui_window);
        if let Some(histogram) = state.success_histogram.as_mut()
            && let Err(err) = histogram.record(latency_ms)
        {
            tracing::warn!("Disabling success latency histogram after error: {}", err);
            state.success_histogram = None;
        }
    } else if msg.timed_out {
        state.timeout_requests = state.timeout_requests.saturating_add(1);
    } else if msg.transport_error {
        state.transport_errors = state.transport_errors.saturating_add(1);
    } else if status_code != expected_status_code {
        state.non_expected_status = state.non_expected_status.saturating_add(1);
    }

    state.status_window.push_back((
        now,
        bucket_status(status_code, msg.timed_out, msg.transport_error),
    ));
    prune_status_window(&mut state.status_window, now, state.ui_window);

    state.latency_window.push_back((now, latency_ms));
    prune_latency_window(&mut state.latency_window, now, state.ui_window);

    record_rps(&mut state.rps_window, now);
    prune_rps_window(&mut state.rps_window, now);

    state.total_bytes = state
        .total_bytes
        .saturating_add(u128::from(msg.response_bytes));
    record_bytes(&mut state.bytes_window, now, msg.response_bytes);
    prune_bytes_window(&mut state.bytes_window, now);

    state.latency_sum_ms = state.latency_sum_ms.saturating_add(u128::from(latency_ms));
    if latency_ms < state.min_latency_ms {
        state.min_latency_ms = latency_ms;
    }
    if latency_ms > state.max_latency_ms {
        state.max_latency_ms = latency_ms;
    }

    if let Some(histogram) = state.histogram.as_mut()
        && let Err(err) = histogram.record(latency_ms)
    {
        tracing::warn!("Disabling latency histogram after error: {}", err);
        state.histogram = None;
    }
}

fn record_rps(window: &mut VecDeque<(Instant, u64)>, now: Instant) {
    if let Some((ts, count)) = window.back_mut() {
        if now.duration_since(*ts) < Duration::from_millis(100) {
            *count = count.saturating_add(1);
        } else {
            window.push_back((now, 1));
        }
    } else {
        window.push_back((now, 1));
    }
}

fn prune_rps_window(window: &mut VecDeque<(Instant, u64)>, now: Instant) {
    while window
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > Duration::from_secs(60))
    {
        window.pop_front();
    }
}

#[derive(Clone, Copy)]
enum StatusBucket {
    Status2xx,
    Status3xx,
    Status4xx,
    Status5xx,
    Other,
}

const fn bucket_status(status_code: u16, timed_out: bool, transport_error: bool) -> StatusBucket {
    if timed_out || transport_error {
        return StatusBucket::Other;
    }
    match status_code {
        200..=299 => StatusBucket::Status2xx,
        300..=399 => StatusBucket::Status3xx,
        400..=499 => StatusBucket::Status4xx,
        500..=599 => StatusBucket::Status5xx,
        _ => StatusBucket::Other,
    }
}

fn prune_status_window(
    window: &mut VecDeque<(Instant, StatusBucket)>,
    now: Instant,
    window_span: Duration,
) {
    while window
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > window_span)
    {
        window.pop_front();
    }
}

fn compute_status_counts(window: &VecDeque<(Instant, StatusBucket)>) -> StatusCounts {
    let mut counts = StatusCounts::default();
    for (_, bucket) in window {
        match bucket {
            StatusBucket::Status2xx => counts.status_2xx = counts.status_2xx.saturating_add(1),
            StatusBucket::Status3xx => counts.status_3xx = counts.status_3xx.saturating_add(1),
            StatusBucket::Status4xx => counts.status_4xx = counts.status_4xx.saturating_add(1),
            StatusBucket::Status5xx => counts.status_5xx = counts.status_5xx.saturating_add(1),
            StatusBucket::Other => counts.status_other = counts.status_other.saturating_add(1),
        }
    }
    counts
}

fn record_bytes(window: &mut VecDeque<(Instant, u64)>, now: Instant, bytes: u64) {
    if let Some((ts, count)) = window.back_mut()
        && now.duration_since(*ts) < Duration::from_millis(100)
    {
        *count = count.saturating_add(bytes);
        return;
    }
    window.push_back((now, bytes));
}

fn prune_bytes_window(window: &mut VecDeque<(Instant, u64)>, now: Instant) {
    while window
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > Duration::from_secs(60))
    {
        window.pop_front();
    }
}

fn record_rps_sample(
    samples: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    rps: u64,
    window_span: Duration,
) {
    samples.push_back((now, rps));
    prune_rps_samples(samples, now, window_span);
}

fn prune_rps_samples(samples: &mut VecDeque<(Instant, u64)>, now: Instant, window_span: Duration) {
    while samples
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > window_span)
    {
        samples.pop_front();
    }
}

fn record_bytes_sample(
    samples: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    bytes_per_sec: u64,
    window_span: Duration,
) {
    samples.push_back((now, bytes_per_sec));
    prune_bytes_samples(samples, now, window_span);
}

fn prune_bytes_samples(
    samples: &mut VecDeque<(Instant, u64)>,
    now: Instant,
    window_span: Duration,
) {
    while samples
        .front()
        .is_some_and(|(ts, _)| now.duration_since(*ts) > window_span)
    {
        samples.pop_front();
    }
}

fn build_sink_stats(state: &UiAggregationState, duration: Duration) -> SinkStats {
    let total_requests = state.current_requests;
    let successful_requests = state.successful_requests;
    let error_requests = total_requests.saturating_sub(successful_requests);

    let avg_latency_ms = if total_requests > 0 {
        let avg = state
            .latency_sum_ms
            .checked_div(u128::from(total_requests))
            .unwrap_or(0);
        u64::try_from(avg).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let min_latency_ms = if total_requests > 0 {
        state.min_latency_ms
    } else {
        0
    };
    let max_latency_ms = if total_requests > 0 {
        state.max_latency_ms
    } else {
        0
    };

    let (p50_latency_ms, p90_latency_ms, p99_latency_ms) = state
        .histogram
        .as_ref()
        .map(|histogram| histogram.percentiles())
        .unwrap_or((0, 0, 0));

    let (success_rate_x100, avg_rps_x100, avg_rpm_x100) =
        compute_rate_stats(total_requests, successful_requests, duration);

    SinkStats {
        duration,
        total_requests,
        successful_requests,
        error_requests,
        timeout_requests: state.timeout_requests,
        min_latency_ms,
        max_latency_ms,
        avg_latency_ms,
        p50_latency_ms,
        p90_latency_ms,
        p99_latency_ms,
        success_rate_x100,
        avg_rps_x100,
        avg_rpm_x100,
    }
}

fn build_stream_snapshot(state: &UiAggregationState, duration: Duration) -> Option<StreamSnapshot> {
    let histogram = state.histogram.as_ref()?;
    let histogram_b64 = match histogram.encode_base64() {
        Ok(value) => value,
        Err(err) => {
            tracing::warn!("Failed to encode histogram for stream snapshot: {}", err);
            return None;
        }
    };

    let total_requests = state.current_requests;
    let successful_requests = state.successful_requests;
    let error_requests = total_requests.saturating_sub(successful_requests);
    let min_latency_ms = if total_requests > 0 {
        state.min_latency_ms
    } else {
        0
    };
    let max_latency_ms = if total_requests > 0 {
        state.max_latency_ms
    } else {
        0
    };
    let success_min_latency_ms = if successful_requests > 0 {
        state.success_min_latency_ms
    } else {
        0
    };
    let success_max_latency_ms = if successful_requests > 0 {
        state.success_max_latency_ms
    } else {
        0
    };

    Some(StreamSnapshot {
        duration,
        total_requests,
        successful_requests,
        error_requests,
        timeout_requests: state.timeout_requests,
        transport_errors: state.transport_errors,
        non_expected_status: state.non_expected_status,
        min_latency_ms,
        max_latency_ms,
        latency_sum_ms: state.latency_sum_ms,
        success_min_latency_ms,
        success_max_latency_ms,
        success_latency_sum_ms: state.success_latency_sum_ms,
        histogram_b64,
    })
}

fn compute_rate_stats(total: u64, success: u64, duration: Duration) -> (u64, u64, u64) {
    let duration_ms = duration.as_millis().max(1);

    let success_rate_x100 = if total > 0 {
        let scaled = u128::from(success)
            .saturating_mul(10_000)
            .checked_div(u128::from(total))
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let avg_rps_x100 = if total > 0 {
        let scaled = u128::from(total)
            .saturating_mul(100_000)
            .checked_div(duration_ms)
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let avg_rpm_x100 = avg_rps_x100.saturating_mul(60);

    (success_rate_x100, avg_rps_x100, avg_rpm_x100)
}

fn resolve_sink_interval(config: &Option<SinksConfig>) -> Duration {
    let Some(config) = config.as_ref() else {
        return SINK_UPDATE_INTERVAL;
    };
    match config.update_interval_ms {
        Some(0) => {
            tracing::warn!(
                "sinks.update_interval_ms must be > 0; using default {}ms",
                SINK_UPDATE_INTERVAL.as_millis()
            );
            SINK_UPDATE_INTERVAL
        }
        Some(ms) => Duration::from_millis(ms),
        None => SINK_UPDATE_INTERVAL,
    }
}

const fn resolve_stream_interval(interval: Option<&PositiveU64>) -> Duration {
    let Some(value) = interval else {
        return STREAM_UPDATE_INTERVAL;
    };
    Duration::from_millis(value.get())
}

fn compute_percentiles(window: &VecDeque<(Instant, u64)>) -> (u64, u64, u64) {
    if window.is_empty() {
        return (0, 0, 0);
    }

    let mut values: Vec<u64> = window.iter().map(|(_, latency)| *latency).collect();
    values.sort_unstable();

    let p50 = percentile(&values, 50);
    let p90 = percentile(&values, 90);
    let p99 = percentile(&values, 99);

    (p50, p90, p99)
}

fn percentile(data: &[u64], percentile: u64) -> u64 {
    if data.is_empty() {
        return 0;
    }
    let count = data.len().saturating_sub(1) as u64;
    let index = (percentile.saturating_mul(count).saturating_add(50) / 100) as usize;
    *data.get(index).unwrap_or(&0)
}
