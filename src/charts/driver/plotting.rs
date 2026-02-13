use std::path::Path;

use tokio::fs;
use tracing::{error, info};

use crate::args::TesterArgs;
use crate::error::AppResult;
use crate::metrics::{AggregatedMetricSample, StreamingChartData};

use super::super::{
    LatencyPercentilesSeries, plot_aggregated_average_response_time,
    plot_aggregated_cumulative_error_rate, plot_aggregated_cumulative_successful_requests,
    plot_aggregated_cumulative_total_requests, plot_aggregated_latency_percentiles,
    plot_aggregated_requests_per_second, plot_average_response_time_from_buckets,
    plot_cumulative_error_rate_from_buckets, plot_cumulative_successful_requests_from_buckets,
    plot_cumulative_total_requests_from_buckets, plot_error_rate_breakdown_from_counts,
    plot_inflight_requests_from_counts, plot_latency_percentiles_series,
    plot_requests_per_second_from_counts, plot_status_code_distribution_from_counts,
    plot_timeouts_per_second_from_counts,
};
use super::naming::resolve_chart_output_dir;

#[cfg(feature = "legacy-charts")]
use super::super::{
    plot_average_response_time, plot_cumulative_error_rate, plot_cumulative_successful_requests,
    plot_cumulative_total_requests, plot_error_rate_breakdown, plot_inflight_requests,
    plot_latency_percentiles, plot_requests_per_second, plot_status_code_distribution,
    plot_timeouts_per_second,
};
#[cfg(feature = "legacy-charts")]
use crate::metrics::MetricRecord;

#[cfg(feature = "legacy-charts")]
pub async fn plot_metrics(
    metrics: &[MetricRecord],
    args: &TesterArgs,
) -> AppResult<Option<String>> {
    if metrics.is_empty() {
        return Ok(None);
    }
    let output_dir = resolve_chart_output_dir(args);
    let path = output_dir.to_string_lossy().to_string();
    let expected_status_code = args.expected_status_code;

    if let Err(e) = fs::create_dir_all(Path::new(&path)).await {
        error!("Failed to create output directory '{}': {}", path, e);
        return Err(e.into());
    }

    info!("Plotting average response time...");

    plot_average_response_time(metrics, &format!("{}/average_response_time.png", path))?;

    info!("Plotting cumulative successful requests...");

    plot_cumulative_successful_requests(
        metrics,
        expected_status_code,
        &format!("{}/cumulative_successful_requests.png", path),
    )?;

    info!("Plotting cumulative error rate...");

    plot_cumulative_error_rate(
        metrics,
        expected_status_code,
        &format!("{}/cumulative_error_rate.png", path),
    )?;

    info!("Plotting latency percentiles...");

    plot_latency_percentiles(
        metrics,
        expected_status_code,
        &format!("{}/latency_percentiles", path),
    )?;

    info!("Plotting requests per second...");

    plot_requests_per_second(metrics, &format!("{}/requests_per_second.png", path))?;

    info!("Plotting timeouts per second...");

    plot_timeouts_per_second(metrics, &format!("{}/timeouts_per_second.png", path))?;

    info!("Plotting error rate breakdown...");

    plot_error_rate_breakdown(
        metrics,
        expected_status_code,
        &format!("{}/error_rate_breakdown.png", path),
    )?;

    info!("Plotting status code distribution...");

    plot_status_code_distribution(metrics, &format!("{}/status_code_distribution.png", path))?;

    info!("Plotting in-flight requests...");

    plot_inflight_requests(metrics, &format!("{}/inflight_requests.png", path))?;

    info!("Plotting cumulative total requests...");

    plot_cumulative_total_requests(metrics, &format!("{}/cumulative_total_requests.png", path))?;

    Ok(Some(path))
}

pub async fn plot_aggregated_metrics(
    samples: &[AggregatedMetricSample],
    args: &TesterArgs,
) -> AppResult<Option<String>> {
    if samples.is_empty() {
        return Ok(None);
    }
    let output_dir = resolve_chart_output_dir(args);
    let path = output_dir.to_string_lossy().to_string();

    if let Err(e) = fs::create_dir_all(Path::new(&path)).await {
        error!("Failed to create output directory '{}': {}", path, e);
        return Err(e.into());
    }

    info!("Plotting average response time (aggregated)...");
    plot_aggregated_average_response_time(samples, &format!("{}/average_response_time.png", path))?;

    info!("Plotting cumulative successful requests (aggregated)...");
    plot_aggregated_cumulative_successful_requests(
        samples,
        &format!("{}/cumulative_successful_requests.png", path),
    )?;

    info!("Plotting cumulative error rate (aggregated)...");
    plot_aggregated_cumulative_error_rate(samples, &format!("{}/cumulative_error_rate.png", path))?;

    info!("Plotting latency percentiles (aggregated)...");
    plot_aggregated_latency_percentiles(samples, &format!("{}/latency_percentiles", path))?;

    info!("Plotting requests per second (aggregated)...");
    plot_aggregated_requests_per_second(samples, &format!("{}/requests_per_second.png", path))?;

    info!("Plotting cumulative total requests (aggregated)...");
    plot_aggregated_cumulative_total_requests(
        samples,
        &format!("{}/cumulative_total_requests.png", path),
    )?;

    Ok(Some(path))
}

pub async fn plot_streaming_metrics(
    data: &StreamingChartData,
    args: &TesterArgs,
) -> AppResult<Option<String>> {
    if data.avg_buckets.is_empty() && data.rps_counts.is_empty() {
        return Ok(None);
    }
    let output_dir = resolve_chart_output_dir(args);
    let path = output_dir.to_string_lossy().to_string();

    if let Err(e) = fs::create_dir_all(Path::new(&path)).await {
        error!("Failed to create output directory '{}': {}", path, e);
        return Err(e.into());
    }

    info!("Plotting average response time...");
    plot_average_response_time_from_buckets(
        &data.avg_buckets,
        &format!("{}/average_response_time.png", path),
    )?;

    info!("Plotting cumulative successful requests...");
    plot_cumulative_successful_requests_from_buckets(
        &data.success_buckets,
        &format!("{}/cumulative_successful_requests.png", path),
    )?;

    info!("Plotting cumulative error rate...");
    plot_cumulative_error_rate_from_buckets(
        &data.error_buckets,
        &format!("{}/cumulative_error_rate.png", path),
    )?;

    info!("Plotting latency percentiles...");
    let percentiles = LatencyPercentilesSeries {
        buckets_ms: &data.latency_buckets_ms,
        bucket_ms: data.latency_bucket_ms,
        p50: &data.p50,
        p90: &data.p90,
        p99: &data.p99,
        p50_ok: &data.p50_ok,
        p90_ok: &data.p90_ok,
        p99_ok: &data.p99_ok,
    };
    plot_latency_percentiles_series(&percentiles, &format!("{}/latency_percentiles", path))?;

    info!("Plotting requests per second...");
    plot_requests_per_second_from_counts(
        &data.rps_counts,
        &format!("{}/requests_per_second.png", path),
    )?;

    info!("Plotting timeouts per second...");
    plot_timeouts_per_second_from_counts(
        &data.timeouts,
        &format!("{}/timeouts_per_second.png", path),
    )?;

    info!("Plotting error rate breakdown...");
    plot_error_rate_breakdown_from_counts(
        &data.timeouts,
        &data.transports,
        &data.non_expected,
        &format!("{}/error_rate_breakdown.png", path),
    )?;

    info!("Plotting status code distribution...");
    plot_status_code_distribution_from_counts(
        &data.status_2xx,
        &data.status_3xx,
        &data.status_4xx,
        &data.status_5xx,
        &data.status_other,
        &format!("{}/status_code_distribution.png", path),
    )?;

    info!("Plotting in-flight requests...");
    plot_inflight_requests_from_counts(&data.inflight, &format!("{}/inflight_requests.png", path))?;

    info!("Plotting cumulative total requests...");
    plot_cumulative_total_requests_from_buckets(
        &data.total_buckets,
        &format!("{}/cumulative_total_requests.png", path),
    )?;

    Ok(Some(path))
}
