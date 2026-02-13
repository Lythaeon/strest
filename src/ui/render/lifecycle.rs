use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use ratatui::{
    Terminal,
    prelude::{Backend, text},
    text::Span,
    widgets::{Block, Paragraph, Wrap},
};
use tokio::sync::watch;

use crate::error::AppResult;
use crate::shutdown::ShutdownSender;
use crate::ui::model::{UiData, UiRenderData};

use super::dashboard::{Ui, UiActions};
use super::theme::{
    BANNER_LINES, BANNER_PADDING_LINES, COLOR_END, COLOR_MID, COLOR_START, SPLASH_DURATION_SECS,
    SPLASH_SUBTITLE_RGB, app_background_style, rgb, style_color, tri_gradient_color,
};

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        Ui::cleanup();
    }
}

#[must_use]
pub fn setup_render_ui(
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
pub async fn run_splash_screen(no_color: bool) -> AppResult<bool> {
    let mut terminal = Ui::setup_terminal()?;
    let _guard = TerminalGuard;

    render_splash(&mut terminal, no_color);
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(SPLASH_DURATION_SECS))
        .unwrap_or_else(Instant::now);
    loop {
        if Instant::now() >= deadline {
            return Ok(true);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let timeout = remaining.min(Duration::from_millis(50));
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
        {
            match key.code {
                KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(false),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(false);
                }
                KeyCode::Backspace
                | KeyCode::Enter
                | KeyCode::Left
                | KeyCode::Right
                | KeyCode::Up
                | KeyCode::Down
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Delete
                | KeyCode::Insert
                | KeyCode::F(_)
                | KeyCode::Char(_)
                | KeyCode::Null
                | KeyCode::Esc
                | KeyCode::CapsLock
                | KeyCode::ScrollLock
                | KeyCode::NumLock
                | KeyCode::PrintScreen
                | KeyCode::Pause
                | KeyCode::Menu
                | KeyCode::KeypadBegin
                | KeyCode::Media(_)
                | KeyCode::Modifier(_) => {}
            }
        }
    }
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
            .alignment(ratatui::layout::Alignment::Center)
            .wrap(Wrap { trim: false });
        f.render_widget(banner, size);
    }) {
        eprintln!("Failed to render splash screen: {}", err);
    }
}
