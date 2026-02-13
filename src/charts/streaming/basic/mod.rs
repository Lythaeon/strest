mod buckets;
mod counts;

pub use buckets::{
    plot_average_response_time_from_buckets, plot_cumulative_error_rate_from_buckets,
    plot_cumulative_successful_requests_from_buckets, plot_cumulative_total_requests_from_buckets,
};
pub use counts::{
    plot_inflight_requests_from_counts, plot_requests_per_second_from_counts,
    plot_timeouts_per_second_from_counts,
};
