//! Waves clock face: an advanced sci-fi oscilloscope terminal screen drawn
//! with braille sub-pixels.
//!
//! Three smooth, continuous mathematical sine waves represent the flow of
//! hours, minutes, and seconds, with the current digital time displayed
//! in a high-contrast central card overlay.

use crate::braille::Canvas;
use crate::color;
use crate::config::Config;
use crate::faces::digital::time_text;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, Timelike};
use std::f64::consts::TAU;

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

    let usable_h = avail_h.saturating_sub(extra.len());
    let cols = avail_w.max(20);
    let rows = usable_h.max(6);

    let mut hr_wave = Canvas::new(cols, rows);
    let mut min_wave = Canvas::new(cols, rows);
    let mut sec_wave = Canvas::new(cols, rows);

    let w_px = hr_wave.width_px();
    let h_px = hr_wave.height_px();
    let cy = h_px / 2.0;

    // Hours wave: 1.0 cycle, large amplitude
    let hr_amp = h_px * 0.28;
    let hr_phase = (now.hour() % 12) as f64 / 12.0 * TAU;
    draw_sine(&mut hr_wave, cy, hr_amp, 1.0, hr_phase, w_px);

    // Minutes wave: 2.0 cycles, medium amplitude
    let min_amp = h_px * 0.20;
    let min_phase = now.minute() as f64 / 60.0 * TAU;
    draw_sine(&mut min_wave, cy, min_amp, 2.0, min_phase, w_px);

    // Seconds wave: 3.5 cycles, small amplitude
    if cfg.show_seconds {
        let sec_amp = h_px * 0.12;
        let sec_phase = now.second() as f64 / 60.0 * TAU;
        draw_sine(&mut sec_wave, cy, sec_amp, 3.5, sec_phase, w_px);
    }

    let hr_lines = hr_wave.lines();
    let min_lines = min_wave.lines();
    let sec_lines = sec_wave.lines();

    // Central Time Card overlay
    let (t_str, _, suffix) = time_text(now, cfg);
    let display_text = if suffix.is_empty() {
        format!("  {t_str}  ")
    } else {
        format!("  {t_str} {suffix}  ")
    };
    let card_w = display_text.chars().count();
    let card_h = 3; // 1 border, 1 text, 1 border

    let card_left = cols.saturating_sub(card_w) / 2;
    let card_top = rows.saturating_sub(card_h);

    let hr_c = accent;
    let min_c = color::lerp(accent, primary, 0.4);
    let sec_c = primary;
    let border_c = color::dim(primary, 0.45);

    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());
    for r_idx in 0..rows {
        let mut l: Line = Vec::new();
        let hc: Vec<char> = hr_lines[r_idx].chars().collect();
        let mc: Vec<char> = min_lines[r_idx].chars().collect();
        let sc: Vec<char> = sec_lines[r_idx].chars().collect();

        // Check if we are drawing the central card on this row
        let in_card_y = r_idx >= card_top && r_idx < card_top + card_h;

        for i in 0..cols {
            let at = |v: &Vec<char>| v.get(i).copied().unwrap_or(' ');
            let in_card_x = i >= card_left && i < card_left + card_w;

            if in_card_y && in_card_x {
                // Render the central card cell
                let cx_offset = i - card_left;
                let cy_offset = r_idx - card_top;
                let (ch, color) = if cy_offset == 0 {
                    if cx_offset == 0 {
                        ('\u{250c}', border_c)
                    } else if cx_offset == card_w - 1 {
                        ('\u{2510}', border_c)
                    } else {
                        ('\u{2500}', border_c)
                    }
                } else if cy_offset == card_h - 1 {
                    if cx_offset == 0 {
                        ('\u{2514}', border_c)
                    } else if cx_offset == card_w - 1 {
                        ('\u{2518}', border_c)
                    } else {
                        ('\u{2500}', border_c)
                    }
                } else if cx_offset == 0 || cx_offset == card_w - 1 {
                    ('\u{2502}', border_c)
                } else {
                    (display_text.chars().nth(cx_offset).unwrap_or(' '), primary)
                };
                match l.last_mut() {
                    Some(last) if last.color == color => last.text.push(ch),
                    _ => l.push(span(ch.to_string(), color)),
                }
            } else {
                // Render the waves
                let (ch, color) = if at(&sc) != ' ' {
                    (at(&sc), sec_c)
                } else if at(&mc) != ' ' {
                    (at(&mc), min_c)
                } else if at(&hc) != ' ' {
                    (at(&hc), hr_c)
                } else {
                    (' ', primary)
                };
                match l.last_mut() {
                    Some(last) if last.color == color => last.text.push(ch),
                    _ => l.push(span(ch.to_string(), color)),
                }
            }
        }
        lines.push(l);
    }

    lines.extend(extra);
    lines
}

fn draw_sine(canvas: &mut Canvas, cy: f64, amp: f64, freq: f64, phase: f64, w_px: f64) {
    let mut prev: Option<(f64, f64)> = None;
    for x in 0..=(w_px as usize) {
        let x_f = x as f64;
        let theta = (x_f / w_px) * freq * TAU + phase;
        let y = cy + amp * theta.sin();
        if let Some((qx, qy)) = prev {
            canvas.line(qx, qy, x_f, y);
        }
        prev = Some((x_f, y));
    }
}
