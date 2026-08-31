//! Seven-segment digits composed directly in terminal cells.
//!
//! A seven-segment digit is already discrete — three horizontal bars and four
//! vertical ones on a small integer grid — so there is nothing to rasterize.
//! Every cell here is placed deliberately from integer bar thicknesses and
//! lengths, which means the output is exact at any size: no partial coverage,
//! no stray single-cell spikes, no gaps that round away to nothing.
//!
//! All measurements are multiples of a unit `u`, chosen to fill the space.
//! A terminal cell is about twice as tall as it is wide, so vertical bars are
//! made twice as thick (in columns) as horizontal ones (in rows) to keep the
//! stroke weight even.

use crate::render::{span, Line};
use crossterm::style::Color;

/// Horizontal bar thickness is `u` rows; vertical bar thickness `2u` columns.
/// A digit is then `8u` columns wide and `7u` rows tall.
pub fn digit_w(u: usize) -> usize {
    8 * u
}
pub fn digit_h(u: usize) -> usize {
    7 * u
}
/// Cells are twice as tall as they are wide, so a `2u x u` dot is square.
fn colon_w(u: usize) -> usize {
    2 * u
}
fn gap(u: usize) -> usize {
    u
}

fn advance(c: char, u: usize) -> usize {
    if c == ':' {
        colon_w(u)
    } else {
        digit_w(u)
    }
}

/// Total width of `text` at unit `u`, including the gaps between glyphs.
pub fn width_of(text: &str, u: usize) -> usize {
    let n = text.chars().count();
    if n == 0 {
        return 0;
    }
    text.chars().map(|c| advance(c, u)).sum::<usize>() + gap(u) * (n - 1)
}

/// The largest unit whose rendering of `text` fits the given area.
pub fn fit_unit(
    text: &str,
    show_seconds: bool,
    avail_w: usize,
    avail_h: usize,
    max_u: usize,
) -> usize {
    let fit_text = if show_seconds || text.chars().count() > 5 {
        text.to_string()
    } else {
        format!("{text}:00")
    };
    (1..=max_u.max(1))
        .rev()
        .find(|&u| width_of(&fit_text, u) <= avail_w && digit_h(u) <= avail_h)
        .unwrap_or(1)
}

/// Which of the seven segments are lit, in the usual a..g order: a top,
/// b upper right, c lower right, d bottom, e lower left, f upper left,
/// g middle.
fn segments(c: char) -> Option<[bool; 7]> {
    Some(match c.to_ascii_uppercase() {
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
        'A' => [true, true, true, false, true, true, true],
        'P' => [true, true, false, false, true, true, true],
        'M' => [true, true, true, false, true, true, false],
        _ => return None,
    })
}

/// Paints one glyph into `grid` and `ghost_grid` (row-major, `stride` wide) at column `x0`.
fn draw_glyph(
    grid: &mut [bool],
    ghost_grid: &mut [bool],
    stride: usize,
    x0: usize,
    c: char,
    u: usize,
    ghost_segments: bool,
) {
    let (w, h) = (digit_w(u), digit_h(u));

    if c == ':' {
        // Two square dots at the thirds, sized to the bar thickness.
        let dot_w = colon_w(u).max(1);
        for (top, _) in [(2 * u, 0), (4 * u + u, 0)] {
            for r in top..(top + u).min(h) {
                for cx in 0..dot_w {
                    let col = x0 + cx;
                    if col < stride {
                        grid[r * stride + col] = true;
                    }
                }
            }
        }
        return;
    }

    let Some(s) = segments(c) else { return };
    let tv = 2 * u; // vertical bar thickness, in columns
    let th = u; // horizontal bar thickness, in rows

    // Horizontal bars span the full width; vertical bars the full half-height.
    // They meet flush, so corners are solid.
    let bars: [(bool, usize, usize, usize, usize); 7] = [
        (s[0], 0, th, 0, w),             // a
        (s[1], 0, 4 * u, w - tv, w),     // b
        (s[2], 3 * u, 7 * u, w - tv, w), // c
        (s[3], 6 * u, 7 * u, 0, w),      // d
        (s[4], 3 * u, 7 * u, 0, tv),     // e
        (s[5], 0, 4 * u, 0, tv),         // f
        (s[6], 3 * u, 4 * u, 0, w),      // g
    ];
    for (on, r0, r1, c0, c1) in bars {
        let is_ghost = !on && ghost_segments;
        if !on && !is_ghost {
            continue;
        }
        for r in r0..r1.min(h) {
            for cx in c0..c1 {
                let col = x0 + cx;
                if col < stride {
                    let idx = r * stride + col;
                    if on {
                        grid[idx] = true;
                    } else if is_ghost {
                        ghost_grid[idx] = true;
                    }
                }
            }
        }
    }

    // Chamfer the four outer corners, counting rows double since a cell is
    // twice as tall as it is wide. Only worth doing once the bars are thick
    // enough to spare the cells — at small units it eats the digit.
    let k = u / 3;
    if k == 0 {
        return;
    }
    for r in 0..h {
        for cx in 0..w {
            let (rt, rb) = (r, h - 1 - r);
            let (cl, cr) = (cx, w - 1 - cx);
            let cut = (2 * rt + cl < 2 * k)
                || (2 * rt + cr < 2 * k)
                || (2 * rb + cl < 2 * k)
                || (2 * rb + cr < 2 * k);
            if cut {
                let col = x0 + cx;
                if col < stride {
                    let idx = r * stride + col;
                    grid[idx] = false;
                    ghost_grid[idx] = false;
                }
            }
        }
    }
}

/// Renders `text` as seven-segment digits at unit `u`.
///
/// `color_at(t)` gives the color for horizontal position `t` in 0..=1 across
/// the block. Glyph indices listed in `blink_mask` are left blank.
pub fn render(
    text: &str,
    u: usize,
    blink_mask: &[bool],
    ghost_segments: bool,
    color_at: &dyn Fn(f64) -> Color,
) -> Vec<Line> {
    let u = u.max(1);
    let w = width_of(text, u);
    let h = digit_h(u);
    if w == 0 {
        return Vec::new();
    }
    let mut grid = vec![false; w * h];
    let mut ghost_grid = vec![false; w * h];

    let mut x = 0usize;
    for (i, c) in text.chars().enumerate() {
        if !blink_mask.get(i).copied().unwrap_or(false) {
            draw_glyph(&mut grid, &mut ghost_grid, w, x, c, u, ghost_segments);
        }
        x += advance(c, u) + gap(u);
    }

    let denom = (w.max(2) - 1) as f64;
    (0..h)
        .map(|r| {
            let mut line: Line = Vec::new();
            for cx in 0..w {
                let is_lit = grid[r * w + cx];
                let is_ghost = ghost_grid[r * w + cx];
                let (ch, c) = if is_lit {
                    ('\u{2588}', color_at(cx as f64 / denom))
                } else if is_ghost {
                    (
                        '\u{2588}',
                        crate::color::dim(color_at(cx as f64 / denom), 0.15),
                    )
                } else {
                    (' ', crossterm::style::Color::Reset)
                };
                match line.last_mut() {
                    Some(last) if last.color == c => last.text.push(ch),
                    _ => line.push(span(ch.to_string(), c)),
                }
            }
            line
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_digits() {
        let u = 2;
        for &ghost in &[false, true] {
            println!("=== GHOST SEGMENTS = {ghost} ===");
            for &digit in &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"] {
                println!("=== Digit {digit} ===");
                let lines = render(digit, u, &[], ghost, &|_| crossterm::style::Color::Reset);
                for (r, line) in lines.iter().enumerate() {
                    let row: String = line.iter().map(|s| &s.text).cloned().collect();
                    println!("{r:02}: |{row}|");
                }
            }
        }
    }

    #[test]
    fn test_digit_layout_regression() {
        // Use scale u = 2 so chamfering is inactive, enabling exact block checks.
        let u = 2;
        let tv = 2 * u; // 4 columns
        let w = digit_w(u); // 16 columns
        let h = digit_h(u); // 14 rows

        let get_grid = |digit_char: char| -> Vec<bool> {
            let mut grid = vec![false; w * h];
            let mut ghost_grid = vec![false; w * h];
            draw_glyph(&mut grid, &mut ghost_grid, w, 0, digit_char, u, false);
            grid
        };

        // Validate ALL digits ('0'..='9', 'A', 'P', 'M')
        for &digit_char in &[
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'P', 'M',
        ] {
            let grid = get_grid(digit_char);
            let s = segments(digit_char).unwrap();

            // 1. Every single digit must always span the full height (meaning row 0 and row h-1 have at least one active pixel)
            let has_active_top = (0..w).any(|col| grid[col]);
            assert!(has_active_top, "Digit '{digit_char}' is missing active pixels on the top row (row 0), meaning it's too short!");

            let has_active_bottom = (0..w).any(|col| grid[(h - 1) * w + col]);
            assert!(has_active_bottom, "Digit '{}' is missing active pixels on the bottom row (row {}), meaning it's too short!", digit_char, h - 1);

            // 2. If upper-right segment 'b' (s[1]) is active, columns (w - tv)..w of rows 0..4*u must be solid (no gaps)
            if s[1] {
                for r in 0..(4 * u) {
                    let is_filled = ((w - tv)..w).any(|col| grid[r * w + col]);
                    assert!(is_filled, "Digit '{digit_char}' has a gap or discontinuity in upper-right segment 'b' at row {r}");
                }
            }

            // 3. If lower-right segment 'c' (s[2]) is active, columns (w - tv)..w of rows 3*u..7*u must be solid (no gaps)
            if s[2] {
                for r in (3 * u)..(7 * u) {
                    let is_filled = ((w - tv)..w).any(|col| grid[r * w + col]);
                    assert!(is_filled, "Digit '{digit_char}' has a gap or discontinuity in lower-right segment 'c' at row {r}");
                }
            }

            // 4. If lower-left segment 'e' (s[4]) is active, columns 0..tv of rows 3*u..7*u must be solid (no gaps)
            if s[4] {
                for r in (3 * u)..(7 * u) {
                    let is_filled = (0..tv).any(|col| grid[r * w + col]);
                    assert!(
                        is_filled,
                        "Digit '{digit_char}' has a gap or discontinuity in lower-left segment 'e' at row {r}"
                    );
                }
            }

            // 5. If upper-left segment 'f' (s[5]) is active, columns 0..tv of rows 0..4*u must be solid (no gaps)
            if s[5] {
                for r in 0..(4 * u) {
                    let is_filled = (0..tv).any(|col| grid[r * w + col]);
                    assert!(
                        is_filled,
                        "Digit '{digit_char}' has a gap or discontinuity in upper-left segment 'f' at row {r}"
                    );
                }
            }
        }
    }
}
