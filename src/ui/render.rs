use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    prelude::{Backend, text},
    style::{Color, Style},
    text::Span,
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Gauge, Paragraph, Wrap},
};
use std::io;
use std::time::Duration;
use tokio::sync::watch;

use crate::args::TesterArgs;
use crate::error::AppResult;
use crate::shutdown::ShutdownSender;

use super::model::{UiData, UiRenderData};

fn style_color(no_color: bool, color: Color) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(color)
    }
}

const fn rgb(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

fn panel_block_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default()
            .bg(rgb(CHART_BG_RGB))
            .fg(rgb(PANEL_TEXT_RGB))
    }
}

fn panel_border_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(rgb(PANEL_BORDER_RGB))
    }
}

fn panel_title_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(rgb(PANEL_TEXT_RGB))
    }
}

fn axis_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(rgb(PANEL_MUTED_RGB))
    }
}

fn app_background_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().bg(rgb(CHART_BG_RGB))
    }
}

pub trait UiActions {
    /// Initializes the terminal for UI rendering.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal setup fails.
    fn setup_terminal() -> AppResult<Terminal<CrosstermBackend<std::io::Stdout>>>;
    fn cleanup();
    fn render<B: Backend>(terminal: &mut Terminal<B>, data: &UiRenderData);
}

pub struct Ui;

const BANNER_LINES: [&str; 7] = [
    "███████╗████████╗██████╗ ███████╗███████╗████████╗",
    "██╔════╝╚══██╔══╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝",
    "███████╗   ██║   ██████╔╝█████╗  ███████╗   ██║   ",
    "╚════██║   ██║   ██╔══██╗██╔══╝  ╚════██║   ██║   ",
    "███████║   ██║   ██║  ██║███████╗███████║   ██║   ",
    "╚══════╝   ╚═╝   ╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝   ",
    "                                                   ",
];
/// Outer margin for the full UI layout.
const UI_MARGIN: u16 = 1;
/// Fixed height for the summary row.
const SUMMARY_HEIGHT: u16 = 7;
/// Minimum height for the chart area to avoid rendering errors.
const CHART_MIN_HEIGHT: u16 = 10;
/// Column width percentages for chart panels.
const CHART_COL_LEFT: u16 = 50;
const CHART_COL_RIGHT: u16 = 50;
/// Bottom row status panel max width percentage.
const STATUS_PANEL_MAX_WIDTH: u16 = 40;
/// Bottom row data panel width percentage.
const DATA_PANEL_WIDTH: u16 = 60;
/// Row height percentages for chart panels.
const CHART_ROW_TOP: u16 = 50;
const CHART_ROW_BOTTOM: u16 = 50;
/// Background color for chart surfaces.
const CHART_BG_RGB: (u8, u8, u8) = (0x0a, 0x0a, 0x0a);
/// Border color for panel chrome.
const PANEL_BORDER_RGB: (u8, u8, u8) = (0xe5, 0xe7, 0xeb);
/// Primary text color for panel content.
const PANEL_TEXT_RGB: (u8, u8, u8) = (0xff, 0xff, 0xff);
/// Muted text color for axes and secondary labels.
const PANEL_MUTED_RGB: (u8, u8, u8) = (0xd1, 0xd5, 0xdb);
/// Progress indicator color.
const ACCENT_PROGRESS_RGB: (u8, u8, u8) = (0x22, 0xd3, 0xee);
/// Concurrency and in-flight counters.
const ACCENT_LOAD_RGB: (u8, u8, u8) = (0x38, 0xbd, 0xf8);
/// Throughput counters (RPM/Requests).
const ACCENT_RATE_RGB: (u8, u8, u8) = (0x60, 0xa5, 0xfa);
/// Data throughput and transfer signals.
const ACCENT_DATA_RGB: (u8, u8, u8) = (0xa7, 0x8b, 0xfa);
/// Replay state and lifecycle markers.
const ACCENT_REPLAY_RGB: (u8, u8, u8) = (0xc0, 0x84, 0xfc);
/// Latency-series plotting color.
const ACCENT_LATENCY_RGB: (u8, u8, u8) = (0xf4, 0x72, 0xb6);
/// Positive metric color.
const ACCENT_GREEN_RGB: (u8, u8, u8) = (0x22, 0xc5, 0x5e);
/// Warning metric color.
const ACCENT_AMBER_RGB: (u8, u8, u8) = (0xf5, 0x9e, 0x0b);
/// Error metric color.
const ACCENT_RED_RGB: (u8, u8, u8) = (0xef, 0x44, 0x44);
/// Column width percentages for summary KPI panels.
const SUMMARY_COL_PROGRESS: u16 = 20;
const SUMMARY_COL_THROUGHPUT: u16 = 20;
const SUMMARY_COL_RELIABILITY: u16 = 20;
const SUMMARY_COL_LATENCY: u16 = 20;
const SUMMARY_COL_FLOW: u16 = 20;
/// Number of axis segments (4 segments = 5 tick labels).
const AXIS_SEGMENTS: u64 = 4;
/// Extra characters reserved for y-axis label centering.
const Y_AXIS_LABEL_EXTRA_WIDTH: usize = 2;
/// Scale factor for percent values (x100 = 10_000).
const SUCCESS_RATE_SCALE: u128 = 10_000;
/// Percent divisor to format x100 values as `xx.yy`.
const PERCENT_DIVISOR: u64 = 100;
/// Milliseconds per second for UI time math.
const MS_PER_SEC: u64 = 1_000;
/// Divisor for tenths of a second in `ss.t` formatting.
const TENTHS_DIVISOR: u64 = 100;
/// Smallest allowed latency value in charts.
const MIN_LATENCY_MS: u64 = 1;
/// Minimum Y-axis max to avoid a flat chart.
const MIN_Y_MAX: u64 = 10;
/// Minimum Y-axis max for RPS to avoid a flat chart.
const MIN_RPS_Y_MAX: u64 = 1;
/// Minimum Y-axis max for data usage to avoid a flat chart.
const MIN_DATA_Y_MAX: u64 = 1;
/// Minimum window length for chart rendering.
const MIN_WINDOW_MS: u64 = 1;
/// Splash screen display duration.
const SPLASH_DURATION_SECS: u64 = 3;
/// Extra empty line padding around the banner.
const BANNER_PADDING_LINES: usize = 1;
/// Gradient start color for the banner.
const COLOR_START: (u8, u8, u8) = (0x80, 0x4c, 0xff);
/// Gradient midpoint color for the banner.
const COLOR_MID: (u8, u8, u8) = (0xff, 0x5f, 0xc8);
/// Gradient end color for the banner.
const COLOR_END: (u8, u8, u8) = (0x3a, 0xa9, 0xff);
/// Subtitle color under the splash banner.
const SPLASH_SUBTITLE_RGB: (u8, u8, u8) = (0xff, 0x5f, 0xc8);

impl UiActions for Ui {
    fn setup_terminal() -> AppResult<Terminal<CrosstermBackend<std::io::Stdout>>> {
        enable_raw_mode()?;
        if let Err(err) = execute!(io::stdout(), EnterAlternateScreen) {
            disable_raw_mode().ok();
            return Err(err.into());
        }

        let backend = CrosstermBackend::new(io::stdout());
        match Terminal::new(backend) {
            Ok(mut terminal) => {
                if let Err(err) = terminal.clear() {
                    Self::cleanup();
                    return Err(err.into());
                }
                Ok(terminal)
            }
            Err(err) => {
                Self::cleanup();
                Err(err.into())
            }
        }
    }

    fn cleanup() {
        disable_raw_mode().ok();
        execute!(std::io::stdout(), LeaveAlternateScreen).ok();
    }

    fn render<B: Backend>(terminal: &mut Terminal<B>, data: &UiRenderData) {
        if let Err(err) = terminal.draw(|f| {
            let size = f.size();
            f.render_widget(
                Block::default().style(app_background_style(data.no_color)),
                size,
            );

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(UI_MARGIN)
                .constraints([
                    Constraint::Length(SUMMARY_HEIGHT),
                    Constraint::Min(CHART_MIN_HEIGHT),
                ])
                .split(size);

            let (summary_chunk, chart_chunk) = match chunks.as_ref() {
                [a, b] => (a, b),
                _ => return,
            };

            let summary_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(SUMMARY_COL_PROGRESS),
                    Constraint::Percentage(SUMMARY_COL_THROUGHPUT),
                    Constraint::Percentage(SUMMARY_COL_RELIABILITY),
                    Constraint::Percentage(SUMMARY_COL_LATENCY),
                    Constraint::Percentage(SUMMARY_COL_FLOW),
                ])
                .split(*summary_chunk);

            let (run_chunk, throughput_chunk, reliability_chunk, latency_kpi_chunk, flow_chunk) =
                match summary_chunks.as_ref() {
                    [a, b, c, d, e] => (a, b, c, d, e),
                    _ => return,
                };

            let total_requests = data.current_request;
            let success_requests = data.successful_requests;
            let error_requests = total_requests.saturating_sub(success_requests);
            let success_rate_x100 = if total_requests > 0 {
                let scaled = u128::from(success_requests)
                    .saturating_mul(SUCCESS_RATE_SCALE)
                    .checked_div(u128::from(total_requests))
                    .unwrap_or(0);
                u64::try_from(scaled).unwrap_or(u64::MAX)
            } else {
                0
            };
            let error_rate_x100 = if total_requests > 0 {
                let scaled = u128::from(error_requests)
                    .saturating_mul(SUCCESS_RATE_SCALE)
                    .checked_div(u128::from(total_requests))
                    .unwrap_or(0);
                u64::try_from(scaled).unwrap_or(u64::MAX)
            } else {
                0
            };

            let run_block = Block::default()
                .title("Run")
                .borders(Borders::ALL)
                .style(panel_block_style(data.no_color))
                .border_style(panel_border_style(data.no_color))
                .title_style(panel_title_style(data.no_color));
            let run_inner = run_block.inner(*run_chunk);
            f.render_widget(run_block, *run_chunk);

            let run_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(0)])
                .split(run_inner);

            let target_ms = data.target_duration.as_millis();
            let elapsed_ms = data.elapsed_time.as_millis();
            let progress_percent = if target_ms > 0 {
                let scaled = elapsed_ms
                    .saturating_mul(100)
                    .checked_div(target_ms)
                    .unwrap_or(0)
                    .min(100);
                u16::try_from(scaled).unwrap_or(100)
            } else {
                0
            };
            let progress_label = if target_ms > 0 {
                format!(
                    "{} / {}",
                    format_ms_as_tenths(elapsed_ms),
                    format_ms_as_tenths(target_ms)
                )
            } else {
                format!("{} / --", format_ms_as_tenths(elapsed_ms))
            };
            let progress = Gauge::default()
                .percent(progress_percent)
                .label(progress_label)
                .gauge_style(style_color(data.no_color, rgb(ACCENT_PROGRESS_RGB)));

            let run_details = data.replay.as_ref().map_or_else(
                || {
                    vec![text::Line::from(vec![
                        Span::from("In-flight: "),
                        Span::styled(
                            format_count_compact(data.in_flight_ops),
                            style_color(data.no_color, rgb(ACCENT_LOAD_RGB)),
                        ),
                    ])]
                },
                |replay| {
                    let status = if replay.playing { "playing" } else { "paused" };
                    let snapshot_start = replay.snapshot_start_ms.map_or_else(
                        || "-".to_owned(),
                        |value| format_ms_as_tenths(u128::from(value)),
                    );
                    let snapshot_end = replay.snapshot_end_ms.map_or_else(
                        || "-".to_owned(),
                        |value| format_ms_as_tenths(u128::from(value)),
                    );
                    vec![
                        text::Line::from(vec![
                            Span::from("Replay: "),
                            Span::styled(
                                status,
                                style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                            ),
                            Span::from("  Cursor: "),
                            Span::styled(
                                format_ms_as_tenths(u128::from(replay.cursor_ms)),
                                style_color(data.no_color, rgb(PANEL_MUTED_RGB)),
                            ),
                        ]),
                        text::Line::from(vec![
                            Span::from("Window: "),
                            Span::styled(
                                format!(
                                    "{} -> {}",
                                    format_ms_as_tenths(u128::from(replay.window_start_ms)),
                                    format_ms_as_tenths(u128::from(replay.window_end_ms))
                                ),
                                style_color(data.no_color, rgb(PANEL_MUTED_RGB)),
                            ),
                            Span::from("  Snap: "),
                            Span::styled(
                                format!("{snapshot_start} / {snapshot_end}"),
                                style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                            ),
                        ]),
                    ]
                },
            );
            let run_text = Paragraph::new(run_details)
                .style(panel_block_style(data.no_color))
                .wrap(Wrap { trim: true });

            let throughput_text = Paragraph::new(vec![
                text::Line::from(vec![
                    Span::from("RPS: "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(data.rps)),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from("   RPM: "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(data.rpm)),
                        style_color(data.no_color, rgb(ACCENT_RATE_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Requests: "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(total_requests)),
                        style_color(data.no_color, rgb(ACCENT_RATE_RGB)),
                    ),
                ]),
            ])
            .block(
                Block::default()
                    .title("Throughput")
                    .borders(Borders::ALL)
                    .style(panel_block_style(data.no_color))
                    .border_style(panel_border_style(data.no_color))
                    .title_style(panel_title_style(data.no_color)),
            )
            .style(panel_block_style(data.no_color))
            .wrap(Wrap { trim: true });

            let reliability_text = Paragraph::new(vec![
                text::Line::from(vec![
                    Span::from("Success: "),
                    Span::styled(
                        format!(
                            "{}.{:02}%",
                            success_rate_x100 / PERCENT_DIVISOR,
                            success_rate_x100 % PERCENT_DIVISOR
                        ),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Error: "),
                    Span::styled(
                        format!(
                            "{}.{:02}%",
                            error_rate_x100 / PERCENT_DIVISOR,
                            error_rate_x100 % PERCENT_DIVISOR
                        ),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from(" ("),
                    Span::styled(
                        format_count_compact(error_requests),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from(")"),
                ]),
            ])
            .block(
                Block::default()
                    .title("Reliability")
                    .borders(Borders::ALL)
                    .style(panel_block_style(data.no_color))
                    .border_style(panel_border_style(data.no_color))
                    .title_style(panel_title_style(data.no_color)),
            )
            .style(panel_block_style(data.no_color))
            .wrap(Wrap { trim: true });

            let latency_text = Paragraph::new(vec![
                text::Line::from(vec![
                    Span::from("P50: "),
                    Span::styled(
                        format!("{}ms", data.p50),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from("   P90: "),
                    Span::styled(
                        format!("{}ms", data.p90),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from("   P99: "),
                    Span::styled(
                        format!("{}ms", data.p99),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("OK P50: "),
                    Span::styled(
                        format!("{}ms", data.p50_ok),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from("  P90: "),
                    Span::styled(
                        format!("{}ms", data.p90_ok),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from("  P99: "),
                    Span::styled(
                        format!("{}ms", data.p99_ok),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                ]),
            ])
            .block(
                Block::default()
                    .title("Latency")
                    .borders(Borders::ALL)
                    .style(panel_block_style(data.no_color))
                    .border_style(panel_border_style(data.no_color))
                    .title_style(panel_title_style(data.no_color)),
            )
            .style(panel_block_style(data.no_color))
            .wrap(Wrap { trim: true });

            let flow_text = Paragraph::new(vec![
                text::Line::from(vec![
                    Span::from("In-flight: "),
                    Span::styled(
                        format!("{:>5}", format_count_compact(data.in_flight_ops)),
                        style_color(data.no_color, rgb(ACCENT_LOAD_RGB)),
                    ),
                    Span::from("   RX/s: "),
                    Span::styled(
                        data.data_usage.as_ref().map_or_else(
                            || "0B/s".to_owned(),
                            |usage| {
                                format!(
                                    "{}/s",
                                    format_bytes_compact(u128::from(usage.bytes_per_sec))
                                )
                            },
                        ),
                        style_color(data.no_color, rgb(ACCENT_DATA_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Timeout: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.timeout_requests)),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from("  Transport: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.transport_errors)),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from("  Non-Exp: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.non_expected_status)),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                ]),
            ])
            .block(
                Block::default()
                    .title("Flow")
                    .borders(Borders::ALL)
                    .style(panel_block_style(data.no_color))
                    .border_style(panel_border_style(data.no_color))
                    .title_style(panel_title_style(data.no_color)),
            )
            .style(panel_block_style(data.no_color))
            .wrap(Wrap { trim: true });

            if let Some(chunk) = run_chunks.first() {
                f.render_widget(progress, *chunk);
            }
            if let Some(chunk) = run_chunks.get(1) {
                f.render_widget(run_text, *chunk);
            }
            f.render_widget(throughput_text, *throughput_chunk);
            f.render_widget(reliability_text, *reliability_chunk);
            f.render_widget(latency_text, *latency_kpi_chunk);
            f.render_widget(flow_text, *flow_chunk);

            let chart_rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(CHART_ROW_TOP),
                    Constraint::Percentage(CHART_ROW_BOTTOM),
                ])
                .split(*chart_chunk);

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

            let latency_points = data.latencies.as_slice();
            let rps_points = data.rps_series.as_slice();
            let data_points = data
                .data_usage
                .as_ref()
                .map(|usage| usage.series.as_slice())
                .unwrap_or(&[]);
            let y_max = data
                .latencies
                .iter()
                .map(|(_, latency)| (*latency).max(MIN_LATENCY_MS))
                .fold(MIN_LATENCY_MS, u64::max)
                .max(MIN_Y_MAX);
            let rps_y_max = data
                .rps_series
                .iter()
                .map(|(_, value)| *value)
                .fold(0, u64::max)
                .max(MIN_RPS_Y_MAX);
            let data_y_max = data_points
                .iter()
                .map(|(_, value)| *value)
                .fold(0, u64::max)
                .max(MIN_DATA_Y_MAX);
            let window_ms = data.ui_window_ms.max(MIN_WINDOW_MS);
            let x_max = latency_points
                .last()
                .map(|(x, _)| *x)
                .unwrap_or(0)
                .max(rps_points.last().map(|(x, _)| *x).unwrap_or(0))
                .max(data_points.last().map(|(x, _)| *x).unwrap_or(0));
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
            let rps_chart_points: Vec<(f64, f64)> = rps_points
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
            let window_label = fmt_secs(window_ms);
            let latency_datasets = vec![
                ratatui::widgets::Dataset::default()
                    .name("Latency Chart")
                    .marker(ratatui::symbols::Marker::Dot)
                    .style(style_color(data.no_color, rgb(ACCENT_LATENCY_RGB)))
                    .data(&latency_chart_points),
            ];

            let latency_chart = ratatui::widgets::Chart::new(latency_datasets)
                .style(chart_surface_style(data.no_color))
                .block(
                    Block::default()
                        .title(format!("Latency (last {})", window_label))
                        .borders(Borders::ALL)
                        .style(panel_block_style(data.no_color))
                        .border_style(panel_border_style(data.no_color))
                        .title_style(panel_title_style(data.no_color)),
                )
                .x_axis(
                    ratatui::widgets::Axis::default()
                        .title("Elapsed (s)")
                        .style(axis_style(data.no_color))
                        .bounds([0.0, x_span as f64])
                        .labels(x_axis_labels.clone()),
                )
                .y_axis(
                    ratatui::widgets::Axis::default()
                        .title("Latency (ms)")
                        .style(axis_style(data.no_color))
                        .bounds([0.0, y_max as f64])
                        .labels_alignment(Alignment::Center)
                        .labels(latency_y_labels),
                );

            let rps_datasets = vec![
                ratatui::widgets::Dataset::default()
                    .name("RPS Chart")
                    .marker(ratatui::symbols::Marker::Braille)
                    .style(style_color(data.no_color, rgb(ACCENT_GREEN_RGB)))
                    .data(&rps_chart_points),
            ];

            let rps_chart = ratatui::widgets::Chart::new(rps_datasets)
                .style(chart_surface_style(data.no_color))
                .block(
                    Block::default()
                        .title(format!("RPS (last {})", window_label))
                        .borders(Borders::ALL)
                        .style(panel_block_style(data.no_color))
                        .border_style(panel_border_style(data.no_color))
                        .title_style(panel_title_style(data.no_color)),
                )
                .x_axis(
                    ratatui::widgets::Axis::default()
                        .title("Elapsed (s)")
                        .style(axis_style(data.no_color))
                        .bounds([0.0, x_span as f64])
                        .labels(x_axis_labels.clone()),
                )
                .y_axis(
                    ratatui::widgets::Axis::default()
                        .title("Requests/s")
                        .style(axis_style(data.no_color))
                        .bounds([0.0, rps_y_max as f64])
                        .labels_alignment(Alignment::Center)
                        .labels(rps_y_labels),
                );

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
            let status_inner = status_block.inner(*status_chunk);
            f.render_widget(status_block, *status_chunk);

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

            f.render_widget(latency_chart, *latency_chart_chunk);
            f.render_widget(rps_chart, *rps_chunk);
            f.render_widget(status_widget, status_inner);
            if data_points.is_empty() {
                let placeholder = Paragraph::new(text::Line::from("Data usage unavailable"))
                    .block(Block::default().title("Data Usage").borders(Borders::ALL))
                    .wrap(Wrap { trim: true });
                f.render_widget(placeholder, *data_chunk);
            } else {
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

                let data_datasets = vec![
                    ratatui::widgets::Dataset::default()
                        .name("Data Usage")
                        .marker(ratatui::symbols::Marker::Dot)
                        .style(style_color(data.no_color, rgb(ACCENT_DATA_RGB)))
                        .data(&data_chart_points),
                ];

                let data_chart = ratatui::widgets::Chart::new(data_datasets)
                    .style(chart_surface_style(data.no_color))
                    .block(
                        Block::default()
                            .title(format!(
                                "Data (last {}, total {}, rate {})",
                                window_label, total_label, rate_label
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
                            .bounds([0.0, x_span as f64])
                            .labels(x_axis_labels),
                    )
                    .y_axis(
                        ratatui::widgets::Axis::default()
                            .title(format!("{}/s", data_unit))
                            .style(axis_style(data.no_color))
                            .bounds([0.0, data_y_max_scaled as f64])
                            .labels_alignment(Alignment::Center)
                            .labels(data_y_labels),
                    );
                f.render_widget(data_chart, *data_chunk);
            }
        }) {
            eprintln!("Failed to render UI: {}", err);
        }
    }
}

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        Ui::cleanup();
    }
}

#[must_use]
pub fn setup_render_ui(
    _args: &TesterArgs,
    shutdown_tx: &ShutdownSender,
    ui_tx: &watch::Sender<UiData>,
) -> tokio::task::JoinHandle<()> {
    let mut ui_rx = ui_tx.subscribe();
    let mut shutdown_rx = shutdown_tx.subscribe();
    tokio::spawn(async move {
        let mut terminal = match Ui::setup_terminal() {
            Ok(terminal) => terminal,
            Err(err) => {
                eprintln!("Failed to setup terminal: {}", err);
                return;
            }
        };
        let _guard = TerminalGuard;

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => break,
                res = ui_rx.changed() => {
                    if res.is_ok() {
                        let msg = ui_rx.borrow().clone();
                        let data = UiRenderData::from(&msg);
                        Ui::render(&mut terminal, &data);
                    } else {
                        break;
                    }
                }
            }
        }
    })
}

/// Render a short splash screen before the main UI starts.
///
/// # Errors
///
/// Returns an error if the terminal setup fails.
pub async fn run_splash_screen(no_color: bool) -> AppResult<()> {
    let mut terminal = Ui::setup_terminal()?;
    let _guard = TerminalGuard;

    render_splash(&mut terminal, no_color);
    tokio::time::sleep(Duration::from_secs(SPLASH_DURATION_SECS)).await;
    Ok(())
}

fn render_splash<B: Backend>(terminal: &mut Terminal<B>, no_color: bool) {
    if let Err(err) = terminal.draw(|f| {
        let size = f.size();
        f.render_widget(Block::default().style(app_background_style(no_color)), size);
        let banner_height = BANNER_LINES.len().saturating_add(BANNER_PADDING_LINES);
        let available_height = usize::from(size.height);
        let top_pad = available_height.saturating_sub(banner_height) / 2;

        let mut lines = Vec::with_capacity(banner_height.saturating_add(top_pad).saturating_add(1));
        for _ in 0..top_pad {
            lines.push(text::Line::from(""));
        }

        let denom = BANNER_LINES.len().saturating_sub(1);
        for (idx, line) in BANNER_LINES.iter().enumerate() {
            let color = tri_gradient_color(COLOR_START, COLOR_MID, COLOR_END, idx, denom);
            let style = style_color(no_color, color);
            lines.push(text::Line::from(Span::styled((*line).to_owned(), style)));
        }

        lines.push(text::Line::from(""));

        let description = format!(
            "strest v{} | {} | stress testing",
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_LICENSE")
        );
        lines.push(text::Line::from(Span::styled(
            description,
            style_color(no_color, rgb(SPLASH_SUBTITLE_RGB)),
        )));

        let banner = Paragraph::new(lines)
            .style(app_background_style(no_color))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        f.render_widget(banner, size);
    }) {
        eprintln!("Failed to render splash screen: {}", err);
    }
}

fn format_ms_as_tenths(ms: u128) -> String {
    let sec_divisor = u128::from(MS_PER_SEC);
    let tenth_divisor = u128::from(TENTHS_DIVISOR);
    let secs = ms.checked_div(sec_divisor).unwrap_or(0);
    let rem = ms.checked_rem(sec_divisor).unwrap_or(0);
    let tenths = rem.checked_div(tenth_divisor).unwrap_or(0);
    format!("{}.{}s", secs, tenths)
}

fn format_bytes_compact(bytes: u128) -> String {
    const KB: u128 = 1_000;
    const MB: u128 = 1_000_000;
    const GB: u128 = 1_000_000_000;
    const TB: u128 = 1_000_000_000_000;

    if bytes >= TB {
        let whole = bytes / TB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(TB))
            .saturating_mul(100)
            .checked_div(TB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}TB")
    } else if bytes >= GB {
        let whole = bytes / GB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(GB))
            .saturating_mul(100)
            .checked_div(GB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}GB")
    } else if bytes >= MB {
        let whole = bytes / MB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(MB))
            .saturating_mul(100)
            .checked_div(MB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}MB")
    } else if bytes >= KB {
        let whole = bytes / KB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(KB))
            .saturating_mul(100)
            .checked_div(KB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}KB")
    } else {
        format!("{bytes}B")
    }
}

fn format_count_compact(value: u64) -> String {
    let (scale, suffix) = select_count_scale(value);
    if suffix.is_empty() {
        return value.to_string();
    }
    format_scaled_compact(value, scale, suffix)
}

fn format_status_bar_value(value: u64) -> String {
    if value >= 10_000 {
        let whole = value.checked_div(1_000).unwrap_or(0);
        let rem = value.checked_rem(1_000).unwrap_or(0);
        let frac = rem.saturating_mul(10).checked_div(1_000).unwrap_or(0);
        format!("{whole},{frac}k")
    } else {
        value.to_string()
    }
}

const fn select_bytes_scale(value: u64) -> (u64, &'static str) {
    if value >= 1_000_000_000_000 {
        (1_000_000_000_000, "TB")
    } else if value >= 1_000_000_000 {
        (1_000_000_000, "GB")
    } else if value >= 1_000_000 {
        (1_000_000, "MB")
    } else if value >= 1_000 {
        (1_000, "KB")
    } else {
        (1, "B")
    }
}

const fn select_count_scale(value: u64) -> (u64, &'static str) {
    if value >= 1_000_000_000_000 {
        (1_000_000_000_000, "t")
    } else if value >= 1_000_000_000 {
        (1_000_000_000, "g")
    } else if value >= 1_000_000 {
        (1_000_000, "m")
    } else if value >= 10_000 {
        (1_000, "k")
    } else {
        (1, "")
    }
}

fn format_scaled_compact(value: u64, scale: u64, suffix: &str) -> String {
    let whole = value.checked_div(scale).unwrap_or(0);
    let rem = value.checked_rem(scale).unwrap_or(0);

    if whole < 10 {
        let frac = rem.saturating_mul(100).checked_div(scale).unwrap_or(0);
        format!("{whole}.{frac:02}{suffix}")
    } else if whole < 100 {
        let frac = rem.saturating_mul(10).checked_div(scale).unwrap_or(0);
        format!("{whole}.{frac:01}{suffix}")
    } else {
        format!("{whole}{suffix}")
    }
}

fn axis_tick_value(max_value: u64, step: u64) -> u64 {
    max_value
        .saturating_mul(step)
        .checked_div(AXIS_SEGMENTS)
        .unwrap_or(0)
}

fn centered_axis_labels(labels: [String; 5]) -> Vec<Span<'static>> {
    centered_axis_labels_with_extra(labels, 0)
}

fn centered_y_axis_labels(labels: [String; 5]) -> Vec<Span<'static>> {
    centered_axis_labels_with_extra(labels, Y_AXIS_LABEL_EXTRA_WIDTH)
}

fn centered_axis_labels_with_extra(labels: [String; 5], extra: usize) -> Vec<Span<'static>> {
    let width = labels.iter().map(|label| label.len()).max().unwrap_or(1);
    let width = width.saturating_add(extra);
    labels
        .into_iter()
        .map(|label| Span::raw(format!("{label:^w$}", w = width)))
        .collect()
}

fn chart_surface_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default()
            .bg(rgb(CHART_BG_RGB))
            .fg(rgb(PANEL_TEXT_RGB))
    }
}

fn gradient_color(start: (u8, u8, u8), end: (u8, u8, u8), idx: usize, denom: usize) -> Color {
    let denom = denom.max(1) as i32;
    let idx = idx.min(usize::try_from(denom).unwrap_or(0)) as i32;
    let lerp = |a: u8, b: u8| -> u8 {
        let a = i32::from(a);
        let b = i32::from(b);
        let value = b
            .checked_sub(a)
            .and_then(|delta| delta.checked_mul(idx))
            .and_then(|scaled| scaled.checked_div(denom))
            .and_then(|step| a.checked_add(step))
            .unwrap_or(a);
        u8::try_from(value.clamp(0, 255)).unwrap_or(0)
    };
    Color::Rgb(
        lerp(start.0, end.0),
        lerp(start.1, end.1),
        lerp(start.2, end.2),
    )
}

fn tri_gradient_color(
    start: (u8, u8, u8),
    mid: (u8, u8, u8),
    end: (u8, u8, u8),
    idx: usize,
    denom: usize,
) -> Color {
    let denom = denom.max(1);
    let half = denom / 2;
    if idx <= half {
        gradient_color(start, mid, idx, half)
    } else {
        gradient_color(
            mid,
            end,
            idx.saturating_sub(half),
            denom.saturating_sub(half),
        )
    }
}
