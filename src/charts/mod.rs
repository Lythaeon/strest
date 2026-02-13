mod aggregated;
#[cfg(feature = "legacy-charts")]
mod average;
#[cfg(feature = "legacy-charts")]
mod cumulative;
mod driver;
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

pub(crate) use driver::is_chart_run_dir_name;
#[cfg(feature = "legacy-charts")]
pub use driver::plot_metrics;
pub use driver::{plot_aggregated_metrics, plot_streaming_metrics};
