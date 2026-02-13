use std::collections::HashMap;

use crate::args::TesterArgs;
use crate::charts;
use crate::error::AppResult;
use crate::metrics::{AggregatedMetricSample, LatencyHistogram};
use crate::sinks::config::SinkStats;
use crate::sinks::writers::write_sinks;

use super::super::super::summary::{compute_summary_stats, merge_summaries};
use super::events::AgentSnapshot;

pub(in crate::distributed::controller) async fn write_streaming_sinks(
    args: &TesterArgs,
    agent_states: &HashMap<String, AgentSnapshot>,
) -> AppResult<()> {
    if agent_states.is_empty() {
        return Ok(());
    }
    let (summary, merged_hist, _success_hist) = aggregate_snapshots(agent_states)?;
    let (p50, p90, p99) = merged_hist.percentiles();
    let stats = compute_summary_stats(&summary);

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
        write_sinks(sinks, &sink_stats).await?;
    }
    Ok(())
}

pub(in crate::distributed::controller) fn aggregate_snapshots(
    agent_states: &HashMap<String, AgentSnapshot>,
) -> AppResult<(
    crate::metrics::MetricsSummary,
    LatencyHistogram,
    LatencyHistogram,
)> {
    let mut summaries = Vec::with_capacity(agent_states.len());
    let mut merged_hist = LatencyHistogram::new()?;
    let mut merged_success_hist = LatencyHistogram::new()?;
    for snapshot in agent_states.values() {
        summaries.push(snapshot.summary.clone());
        merged_hist.merge(&snapshot.histogram)?;
        merged_success_hist.merge(&snapshot.success_histogram)?;
    }
    Ok((
        merge_summaries(&summaries),
        merged_hist,
        merged_success_hist,
    ))
}

pub(in crate::distributed::controller) fn record_aggregated_sample(
    samples: &mut Vec<AggregatedMetricSample>,
    agent_states: &HashMap<String, AgentSnapshot>,
) {
    let Ok((summary, merged_hist, _success_hist)) = aggregate_snapshots(agent_states) else {
        return;
    };
    let (p50, p90, p99) = merged_hist.percentiles();
    let elapsed_ms = u64::try_from(summary.duration.as_millis()).unwrap_or(u64::MAX);
    let sample = AggregatedMetricSample {
        elapsed_ms,
        total_requests: summary.total_requests,
        successful_requests: summary.successful_requests,
        error_requests: summary.error_requests,
        avg_latency_ms: summary.avg_latency_ms,
        p50_latency_ms: p50,
        p90_latency_ms: p90,
        p99_latency_ms: p99,
    };

    if let Some(last) = samples.last()
        && last.elapsed_ms == sample.elapsed_ms
        && last.total_requests == sample.total_requests
        && last.successful_requests == sample.successful_requests
        && last.error_requests == sample.error_requests
        && last.avg_latency_ms == sample.avg_latency_ms
        && last.p50_latency_ms == sample.p50_latency_ms
        && last.p90_latency_ms == sample.p90_latency_ms
        && last.p99_latency_ms == sample.p99_latency_ms
    {
        return;
    }

    samples.push(sample);
}

pub(in crate::distributed::controller) async fn write_aggregated_charts(
    samples: &[AggregatedMetricSample],
    args: &TesterArgs,
) -> AppResult<Option<String>> {
    if args.no_charts || samples.len() < 2 {
        return Ok(None);
    }
    charts::plot_aggregated_metrics(samples, args).await
}
