//! Maps human-friendly color names (used in the config file / CLI) to crossterm colors.

use crossterm::style::Color;

/// Parses a color name into a `crossterm::style::Color`.
///
/// Accepts the standard ANSI names (plus "grey" as an alias for "gray") and
/// `#rrggbb` / `rgb(r,g,b)` for truecolor terminals. Falls back to `White`
/// for anything unrecognized so a typo in the config never crashes the app.
pub fn parse(name: &str) -> Color {
    let n = name.trim().to_ascii_lowercase();

    if let Some(hex) = n.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                return Color::Rgb {
                    r: ((v >> 16) & 0xff) as u8,
                    g: ((v >> 8) & 0xff) as u8,
                    b: (v & 0xff) as u8,
                };
            }
        }
    }
    if let Some(inner) = n.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
        if parts.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
            ) {
                return Color::Rgb { r, g, b };
            }
        }
    }

    match n.as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" | "purple" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::DarkGrey,
        "dark_red" => Color::DarkRed,
        "dark_green" => Color::DarkGreen,
        "dark_yellow" | "orange" => Color::DarkYellow,
        "dark_blue" => Color::DarkBlue,
        "dark_magenta" => Color::DarkMagenta,
        "dark_cyan" => Color::DarkCyan,
        _ => Color::White,
    }
}

/// Approximate RGB for any color, so we can interpolate between named ANSI
/// colors and truecolor values uniformly.
pub fn to_rgb(c: Color) -> (u8, u8, u8) {
    match c {
        Color::Rgb { r, g, b } => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::DarkGrey => (100, 100, 100),
        Color::Red => (255, 85, 85),
        Color::DarkRed => (170, 0, 0),
        Color::Green => (85, 255, 85),
        Color::DarkGreen => (0, 170, 0),
        Color::Yellow => (255, 255, 85),
        Color::DarkYellow => (255, 145, 0),
        Color::Blue => (85, 130, 255),
        Color::DarkBlue => (0, 0, 170),
        Color::Magenta => (255, 85, 255),
        Color::DarkMagenta => (170, 0, 170),
        Color::Cyan => (85, 255, 255),
        Color::DarkCyan => (0, 170, 170),
        Color::White | Color::Grey => (229, 229, 229),
        _ => (200, 200, 200),
    }
}

/// Linear blend between two colors; `t` is clamped to 0..=1.
pub fn lerp(a: Color, b: Color, t: f64) -> Color {
    let t = t.clamp(0.0, 1.0);
    let (ar, ag, ab) = to_rgb(a);
    let (br, bg, bb) = to_rgb(b);
    let mix = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * t).round() as u8;
    Color::Rgb {
        r: mix(ar, br),
        g: mix(ag, bg),
        b: mix(ab, bb),
    }
}

/// Scales a color's brightness, used to dim the "unfilled" part of bars and
/// rings without needing a second configured color.
pub fn dim(c: Color, factor: f64) -> Color {
    let (r, g, b) = to_rgb(c);
    let f = |v: u8| (v as f64 * factor).clamp(0.0, 255.0).round() as u8;
    Color::Rgb {
        r: f(r),
        g: f(g),
        b: f(b),
    }
}

/// The list of built-in color names, shown in `clock config colors`.
pub const NAMES: &[&str] = &[
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "gray",
    "dark_red",
    "dark_green",
    "dark_yellow",
    "dark_blue",
    "dark_magenta",
    "dark_cyan",
    "#rrggbb (truecolor)",
];
