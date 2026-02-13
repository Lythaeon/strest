use ratatui::text::Span;

use crate::ui::model::UiRenderData;

use super::formatting::{
    MS_PER_SEC, TENTHS_DIVISOR, axis_tick_value, centered_axis_labels, centered_y_axis_labels,
    format_count_compact, select_bytes_scale,
};
use super::theme::{MIN_DATA_Y_MAX, MIN_LATENCY_MS, MIN_RPS_Y_MAX, MIN_WINDOW_MS, MIN_Y_MAX};

pub struct ChartWindowData {
    pub window_label: String,
    pub x_span: u64,
    pub y_max: u64,
    pub rps_y_max: u64,
    pub data_y_max_scaled: u64,
    pub data_unit: &'static str,
    pub x_axis_labels: Vec<Span<'static>>,
    pub latency_y_labels: Vec<Span<'static>>,
    pub rps_y_labels: Vec<Span<'static>>,
    pub data_y_labels: Vec<Span<'static>>,
    pub latency_chart_points: Vec<(f64, f64)>,
    pub compare_latency_chart_points: Vec<(f64, f64)>,
    pub rps_chart_points: Vec<(f64, f64)>,
    pub compare_rps_chart_points: Vec<(f64, f64)>,
    pub data_chart_points: Vec<(f64, f64)>,
    pub compare_data_chart_points: Vec<(f64, f64)>,
}

pub fn compute_chart_window(data: &UiRenderData) -> ChartWindowData {
    let compare = data.compare.as_ref();
    let latency_points = data.latencies.as_slice();
    let rps_points = data.rps_series.as_slice();
    let data_points = data
        .data_usage
        .as_ref()
        .map(|usage| usage.series.as_slice())
        .unwrap_or(&[]);
    let compare_latency_points = compare
        .map(|value| value.latencies.as_slice())
        .unwrap_or(&[]);
    let compare_rps_points = compare
        .map(|value| value.rps_series.as_slice())
        .unwrap_or(&[]);
    let compare_data_points = compare
        .and_then(|value| value.data_usage.as_ref())
        .map(|usage| usage.series.as_slice())
        .unwrap_or(&[]);

    let mut y_max = data
        .latencies
        .iter()
        .map(|(_, latency)| (*latency).max(MIN_LATENCY_MS))
        .fold(MIN_LATENCY_MS, u64::max)
        .max(MIN_Y_MAX);
    if let Some(compare) = compare {
        let compare_max = compare
            .latencies
            .iter()
            .map(|(_, latency)| (*latency).max(MIN_LATENCY_MS))
            .fold(MIN_LATENCY_MS, u64::max);
        y_max = y_max.max(compare_max);
    }

    let mut rps_y_max = data
        .rps_series
        .iter()
        .map(|(_, value)| *value)
        .fold(0, u64::max)
        .max(MIN_RPS_Y_MAX);
    if let Some(compare) = compare {
        let compare_max = compare
            .rps_series
            .iter()
            .map(|(_, value)| *value)
            .fold(0, u64::max);
        rps_y_max = rps_y_max.max(compare_max);
    }

    let mut data_y_max = data_points
        .iter()
        .map(|(_, value)| *value)
        .fold(0, u64::max)
        .max(MIN_DATA_Y_MAX);
    let compare_data_y_max = compare_data_points
        .iter()
        .map(|(_, value)| *value)
        .fold(0, u64::max);
    data_y_max = data_y_max.max(compare_data_y_max);

    let window_ms = data.ui_window_ms.max(MIN_WINDOW_MS);
    let x_max = latency_points
        .last()
        .map(|(x, _)| *x)
        .unwrap_or(0)
        .max(rps_points.last().map(|(x, _)| *x).unwrap_or(0))
        .max(data_points.last().map(|(x, _)| *x).unwrap_or(0))
        .max(compare_latency_points.last().map(|(x, _)| *x).unwrap_or(0))
        .max(compare_rps_points.last().map(|(x, _)| *x).unwrap_or(0))
        .max(compare_data_points.last().map(|(x, _)| *x).unwrap_or(0));
    let x_start = x_max.saturating_sub(window_ms);
    let x_span = x_max.saturating_sub(x_start).max(MIN_WINDOW_MS);

    let latency_chart_points: Vec<(f64, f64)> = latency_points
        .iter()
        .filter(|(x, _)| *x >= x_start)
        .map(|(x, y)| {
            (
                x.saturating_sub(x_start) as f64,
                (*y).max(MIN_LATENCY_MS) as f64,
            )
        })
        .collect();
    let compare_latency_chart_points: Vec<(f64, f64)> = compare_latency_points
        .iter()
        .filter(|(x, _)| *x >= x_start)
        .map(|(x, y)| {
            (
                x.saturating_sub(x_start) as f64,
                (*y).max(MIN_LATENCY_MS) as f64,
            )
        })
        .collect();
    let rps_chart_points: Vec<(f64, f64)> = rps_points
        .iter()
        .filter(|(x, _)| *x >= x_start)
        .map(|(x, y)| (x.saturating_sub(x_start) as f64, *y as f64))
        .collect();
    let compare_rps_chart_points: Vec<(f64, f64)> = compare_rps_points
        .iter()
        .filter(|(x, _)| *x >= x_start)
        .map(|(x, y)| (x.saturating_sub(x_start) as f64, *y as f64))
        .collect();

    let (data_scale, data_unit) = select_bytes_scale(data_y_max);
    let data_y_max_scaled = data_y_max.checked_div(data_scale).unwrap_or(0).max(1);
    let data_chart_points: Vec<(f64, f64)> = data_points
        .iter()
        .filter(|(x, _)| *x >= x_start)
        .map(|(x, y)| {
            let scaled = y.checked_div(data_scale).unwrap_or(0);
            (x.saturating_sub(x_start) as f64, scaled as f64)
        })
        .collect();
    let compare_data_chart_points: Vec<(f64, f64)> = compare_data_points
        .iter()
        .filter(|(x, _)| *x >= x_start)
        .map(|(x, y)| {
            let scaled = y.checked_div(data_scale).unwrap_or(0);
            (x.saturating_sub(x_start) as f64, scaled as f64)
        })
        .collect();

    let fmt_secs = |ms: u64| {
        let secs = ms / MS_PER_SEC;
        let tenths = (ms % MS_PER_SEC) / TENTHS_DIVISOR;
        format!("{}.{:01}s", secs, tenths)
    };
    let label_left = x_start;
    let label_low_mid = x_start.saturating_add(axis_tick_value(x_span, 1));
    let label_mid = x_start.saturating_add(x_span / 2);
    let label_mid_high = x_start.saturating_add(axis_tick_value(x_span, 3));
    let label_right = x_start.saturating_add(x_span);
    let x_axis_labels = centered_axis_labels([
        fmt_secs(label_left),
        fmt_secs(label_low_mid),
        fmt_secs(label_mid),
        fmt_secs(label_mid_high),
        fmt_secs(label_right),
    ]);
    let latency_y_labels = centered_y_axis_labels([
        format_count_compact(axis_tick_value(y_max, 0)),
        format_count_compact(axis_tick_value(y_max, 1)),
        format_count_compact(axis_tick_value(y_max, 2)),
        format_count_compact(axis_tick_value(y_max, 3)),
        format_count_compact(y_max),
    ]);
    let rps_y_labels = centered_y_axis_labels([
        format_count_compact(axis_tick_value(rps_y_max, 0)),
        format_count_compact(axis_tick_value(rps_y_max, 1)),
        format_count_compact(axis_tick_value(rps_y_max, 2)),
        format_count_compact(axis_tick_value(rps_y_max, 3)),
        format_count_compact(rps_y_max),
    ]);
    let data_y_labels = centered_axis_labels([
        format_count_compact(axis_tick_value(data_y_max_scaled, 0)),
        format_count_compact(axis_tick_value(data_y_max_scaled, 1)),
        format_count_compact(axis_tick_value(data_y_max_scaled, 2)),
        format_count_compact(axis_tick_value(data_y_max_scaled, 3)),
        format_count_compact(data_y_max_scaled),
    ]);

    ChartWindowData {
        window_label: fmt_secs(window_ms),
        x_span,
        y_max,
        rps_y_max,
        data_y_max_scaled,
        data_unit,
        x_axis_labels,
        latency_y_labels,
        rps_y_labels,
        data_y_labels,
        latency_chart_points,
        compare_latency_chart_points,
        rps_chart_points,
        compare_rps_chart_points,
        data_chart_points,
        compare_data_chart_points,
    }
}
