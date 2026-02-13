use ratatui::text::Span;

pub(super) const AXIS_SEGMENTS: u64 = 4;
pub(super) const Y_AXIS_LABEL_EXTRA_WIDTH: usize = 2;
pub(super) const SUCCESS_RATE_SCALE: u128 = 10_000;
pub(super) const PERCENT_DIVISOR: u64 = 100;
pub(super) const MS_PER_SEC: u64 = 1_000;
pub(super) const TENTHS_DIVISOR: u64 = 100;

pub(super) fn format_ms_as_tenths(ms: u128) -> String {
    let sec_divisor = u128::from(MS_PER_SEC);
    let tenth_divisor = u128::from(TENTHS_DIVISOR);
    let secs = ms.checked_div(sec_divisor).unwrap_or(0);
    let rem = ms.checked_rem(sec_divisor).unwrap_or(0);
    let tenths = rem.checked_div(tenth_divisor).unwrap_or(0);
    format!("{}.{}s", secs, tenths)
}

pub(super) fn format_bytes_compact(bytes: u128) -> String {
    const KB: u128 = 1_000;
    const MB: u128 = 1_000_000;
    const GB: u128 = 1_000_000_000;
    const TB: u128 = 1_000_000_000_000;

    if bytes >= TB {
        let whole = bytes / TB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(TB))
            .saturating_mul(100)
            .checked_div(TB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}TB")
    } else if bytes >= GB {
        let whole = bytes / GB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(GB))
            .saturating_mul(100)
            .checked_div(GB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}GB")
    } else if bytes >= MB {
        let whole = bytes / MB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(MB))
            .saturating_mul(100)
            .checked_div(MB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}MB")
    } else if bytes >= KB {
        let whole = bytes / KB;
        let frac = bytes
            .saturating_sub(whole.saturating_mul(KB))
            .saturating_mul(100)
            .checked_div(KB)
            .unwrap_or(0);
        format!("{whole}.{frac:02}KB")
    } else {
        format!("{bytes}B")
    }
}

pub(super) fn format_count_compact(value: u64) -> String {
    let (scale, suffix) = select_count_scale(value);
    if suffix.is_empty() {
        return value.to_string();
    }
    format_scaled_compact(value, scale, suffix)
}

pub(super) fn format_status_bar_value(value: u64) -> String {
    if value >= 10_000 {
        let whole = value.checked_div(1_000).unwrap_or(0);
        let rem = value.checked_rem(1_000).unwrap_or(0);
        let frac = rem.saturating_mul(10).checked_div(1_000).unwrap_or(0);
        format!("{whole},{frac}k")
    } else {
        value.to_string()
    }
}

pub(super) const fn select_bytes_scale(value: u64) -> (u64, &'static str) {
    if value >= 1_000_000_000_000 {
        (1_000_000_000_000, "TB")
    } else if value >= 1_000_000_000 {
        (1_000_000_000, "GB")
    } else if value >= 1_000_000 {
        (1_000_000, "MB")
    } else if value >= 1_000 {
        (1_000, "KB")
    } else {
        (1, "B")
    }
}

pub(super) const fn select_count_scale(value: u64) -> (u64, &'static str) {
    if value >= 1_000_000_000_000 {
        (1_000_000_000_000, "t")
    } else if value >= 1_000_000_000 {
        (1_000_000_000, "g")
    } else if value >= 1_000_000 {
        (1_000_000, "m")
    } else if value >= 10_000 {
        (1_000, "k")
    } else {
        (1, "")
    }
}

fn format_scaled_compact(value: u64, scale: u64, suffix: &str) -> String {
    let whole = value.checked_div(scale).unwrap_or(0);
    let rem = value.checked_rem(scale).unwrap_or(0);

    if whole < 10 {
        let frac = rem.saturating_mul(100).checked_div(scale).unwrap_or(0);
        format!("{whole}.{frac:02}{suffix}")
    } else if whole < 100 {
        let frac = rem.saturating_mul(10).checked_div(scale).unwrap_or(0);
        format!("{whole}.{frac:01}{suffix}")
    } else {
        format!("{whole}{suffix}")
    }
}

pub(super) fn axis_tick_value(max_value: u64, step: u64) -> u64 {
    max_value
        .saturating_mul(step)
        .checked_div(AXIS_SEGMENTS)
        .unwrap_or(0)
}

pub(super) fn centered_axis_labels(labels: [String; 5]) -> Vec<Span<'static>> {
    centered_axis_labels_with_extra(labels, 0)
}

pub(super) fn centered_y_axis_labels(labels: [String; 5]) -> Vec<Span<'static>> {
    centered_axis_labels_with_extra(labels, Y_AXIS_LABEL_EXTRA_WIDTH)
}

fn centered_axis_labels_with_extra(labels: [String; 5], extra: usize) -> Vec<Span<'static>> {
    let width = labels.iter().map(|label| label.len()).max().unwrap_or(1);
    let width = width.saturating_add(extra);
    labels
        .into_iter()
        .map(|label| Span::raw(format!("{label:^w$}", w = width)))
        .collect()
}
