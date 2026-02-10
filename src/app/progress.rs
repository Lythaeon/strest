use std::io::{IsTerminal, Write};
use std::time::Duration;

use crossterm::{
    cursor, queue,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{Clear, ClearType},
};
use tokio::sync::broadcast;
use tokio::time::Instant;

use crate::args::TesterArgs;

pub(crate) fn setup_progress_indicator(
    args: &TesterArgs,
    run_start: Instant,
    shutdown_tx: &broadcast::Sender<u16>,
) -> tokio::task::JoinHandle<()> {
    let mut shutdown_rx = shutdown_tx.subscribe();
    let target_secs = args.target_duration.get();
    let goal = usize::try_from(target_secs.max(1)).unwrap_or(1);
    let style = ProgressStyle::new(30);
    let no_color = args.no_color;

    tokio::spawn(async move {
        if !std::io::stderr().is_terminal() {
            return;
        }

        let mut ticker = tokio::time::interval(Duration::from_millis(250));

        loop {
            tokio::select! {
                _ = shutdown_rx.recv() => {
                    let elapsed_ms = u128::from(target_secs).saturating_mul(1000);
                    if render_progress_line(&style, goal, elapsed_ms, no_color).is_err() {
                        break;
                    }
                    if finish_progress_line().is_err() {
                        break;
                    }
                    break;
                }
                _ = ticker.tick() => {
                    let elapsed_ms = run_start.elapsed().as_millis();
                    if render_progress_line(&style, goal, elapsed_ms, no_color).is_err() {
                        break;
                    }
                }
            }
        }
    })
}

fn render_progress_line(
    style: &ProgressStyle,
    goal: usize,
    elapsed_ms: u128,
    no_color: bool,
) -> Result<(), std::io::Error> {
    let elapsed_secs = elapsed_ms.checked_div(1000).unwrap_or(0);
    let current = usize::try_from(elapsed_secs).unwrap_or(goal);
    let current = current.min(goal);
    let line = build_progress_line(style, current, goal, elapsed_ms, no_color);

    let mut out = std::io::stderr();
    queue!(out, cursor::MoveToColumn(0), Clear(ClearType::CurrentLine))?;
    for segment in line {
        if no_color {
            queue!(out, Print(&segment.text))?;
        } else if let Some(color) = segment.color {
            queue!(
                out,
                SetForegroundColor(color),
                Print(&segment.text),
                ResetColor
            )?;
        } else {
            queue!(out, Print(&segment.text))?;
        }
    }
    out.flush()?;
    Ok(())
}

fn finish_progress_line() -> Result<(), std::io::Error> {
    let mut out = std::io::stderr();
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

fn build_progress_line(
    style: &ProgressStyle,
    current: usize,
    goal: usize,
    elapsed_ms: u128,
    no_color: bool,
) -> Vec<ProgressSegment> {
    let size = style.size.max(1);
    let goal = goal.max(1);
    let current = current.min(goal);

    let current_u128 = u128::from(u64::try_from(current).unwrap_or(u64::MAX));
    let size_u128 = u128::from(u64::try_from(size).unwrap_or(u64::MAX));
    let goal_u128 = u128::from(u64::try_from(goal).unwrap_or(u64::MAX));

    let scaled = current_u128
        .saturating_mul(size_u128)
        .checked_div(goal_u128)
        .unwrap_or(0);
    let complete_size = usize::try_from(scaled).unwrap_or(size).min(size);
    let incomplete_size = size.saturating_sub(complete_size);

    let percent_x100 = current_u128
        .saturating_mul(10_000)
        .checked_div(goal_u128)
        .unwrap_or(0);
    let percent_whole = percent_x100.checked_div(100).unwrap_or(0);
    let percent_frac = percent_x100.checked_rem(100).unwrap_or(0);
    let percent_text = format!(" {}.{:02}%", percent_whole, percent_frac);

    let elapsed_tenths = elapsed_ms.checked_div(100).unwrap_or(0);
    let secs = elapsed_tenths.checked_div(10).unwrap_or(0);
    let tenths = elapsed_tenths.checked_rem(10).unwrap_or(0);
    let time_text = format!(" | {}.{}s / {}s", secs, tenths, goal);

    let progress_bar = format!(
        "{}{}{}{}",
        style.begin,
        style.fill.repeat(complete_size),
        style.empty.repeat(incomplete_size),
        style.end
    );

    if no_color {
        vec![
            ProgressSegment::plain(progress_bar),
            ProgressSegment::plain(percent_text),
            ProgressSegment::plain(time_text),
        ]
    } else {
        vec![
            ProgressSegment::plain(progress_bar),
            ProgressSegment::colored(percent_text, Color::Cyan),
            ProgressSegment::colored(time_text, Color::Yellow),
        ]
    }
}

struct ProgressStyle {
    size: usize,
    begin: String,
    end: String,
    fill: String,
    empty: String,
}

impl ProgressStyle {
    fn new(size: usize) -> Self {
        Self {
            size,
            begin: "[".to_owned(),
            end: "]".to_owned(),
            fill: "#".to_owned(),
            empty: "-".to_owned(),
        }
    }
}

struct ProgressSegment {
    text: String,
    color: Option<Color>,
}

impl ProgressSegment {
    const fn plain(text: String) -> Self {
        Self { text, color: None }
    }

    const fn colored(text: String, color: Color) -> Self {
        Self {
            text,
            color: Some(color),
        }
    }
}
