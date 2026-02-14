use ratatui::prelude::text;
use ratatui::style::Style;
use ratatui::text::Span;

use super::theme::{ACCENT_PROGRESS_RGB, PANEL_TEXT_RGB, rgb, style_color};

pub(super) fn progress_bar_line(
    elapsed_ms: u128,
    target_ms: u128,
    width: u16,
    no_color: bool,
    label: &str,
) -> text::Line<'static> {
    let bar_width = usize::from(width.saturating_sub(3)).max(1);
    let total_eighths = u128::try_from(bar_width)
        .unwrap_or(u128::MAX)
        .saturating_mul(8);
    let clamped_elapsed = if target_ms > 0 {
        elapsed_ms.min(target_ms)
    } else {
        0
    };
    let filled_eighths = if target_ms > 0 {
        clamped_elapsed
            .saturating_mul(total_eighths)
            .checked_div(target_ms)
            .unwrap_or(0)
    } else {
        0
    };
    let full_blocks = usize::try_from(filled_eighths.checked_div(8).unwrap_or(0)).unwrap_or(0);
    let rem = usize::try_from(filled_eighths.checked_rem(8).unwrap_or(0)).unwrap_or(0);
    let partial = ["", "▏", "▎", "▍", "▌", "▋", "▊", "▉"];
    let full_count = full_blocks.min(bar_width);
    let partial_block = partial.get(rem).copied().unwrap_or("");
    let partial_count = usize::from(rem > 0 && full_count < bar_width);
    let mut label_chars: Vec<char> = label.chars().collect();
    if label_chars.len() > bar_width {
        label_chars.truncate(bar_width);
    }
    let label_start = bar_width.saturating_sub(label_chars.len()) / 2;
    let mut label_cells: Vec<Option<char>> = vec![None; bar_width];
    for (offset, ch) in label_chars.into_iter().enumerate() {
        if let Some(pos) = label_start
            .checked_add(offset)
            .filter(|value| *value < bar_width)
            && let Some(cell) = label_cells.get_mut(pos)
        {
            *cell = Some(ch);
        }
    }
    let partial_char = partial_block.chars().next().unwrap_or(' ');

    let mut spans = Vec::with_capacity(bar_width.saturating_add(2));
    spans.push(Span::raw("["));
    for idx in 0..bar_width {
        let is_partial_cell = partial_count == 1 && idx == full_count;
        let is_filled_cell = idx < full_count || is_partial_cell;
        if let Some(ch) = label_cells.get(idx).copied().flatten() {
            spans.push(Span::styled(
                ch.to_string(),
                label_cell_style(no_color, is_filled_cell),
            ));
            continue;
        }
        if idx < full_count {
            spans.push(Span::styled(
                "█",
                style_color(no_color, rgb(ACCENT_PROGRESS_RGB)),
            ));
            continue;
        }
        if partial_count == 1 && idx == full_count {
            spans.push(Span::styled(
                partial_char.to_string(),
                style_color(no_color, rgb(ACCENT_PROGRESS_RGB)),
            ));
            continue;
        }
        spans.push(Span::raw(" "));
    }
    spans.push(Span::raw("]"));
    text::Line::from(spans)
}

fn label_cell_style(no_color: bool, on_filled_cell: bool) -> Style {
    if no_color {
        return Style::default();
    }
    let style = Style::default().fg(rgb(PANEL_TEXT_RGB));
    if on_filled_cell {
        style.bg(rgb(ACCENT_PROGRESS_RGB))
    } else {
        style
    }
}

#[cfg(test)]
mod tests {
    use super::{ACCENT_PROGRESS_RGB, progress_bar_line, rgb};

    #[test]
    fn label_on_filled_cell_keeps_progress_background() {
        let line = progress_bar_line(9, 10, 20, false, "x");
        let bg = line
            .spans
            .iter()
            .find(|span| span.content.as_ref() == "x")
            .and_then(|span| span.style.bg);
        assert_eq!(bg, Some(rgb(ACCENT_PROGRESS_RGB)));
    }

    #[test]
    fn label_on_empty_cell_has_no_background_fill() {
        let line = progress_bar_line(1, 10, 20, false, "x");
        let bg = line
            .spans
            .iter()
            .find(|span| span.content.as_ref() == "x")
            .and_then(|span| span.style.bg);
        assert_eq!(bg, None);
    }
}
