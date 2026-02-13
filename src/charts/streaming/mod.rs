mod basic;
mod breakdown;
mod latency;

pub use basic::{
    plot_average_response_time_from_buckets, plot_cumulative_error_rate_from_buckets,
    plot_cumulative_successful_requests_from_buckets, plot_cumulative_total_requests_from_buckets,
    plot_inflight_requests_from_counts, plot_requests_per_second_from_counts,
    plot_timeouts_per_second_from_counts,
};
pub use breakdown::{
    plot_error_rate_breakdown_from_counts, plot_status_code_distribution_from_counts,
};
pub use latency::{LatencyPercentilesSeries, plot_latency_percentiles_series};
