mod lines;
mod percentiles;

use crate::args::TesterArgs;
use crate::metrics;

pub(crate) use lines::summary_lines;
pub(crate) use percentiles::compute_percentiles;

/// Minimum non-zero duration used to avoid divide-by-zero.
const MIN_DURATION_MS: u128 = 1;
/// Percent scale for success rate (x100 = 10_000).
const SUCCESS_RATE_SCALE: u128 = 10_000;
/// Scale for average RPS in hundredths.
const RPS_SCALE: u128 = 100_000;
/// Divisor to format x100 values as `xx.yy`.
pub(crate) const PERCENT_DIVISOR: u64 = 100;
/// RPM conversion factor from RPS.
const RPM_PER_RPS: u64 = 60;

pub(crate) struct SummaryExtras {
    pub(crate) metrics_truncated: bool,
    pub(crate) charts_output_path: Option<String>,
    pub(crate) p50: u64,
    pub(crate) p90: u64,
    pub(crate) p99: u64,
    pub(crate) success_p50: u64,
    pub(crate) success_p90: u64,
    pub(crate) success_p99: u64,
}

pub(crate) struct SummaryStats {
    pub(crate) success_rate_x100: u64,
    pub(crate) avg_rps_x100: u64,
    pub(crate) avg_rpm_x100: u64,
}

pub(crate) fn compute_summary_stats(summary: &metrics::MetricsSummary) -> SummaryStats {
    let duration_ms = summary.duration.as_millis().max(MIN_DURATION_MS);
    let total = summary.total_requests;
    let success = summary.successful_requests;

    let success_rate_x100 = if total > 0 {
        let scaled = u128::from(success)
            .saturating_mul(SUCCESS_RATE_SCALE)
            .checked_div(u128::from(total))
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
    } else {
        0
    };

    let avg_rps_x100 = if total > 0 {
        let scaled = u128::from(total)
            .saturating_mul(RPS_SCALE)
            .checked_div(duration_ms)
            .unwrap_or(0);
        u64::try_from(scaled).map_or(u64::MAX, |value| value)
    } else {
        0
    };
    let avg_rpm_x100 = avg_rps_x100.saturating_mul(RPM_PER_RPS);

    SummaryStats {
        success_rate_x100,
        avg_rps_x100,
        avg_rpm_x100,
    }
}

pub(crate) fn print_summary(
    summary: &metrics::MetricsSummary,
    extras: &SummaryExtras,
    stats: &SummaryStats,
    args: &TesterArgs,
) {
    for line in summary_lines(summary, extras, stats, args) {
        println!("{}", line);
    }
}
