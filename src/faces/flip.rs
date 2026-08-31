//! Retro split-flap / airport-board clock: each digit sits on a card with a
//! horizontal seam across the middle.

use crate::color;
use crate::config::Config;
use crate::faces::digital::time_text;
use crate::render::{self, line_width, span, Line};
use chrono::{DateTime, Local};

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);

    let (text, _, suffix) = time_text(now, cfg);
    // Cards only for the digits; colons become a thin separator column.
    let digits: Vec<char> = text.chars().filter(|c| *c != ':').collect();
    let fit_digits = if cfg.show_seconds {
        digits.len()
    } else {
        digits.len() + 2
    };
    let fit_groups = fit_digits / 2;

    let mut reserved = 0;
    if !suffix.is_empty() {
        reserved += 2;
    }
    if cfg.show_date {
        reserved += 2;
    }
    let usable_h = avail_h.saturating_sub(reserved);

    // Cards add fixed chrome per digit, so solve for the unit size u that
    // makes the whole row of cards fit.
    let sep_w = 3;
    let fit = (1..=12)
        .rev()
        .find(|&u| {
            let cw = 8 * u + 4;
            let ch = 7 * u + 3;
            let total_w = fit_digits * cw + fit_groups.saturating_sub(1) * sep_w;
            total_w <= avail_w && ch <= usable_h
        })
        .unwrap_or(1);
    let u = if cfg.is_auto_scale() {
        fit
    } else {
        (cfg.scale as usize).clamp(1, 12)
    };

    let cw = 8 * u + 4;
    let ch = 7 * u + 3;
    let glyph_rows = 7 * u;
    let seam_row = 1 + glyph_rows / 2;
    let border = color::dim(primary, 0.55);

    // Each digit rendered once using the LCD font (seg7)
    let glyphs: Vec<Vec<Line>> = digits
        .iter()
        .enumerate()
        .map(|(i, d)| {
            // Ramp the gradient across the whole row of cards, not per card.
            let t0 = i as f64 / digits.len().max(2) as f64;
            let from = color::lerp(accent, primary, t0);
            let to = color::lerp(accent, primary, t0 + 0.3);
            crate::seg7::render(&d.to_string(), u, &[], false, &|t| color::lerp(from, to, t))
        })
        .collect();

    let inner_w = cw - 2;
    let mut lines: Vec<Line> = Vec::new();
    for row in 0..ch {
        let mut l: Line = Vec::new();
        for (i, glyph) in glyphs.iter().enumerate() {
            if i > 0 && i % 2 == 0 {
                // Blinking colon between the HH / MM / SS groups.
                let on = !cfg.blink_colon || now.timestamp_millis() / 500 % 2 != 0;
                let dot = row == ch / 3 || row == 2 * ch / 3;
                let mark = if on && dot { "\u{25cf}" } else { " " };
                l.push(span(format!(" {mark} "), accent));
            }

            if row == 0 {
                l.push(span(
                    format!("\u{250c}{}\u{2510}", "\u{2500}".repeat(inner_w)),
                    border,
                ));
            } else if row == ch - 1 {
                l.push(span(
                    format!("\u{2514}{}\u{2518}", "\u{2500}".repeat(inner_w)),
                    border,
                ));
            } else if row == seam_row {
                l.push(span(
                    format!("\u{251c}{}\u{2524}", "\u{2504}".repeat(inner_w)),
                    color::dim(primary, 0.4),
                ));
            } else {
                // Rows past the seam shift up by one to account for it.
                let gi = if row > seam_row { row - 2 } else { row - 1 };
                let empty: Line = Vec::new();
                let g = glyph.get(gi).unwrap_or(&empty);
                let gw = line_width(g);
                let pad = inner_w.saturating_sub(gw);
                let left = pad / 2;

                l.push(span("\u{2502}".to_string(), border));
                l.push(span(" ".repeat(left), primary));
                l.extend(g.iter().cloned());
                l.push(span(" ".repeat(pad - left), primary));
                l.push(span("\u{2502}".to_string(), border));
            }
        }
        lines.push(l);
    }

    if !suffix.is_empty() {
        lines.push(render::blank());
        lines.push(render::line(suffix, accent));
    }
    if cfg.show_date {
        lines.push(render::blank());
        lines.push(render::line(
            now.format("%A, %B %-d %Y").to_string(),
            color::dim(primary, 0.75),
        ));
    }
    lines
}
