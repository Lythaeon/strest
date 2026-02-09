mod aggregated;
#[cfg(feature = "legacy-charts")]
mod average;
#[cfg(feature = "legacy-charts")]
mod cumulative;
#[cfg(feature = "legacy-charts")]
mod errors;
#[cfg(feature = "legacy-charts")]
mod inflight;
#[cfg(feature = "legacy-charts")]
mod latency;
#[cfg(feature = "legacy-charts")]
mod rps;
#[cfg(feature = "legacy-charts")]
mod status;
mod streaming;
#[cfg(feature = "legacy-charts")]
mod timeouts;

#[cfg(test)]
mod tests;

use std::path::Path;

use tokio::fs;
use tracing::{error, info};

use crate::args::TesterArgs;
#[cfg(feature = "legacy-charts")]
use crate::metrics::MetricRecord;

pub use aggregated::{
    plot_aggregated_average_response_time, plot_aggregated_cumulative_error_rate,
    plot_aggregated_cumulative_successful_requests, plot_aggregated_cumulative_total_requests,
    plot_aggregated_latency_percentiles, plot_aggregated_requests_per_second,
};
#[cfg(feature = "legacy-charts")]
pub use average::plot_average_response_time;
#[cfg(feature = "legacy-charts")]
pub use cumulative::{
    plot_cumulative_error_rate, plot_cumulative_successful_requests, plot_cumulative_total_requests,
};
#[cfg(feature = "legacy-charts")]
pub use errors::plot_error_rate_breakdown;
#[cfg(feature = "legacy-charts")]
pub use inflight::plot_inflight_requests;
#[cfg(feature = "legacy-charts")]
pub use latency::plot_latency_percentiles;
#[cfg(feature = "legacy-charts")]
pub use rps::plot_requests_per_second;
#[cfg(feature = "legacy-charts")]
pub use status::plot_status_code_distribution;
pub use streaming::{
    LatencyPercentilesSeries, plot_average_response_time_from_buckets,
    plot_cumulative_error_rate_from_buckets, plot_cumulative_successful_requests_from_buckets,
    plot_cumulative_total_requests_from_buckets, plot_error_rate_breakdown_from_counts,
    plot_inflight_requests_from_counts, plot_latency_percentiles_series,
    plot_requests_per_second_from_counts, plot_status_code_distribution_from_counts,
    plot_timeouts_per_second_from_counts,
};
#[cfg(feature = "legacy-charts")]
pub use timeouts::plot_timeouts_per_second;

#[cfg(feature = "legacy-charts")]
pub async fn plot_metrics(
    metrics: &[MetricRecord],
    args: &TesterArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if metrics.is_empty() {
        return Ok(());
    }
    let path = &args.charts_path;
    let expected_status_code = args.expected_status_code;

    if let Err(e) = fs::create_dir_all(Path::new(path)).await {
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

    Ok(())
}

pub async fn plot_aggregated_metrics(
    samples: &[crate::metrics::AggregatedMetricSample],
    args: &TesterArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if samples.is_empty() {
        return Ok(());
    }
    let path = &args.charts_path;

    if let Err(e) = fs::create_dir_all(Path::new(path)).await {
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

    Ok(())
}

pub async fn plot_streaming_metrics(
    data: &crate::metrics::StreamingChartData,
    args: &TesterArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    if data.avg_buckets.is_empty() && data.rps_counts.is_empty() {
        return Ok(());
    }
    let path = &args.charts_path;

    if let Err(e) = fs::create_dir_all(Path::new(path)).await {
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
        seconds: &data.latency_seconds,
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

    Ok(())
}
