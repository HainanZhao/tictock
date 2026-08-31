//! Classic round clock face, drawn with braille sub-pixels.

use crate::braille::Canvas;
use crate::color;
use crate::config::Config;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, Timelike};
use std::f64::consts::TAU;

/// Braille cells are twice as tall as they are wide relative to a terminal
/// cell. In sub-pixel space, cells are 2x4 dots, so sub-pixels are exactly
/// square. To keep the face round and fill the terminal efficiently, the
/// column radius is twice the row radius.
fn radius_for(avail_w: usize, avail_h: usize, reserved: usize) -> f64 {
    let h = avail_h.saturating_sub(reserved) as f64;
    ((h - 2.0).min(avail_w as f64 / 2.0 - 1.5)).max(3.0)
}

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);

    let mut extra: Vec<Line> = Vec::new();
    let time_fmt = if cfg.hour12 {
        "%I:%M:%S %p"
    } else {
        "%H:%M:%S"
    };
    if cfg.show_seconds {
        extra.push(render::blank());
        extra.push(render::line(now.format(time_fmt).to_string(), accent));
    }
    if cfg.show_date {
        extra.push(render::blank());
        extra.push(render::line(
            now.format("%A, %B %-d %Y").to_string(),
            color::dim(primary, 0.75),
        ));
    }

    let radius = radius_for(avail_w, avail_h, extra.len());
    let cols = (radius * 2.0 + 3.0).ceil() as usize;
    let rows = (radius + 2.0).ceil() as usize;
    let mut face = Canvas::new(cols, rows);
    let mut hour_hand = Canvas::new(cols, rows);
    let mut min_hand = Canvas::new(cols, rows);
    let mut sec_hand = Canvas::new(cols, rows);
    let mut hub = Canvas::new(cols, rows);

    let cx = face.width_px() / 2.0;
    let cy = face.height_px() / 2.0;
    let r = radius * 2.0;

    face.circle(cx, cy, r);
    if cfg.tick_marks {
        for h in 0..12 {
            let theta = (h as f64) / 12.0 * TAU - std::f64::consts::FRAC_PI_2;
            let inner = if h % 3 == 0 { r * 0.80 } else { r * 0.90 };
            face.line(
                cx + inner * theta.cos(),
                cy + inner * theta.sin(),
                cx + r * theta.cos(),
                cy + r * theta.sin(),
            );
        }
    }

    let hand = |canvas: &mut Canvas,
                len_frac: f64,
                tail_frac: f64,
                units: f64,
                per_rev: f64,
                half_thickness: i32| {
        let theta = units / per_rev * TAU - std::f64::consts::FRAC_PI_2;
        let len = r * len_frac;
        let tail = r * tail_frac;
        let normal = (-theta.sin(), theta.cos());
        for offset in -half_thickness..=half_thickness {
            let offset = offset as f64;
            canvas.line(
                cx - tail * theta.cos() + normal.0 * offset,
                cy - tail * theta.sin() + normal.1 * offset,
                cx + len * theta.cos() + normal.0 * offset,
                cy + len * theta.sin() + normal.1 * offset,
            );
        }
    };

    let hour_units = (now.hour() % 12) as f64 + now.minute() as f64 / 60.0;
    let min_units = now.minute() as f64 + now.second() as f64 / 60.0;
    hand(&mut hour_hand, 0.50, 0.02, hour_units, 12.0, 1);
    hand(&mut min_hand, 0.78, 0.03, min_units, 60.0, 0);
    hand(&mut sec_hand, 0.92, 0.12, now.second() as f64, 60.0, 0);
    for pin_r in [0.5, 1.0, 1.5, 2.0] {
        hub.circle(cx, cy, pin_r);
    }

    let face_l = face.lines();
    let hour_l = hour_hand.lines();
    let min_l = min_hand.lines();
    let sec_l = sec_hand.lines();
    let hub_l = hub.lines();

    // Each hand gets its own hue so the three are readable at a glance.
    let hour_c = accent;
    let min_c = color::lerp(accent, primary, 0.5);
    let sec_c = primary;

    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());
    for r_idx in 0..rows {
        let fc: Vec<char> = face_l[r_idx].chars().collect();
        let hc: Vec<char> = hour_l[r_idx].chars().collect();
        let mc: Vec<char> = min_l[r_idx].chars().collect();
        let sc: Vec<char> = sec_l[r_idx].chars().collect();
        let pc: Vec<char> = hub_l[r_idx].chars().collect();

        let mut out: Line = Vec::new();
        for i in 0..cols {
            let at = |v: &Vec<char>| v.get(i).copied().unwrap_or(' ');
            // The center pin masks the hand joins, then the slender seconds
            // hand sits above the minute and substantial hour hand.
            let (ch, c) = if at(&pc) != ' ' {
                (at(&pc), accent)
            } else if at(&sc) != ' ' {
                (at(&sc), sec_c)
            } else if at(&mc) != ' ' {
                (at(&mc), min_c)
            } else if at(&hc) != ' ' {
                (at(&hc), hour_c)
            } else {
                (at(&fc), primary)
            };
            match out.last_mut() {
                Some(last) if last.color == c => last.text.push(ch),
                _ => out.push(span(ch.to_string(), c)),
            }
        }
        lines.push(out);
    }

    lines.extend(extra);
    lines
}
