use ratatui::{
    layout::{Alignment, Rect},
    prelude::{Backend, Frame, text},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, Wrap},
};

use crate::ui::model::UiRenderData;

use super::charts_window::ChartWindowData;
use super::formatting::{format_bytes_compact, format_status_bar_value, select_count_scale};
use super::theme::{
    ACCENT_AMBER_RGB, ACCENT_DATA_RGB, ACCENT_GREEN_RGB, ACCENT_LATENCY_RGB, ACCENT_RATE_RGB,
    ACCENT_RED_RGB, ACCENT_SERIES_COMPARE_RGB, ACCENT_SERIES_PRIMARY_RGB, PANEL_TEXT_RGB,
    axis_style, chart_surface_style, panel_block_style, panel_border_style, panel_title_style, rgb,
    style_color,
};

pub fn render_status_panel<B: Backend>(
    f: &mut Frame<'_, B>,
    data: &UiRenderData,
    status_chunk: Rect,
) {
    let status_counts = data.status_counts.clone().unwrap_or_default();
    let status_data = [
        ("2xx", status_counts.status_2xx),
        ("3xx", status_counts.status_3xx),
        ("4xx", status_counts.status_4xx),
        ("5xx", status_counts.status_5xx),
        ("Other", status_counts.status_other),
    ];
    let status_max_raw = status_data
        .iter()
        .map(|(_, value)| *value)
        .fold(0, u64::max)
        .max(1);
    let (status_scale, _) = select_count_scale(status_max_raw);
    let status_chart_divisor = if status_scale > 1 {
        status_scale.checked_div(10).unwrap_or(1).max(1)
    } else {
        1
    };
    let status_data_scaled = [
        (
            status_data[0].0,
            status_data[0]
                .1
                .checked_div(status_chart_divisor)
                .unwrap_or(0),
        ),
        (
            status_data[1].0,
            status_data[1]
                .1
                .checked_div(status_chart_divisor)
                .unwrap_or(0),
        ),
        (
            status_data[2].0,
            status_data[2]
                .1
                .checked_div(status_chart_divisor)
                .unwrap_or(0),
        ),
        (
            status_data[3].0,
            status_data[3]
                .1
                .checked_div(status_chart_divisor)
                .unwrap_or(0),
        ),
        (
            status_data[4].0,
            status_data[4]
                .1
                .checked_div(status_chart_divisor)
                .unwrap_or(0),
        ),
    ];
    let status_bars = u16::try_from(status_data.len()).unwrap_or(1).max(1);
    let status_title = if data.status_counts.is_some() {
        "Status Codes".to_owned()
    } else {
        "Status Codes (unavailable)".to_owned()
    };
    let status_block = Block::default()
        .title(status_title)
        .borders(Borders::ALL)
        .style(panel_block_style(data.no_color))
        .border_style(panel_border_style(data.no_color))
        .title_style(panel_title_style(data.no_color));
    let status_inner = status_block.inner(status_chunk);
    f.render_widget(status_block, status_chunk);

    let status_inner_width = status_inner.width;
    let min_gap: u16 = if status_inner_width > status_bars.saturating_mul(2) {
        1
    } else {
        0
    };
    let total_gap = min_gap.saturating_mul(status_bars.saturating_sub(1));
    let dynamic_bar_width = status_inner_width
        .saturating_sub(total_gap)
        .checked_div(status_bars)
        .unwrap_or(1)
        .max(1);
    let status_max = status_data_scaled
        .iter()
        .map(|(_, value)| *value)
        .fold(0, u64::max)
        .max(1);
    let status_bars_data: Vec<Bar<'static>> = status_data_scaled
        .iter()
        .zip(status_data.iter())
        .map(|((label, scaled), (_, raw))| {
            let bar_color = match *label {
                "2xx" => rgb(ACCENT_GREEN_RGB),
                "3xx" => rgb(ACCENT_RATE_RGB),
                "4xx" => rgb(ACCENT_AMBER_RGB),
                "5xx" => rgb(ACCENT_RED_RGB),
                _ => rgb(ACCENT_LATENCY_RGB),
            };
            Bar::default()
                .label(text::Line::from(*label))
                .value(*scaled)
                .style(style_color(data.no_color, bar_color))
                .text_value(format_status_bar_value(*raw))
        })
        .collect();
    let status_widget = BarChart::default()
        .data(BarGroup::default().bars(&status_bars_data))
        .bar_width(dynamic_bar_width)
        .bar_gap(min_gap)
        .max(status_max)
        .style(chart_surface_style(data.no_color))
        .bar_style(style_color(data.no_color, rgb(PANEL_TEXT_RGB)))
        .value_style(style_color(data.no_color, rgb(PANEL_TEXT_RGB)))
        .label_style(style_color(data.no_color, rgb(PANEL_TEXT_RGB)));

    f.render_widget(status_widget, status_inner);
}

pub fn render_data_panel<B: Backend>(
    f: &mut Frame<'_, B>,
    data: &UiRenderData,
    chart: &ChartWindowData,
    data_chunk: Rect,
) {
    if chart.data_chart_points.is_empty() && chart.compare_data_chart_points.is_empty() {
        let placeholder = Paragraph::new(text::Line::from("Data usage unavailable"))
            .block(Block::default().title("Data Usage").borders(Borders::ALL))
            .wrap(Wrap { trim: true });
        f.render_widget(placeholder, data_chunk);
        return;
    }

    let compare_mode = data.compare.is_some();
    let (total_label, rate_label) = data
        .data_usage
        .as_ref()
        .map(|usage| {
            (
                format_bytes_compact(usage.total_bytes),
                format!(
                    "{}/s",
                    format_bytes_compact(u128::from(usage.bytes_per_sec))
                ),
            )
        })
        .unwrap_or_else(|| ("0B".to_owned(), "0B/s".to_owned()));

    let mut data_datasets = Vec::new();
    if !chart.data_chart_points.is_empty() {
        data_datasets.push(
            ratatui::widgets::Dataset::default()
                .name("Current")
                .graph_type(ratatui::widgets::GraphType::Line)
                .style(style_color(
                    data.no_color,
                    if compare_mode {
                        rgb(ACCENT_SERIES_PRIMARY_RGB)
                    } else {
                        rgb(ACCENT_DATA_RGB)
                    },
                ))
                .data(&chart.data_chart_points),
        );
    }
    if let Some(compare) = data.compare.as_ref()
        && !chart.compare_data_chart_points.is_empty()
    {
        data_datasets.push(
            ratatui::widgets::Dataset::default()
                .name(compare.label.as_str())
                .graph_type(ratatui::widgets::GraphType::Line)
                .style(style_color(data.no_color, rgb(ACCENT_SERIES_COMPARE_RGB)))
                .data(&chart.compare_data_chart_points),
        );
    }

    let data_chart = ratatui::widgets::Chart::new(data_datasets)
        .style(chart_surface_style(data.no_color))
        .block(
            Block::default()
                .title(format!(
                    "Data (last {}, total {}, rate {})",
                    chart.window_label, total_label, rate_label
                ))
                .borders(Borders::ALL)
                .style(panel_block_style(data.no_color))
                .border_style(panel_border_style(data.no_color))
                .title_style(panel_title_style(data.no_color)),
        )
        .x_axis(
            ratatui::widgets::Axis::default()
                .title("Elapsed (s)")
                .style(axis_style(data.no_color))
                .bounds([0.0, chart.x_span as f64])
                .labels(chart.x_axis_labels.clone()),
        )
        .y_axis(
            ratatui::widgets::Axis::default()
                .title(format!("{}/s", chart.data_unit))
                .style(axis_style(data.no_color))
                .bounds([0.0, chart.data_y_max_scaled as f64])
                .labels_alignment(Alignment::Center)
                .labels(chart.data_y_labels.clone()),
        );
    f.render_widget(data_chart, data_chunk);
}
