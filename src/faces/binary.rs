//! Binary-coded-decimal clock: each decimal digit of HH:MM(:SS) is a column
//! of 4 dots (bit weights 8,4,2,1 top to bottom), sized to fill the terminal.

use crate::color;
use crate::config::Config;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, Timelike};
use crossterm::style::Color;

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);

    let hour = if cfg.hour12 {
        let h = now.hour12().1;
        if h == 0 {
            12
        } else {
            h
        }
    } else {
        now.hour()
    };

    let mut digits = vec![hour / 10, hour % 10, now.minute() / 10, now.minute() % 10];
    let mut groups = vec![0usize, 0, 1, 1];
    if cfg.show_seconds {
        digits.push(now.second() / 10);
        digits.push(now.second() % 10);
        groups.push(2);
        groups.push(2);
    }
    let n = digits.len();
    let fit_n = if cfg.show_seconds { n } else { n + 2 };

    let bcd_height = |cell: usize| {
        let dot_h = cell.div_ceil(2);
        let gap_y = if dot_h > 1 { 1 } else { 0 };
        4 * dot_h + 3 * gap_y
    };

    // Grow dot spacing/size to use the available width, within reason.
    let reserved = if cfg.show_date { 4 } else { 2 };
    let fit = (1..=6)
        .rev()
        .find(|&c| (2 * fit_n - 1) * c <= avail_w && bcd_height(c) + reserved <= avail_h)
        .unwrap_or(1);
    let cell = cfg.resolve_scale(fit);
    let dot_w = cell.max(1);
    let gap = dot_w;

    // One hue per HH / MM / SS group.
    let group_color = |g: usize| match g {
        0 => accent,
        1 => color::lerp(accent, primary, 0.6),
        _ => primary,
    };
    let off = Color::DarkGrey;

    let mut lines: Vec<Line> = Vec::new();

    // Header row labelling the columns.
    let mut header: Line = Vec::new();
    for (i, g) in groups.iter().enumerate() {
        if i > 0 {
            header.push(span(" ".repeat(gap), off));
        }
        let ch = match g {
            0 => 'H',
            1 => 'M',
            _ => 'S',
        };
        let mut cell_text = String::new();
        let pad = dot_w.saturating_sub(1) / 2;
        cell_text.push_str(&" ".repeat(pad));
        cell_text.push(ch);
        cell_text.push_str(&" ".repeat(dot_w.saturating_sub(pad + 1)));
        header.push(span(cell_text, color::dim(group_color(*g), 0.8)));
    }
    lines.push(header);
    lines.push(render::blank());

    let dot_h = dot_w.div_ceil(2);
    let gap_y = if dot_h > 1 { 1 } else { 0 };

    for row in 0..4 {
        let weight = 1 << (3 - row);
        if row > 0 && gap_y > 0 {
            lines.push(render::blank());
        }
        for sub_row in 0..dot_h {
            let mut l: Line = Vec::new();
            for (i, d) in digits.iter().enumerate() {
                if i > 0 {
                    l.push(span(" ".repeat(gap), off));
                }
                let lit = d & weight != 0;
                let c = if lit {
                    group_color(groups[i])
                } else {
                    color::dim(off, 0.55)
                };
                if lit {
                    l.push(span("\u{2588}".repeat(dot_w), c));
                } else if sub_row == dot_h / 2 {
                    let pad = dot_w / 2;
                    let pad_after = dot_w - pad - 1;
                    let cell_text =
                        format!("{}{}{}", " ".repeat(pad), '\u{00b7}', " ".repeat(pad_after));
                    l.push(span(cell_text, c));
                } else {
                    l.push(span(" ".repeat(dot_w), c));
                }
            }
            lines.push(l);
        }
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
