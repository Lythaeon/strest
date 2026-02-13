use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    prelude::{Backend, Frame},
    widgets::{Block, Borders},
};

use crate::ui::model::UiRenderData;

use super::charts_status_data::{render_data_panel, render_status_panel};
use super::charts_window::compute_chart_window;
use super::theme::{
    ACCENT_GREEN_RGB, ACCENT_LATENCY_RGB, ACCENT_SERIES_COMPARE_RGB, ACCENT_SERIES_PRIMARY_RGB,
    CHART_COL_LEFT, CHART_COL_RIGHT, CHART_ROW_BOTTOM, CHART_ROW_TOP, DATA_PANEL_WIDTH,
    STATUS_PANEL_MAX_WIDTH, axis_style, chart_surface_style, panel_block_style, panel_border_style,
    panel_title_style, rgb, style_color,
};

pub fn render_charts<B: Backend>(f: &mut Frame<'_, B>, data: &UiRenderData, chart_chunk: Rect) {
    let chart_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(CHART_ROW_TOP),
            Constraint::Percentage(CHART_ROW_BOTTOM),
        ])
        .split(chart_chunk);

    let (chart_top, chart_bottom) = match chart_rows.as_ref() {
        [a, b] => (a, b),
        _ => return,
    };

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(CHART_COL_LEFT),
            Constraint::Percentage(CHART_COL_RIGHT),
        ])
        .split(*chart_top);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(STATUS_PANEL_MAX_WIDTH),
            Constraint::Percentage(DATA_PANEL_WIDTH),
        ])
        .split(*chart_bottom);

    let (latency_chart_chunk, rps_chunk) = match top_chunks.as_ref() {
        [a, b] => (a, b),
        _ => return,
    };
    let (status_chunk, data_chunk) = match bottom_chunks.as_ref() {
        [a, b] => (a, b),
        _ => return,
    };

    let compare_mode = data.compare.is_some();
    let chart = compute_chart_window(data);

    let mut latency_datasets = vec![
        ratatui::widgets::Dataset::default()
            .name("Current")
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(style_color(
                data.no_color,
                if compare_mode {
                    rgb(ACCENT_SERIES_PRIMARY_RGB)
                } else {
                    rgb(ACCENT_LATENCY_RGB)
                },
            ))
            .data(&chart.latency_chart_points),
    ];
    if let Some(compare) = data.compare.as_ref()
        && !chart.compare_latency_chart_points.is_empty()
    {
        latency_datasets.push(
            ratatui::widgets::Dataset::default()
                .name(compare.label.as_str())
                .graph_type(ratatui::widgets::GraphType::Line)
                .style(style_color(data.no_color, rgb(ACCENT_SERIES_COMPARE_RGB)))
                .data(&chart.compare_latency_chart_points),
        );
    }

    let latency_chart = ratatui::widgets::Chart::new(latency_datasets)
        .style(chart_surface_style(data.no_color))
        .block(
            Block::default()
                .title(format!("Latency (last {})", chart.window_label))
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
                .title("Latency (ms)")
                .style(axis_style(data.no_color))
                .bounds([0.0, chart.y_max as f64])
                .labels_alignment(Alignment::Center)
                .labels(chart.latency_y_labels.clone()),
        );

    let mut rps_datasets = vec![
        ratatui::widgets::Dataset::default()
            .name("Current")
            .graph_type(ratatui::widgets::GraphType::Line)
            .style(style_color(
                data.no_color,
                if compare_mode {
                    rgb(ACCENT_SERIES_PRIMARY_RGB)
                } else {
                    rgb(ACCENT_GREEN_RGB)
                },
            ))
            .data(&chart.rps_chart_points),
    ];
    if let Some(compare) = data.compare.as_ref()
        && !chart.compare_rps_chart_points.is_empty()
    {
        rps_datasets.push(
            ratatui::widgets::Dataset::default()
                .name(compare.label.as_str())
                .graph_type(ratatui::widgets::GraphType::Line)
                .style(style_color(data.no_color, rgb(ACCENT_SERIES_COMPARE_RGB)))
                .data(&chart.compare_rps_chart_points),
        );
    }

    let rps_chart = ratatui::widgets::Chart::new(rps_datasets)
        .style(chart_surface_style(data.no_color))
        .block(
            Block::default()
                .title(format!("RPS (last {})", chart.window_label))
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
                .title("Requests/s")
                .style(axis_style(data.no_color))
                .bounds([0.0, chart.rps_y_max as f64])
                .labels_alignment(Alignment::Center)
                .labels(chart.rps_y_labels.clone()),
        );

    f.render_widget(latency_chart, *latency_chart_chunk);
    f.render_widget(rps_chart, *rps_chunk);
    render_status_panel(f, data, *status_chunk);
    render_data_panel(f, data, &chart, *data_chunk);
}
