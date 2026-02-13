use std::sync::{Arc, atomic::Ordering};

use reqwest::{Client, Request};
use tokio::time::{Instant, sleep};
use tracing::error;

use crate::{
    error::{AppError, AppResult, HttpError},
    metrics::Metrics,
    shutdown::ShutdownReceiver,
};

use super::builders::{StepRequestContext, build_request_from_spec, build_step_request};
use super::data::{ScenarioRunContext, SingleRequestSpec, WorkerContext, Workload};
use super::execution::{execute_request, execute_request_status, execute_request_with_asserts};
use super::runner_common::{InflightGuard, prepare_iteration, run_and_record};
use super::template::{build_template_vars, step_label};

/// Synthetic status used when an assert fails before a real HTTP response.
const ASSERT_FAILED_STATUS: u16 = 0;

pub(in crate::http) async fn preflight_request(
    client: &Client,
    workload: &Workload,
) -> AppResult<()> {
    match workload {
        Workload::Single(request_template) => {
            let request = request_template
                .try_clone()
                .ok_or_else(|| AppError::http(HttpError::CloneRequestFailed))?;
            execute_request(client, request, true)
                .await
                .map_err(|err| AppError::http(HttpError::TestRequestFailed { source: err }))?;
            Ok(())
        }
        Workload::SingleDynamic(spec) => {
            let request = build_request_from_spec(client, spec)?;
            execute_request(client, request, true)
                .await
                .map_err(|err| AppError::http(HttpError::TestRequestFailed { source: err }))?;
            Ok(())
        }
        Workload::Scenario(scenario, connect_to, host_header, auth) => {
            let step = scenario
                .steps
                .first()
                .ok_or_else(|| AppError::http(HttpError::ScenarioHasNoSteps))?;
            let vars = build_template_vars(scenario, step, 0, 0);
            let request = build_step_request(
                client,
                scenario,
                step,
                &vars,
                &StepRequestContext {
                    connect_to,
                    host_header: host_header.as_deref(),
                    auth: auth.as_ref(),
                },
            )?;
            execute_request(client, request, true)
                .await
                .map_err(|err| {
                    AppError::http(HttpError::ScenarioPreflightFailed {
                        source: Box::new(AppError::from(err)),
                    })
                })?;
            Ok(())
        }
    }
}

pub(in crate::http) async fn run_single_iteration(
    shutdown_rx: &mut ShutdownReceiver,
    context: &WorkerContext<'_>,
    request_template: &Arc<Request>,
) -> bool {
    let Some(latency_start) = prepare_iteration(
        shutdown_rx,
        context.shutdown_tx,
        context.request_limiter,
        context.rate_limiter,
        context.wait_ongoing,
        context.latency_correction,
    )
    .await
    else {
        return true;
    };

    let run_request = async {
        match request_template.try_clone() {
            Some(req_clone) => execute_request_status(context.client, req_clone).await,
            None => {
                error!("Failed to clone request template.");
                (500, false, true, 0)
            }
        }
    };

    run_and_record(shutdown_rx, context, latency_start, run_request).await
}

pub(in crate::http) async fn run_single_dynamic_iteration(
    shutdown_rx: &mut ShutdownReceiver,
    context: &WorkerContext<'_>,
    spec: &Arc<SingleRequestSpec>,
) -> bool {
    let Some(latency_start) = prepare_iteration(
        shutdown_rx,
        context.shutdown_tx,
        context.request_limiter,
        context.rate_limiter,
        context.wait_ongoing,
        context.latency_correction,
    )
    .await
    else {
        return true;
    };

    let request = match build_request_from_spec(context.client, spec) {
        Ok(request) => request,
        Err(err) => {
            error!("Failed to build request: {}", err);
            return true;
        }
    };

    run_and_record(
        shutdown_rx,
        context,
        latency_start,
        execute_request_status(context.client, request),
    )
    .await
}

pub(in crate::http) async fn run_scenario_iteration(
    shutdown_rx: &mut ShutdownReceiver,
    worker: &WorkerContext<'_>,
    context: &mut ScenarioRunContext<'_>,
) -> bool {
    for (step_index, step) in context.scenario.steps.iter().enumerate() {
        let Some(latency_start) = prepare_iteration(
            shutdown_rx,
            worker.shutdown_tx,
            worker.request_limiter,
            worker.rate_limiter,
            worker.wait_ongoing,
            worker.latency_correction,
        )
        .await
        else {
            return true;
        };

        let vars = build_template_vars(context.scenario, step, *context.request_seq, step_index);
        let request = match build_step_request(
            context.client,
            context.scenario,
            step,
            &vars,
            &StepRequestContext {
                connect_to: context.connect_to,
                host_header: context.host_header,
                auth: context.auth,
            },
        ) {
            Ok(request) => request,
            Err(err) => {
                error!("Failed to build scenario request: {}", err);
                return true;
            }
        };

        let expected = step.assert_status.unwrap_or(context.expected_status_code);
        let start = latency_start.unwrap_or_else(Instant::now);
        let in_flight_guard = InflightGuard::acquire(worker.in_flight_counter);
        let run_request = async {
            execute_request_with_asserts(
                context.client,
                request,
                context.expected_status_code,
                step.assert_status,
                step.assert_body_contains.as_deref(),
            )
            .await
        };
        let outcome = if worker.wait_ongoing {
            run_request.await
        } else {
            tokio::select! {
                _ = shutdown_rx.recv() => return true,
                result = run_request => result,
            }
        };
        drop(in_flight_guard);

        if !outcome.success {
            let label = step_label(step, step_index);
            if let Some(fragment) = step.assert_body_contains.as_deref() {
                error!(
                    "Scenario step {} failed: status {} (expected {}) or body missing '{}'.",
                    label, outcome.status, expected, fragment
                );
            } else {
                error!(
                    "Scenario step {} failed: status {} (expected {}).",
                    label, outcome.status, expected
                );
            }
        }

        let metric_status = if outcome.success {
            context.expected_status_code
        } else {
            ASSERT_FAILED_STATUS
        };
        let in_flight_ops = worker.in_flight_counter.load(Ordering::Relaxed);
        let metric = Metrics::new(
            start,
            metric_status,
            outcome.timed_out,
            outcome.transport_error,
            outcome.response_bytes,
            in_flight_ops,
        );
        if let Some(log_sink) = context.log_sink
            && !log_sink.send(metric)
        {
            return true;
        }
        if context.metrics_tx.try_send(metric).is_err() {
            // Ignore UI backpressure; summary and charts use log pipeline.
        }

        *context.request_seq = context.request_seq.saturating_add(1);

        if let Some(think_time) = step.think_time {
            tokio::select! {
                _ = shutdown_rx.recv() => return true,
                () = sleep(think_time) => {},
            };
        }
    }

    false
}
