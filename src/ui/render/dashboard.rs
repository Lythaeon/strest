use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, prelude::Backend};
use std::io;

use crate::error::AppResult;
use crate::ui::model::UiRenderData;

use super::frame::draw_frame;

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
        if let Err(err) = terminal.draw(|f| draw_frame(f, data)) {
            eprintln!("Failed to render UI: {}", err);
        }
    }
}
