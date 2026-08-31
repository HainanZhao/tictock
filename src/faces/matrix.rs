//! A sharper 7-segment digital face drawn with braille sub-pixels.

use crate::braille::Canvas;
use crate::color;
use crate::config::{Config, MAX_SCALE};
use crate::faces::digital::{blink_mask, time_text};
use crate::render::{self, Line};
use chrono::{DateTime, Local};

/// Which of the 7 segments (a=top, b=upper-right, c=lower-right, d=bottom,
/// e=lower-left, f=upper-left, g=middle) are lit for a digit.
fn segments(c: char) -> [bool; 7] {
    match c {
        '0' => [true, true, true, true, true, true, false],
        '1' => [false, true, true, false, false, false, false],
        '2' => [true, true, false, true, true, false, true],
        '3' => [true, true, true, true, false, false, true],
        '4' => [false, true, true, false, false, true, true],
        '5' => [true, false, true, true, false, true, true],
        '6' => [true, false, true, true, true, true, true],
        '7' => [true, true, true, false, false, false, false],
        '8' => [true, true, true, true, true, true, true],
        '9' => [true, true, true, true, false, true, true],
        _ => [false; 7],
    }
}

/// Sub-pixel geometry for one digit at `scale`.
fn digit_w(scale: usize) -> f64 {
    8.0 * scale as f64
}
fn digit_h(scale: usize) -> f64 {
    16.0 * scale as f64
}
fn colon_w(scale: usize) -> f64 {
    4.0 * scale as f64
}
fn gutter(scale: usize) -> f64 {
    4.0 * scale as f64
}

fn total_px_w(text: &str, scale: usize) -> f64 {
    let n = text.chars().count();
    let sum: f64 = text
        .chars()
        .map(|c| {
            if c == ':' {
                colon_w(scale)
            } else {
                digit_w(scale)
            }
        })
        .sum();
    sum + gutter(scale) * (n.max(1) - 1) as f64
}

fn draw(text: &str, scale: usize, mask: &[bool]) -> Vec<String> {
    let w = digit_w(scale);
    let h = digit_h(scale);
    let cols = (total_px_w(text, scale) / 2.0).ceil() as usize + 1;
    let rows = (h / 4.0).ceil() as usize + 1;
    let mut canvas = Canvas::new(cols, rows);

    let mut x = 0.0;
    for (i, c) in text.chars().enumerate() {
        let blank = mask.get(i).copied().unwrap_or(false);
        if !blank {
            if c == ':' {
                let cx = x + colon_w(scale) / 2.0;
                let dot = (scale as f64).max(1.0);
                canvas.line(cx, h * 0.30 - dot, cx, h * 0.30 + dot);
                canvas.line(cx, h * 0.70 - dot, cx, h * 0.70 + dot);
            } else {
                let seg = segments(c);
                let pts = [
                    (seg[0], (x, 0.0), (x + w, 0.0)),
                    (seg[1], (x + w, 0.0), (x + w, h / 2.0)),
                    (seg[2], (x + w, h / 2.0), (x + w, h)),
                    (seg[3], (x, h), (x + w, h)),
                    (seg[4], (x, h / 2.0), (x, h)),
                    (seg[5], (x, 0.0), (x, h / 2.0)),
                    (seg[6], (x, h / 2.0), (x + w, h / 2.0)),
                ];
                for (on, p0, p1) in pts {
                    if on {
                        canvas.line(p0.0, p0.1, p1.0, p1.1);
                    }
                }
            }
        }
        x += if c == ':' { colon_w(scale) } else { w } + gutter(scale);
    }
    canvas.lines()
}

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let (text, colons, suffix) = time_text(now, cfg);
    let n = text.chars().count();

    let mut reserved = 0;
    if !suffix.is_empty() {
        reserved += 2;
    }
    if cfg.show_date {
        reserved += 2;
    }
    let usable_h = avail_h.saturating_sub(reserved);

    let fit = (1..=MAX_SCALE as usize)
        .rev()
        .find(|&s| {
            let fit_text = if cfg.show_seconds {
                text.to_string()
            } else {
                format!("{text}:00")
            };
            ((total_px_w(&fit_text, s) / 2.0).ceil() as usize) < avail_w
                && ((digit_h(s) / 4.0).ceil() as usize) < usable_h
        })
        .unwrap_or(1);
    let scale = cfg.resolve_scale(fit);

    let mask = blink_mask(now, cfg, n, &colons);
    let plain = draw(&text, scale, &mask);

    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);
    let mut lines = render::gradient_block(&plain, primary, accent);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_digits() {
        for scale in 1..=4 {
            println!("=== SCALE {scale} ===");
            for &digit in &["0", "4"] {
                println!("=== Digit {digit} ===");
                let plain = draw(digit, scale, &[]);
                for line in plain {
                    println!("{line}");
                }
            }
        }
    }
}
