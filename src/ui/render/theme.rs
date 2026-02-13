use ratatui::style::{Color, Style};

pub(super) const BANNER_LINES: [&str; 7] = [
    "███████╗████████╗██████╗ ███████╗███████╗████████╗",
    "██╔════╝╚══██╔══╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝",
    "███████╗   ██║   ██████╔╝█████╗  ███████╗   ██║   ",
    "╚════██║   ██║   ██╔══██╗██╔══╝  ╚════██║   ██║   ",
    "███████║   ██║   ██║  ██║███████╗███████║   ██║   ",
    "╚══════╝   ╚═╝   ╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝   ",
    "                                                   ",
];
pub(super) const UI_MARGIN: u16 = 1;
pub(super) const SUMMARY_HEIGHT: u16 = 7;
pub(super) const CHART_MIN_HEIGHT: u16 = 10;
pub(super) const CHART_COL_LEFT: u16 = 50;
pub(super) const CHART_COL_RIGHT: u16 = 50;
pub(super) const STATUS_PANEL_MAX_WIDTH: u16 = 40;
pub(super) const DATA_PANEL_WIDTH: u16 = 60;
pub(super) const CHART_ROW_TOP: u16 = 50;
pub(super) const CHART_ROW_BOTTOM: u16 = 50;
pub(super) const CHART_BG_RGB: (u8, u8, u8) = (0x0a, 0x0a, 0x0a);
pub(super) const PANEL_BORDER_RGB: (u8, u8, u8) = (0xe5, 0xe7, 0xeb);
pub(super) const PANEL_TEXT_RGB: (u8, u8, u8) = (0xff, 0xff, 0xff);
pub(super) const PANEL_MUTED_RGB: (u8, u8, u8) = (0xd1, 0xd5, 0xdb);
pub(super) const ACCENT_PROGRESS_RGB: (u8, u8, u8) = (0x22, 0xd3, 0xee);
pub(super) const ACCENT_LOAD_RGB: (u8, u8, u8) = (0x38, 0xbd, 0xf8);
pub(super) const ACCENT_RATE_RGB: (u8, u8, u8) = (0x60, 0xa5, 0xfa);
pub(super) const ACCENT_DATA_RGB: (u8, u8, u8) = (0xa7, 0x8b, 0xfa);
pub(super) const ACCENT_SERIES_PRIMARY_RGB: (u8, u8, u8) = (0x22, 0xd3, 0xee);
pub(super) const ACCENT_SERIES_COMPARE_RGB: (u8, u8, u8) = (0xff, 0xa9, 0x4d);
pub(super) const ACCENT_REPLAY_RGB: (u8, u8, u8) = (0xc0, 0x84, 0xfc);
pub(super) const ACCENT_LATENCY_RGB: (u8, u8, u8) = (0xf4, 0x72, 0xb6);
pub(super) const ACCENT_GREEN_RGB: (u8, u8, u8) = (0x22, 0xc5, 0x5e);
pub(super) const ACCENT_AMBER_RGB: (u8, u8, u8) = (0xf5, 0x9e, 0x0b);
pub(super) const ACCENT_RED_RGB: (u8, u8, u8) = (0xef, 0x44, 0x44);
pub(super) const SUMMARY_COL_PROGRESS: u16 = 20;
pub(super) const SUMMARY_COL_THROUGHPUT: u16 = 20;
pub(super) const SUMMARY_COL_RELIABILITY: u16 = 20;
pub(super) const SUMMARY_COL_LATENCY: u16 = 20;
pub(super) const SUMMARY_COL_FLOW: u16 = 20;
pub(super) const MIN_LATENCY_MS: u64 = 1;
pub(super) const MIN_Y_MAX: u64 = 10;
pub(super) const MIN_RPS_Y_MAX: u64 = 1;
pub(super) const MIN_DATA_Y_MAX: u64 = 1;
pub(super) const MIN_WINDOW_MS: u64 = 1;
pub(super) const SPLASH_DURATION_SECS: u64 = 3;
pub(super) const BANNER_PADDING_LINES: usize = 1;
pub(super) const COLOR_START: (u8, u8, u8) = (0x80, 0x4c, 0xff);
pub(super) const COLOR_MID: (u8, u8, u8) = (0xff, 0x5f, 0xc8);
pub(super) const COLOR_END: (u8, u8, u8) = (0x3a, 0xa9, 0xff);
pub(super) const SPLASH_SUBTITLE_RGB: (u8, u8, u8) = (0xff, 0x5f, 0xc8);

pub(super) fn style_color(no_color: bool, color: Color) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(color)
    }
}

pub(super) const fn rgb(rgb: (u8, u8, u8)) -> Color {
    Color::Rgb(rgb.0, rgb.1, rgb.2)
}

pub(super) fn panel_block_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default()
            .bg(rgb(CHART_BG_RGB))
            .fg(rgb(PANEL_TEXT_RGB))
    }
}

pub(super) fn panel_border_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(rgb(PANEL_BORDER_RGB))
    }
}

pub(super) fn panel_title_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(rgb(PANEL_TEXT_RGB))
    }
}

pub(super) fn axis_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().fg(rgb(PANEL_MUTED_RGB))
    }
}

pub(super) fn app_background_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default().bg(rgb(CHART_BG_RGB))
    }
}

pub(super) fn chart_surface_style(no_color: bool) -> Style {
    if no_color {
        Style::default()
    } else {
        Style::default()
            .bg(rgb(CHART_BG_RGB))
            .fg(rgb(PANEL_TEXT_RGB))
    }
}

fn gradient_color(start: (u8, u8, u8), end: (u8, u8, u8), idx: usize, denom: usize) -> Color {
    let denom = denom.max(1) as i32;
    let idx = idx.min(usize::try_from(denom).unwrap_or(0)) as i32;
    let lerp = |a: u8, b: u8| -> u8 {
        let a = i32::from(a);
        let b = i32::from(b);
        let value = b
            .checked_sub(a)
            .and_then(|delta| delta.checked_mul(idx))
            .and_then(|scaled| scaled.checked_div(denom))
            .and_then(|step| a.checked_add(step))
            .unwrap_or(a);
        u8::try_from(value.clamp(0, 255)).unwrap_or(0)
    };
    Color::Rgb(
        lerp(start.0, end.0),
        lerp(start.1, end.1),
        lerp(start.2, end.2),
    )
}

pub(super) fn tri_gradient_color(
    start: (u8, u8, u8),
    mid: (u8, u8, u8),
    end: (u8, u8, u8),
    idx: usize,
    denom: usize,
) -> Color {
    let denom = denom.max(1);
    let half = denom / 2;
    if idx <= half {
        gradient_color(start, mid, idx, half)
    } else {
        gradient_color(
            mid,
            end,
            idx.saturating_sub(half),
            denom.saturating_sub(half),
        )
    }
}
