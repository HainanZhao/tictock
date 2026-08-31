//! An hourglass that drains once an hour: the top chamber empties as the hour
//! runs out, a thread of sand falls through the neck, and a cone piles up
//! below. Drawn with braille sub-pixels.

use crate::braille::Canvas;
use crate::color;
use crate::config::Config;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, Timelike};
use crossterm::style::Color;

/// Neck half-width and how far the glass walls sit outside the sand, both in
/// sub-pixels relative to the overall width.
const NECK_FRAC: f64 = 0.05;
const WALL_GAP: f64 = 1.5;

/// Half-width of the chamber interior at height `y`, where the glass spans
/// `0..h` and pinches at `h/2`.
fn half_width_at(y: f64, w: f64, h: f64) -> f64 {
    let neck = w * NECK_FRAC;
    let half = w / 2.0;
    let mid = h / 2.0;
    if y <= mid {
        // Top chamber: widest at the very top, narrowing to the neck.
        half + (neck - half) * (y / mid)
    } else {
        // Bottom chamber: mirror image.
        neck + (half - neck) * ((y - mid) / mid)
    }
}

/// Draws a line thickened by `t` sub-pixels, so the glass reads as a vessel
/// rather than a hairline. Braille strokes are one dot wide on their own.
fn thick_line(c: &mut Canvas, x0: f64, y0: f64, x1: f64, y1: f64, t: usize) {
    for i in 0..t {
        let o = i as f64;
        // Offset across the stroke: horizontal runs thicken vertically and
        // vice versa, so corners stay closed.
        if (x1 - x0).abs() >= (y1 - y0).abs() {
            c.line(x0, y0 + o, x1, y1 + o);
        } else {
            c.line(x0 + o, y0, x1 + o, y1);
        }
    }
}

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);
    let sand_c = accent;

    let mut extra: Vec<Line> = Vec::new();
    let fmt = if cfg.hour12 {
        "%I:%M:%S %p"
    } else {
        "%H:%M:%S"
    };
    extra.push(render::blank());
    extra.push(render::line(now.format(fmt).to_string(), accent));
    if cfg.show_date {
        extra.push(render::blank());
        extra.push(render::line(
            now.format("%A, %B %-d %Y").to_string(),
            color::dim(primary, 0.75),
        ));
    }

    // Braille sub-pixels are square, so pick a cell box that gives the glass
    // a taller-than-wide silhouette.
    let rows = avail_h.saturating_sub(extra.len()).max(4);
    let cols = ((rows as f64 * 1.05) as usize).clamp(4, avail_w.max(4));
    let rows = rows.min(((cols as f64) / 1.05).ceil() as usize).max(4);

    let mut glass = Canvas::new(cols, rows);
    let mut sand = Canvas::new(cols, rows);
    let mut stream = Canvas::new(cols, rows);

    let px_w = glass.width_px();
    let px_h = glass.height_px();
    let cx = px_w / 2.0;
    // Leave a margin so the widest part of the glass isn't clipped.
    let w = px_w - 2.0;
    let h = px_h - 1.0;
    let mid = h / 2.0;
    let neck = w * NECK_FRAC;
    let half = w / 2.0;

    // The glass itself: two funnels plus chunkier end caps.
    thick_line(&mut glass, cx - half, 0.0, cx - neck, mid, 2);
    thick_line(&mut glass, cx + half, 0.0, cx + neck, mid, 2);
    thick_line(&mut glass, cx - neck, mid, cx - half, h, 2);
    thick_line(&mut glass, cx + neck, mid, cx + half, h, 2);
    thick_line(&mut glass, cx - half, 0.0, cx + half, 0.0, 3);
    thick_line(&mut glass, cx - half, h - 2.0, cx + half, h - 2.0, 3);

    // How far through the current hour we are — the glass runs for an hour,
    // which is the whole point of the name.
    let secs = now.minute() as f64 * 60.0
        + now.second() as f64
        + now.timestamp_subsec_millis() as f64 / 1000.0;
    let elapsed = (secs / 3600.0).clamp(0.0, 1.0);

    // Top chamber: sand rests on the neck, its surface falling as it drains.
    let surface_y = mid * elapsed;
    let mut y = surface_y;
    while y < mid {
        let hw = (half_width_at(y, w, h) - WALL_GAP).max(0.0);
        let mut x = cx - hw;
        while x <= cx + hw {
            sand.set(x, y);
            x += 1.0;
        }
        y += 1.0;
    }

    // Bottom chamber: a cone piling up under the neck.
    let pile_h = mid * elapsed;
    let pile_top = h - pile_h;
    let mut y = pile_top;
    while y <= h - 2.0 {
        let depth = y - pile_top;
        let chamber = (half_width_at(y, w, h) - WALL_GAP).max(0.0);
        let cone = depth * 1.6 + neck;
        let hw = chamber.min(cone);
        let mut x = cx - hw;
        while x <= cx + hw {
            sand.set(x, y);
            x += 1.0;
        }
        y += 1.0;
    }

    // The falling thread, while there is anything left to fall.
    if elapsed > 0.0 && elapsed < 1.0 {
        let top = mid;
        let bottom = (pile_top - 1.0).max(mid);
        let mut y = top;
        while y <= bottom {
            stream.set(cx - 1.0, y);
            stream.set(cx, y);
            y += 1.0;
        }
    }

    let glass_l = glass.lines();
    let sand_l = sand.lines();
    let stream_l = stream.lines();

    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());
    for r in 0..rows {
        let gc: Vec<char> = glass_l[r].chars().collect();
        let sc: Vec<char> = sand_l[r].chars().collect();
        let tc: Vec<char> = stream_l[r].chars().collect();

        let mut out: Line = Vec::new();
        for i in 0..cols {
            let at = |v: &Vec<char>| v.get(i).copied().unwrap_or(' ');
            // Falling sand reads over the pile, which reads over the glass.
            let (ch, c): (char, Color) = if at(&tc) != ' ' {
                (at(&tc), color::lerp(sand_c, Color::White, 0.35))
            } else if at(&sc) != ' ' {
                (at(&sc), sand_c)
            } else {
                (at(&gc), color::dim(primary, 0.9))
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
