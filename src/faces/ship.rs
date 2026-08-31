//! Ship's Wheel clock face: an elegant maritime ship steering wheel analog clock
//! drawn with braille sub-pixels.

use crate::braille::Canvas;
use crate::color;
use crate::config::Config;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, Timelike};
use std::f64::consts::{PI, TAU};

fn radius_for(avail_w: usize, avail_h: usize, reserved: usize) -> f64 {
    let h = avail_h.saturating_sub(reserved) as f64;
    ((h - 2.0).min(avail_w as f64 / 2.0 - 1.5)).max(4.0)
}

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);

    let mut extra: Vec<Line> = Vec::new();
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

    let mut wheel = Canvas::new(cols, rows);
    let mut hands = Canvas::new(cols, rows);
    let mut sec_hand = Canvas::new(cols, rows);

    let cx = wheel.width_px() / 2.0;
    let cy = wheel.height_px() / 2.0;
    let r = radius * 2.0;

    let r_inner = r * 0.60;
    let r_outer = r * 0.80;
    // Keep the handle tips and cross-guards inside the canvas. Extending
    // beyond the nominal radius clips the cardinal handles and makes the
    // wheel look accidentally cropped.
    let r_spoke = r * 0.92;

    // Draw the steering wheel rims (inner and outer rings)
    wheel.circle(cx, cy, r_inner);
    wheel.circle(cx, cy, r_outer);

    // Draw the 8 steering wheel handles / spokes
    for s in 0..8 {
        let theta = (s as f64) * PI / 4.0;
        // Draw spokes from near center out past the outer rim to form handles
        wheel.line(
            cx + (r_inner * 0.4) * theta.cos(),
            cy + (r_inner * 0.4) * theta.sin(),
            cx + r_spoke * theta.cos(),
            cy + r_spoke * theta.sin(),
        );
        // Draw small handle cross-guards at the tip of each spoke
        let guard_len = r * 0.055;
        let guard_angle = theta + PI / 2.0;
        let tip_x = cx + r_spoke * theta.cos();
        let tip_y = cy + r_spoke * theta.sin();
        wheel.line(
            tip_x - guard_len * guard_angle.cos(),
            tip_y - guard_len * guard_angle.sin(),
            tip_x + guard_len * guard_angle.cos(),
            tip_y + guard_len * guard_angle.sin(),
        );
    }

    // Draw the elegant center dial hub
    wheel.circle(cx, cy, r_inner * 0.25);

    // Draw Clock Hands (inside the inner rim)
    let hand = |canvas: &mut Canvas, len: f64, units: f64, per_rev: f64| {
        let theta = units / per_rev * TAU - PI / 2.0;
        canvas.line(cx, cy, cx + len * theta.cos(), cy + len * theta.sin());
    };

    let hour_units = (now.hour() % 12) as f64 + now.minute() as f64 / 60.0;
    let min_units = now.minute() as f64 + now.second() as f64 / 60.0;
    hand(&mut hands, r * 0.38, hour_units, 12.0);
    hand(&mut hands, r * 0.55, min_units, 60.0);
    hand(&mut sec_hand, r * 0.64, now.second() as f64, 60.0);

    let wheel_l = wheel.lines();
    let hands_l = hands.lines();
    let sec_l = sec_hand.lines();

    // Maritime colors: weathered wood/bronze (primary), ocean accent (accent), crimson seconds.
    let wheel_c = primary;
    let hands_c = accent;
    let sec_c = primary;

    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());
    for r_idx in 0..rows {
        let wc: Vec<char> = wheel_l[r_idx].chars().collect();
        let hc: Vec<char> = hands_l[r_idx].chars().collect();
        let sc: Vec<char> = sec_l[r_idx].chars().collect();

        let mut out: Line = Vec::new();
        for i in 0..cols {
            let at = |v: &Vec<char>| v.get(i).copied().unwrap_or(' ');
            // Priority: seconds on top, then minute/hour hands, then the wheel rim/spokes.
            let (ch, c) = if at(&sc) != ' ' {
                (at(&sc), sec_c)
            } else if at(&hc) != ' ' {
                (at(&hc), hands_c)
            } else {
                (at(&wc), wheel_c)
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
