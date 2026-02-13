use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{Backend, Frame},
    widgets::Block,
};

use crate::ui::model::UiRenderData;

use super::charts::render_charts;
use super::summary::render_summary;
use super::theme::{CHART_MIN_HEIGHT, SUMMARY_HEIGHT, UI_MARGIN, app_background_style};

pub fn draw_frame<B: Backend>(f: &mut Frame<'_, B>, data: &UiRenderData) {
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

    render_summary(f, data, *summary_chunk);
    render_charts(f, data, *chart_chunk);
}
