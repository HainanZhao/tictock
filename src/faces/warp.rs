//! Warp clock face: an immersive 3D Star Trek warp-speed time travel zoom effect
//! drawn with braille sub-pixels.
//!
//! Stars fly out from the center and stretch into long motion-blurred warp trails,
//! with the current time displayed in a high-contrast central card overlay.

use crate::braille::Canvas;
use crate::color;
use crate::config::Config;
use crate::faces::digital::time_text;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local};
use std::sync::OnceLock;

struct Star {
    rx: f64,
    ry: f64,
    speed: f64,
    start_z: f64,
}

static STARS: OnceLock<Vec<Star>> = OnceLock::new();

fn get_stars() -> &'static [Star] {
    STARS.get_or_init(|| {
        let mut stars = Vec::with_capacity(100);
        for i in 0..100 {
            // Pre-compute static pseudo-random parameters on first run
            let seed_x = (i as f64 * 17.293).sin();
            let seed_y = (i as f64 * 43.821).sin();
            let seed_z = (i as f64 * 73.197).sin();
            let seed_sp = (i as f64 * 97.433).sin();

            let rx = seed_x - seed_x.floor() - 0.5;
            let ry = seed_y - seed_y.floor() - 0.5;
            let speed = 0.25 + (seed_sp - seed_sp.floor()) * 0.35;
            let start_z = seed_z - seed_z.floor();

            stars.push(Star {
                rx,
                ry,
                speed,
                start_z,
            });
        }
        stars
    })
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

    let usable_h = avail_h.saturating_sub(extra.len());
    let cols = avail_w.max(20);
    let rows = usable_h.max(6);

    let mut canvas = Canvas::new(cols, rows);

    let w_px = canvas.width_px();
    let h_px = canvas.height_px();
    let cx = w_px / 2.0;
    let cy = h_px / 2.0;

    // Use current timestamp in seconds for smooth movement
    let t = now.timestamp_millis() as f64 / 1000.0;

    // Render pre-computed 3D stars with warp speed trails
    let stars = get_stars();
    for star in stars {
        // Current depth z loops smoothly from 1.0 (far) down to 0.0 (near)
        let z = (star.start_z - t * star.speed).rem_euclid(1.0);

        // Skip stars that are too close to prevent division by zero / infinite stretching
        if z < 0.04 {
            continue;
        }

        // Project current position to 2D screen coordinates
        let sx = cx + (star.rx * w_px) / z;
        let sy = cy + (star.ry * h_px) / z;

        // Calculate a slightly past position to create the motion-blurred warp trail
        let z_prev = (star.start_z - (t - 0.04) * star.speed).rem_euclid(1.0);
        if z_prev >= z {
            let px = cx + (star.rx * w_px) / z_prev;
            let py = cy + (star.ry * h_px) / z_prev;

            // Only draw trail if it is within reasonable bounds of the canvas
            if sx >= 0.0 && sx < w_px && sy >= 0.0 && sy < h_px {
                canvas.line(px, py, sx, sy);
            }
        } else {
            // Star just wrapped around, draw it as a single point
            if sx >= 0.0 && sx < w_px && sy >= 0.0 && sy < h_px {
                canvas.set(sx, sy);
            }
        }
    }

    let c_lines = canvas.lines();

    // Central Time Card overlay (our destination in the space-time continuum!)
    let (t_str, _, suffix) = time_text(now, cfg);
    let display_text = if suffix.is_empty() {
        format!("  {t_str}  ")
    } else {
        format!("  {t_str} {suffix}  ")
    };
    let card_w = display_text.chars().count();
    let card_h = 3; // 1 border, 1 text, 1 border

    let card_left = cols.saturating_sub(card_w) / 2;
    let card_top = rows.saturating_sub(card_h) / 2;

    let border_c = color::dim(primary, 0.45);

    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());
    for (r_idx, canvas_line) in c_lines.iter().enumerate().take(rows) {
        let mut l: Line = Vec::new();
        let cc: Vec<char> = canvas_line.chars().collect();

        // Check if we are drawing the central card on this row
        let in_card_y = r_idx >= card_top && r_idx < card_top + card_h;

        for i in 0..cols {
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
                // Render the warp stars in accent color (or primary if accent is none)
                let ch = cc.get(i).copied().unwrap_or(' ');
                let color = if ch != ' ' { accent } else { primary };
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
