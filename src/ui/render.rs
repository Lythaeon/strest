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
    widgets::{Block, Borders, Paragraph, Wrap},
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
/// Column width percentages for summary panels.
const SUMMARY_COL_LEFT: u16 = 34;
const SUMMARY_COL_MID: u16 = 33;
const SUMMARY_COL_RIGHT: u16 = 33;
/// Scale factor for percent values (x100 = 10_000).
const SUCCESS_RATE_SCALE: u128 = 10_000;
/// Percent divisor to format x100 values as `xx.yy`.
const PERCENT_DIVISOR: u64 = 100;
/// Milliseconds per second for UI time math.
const MS_PER_SEC: u64 = 1_000;
/// Divisor for centiseconds in `ss.cc` formatting.
const CENTIS_DIVISOR: u64 = 10;
/// Divisor for tenths of a second in `ss.t` formatting.
const TENTHS_DIVISOR: u64 = 100;
/// Smallest allowed latency value in charts.
const MIN_LATENCY_MS: u64 = 1;
/// Minimum Y-axis max to avoid a flat chart.
const MIN_Y_MAX: u64 = 10;
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

impl UiActions for Ui {
    fn setup_terminal() -> AppResult<Terminal<CrosstermBackend<std::io::Stdout>>> {
        enable_raw_mode()?;
        if let Err(err) = execute!(io::stdout(), EnterAlternateScreen) {
            disable_raw_mode().ok();
            return Err(err.into());
        }

        let backend = CrosstermBackend::new(io::stdout());
        match Terminal::new(backend) {
            Ok(terminal) => Ok(terminal),
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
                    Constraint::Percentage(SUMMARY_COL_LEFT),
                    Constraint::Percentage(SUMMARY_COL_MID),
                    Constraint::Percentage(SUMMARY_COL_RIGHT),
                ])
                .split(*summary_chunk);

            let (run_chunk, results_chunk, latency_chunk) = match summary_chunks.as_ref() {
                [a, b, c] => (a, b, c),
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

            let mut run_lines = vec![
                text::Line::from(vec![
                    Span::from("Elapsed Time: "),
                    Span::styled(
                        format!("{:.2}s", data.elapsed_time.as_secs_f64()),
                        style_color(data.no_color, Color::Green),
                    ),
                    Span::from("   Target: "),
                    Span::styled(
                        format!("{}s", data.target_duration.as_secs()),
                        style_color(data.no_color, Color::Yellow),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Success %: "),
                    Span::styled(
                        format!(
                            "{}.{:02}%",
                            success_rate_x100 / PERCENT_DIVISOR,
                            success_rate_x100 % PERCENT_DIVISOR
                        ),
                        style_color(data.no_color, Color::Cyan),
                    ),
                ]),
            ];

            if let Some(replay) = data.replay.as_ref() {
                let fmt_ms = |value: u64| {
                    let secs = value / MS_PER_SEC;
                    let centis = (value % MS_PER_SEC) / CENTIS_DIVISOR;
                    format!("{}.{:02}s", secs, centis)
                };
                let status = if replay.playing { "playing" } else { "paused" };
                run_lines.push(text::Line::from(vec![
                    Span::from("Replay: "),
                    Span::styled(status, Style::default().fg(Color::Magenta)),
                    Span::from(" "),
                    Span::styled(
                        format!(
                            "{} -> {} | cursor {}",
                            fmt_ms(replay.window_start_ms),
                            fmt_ms(replay.window_end_ms),
                            fmt_ms(replay.cursor_ms)
                        ),
                        Style::default().fg(Color::Gray),
                    ),
                ]));

                let snapshot_start = replay
                    .snapshot_start_ms
                    .map(fmt_ms)
                    .unwrap_or_else(|| "-".to_owned());
                let snapshot_end = replay
                    .snapshot_end_ms
                    .map(fmt_ms)
                    .unwrap_or_else(|| "-".to_owned());
                run_lines.push(text::Line::from(vec![
                    Span::from("Snapshot: "),
                    Span::styled(
                        format!("start {} end {}", snapshot_start, snapshot_end),
                        style_color(data.no_color, Color::LightBlue),
                    ),
                ]));
                run_lines.push(text::Line::from(Span::styled(
                    "Keys: space play/pause, ←/→ seek, r restart, q quit, s start, e end, w write",
                    Style::default().fg(Color::Gray),
                )));
            }

            let run_text = Paragraph::new(run_lines)
                .block(Block::default().title("Run").borders(Borders::ALL))
                .wrap(Wrap { trim: true });

            let results_text = Paragraph::new(vec![
                text::Line::from(vec![
                    Span::from("Requests: "),
                    Span::styled(
                        format!("{:>6}", total_requests),
                        style_color(data.no_color, Color::LightBlue),
                    ),
                    Span::from("   OK: "),
                    Span::styled(
                        format!("{:>6}", success_requests),
                        style_color(data.no_color, Color::Green),
                    ),
                    Span::from("   Errors: "),
                    Span::styled(
                        format!("{:>6}", error_requests),
                        style_color(data.no_color, Color::Red),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Timeouts: "),
                    Span::styled(
                        format!("{:>6}", data.timeout_requests),
                        style_color(data.no_color, Color::Red),
                    ),
                    Span::from("   Transport: "),
                    Span::styled(
                        format!("{:>6}", data.transport_errors),
                        style_color(data.no_color, Color::Yellow),
                    ),
                    Span::from("   Non-Expected: "),
                    Span::styled(
                        format!("{:>6}", data.non_expected_status),
                        style_color(data.no_color, Color::Yellow),
                    ),
                ]),
            ])
            .block(Block::default().title("Results").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

            let latency_text = Paragraph::new(vec![
                text::Line::from(vec![
                    Span::from("All  P50: "),
                    Span::styled(
                        format!("{}ms", data.p50),
                        style_color(data.no_color, Color::Green),
                    ),
                    Span::from("   P90: "),
                    Span::styled(
                        format!("{}ms", data.p90),
                        style_color(data.no_color, Color::Yellow),
                    ),
                    Span::from("   P99: "),
                    Span::styled(
                        format!("{}ms", data.p99),
                        style_color(data.no_color, Color::Red),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("OK   P50: "),
                    Span::styled(
                        format!("{}ms", data.p50_ok),
                        style_color(data.no_color, Color::Green),
                    ),
                    Span::from("   P90: "),
                    Span::styled(
                        format!("{}ms", data.p90_ok),
                        style_color(data.no_color, Color::Yellow),
                    ),
                    Span::from("   P99: "),
                    Span::styled(
                        format!("{}ms", data.p99_ok),
                        style_color(data.no_color, Color::Red),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("RPS: "),
                    Span::styled(
                        format!("{:>6}", data.rps),
                        style_color(data.no_color, Color::Cyan),
                    ),
                    Span::from("   RPM: "),
                    Span::styled(
                        format!("{:>6}", data.rpm),
                        style_color(data.no_color, Color::Cyan),
                    ),
                ]),
            ])
            .block(Block::default().title("Latency").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

            f.render_widget(run_text, *run_chunk);
            f.render_widget(results_text, *results_chunk);
            f.render_widget(latency_text, *latency_chunk);

            let data_points: Vec<(u64, u64)> = data.latencies.clone();
            let y_max = data
                .latencies
                .iter()
                .map(|(_, latency)| (*latency).max(MIN_LATENCY_MS))
                .fold(MIN_LATENCY_MS, u64::max)
                .max(MIN_Y_MAX);
            let window_ms = data.ui_window_ms.max(MIN_WINDOW_MS);
            let x_max = data_points.last().map(|(x, _)| *x).unwrap_or(0);
            let x_start = x_max.saturating_sub(window_ms);
            let x_span = x_max.saturating_sub(x_start).max(MIN_WINDOW_MS);

            let chart_points: Vec<(f64, f64)> = data_points
                .iter()
                .filter(|(x, _)| *x >= x_start)
                .map(|(x, y)| {
                    (
                        x.saturating_sub(x_start) as f64,
                        (*y).max(MIN_LATENCY_MS) as f64,
                    )
                })
                .collect();
            let fmt_secs = |ms: u64| {
                let secs = ms / MS_PER_SEC;
                let tenths = (ms % MS_PER_SEC) / TENTHS_DIVISOR;
                format!("{}.{:01}s", secs, tenths)
            };
            let label_left = x_start;
            let label_mid = x_start.saturating_add(x_span / 2);
            let label_right = x_start.saturating_add(x_span);
            let window_label = fmt_secs(window_ms);
            let datasets = vec![
                ratatui::widgets::Dataset::default()
                    .name("Latency Chart")
                    .marker(ratatui::symbols::Marker::Dot)
                    .style(style_color(data.no_color, Color::Cyan))
                    .data(&chart_points),
            ];

            let chart = ratatui::widgets::Chart::new(datasets)
                .block(
                    Block::default()
                        .title(format!("Latency (last {})", window_label))
                        .borders(Borders::ALL),
                )
                .x_axis(
                    ratatui::widgets::Axis::default()
                        .title("Elapsed (s)")
                        .style(Style::default().fg(Color::Gray))
                        .bounds([0.0, x_span as f64])
                        .labels(vec![
                            Span::raw(fmt_secs(label_left)),
                            Span::raw(fmt_secs(label_mid)),
                            Span::raw(fmt_secs(label_right)),
                        ]),
                )
                .y_axis(
                    ratatui::widgets::Axis::default()
                        .title("Latency (ms)")
                        .style(Style::default().fg(Color::Gray))
                        .bounds([0.0, y_max as f64])
                        .labels(vec![
                            Span::raw("0"),
                            Span::raw(format!("{}", y_max / 2)),
                            Span::raw(format!("{}", y_max)),
                        ]),
                );

            f.render_widget(chart, *chart_chunk);
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
            style_color(no_color, Color::LightMagenta),
        )));

        let banner = Paragraph::new(lines)
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: false });
        f.render_widget(banner, size);
    }) {
        eprintln!("Failed to render splash screen: {}", err);
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
