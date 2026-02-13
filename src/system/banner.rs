use std::io::IsTerminal;

use crossterm::style::{Color, Stylize};

const BANNER_LINES: [&str; 7] = [
    "███████╗████████╗██████╗ ███████╗███████╗████████╗",
    "██╔════╝╚══██╔══╝██╔══██╗██╔════╝██╔════╝╚══██╔══╝",
    "███████╗   ██║   ██████╔╝█████╗  ███████╗   ██║   ",
    "╚════██║   ██║   ██╔══██╗██╔══╝  ╚════██║   ██║   ",
    "███████║   ██║   ██║  ██║███████╗███████║   ██║   ",
    "╚══════╝   ╚═╝   ╚═╝  ╚═╝╚══════╝╚══════╝   ╚═╝   ",
    "                                                  ",
];

const COLOR_START: (u8, u8, u8) = (0x80, 0x4c, 0xff);
const COLOR_MID: (u8, u8, u8) = (0xff, 0x5f, 0xc8);
const COLOR_END: (u8, u8, u8) = (0x3a, 0xa9, 0xff);
const SUBTITLE_RGB: (u8, u8, u8) = (0xff, 0x5f, 0xc8);

pub(crate) fn print_cli_banner(no_color: bool) {
    let use_color = !no_color && std::io::stdout().is_terminal();
    let denom = BANNER_LINES.len().saturating_sub(1);
    for (idx, line) in BANNER_LINES.iter().enumerate() {
        if use_color {
            let (r, g, b) = tri_gradient_rgb(COLOR_START, COLOR_MID, COLOR_END, idx, denom);
            println!("{}", line.with(Color::Rgb { r, g, b }));
        } else {
            println!("{line}");
        }
    }

    let description = format!(
        "strest v{} | {} | stress testing",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_LICENSE")
    );
    if use_color {
        println!(
            "{}",
            description.with(Color::Rgb {
                r: SUBTITLE_RGB.0,
                g: SUBTITLE_RGB.1,
                b: SUBTITLE_RGB.2
            })
        );
    } else {
        println!("{description}");
    }
}

fn gradient_rgb(start: (u8, u8, u8), end: (u8, u8, u8), idx: usize, denom: usize) -> (u8, u8, u8) {
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
    (
        lerp(start.0, end.0),
        lerp(start.1, end.1),
        lerp(start.2, end.2),
    )
}

fn tri_gradient_rgb(
    start: (u8, u8, u8),
    mid: (u8, u8, u8),
    end: (u8, u8, u8),
    idx: usize,
    denom: usize,
) -> (u8, u8, u8) {
    let denom = denom.max(1);
    let half = denom / 2;
    if idx <= half {
        gradient_rgb(start, mid, idx, half)
    } else {
        gradient_rgb(
            mid,
            end,
            idx.saturating_sub(half),
            denom.saturating_sub(half),
        )
    }
}
