use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    prelude::{Backend, text},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::error::Error;
use std::io;
use tokio::sync::{
    broadcast::{self},
    watch,
};

use crate::args::TesterArgs;

use super::model::{UiData, UiRenderData};

pub trait UiActions {
    /// Initializes the terminal for UI rendering.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal setup fails.
    fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, Box<dyn Error>>;
    fn cleanup();
    fn render<B: Backend>(terminal: &mut Terminal<B>, data: &UiRenderData);
}

pub struct Ui;

impl UiActions for Ui {
    fn setup_terminal() -> Result<Terminal<CrosstermBackend<std::io::Stdout>>, Box<dyn Error>> {
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
                .margin(1)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Min(10),
                ])
                .split(size);

            let (stats_chunk, percentiles_chunk, chart_chunk) = match chunks.as_ref() {
                [a, b, c] => (a, b, c),
                _ => return,
            };

            let stats_text = Paragraph::new(vec![
                text::Line::from(vec![
                    Span::from("Elapsed Time: "),
                    Span::styled(
                        format!("{:.2}s", data.elapsed_time.as_secs_f64()),
                        Style::default().fg(Color::Green),
                    ),
                    Span::from("   Target: "),
                    Span::styled(
                        format!("{}s", data.target_duration.as_secs()),
                        Style::default().fg(Color::Yellow),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Requests: "),
                    Span::styled(
                        data.current_request.to_string(),
                        Style::default().fg(Color::LightBlue),
                    ),
                    Span::from("   Success: "),
                    Span::styled(
                        data.successful_requests.to_string(),
                        Style::default().fg(Color::Magenta),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("RPS: "),
                    Span::styled(format!("{}", data.rps), Style::default().fg(Color::Cyan)),
                    Span::from("   RPM: "),
                    Span::styled(format!("{}", data.rpm), Style::default().fg(Color::Cyan)),
                ]),
            ])
            .block(Block::default().title("Stats").borders(Borders::ALL))
            .wrap(Wrap { trim: true });

            f.render_widget(stats_text, *stats_chunk);

            let percentiles_text = Paragraph::new(vec![text::Line::from(vec![
                Span::from("P50: "),
                Span::styled(format!("{}ms", data.p50), Style::default().fg(Color::Green)),
                Span::from("   P90: "),
                Span::styled(
                    format!("{}ms", data.p90),
                    Style::default().fg(Color::Yellow),
                ),
                Span::from("   P99: "),
                Span::styled(format!("{}ms", data.p99), Style::default().fg(Color::Red)),
            ])])
            .block(
                Block::default()
                    .title("Latency Percentiles")
                    .borders(Borders::ALL),
            )
            .wrap(Wrap { trim: true });

            f.render_widget(percentiles_text, *percentiles_chunk);

            let data_points: Vec<(u64, u64)> = data.latencies.clone();
            let y_max = data
                .latencies
                .iter()
                .map(|(_, latency)| *latency)
                .fold(0, u64::max)
                .max(10);
            let x_max = data_points
                .last()
                .map(|(x, _)| x.saturating_add(1))
                .unwrap_or(1);
            let x_min = x_max.saturating_sub(10_000);

            let chart_points: Vec<(f64, f64)> = data_points
                .iter()
                .map(|(x, y)| (*x as f64, *y as f64))
                .collect();
            let datasets = vec![
                ratatui::widgets::Dataset::default()
                    .name("Latency Chart")
                    .marker(ratatui::symbols::Marker::Dot)
                    .style(Style::default().fg(Color::Cyan))
                    .data(&chart_points),
            ];

            let chart = ratatui::widgets::Chart::new(datasets)
                .block(Block::default().borders(Borders::ALL))
                .x_axis(
                    ratatui::widgets::Axis::default()
                        .title("Window Second")
                        .style(Style::default().fg(Color::Gray))
                        .bounds([x_min as f64, x_max as f64])
                        .labels(vec![
                            Span::raw(format!("{}s", x_min / 1000)),
                            Span::raw(format!("{}s", x_min.saturating_add(x_max) / 2 / 1000)),
                            Span::raw(format!("{}s", x_max / 1000)),
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
    shutdown_tx: &broadcast::Sender<u16>,
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
