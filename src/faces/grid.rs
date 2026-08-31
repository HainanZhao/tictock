//! Grid clock face: a retro 3x5 block matrix clock drawn with standard-width
//! solid square blocks (■). Highly legible and perfectly centered.

use crate::color;
use crate::config::Config;
use crate::faces::digital::time_text;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local};

fn digit_grid(c: char) -> [[bool; 3]; 5] {
    match c {
        '0' => [
            [true, true, true],
            [true, false, true],
            [true, false, true],
            [true, false, true],
            [true, true, true],
        ],
        '1' => [
            [false, false, true],
            [false, false, true],
            [false, false, true],
            [false, false, true],
            [false, false, true],
        ],
        '2' => [
            [true, true, true],
            [false, false, true],
            [true, true, true],
            [true, false, false],
            [true, true, true],
        ],
        '3' => [
            [true, true, true],
            [false, false, true],
            [true, true, true],
            [false, false, true],
            [true, true, true],
        ],
        '4' => [
            [true, false, true],
            [true, false, true],
            [true, true, true],
            [false, false, true],
            [false, false, true],
        ],
        '5' => [
            [true, true, true],
            [true, false, false],
            [true, true, true],
            [false, false, true],
            [true, true, true],
        ],
        '6' => [
            [true, true, true],
            [true, false, false],
            [true, true, true],
            [true, false, true],
            [true, true, true],
        ],
        '7' => [
            [true, true, true],
            [false, false, true],
            [false, false, true],
            [false, false, true],
            [false, false, true],
        ],
        '8' => [
            [true, true, true],
            [true, false, true],
            [true, true, true],
            [true, false, true],
            [true, true, true],
        ],
        '9' => [
            [true, true, true],
            [true, false, true],
            [true, true, true],
            [false, false, true],
            [true, true, true],
        ],
        ':' => [
            [false, false, false],
            [false, true, false],
            [false, false, false],
            [false, true, false],
            [false, false, false],
        ],
        _ => [[false; 3]; 5],
    }
}

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);

    let (text, colons, suffix) = time_text(now, cfg);
    let n = text.chars().count();
    let fit_n = if cfg.show_seconds { n } else { n + 3 };

    let mut extra: Vec<Line> = Vec::new();
    if !suffix.is_empty() {
        extra.push(render::blank());
        extra.push(render::line(suffix, primary));
    }
    if cfg.show_date {
        extra.push(render::blank());
        extra.push(render::line(
            now.format("%A, %B %-d %Y").to_string(),
            color::dim(primary, 0.75),
        ));
    }
    let usable_h = avail_h.saturating_sub(extra.len());

    // Fit unit scaling: each block is u*2 columns wide and u rows high (proportional square)
    let fit = (1..=6)
        .rev()
        .find(|&u| {
            let total_w = (14 * fit_n - 4) * u;
            let total_h = 9 * u;
            total_w <= avail_w && total_h <= usable_h
        })
        .unwrap_or(1);

    let u = if cfg.is_auto_scale() {
        fit
    } else {
        (cfg.scale as usize).clamp(1, 6)
    };

    let block_w = u * 2;
    let block_h = u;
    let inner_gap = u * 2;
    let char_gap = u * 4;
    let v_gap = u;

    let char_h = 5 * block_h + 4 * v_gap;
    let mut lines: Vec<Line> = Vec::with_capacity(char_h);

    for r_idx in 0..char_h {
        let mut l: Line = Vec::new();
        let block_row = r_idx / (block_h + v_gap);
        let intra_row = r_idx % (block_h + v_gap);
        let is_gap_row = intra_row >= block_h;

        for (i, c) in text.chars().enumerate() {
            if i > 0 {
                l.push(span(" ".repeat(char_gap), primary));
            }

            // Blink colons if configured
            let is_blinked =
                colons.contains(&i) && cfg.blink_colon && now.timestamp_millis() / 500 % 2 == 0;
            let grid = if is_blinked {
                [[false; 3]; 5]
            } else {
                digit_grid(c)
            };

            for col in 0..3 {
                if col > 0 {
                    l.push(span(" ".repeat(inner_gap), primary));
                }

                let active = !is_gap_row && grid[block_row][col];
                let c_color = primary;

                if active {
                    l.push(span("\u{2588}".repeat(block_w), c_color)); // solid full block █
                } else {
                    l.push(span(" ".repeat(block_w), c_color));
                }
            }
        }
        lines.push(l);
    }

    lines.extend(extra);
    lines
}
