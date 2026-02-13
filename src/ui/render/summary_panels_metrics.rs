use ratatui::{
    prelude::text,
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::ui::model::UiRenderData;

use super::formatting::{PERCENT_DIVISOR, SUCCESS_RATE_SCALE, format_count_compact};
use super::theme::{
    ACCENT_GREEN_RGB, ACCENT_RATE_RGB, ACCENT_RED_RGB, ACCENT_REPLAY_RGB, panel_block_style,
    panel_border_style, panel_title_style, rgb, style_color,
};

pub type CompareTotals = (u64, u64, u64, u64, u64);

pub fn throughput_panel(data: &UiRenderData, total_requests: u64) -> Paragraph<'static> {
    let compare = data.compare.as_ref();
    let throughput_lines = compare.map_or_else(
        || {
            vec![
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
            ]
        },
        |compare| {
            vec![
                text::Line::from(vec![
                    Span::from("RPS: "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(data.rps)),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(compare.rps)),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("   RPM: "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(data.rpm)),
                        style_color(data.no_color, rgb(ACCENT_RATE_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(compare.rpm)),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Requests: "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(total_requests)),
                        style_color(data.no_color, rgb(ACCENT_RATE_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{:>6}", format_count_compact(compare.current_requests)),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                ]),
            ]
        },
    );

    Paragraph::new(throughput_lines)
        .block(
            Block::default()
                .title("Throughput")
                .borders(Borders::ALL)
                .style(panel_block_style(data.no_color))
                .border_style(panel_border_style(data.no_color))
                .title_style(panel_title_style(data.no_color)),
        )
        .style(panel_block_style(data.no_color))
        .wrap(Wrap { trim: true })
}

pub fn reliability_panel(
    data: &UiRenderData,
    error_requests: u64,
    compare_totals: Option<CompareTotals>,
) -> Paragraph<'static> {
    let total_requests = data.current_request;
    let success_requests = data.successful_requests;
    let success_rate_x100 = rate_x100(success_requests, total_requests);
    let error_rate_x100 = rate_x100(error_requests, total_requests);

    let reliability_lines =
        if let Some((_, _, compare_error, compare_success_rate, compare_error_rate)) =
            compare_totals
        {
            vec![
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
                    Span::from(" / "),
                    Span::styled(
                        format!(
                            "{}.{:02}%",
                            compare_success_rate / PERCENT_DIVISOR,
                            compare_success_rate % PERCENT_DIVISOR
                        ),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
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
                    Span::from(" / "),
                    Span::styled(
                        format!(
                            "{}.{:02}%",
                            compare_error_rate / PERCENT_DIVISOR,
                            compare_error_rate % PERCENT_DIVISOR
                        ),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from(" ("),
                    Span::styled(
                        format_count_compact(error_requests),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format_count_compact(compare_error),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from(")"),
                ]),
            ]
        } else {
            vec![
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
            ]
        };

    Paragraph::new(reliability_lines)
        .block(
            Block::default()
                .title("Reliability")
                .borders(Borders::ALL)
                .style(panel_block_style(data.no_color))
                .border_style(panel_border_style(data.no_color))
                .title_style(panel_title_style(data.no_color)),
        )
        .style(panel_block_style(data.no_color))
        .wrap(Wrap { trim: true })
}

fn rate_x100(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let scaled = u128::from(numerator)
        .saturating_mul(SUCCESS_RATE_SCALE)
        .checked_div(u128::from(denominator))
        .unwrap_or(0);
    u64::try_from(scaled).unwrap_or(u64::MAX)
}
