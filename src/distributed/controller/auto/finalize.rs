use std::collections::{HashMap, HashSet};

use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};
use crate::metrics::AggregatedMetricSample;
use crate::sinks::config::SinkStats;
use crate::sinks::writers::write_sinks;

use super::super::shared::{AgentSnapshot, aggregate_snapshots, write_aggregated_charts};
use super::events::AutoRunOutcome;
use crate::distributed::summary::{
    Percentiles, SummaryPercentiles, compute_summary_stats, print_summary,
};

pub(super) async fn finalize_auto_run(args: &TesterArgs, outcome: AutoRunOutcome) -> AppResult<()> {
    let AutoRunOutcome {
        run_id: _run_id,
        shutdown_tx,
        agent_states,
        aggregated_samples,
        mut runtime_errors,
        channel_closed,
        pending_agents,
    } = outcome;

    append_channel_closure_errors(channel_closed, &pending_agents, &mut runtime_errors);
    append_summary_errors(
        args,
        &agent_states,
        &aggregated_samples,
        &mut runtime_errors,
    )
    .await;

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

fn append_channel_closure_errors(
    channel_closed: bool,
    pending_agents: &HashSet<String>,
    runtime_errors: &mut Vec<String>,
) {
    if channel_closed && !pending_agents.is_empty() {
        for agent_id in pending_agents {
            runtime_errors.push(format!(
                "Agent {} disconnected before sending a report.",
                agent_id
            ));
        }
    }
}

async fn append_summary_errors(
    args: &TesterArgs,
    agent_states: &HashMap<String, AgentSnapshot>,
    aggregated_samples: &[AggregatedMetricSample],
    runtime_errors: &mut Vec<String>,
) {
    if agent_states.is_empty() {
        runtime_errors.push("No successful agent reports received.".to_owned());
        return;
    }

    let Ok((summary, merged_hist, success_hist)) = aggregate_snapshots(agent_states) else {
        runtime_errors.push("Failed to aggregate agent summaries.".to_owned());
        return;
    };
    let (p50, p90, p99) = merged_hist.percentiles();
    let (success_p50, success_p90, success_p99) = success_hist.percentiles();
    let stats = compute_summary_stats(&summary);
    let mut charts_output_path: Option<String> = None;
    if !args.no_charts {
        match write_aggregated_charts(aggregated_samples, args).await {
            Ok(path) => charts_output_path = path,
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

    print_summary(&summary, percentiles, args, charts_output_path.as_deref());

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
        if let Err(err) = write_sinks(sinks, &sink_stats).await {
            runtime_errors.push(format!("Sinks: {}", err));
        }
    }
}
