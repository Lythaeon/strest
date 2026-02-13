use ratatui::{
    prelude::text,
    text::Span,
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::ui::model::UiRenderData;

use super::formatting::{format_bytes_compact, format_count_compact};
use super::theme::{
    ACCENT_AMBER_RGB, ACCENT_DATA_RGB, ACCENT_GREEN_RGB, ACCENT_LOAD_RGB, ACCENT_RED_RGB,
    ACCENT_REPLAY_RGB, panel_block_style, panel_border_style, panel_title_style, rgb, style_color,
};

pub fn latency_panel(data: &UiRenderData) -> Paragraph<'static> {
    let compare = data.compare.as_ref();
    let latency_lines = compare.map_or_else(
        || {
            vec![
                text::Line::from(vec![
                    Span::from("P50: "),
                    Span::styled(
                        format!("{}ms", data.p50),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from("   P90: "),
                    Span::styled(
                        format!("{}ms", data.p90),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from("   P99: "),
                    Span::styled(
                        format!("{}ms", data.p99),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("OK P50: "),
                    Span::styled(
                        format!("{}ms", data.p50_ok),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from("  P90: "),
                    Span::styled(
                        format!("{}ms", data.p90_ok),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from("  P99: "),
                    Span::styled(
                        format!("{}ms", data.p99_ok),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                ]),
            ]
        },
        |compare| {
            vec![
                text::Line::from(vec![
                    Span::from("P50: "),
                    Span::styled(
                        format!("{}ms", data.p50),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{}ms", compare.p50),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("   P90: "),
                    Span::styled(
                        format!("{}ms", data.p90),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{}ms", compare.p90),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("   P99: "),
                    Span::styled(
                        format!("{}ms", data.p99),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{}ms", compare.p99),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("OK P50: "),
                    Span::styled(
                        format!("{}ms", data.p50_ok),
                        style_color(data.no_color, rgb(ACCENT_GREEN_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{}ms", compare.p50_ok),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("  P90: "),
                    Span::styled(
                        format!("{}ms", data.p90_ok),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{}ms", compare.p90_ok),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("  P99: "),
                    Span::styled(
                        format!("{}ms", data.p99_ok),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{}ms", compare.p99_ok),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                ]),
            ]
        },
    );

    Paragraph::new(latency_lines)
        .block(
            Block::default()
                .title("Latency")
                .borders(Borders::ALL)
                .style(panel_block_style(data.no_color))
                .border_style(panel_border_style(data.no_color))
                .title_style(panel_title_style(data.no_color)),
        )
        .style(panel_block_style(data.no_color))
        .wrap(Wrap { trim: true })
}

pub fn flow_panel(data: &UiRenderData) -> Paragraph<'static> {
    let compare = data.compare.as_ref();
    let flow_lines = compare.map_or_else(
        || {
            vec![
                text::Line::from(vec![
                    Span::from("In-flight: "),
                    Span::styled(
                        format!("{:>5}", format_count_compact(data.in_flight_ops)),
                        style_color(data.no_color, rgb(ACCENT_LOAD_RGB)),
                    ),
                    Span::from("   RX/s: "),
                    Span::styled(
                        data.data_usage.as_ref().map_or_else(
                            || "0B/s".to_owned(),
                            |usage| {
                                format!(
                                    "{}/s",
                                    format_bytes_compact(u128::from(usage.bytes_per_sec))
                                )
                            },
                        ),
                        style_color(data.no_color, rgb(ACCENT_DATA_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Timeout: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.timeout_requests)),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from("  Transport: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.transport_errors)),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from("  Non-Exp: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.non_expected_status)),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                ]),
            ]
        },
        |compare| {
            let compare_rate = compare
                .data_usage
                .as_ref()
                .map(|usage| {
                    format!(
                        "{}/s",
                        format_bytes_compact(u128::from(usage.bytes_per_sec))
                    )
                })
                .unwrap_or_else(|| "0B/s".to_owned());
            vec![
                text::Line::from(vec![
                    Span::from("In-flight: "),
                    Span::styled(
                        format!("{:>5}", format_count_compact(data.in_flight_ops)),
                        style_color(data.no_color, rgb(ACCENT_LOAD_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{:>5}", format_count_compact(compare.in_flight_ops)),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("   RX/s: "),
                    Span::styled(
                        data.data_usage.as_ref().map_or_else(
                            || "0B/s".to_owned(),
                            |usage| {
                                format!(
                                    "{}/s",
                                    format_bytes_compact(u128::from(usage.bytes_per_sec))
                                )
                            },
                        ),
                        style_color(data.no_color, rgb(ACCENT_DATA_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        compare_rate,
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                ]),
                text::Line::from(vec![
                    Span::from("Timeout: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.timeout_requests)),
                        style_color(data.no_color, rgb(ACCENT_RED_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(compare.timeout_requests)),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("  Transport: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.transport_errors)),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(compare.transport_errors)),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                    Span::from("  Non-Exp: "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(data.non_expected_status)),
                        style_color(data.no_color, rgb(ACCENT_AMBER_RGB)),
                    ),
                    Span::from(" / "),
                    Span::styled(
                        format!("{:>4}", format_count_compact(compare.non_expected_status)),
                        style_color(data.no_color, rgb(ACCENT_REPLAY_RGB)),
                    ),
                ]),
            ]
        },
    );

    Paragraph::new(flow_lines)
        .block(
            Block::default()
                .title("Flow")
                .borders(Borders::ALL)
                .style(panel_block_style(data.no_color))
                .border_style(panel_border_style(data.no_color))
                .title_style(panel_title_style(data.no_color)),
        )
        .style(panel_block_style(data.no_color))
        .wrap(Wrap { trim: true })
}
