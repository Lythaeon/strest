use crate::args::TesterArgs;
use crate::error::{AppError, AppResult, DistributedError};
use crate::sinks::config::SinkStats;

use super::super::shared::{aggregate_snapshots, write_aggregated_charts};
use super::state::ManualRunState;
use crate::distributed::summary::{
    Percentiles, SummaryPercentiles, compute_summary_stats, print_summary,
};

pub(super) async fn finalize_manual_run(
    args: &TesterArgs,
    state: &mut ManualRunState,
) -> AppResult<()> {
    if state.agent_states.is_empty() {
        state
            .runtime_errors
            .push("No successful agent reports received.".to_owned());
    } else if let Ok((summary, merged_hist, success_hist)) =
        aggregate_snapshots(&state.agent_states)
    {
        let (p50, p90, p99) = merged_hist.percentiles();
        let (success_p50, success_p90, success_p99) = success_hist.percentiles();
        let stats = compute_summary_stats(&summary);
        let mut charts_output_path: Option<String> = None;
        if state.charts_enabled {
            match write_aggregated_charts(&state.aggregated_samples, args).await {
                Ok(path) => charts_output_path = path,
                Err(err) => state.runtime_errors.push(err.to_string()),
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
        drop(shutdown_tx.send(()));
    }

    if !state.runtime_errors.is_empty() {
        eprintln!("Runtime errors:");
        for err in &state.runtime_errors {
            eprintln!("- {}", err);
        }
        return Err(AppError::distributed(
            DistributedError::RunCompletedWithErrors,
        ));
    }

    Ok(())
}
