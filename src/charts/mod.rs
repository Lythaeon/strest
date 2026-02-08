mod aggregated;
mod average;
mod cumulative;
mod errors;
mod inflight;
mod latency;
mod rps;
mod status;
mod timeouts;

#[cfg(test)]
mod tests;

use std::path::Path;

use tokio::fs;
use tracing::{error, info};

use crate::{args::TesterArgs, metrics::MetricRecord};

pub use aggregated::{
    plot_aggregated_average_response_time, plot_aggregated_cumulative_error_rate,
    plot_aggregated_cumulative_successful_requests, plot_aggregated_cumulative_total_requests,
    plot_aggregated_latency_percentiles, plot_aggregated_requests_per_second,
};
pub use average::plot_average_response_time;
pub use cumulative::{
    plot_cumulative_error_rate, plot_cumulative_successful_requests, plot_cumulative_total_requests,
};
pub use errors::plot_error_rate_breakdown;
pub use inflight::plot_inflight_requests;
pub use latency::plot_latency_percentiles;
pub use rps::plot_requests_per_second;
pub use status::plot_status_code_distribution;
pub use timeouts::plot_timeouts_per_second;

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
