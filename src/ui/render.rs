use crossterm::{
    cursor, execute,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use std::error::Error;
use std::io::{self, Write};
use tokio::sync::{
    broadcast::{self},
    watch,
};

use crate::args::TesterArgs;

use super::model::{UiData, UiRenderData};

pub struct UiTerminal {
    stdout: io::Stdout,
}

impl UiTerminal {
    fn new() -> Self {
        Self {
            stdout: io::stdout(),
        }
    }
}

pub trait UiActions {
    /// Initializes the terminal for UI rendering.
    ///
    /// # Errors
    ///
    /// Returns an error when terminal setup fails.
    fn setup_terminal() -> Result<UiTerminal, Box<dyn Error>>;
    fn cleanup();
    fn render(terminal: &mut UiTerminal, data: &UiRenderData);
}

pub struct Ui;

impl UiActions for Ui {
    fn setup_terminal() -> Result<UiTerminal, Box<dyn Error>> {
        enable_raw_mode()?;
        if let Err(err) = execute!(io::stdout(), EnterAlternateScreen) {
            disable_raw_mode().ok();
            return Err(err.into());
        }

        Ok(UiTerminal::new())
    }

    fn cleanup() {
        disable_raw_mode().ok();
        execute!(std::io::stdout(), LeaveAlternateScreen).ok();
    }

    fn render(terminal: &mut UiTerminal, data: &UiRenderData) {
        let width = crossterm::terminal::size()
            .map(|(cols, _)| usize::from(cols))
            .unwrap_or(80);
        let lines = format_lines(data, width);

        if execute!(terminal.stdout, cursor::MoveTo(0, 0), Clear(ClearType::All)).is_err() {
            eprintln!("Failed to clear UI terminal.");
            return;
        }

        for mut line in lines {
            if line.len() > width {
                line.truncate(width);
            }
            if writeln!(terminal.stdout, "{}", line).is_err() {
                eprintln!("Failed to render UI.");
                return;
            }
        }

        terminal.stdout.flush().ok();
    }
}

pub(crate) fn format_lines(data: &UiRenderData, width: usize) -> Vec<String> {
    let mut lines = Vec::with_capacity(5);
    lines.push(format!(
        "Elapsed: {:.2}s   Target: {}s",
        data.elapsed_time.as_secs_f64(),
        data.target_duration.as_secs()
    ));
    lines.push(format!(
        "Requests: {}   Success: {}",
        data.current_request, data.successful_requests
    ));
    lines.push(format!("RPS: {}   RPM: {}", data.rps, data.rpm));
    lines.push(format!(
        "P50: {}ms   P90: {}ms   P99: {}ms",
        data.p50, data.p90, data.p99
    ));
    lines.push(format_latency_line(&data.latencies, width));
    lines
}

fn format_latency_line(latencies: &[(u64, u64)], width: usize) -> String {
    if latencies.is_empty() {
        return "Latencies (ms): <no data>".to_owned();
    }

    let prefix = "Latencies (ms):";
    let usable = width.saturating_sub(prefix.len().saturating_add(1));
    let max_points = (usable / 4).max(1);
    let mut points: Vec<u64> = latencies
        .iter()
        .rev()
        .take(max_points)
        .map(|(_, latency)| *latency)
        .collect();
    points.reverse();

    let capacity = prefix
        .len()
        .saturating_add(points.len().saturating_mul(4))
        .saturating_add(1);
    let mut line = String::with_capacity(capacity);
    line.push_str(prefix);
    for value in points {
        line.push(' ');
        line.push_str(&value.to_string());
    }

    line
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
