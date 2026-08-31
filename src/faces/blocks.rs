//! The whole day as a grid of blocks, one lit per interval elapsed.
//!
//! A day is 86,400 seconds and a terminal rarely has that many cells, so the
//! interval each block stands for is chosen from a ladder of round durations —
//! the finest one whose grid still fits the screen.

use crate::color;
use crate::config::Config;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, Timelike};

const DAY_SECS: usize = 24 * 60 * 60;

/// Round intervals a block may represent, finest first. All of them divide an
/// hour, so hour boundaries always land at a block boundary.
const LADDER: [usize; 15] = [1, 2, 3, 4, 5, 6, 10, 12, 15, 20, 30, 60, 120, 300, 600];

/// Picks the finest interval whose grid fits, and reports the grid size.
/// Falls back to the coarsest rung when even that overflows.
fn choose(cols: usize, rows_avail: usize) -> (usize, usize) {
    for step in LADDER {
        let blocks = DAY_SECS / step;
        if blocks.div_ceil(cols.max(1)) <= rows_avail {
            return (step, blocks);
        }
    }
    let step = *LADDER.last().unwrap();
    (step, DAY_SECS / step)
}

/// A human label for an interval, for the legend.
fn label(step: usize) -> String {
    match step {
        s if s % 60 == 0 && s >= 60 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);

    let fmt = if cfg.hour12 {
        "%I:%M:%S %p"
    } else {
        "%H:%M:%S"
    };

    // Leave a margin so the grid doesn't run into the terminal edges.
    let cols = avail_w.saturating_sub(2).max(1);
    let reserved = 2 + if cfg.show_date { 2 } else { 0 };
    let rows_avail = avail_h.saturating_sub(reserved).max(1);
    let (step, total) = choose(cols, rows_avail);
    let rows = total.div_ceil(cols);

    let elapsed = now.num_seconds_from_midnight() as usize;
    let done = elapsed / step;
    let frac = elapsed as f64 / DAY_SECS as f64;

    let time = now.format(fmt).to_string();
    let legend = if avail_w >= 72 {
        format!(
            "{}   ·   1 block = {}   ·   {} / {} blocks   ·   {:.1}% of today",
            time,
            label(step),
            done,
            total,
            frac * 100.0
        )
    } else if avail_w >= 40 {
        format!("{time}   ·   {done}/{total}   ·   {:.1}%", frac * 100.0)
    } else {
        format!("{time}   ·   {:.1}%", frac * 100.0)
    };
    let mut extra = vec![
        render::blank(),
        render::line(legend, color::dim(primary, 0.8)),
    ];
    if cfg.show_date {
        extra.push(render::blank());
        extra.push(render::line(
            now.format("%A, %B %-d %Y").to_string(),
            color::dim(primary, 0.75),
        ));
    }

    let spent = color::dim(primary, 0.13);
    let mut lines: Vec<Line> = Vec::with_capacity(rows + extra.len());

    for row in 0..rows {
        let mut l: Line = Vec::new();
        for col in 0..cols {
            let i = row * cols + col;
            if i >= total {
                // Past the end of the day: pad so the block stays put.
                match l.last_mut() {
                    Some(last) if last.color == spent => last.text.push(' '),
                    _ => l.push(span(" ".to_string(), spent)),
                }
                continue;
            }

            let (ch, c) = if i < done {
                // Smooth gradient ramp across the day.
                let t = i as f64 / total.max(2) as f64;
                ('\u{2588}', color::lerp(primary, accent, t))
            } else if i == done {
                // The block currently filling.
                ('\u{2588}', accent)
            } else {
                // Unspent blocks rendered in a uniform dim color.
                ('\u{00b7}', spent)
            };
            match l.last_mut() {
                Some(last) if last.color == c => last.text.push(ch),
                _ => l.push(span(ch.to_string(), c)),
            }
        }
        lines.push(l);
    }

    lines.extend(extra);
    lines
}
