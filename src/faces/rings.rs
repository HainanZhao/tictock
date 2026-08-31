//! Concentric progress arcs — outer ring is seconds, then minutes, then
//! hours — drawn with braille sub-pixels, with the time in the middle.

use crate::braille::Canvas;
use crate::color;
use crate::config::Config;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, Timelike};
use crossterm::style::Color;
use std::f64::consts::{FRAC_PI_2, TAU};

/// Draws the arc from 12 o'clock clockwise through `frac` of a full turn.
fn arc(canvas: &mut Canvas, cx: f64, cy: f64, r: f64, frac: f64) {
    let steps = ((r * 6.0).max(48.0)) as u32;
    let end = (frac.clamp(0.0, 1.0) * steps as f64) as u32;
    for i in 0..=end {
        let theta = (i as f64) / (steps as f64) * TAU - FRAC_PI_2;
        canvas.set(cx + r * theta.cos(), cy + r * theta.sin());
    }
}

fn full_circle(canvas: &mut Canvas, cx: f64, cy: f64, r: f64) {
    canvas.circle(cx, cy, r);
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

    let h = avail_h.saturating_sub(extra.len()) as f64;
    let radius = ((h - 2.0).min(avail_w as f64 / 2.0 - 1.5)).max(4.0);
    let cols = (radius * 2.0 + 3.0).ceil() as usize;
    let rows = (radius + 2.0).ceil() as usize;

    let hour_max = if cfg.hour12 { 12.0 } else { 24.0 };
    let h_frac = ((now.hour() as f64 % hour_max) + now.minute() as f64 / 60.0) / hour_max;
    let m_frac = (now.minute() as f64 + now.second() as f64 / 60.0) / 60.0;
    let s_frac = now.second() as f64 / 60.0;

    // Outermost first so inner rings sit inside it.
    let specs = vec![
        (s_frac, 1.00, primary),
        (m_frac, 0.74, color::lerp(primary, accent, 0.5)),
        (h_frac, 0.48, accent),
    ];

    let mut track_canvases = Vec::new();
    let mut arc_canvases = Vec::new();
    for (frac, r_frac, _) in &specs {
        let mut track = Canvas::new(cols, rows);
        let mut fill = Canvas::new(cols, rows);
        let cx = track.width_px() / 2.0;
        let cy = track.height_px() / 2.0;
        let r = radius * 2.0 * r_frac;
        full_circle(&mut track, cx, cy, r);
        arc(&mut fill, cx, cy, r, *frac);
        track_canvases.push(track.lines());
        arc_canvases.push(fill.lines());
    }

    // Centered time readout, overlaid on the middle of the rings.
    let time_fmt = if cfg.hour12 { "%I:%M" } else { "%H:%M" };
    let center_text = format!(" {} ", now.format(time_fmt));
    let center_row = rows / 2;
    let center_start = cols.saturating_sub(center_text.chars().count()) / 2;

    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());
    for r_idx in 0..rows {
        let mut out: Line = Vec::new();
        for i in 0..cols {
            // The time label wins over any ring pixel underneath it.
            if r_idx == center_row
                && i >= center_start
                && i < center_start + center_text.chars().count()
            {
                let ch = center_text.chars().nth(i - center_start).unwrap();
                match out.last_mut() {
                    Some(last) if last.color == primary => last.text.push(ch),
                    _ => out.push(span(ch.to_string(), primary)),
                }
                continue;
            }

            let mut picked: Option<(char, Color)> = None;
            for (idx, (_, _, base)) in specs.iter().enumerate() {
                let at = |v: &Vec<String>| -> char { v[r_idx].chars().nth(i).unwrap_or(' ') };
                let a = at(&arc_canvases[idx]);
                if a != ' ' {
                    picked = Some((a, *base));
                    break;
                }
                let t = at(&track_canvases[idx]);
                if t != ' ' && picked.is_none() {
                    picked = Some((t, color::dim(*base, 0.25)));
                }
            }
            let (ch, c) = picked.unwrap_or((' ', primary));
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
