mod processing;
mod summary;
mod windows;

pub(in crate::metrics::collector) use processing::process_metric_ui;
pub(in crate::metrics::collector) use summary::{
    build_sink_stats, build_stream_snapshot, resolve_sink_interval, resolve_stream_interval,
};
pub(in crate::metrics::collector) use windows::{
    compute_percentiles, prune_bytes_window, prune_latency_window, prune_rps_window,
    record_bytes_sample, record_rps_sample,
};
