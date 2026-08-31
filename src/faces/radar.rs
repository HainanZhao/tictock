//! Radar clock face: an aviation/marine style radar sweep drawn with braille sub-pixels.
//!
//! A sweeping radar ray rotates once a minute, while target "blips" represent
//! the hours and minutes at different radial depths.

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

fn disk(canvas: &mut Canvas, cx: f64, cy: f64, r: f64) {
    let steps = (r * 2.0) as usize;
    for i in 0..=steps {
        canvas.circle(cx, cy, i as f64 * 0.5);
    }
}

fn dotted_circle(canvas: &mut Canvas, cx: f64, cy: f64, r: f64, dot_spacing: usize) {
    let arc_px = r * TAU;
    let steps = (arc_px * 2.0).max(1.0) as u32;
    for i in 0..=steps {
        if i % (dot_spacing as u32) == 0 {
            let theta = i as f64 / steps as f64 * TAU;
            canvas.set(cx + r * theta.cos(), cy + r * theta.sin());
        }
    }
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

    let mut grid = Canvas::new(cols, rows);
    let mut ray = Canvas::new(cols, rows);
    let mut blips = Canvas::new(cols, rows);

    let cx = grid.width_px() / 2.0;
    let cy = grid.height_px() / 2.0;
    let r = radius * 2.0;

    let r_hr = r * 0.50;
    let r_min = r * 0.80;

    // Draw the radar concentric grid rings.
    dotted_circle(&mut grid, cx, cy, r_hr, 12);
    dotted_circle(&mut grid, cx, cy, r_min, 16);
    grid.circle(cx, cy, r); // Outer solid radar rim

    // Draw cardinal crosshairs.
    for angle in [0.0, PI / 2.0, PI, 3.0 * PI / 2.0] {
        dotted_circle_line(&mut grid, cx, cy, r, angle, 6);
    }

    // Sweeping Radar Ray (Seconds hand position).
    let sec_angle = now.second() as f64 / 60.0 * TAU - PI / 2.0;
    ray.line(cx, cy, cx + r * sec_angle.cos(), cy + r * sec_angle.sin());

    // Target Blips for Hours and Minutes.
    let hr_angle = ((now.hour() % 12) as f64 + now.minute() as f64 / 60.0) / 12.0 * TAU - PI / 2.0;
    let hr_x = cx + r_hr * hr_angle.cos();
    let hr_y = cy + r_hr * hr_angle.sin();
    disk(&mut blips, hr_x, hr_y, 2.2);

    let min_angle = (now.minute() as f64 + now.second() as f64 / 60.0) / 60.0 * TAU - PI / 2.0;
    let min_x = cx + r_min * min_angle.cos();
    let min_y = cy + r_min * min_angle.sin();
    disk(&mut blips, min_x, min_y, 1.8);

    let grid_l = grid.lines();
    let ray_l = ray.lines();
    let blips_l = blips.lines();

    // Classic Radar Green or customized primary, bright ray, orange target blips.
    let ray_c = primary;
    let blip_c = accent;
    let grid_c = color::dim(primary, 0.35);
    let time_fmt = if cfg.hour12 { "%I:%M %p" } else { "%H:%M" };
    let time_label = format!(" {} ", now.format(time_fmt));
    let label_row = rows * 3 / 4;
    let label_start = cols.saturating_sub(time_label.chars().count()) / 2;

    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());
    for r_idx in 0..rows {
        let gc: Vec<char> = grid_l[r_idx].chars().collect();
        let rc: Vec<char> = ray_l[r_idx].chars().collect();
        let bc: Vec<char> = blips_l[r_idx].chars().collect();

        let mut out: Line = Vec::new();
        for i in 0..cols {
            let at = |v: &Vec<char>| v.get(i).copied().unwrap_or(' ');
            let in_label = r_idx == label_row
                && i >= label_start
                && i < label_start + time_label.chars().count();
            let (ch, c) = if in_label {
                (
                    time_label.chars().nth(i - label_start).unwrap_or(' '),
                    accent,
                )
            } else if at(&bc) != ' ' {
                (at(&bc), blip_c)
            } else if at(&rc) != ' ' {
                (at(&rc), ray_c)
            } else {
                (at(&gc), grid_c)
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

fn dotted_circle_line(
    canvas: &mut Canvas,
    cx: f64,
    cy: f64,
    max_r: f64,
    angle: f64,
    spacing: usize,
) {
    let steps = (max_r / 2.0) as usize;
    for i in 0..=steps {
        if i % spacing == 0 {
            let curr_r = i as f64 * 2.0;
            canvas.set(cx + curr_r * angle.cos(), cy + curr_r * angle.sin());
        }
    }
}
