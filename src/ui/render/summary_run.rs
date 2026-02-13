use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{Backend, Frame, text},
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::ui::model::UiRenderData;

use super::formatting::{format_count_compact, format_ms_as_tenths};
use super::progress::progress_bar_line;
use super::theme::{
    ACCENT_LOAD_RGB, ACCENT_REPLAY_RGB, ACCENT_SERIES_COMPARE_RGB, ACCENT_SERIES_PRIMARY_RGB,
    PANEL_MUTED_RGB, panel_block_style, panel_border_style, panel_title_style, rgb, style_color,
};

pub fn render_run_panel<B: Backend>(f: &mut Frame<'_, B>, data: &UiRenderData, run_chunk: Rect) {
    let run_block = Block::default()
        .title("Run")
        .borders(Borders::ALL)
        .style(panel_block_style(data.no_color))
        .border_style(panel_border_style(data.no_color))
        .title_style(panel_title_style(data.no_color));
    let run_inner = run_block.inner(run_chunk);
    f.render_widget(run_block, run_chunk);

    let run_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(run_inner);

    let target_ms = data.target_duration.as_millis();
    let elapsed_ms = data.elapsed_time.as_millis();
    let progress_label = if target_ms > 0 {
        format!(
            "{} / {}",
            format_ms_as_tenths(elapsed_ms),
            format_ms_as_tenths(target_ms)
        )
    } else {
        format!("{} / --", format_ms_as_tenths(elapsed_ms))
    };

    let run_details = data.replay.as_ref().map_or_else(
        || {
            let mut lines = vec![text::Line::from(vec![
                Span::from("In-flight: "),
                Span::styled(
                    format_count_compact(data.in_flight_ops),
                    style_color(data.no_color, rgb(ACCENT_LOAD_RGB)),
                ),
            ])];
            if let Some(compare) = data.compare.as_ref() {
                lines.push(text::Line::from(vec![
                    Span::from("Legend: "),
                    Span::styled(
                        "Current",
                        style_color(data.no_color, rgb(ACCENT_SERIES_PRIMARY_RGB)),
                    ),
                    Span::from(" vs "),
                    Span::styled(
                        compare.label.clone(),
                        style_color(data.no_color, rgb(ACCENT_SERIES_COMPARE_RGB)),
                    ),
                ]));
            }
            lines
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
                    Span::styled(status, style_color(data.no_color, rgb(ACCENT_REPLAY_RGB))),
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

    if let Some(chunk) = run_chunks.first() {
        let progress = Paragraph::new(progress_bar_line(
            elapsed_ms,
            target_ms,
            chunk.width,
            data.no_color,
            &progress_label,
        ))
        .style(panel_block_style(data.no_color))
        .wrap(Wrap { trim: true });
        f.render_widget(progress, *chunk);
    }
    if let Some(chunk) = run_chunks.get(1) {
        f.render_widget(run_text, *chunk);
    }
}
