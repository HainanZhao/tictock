//! Snake clock face: a composed, self-playing arcade board with square cells,
//! a restrained HUD, deterministic movement, and a clear visual hierarchy.
//!
//! The whole game is a deterministic function of the wall clock rather than
//! stored state: the current 15-minute window seeds the run, and every frame
//! replays it from tick zero up to "now". That keeps this face a pure
//! function like every other one (no persisted state, resize-safe, and a
//! restarted process resumes the same run other instances would show).

use crate::color;
use crate::config::Config;
use crate::render::{self, span, Line};
use chrono::{DateTime, Local, TimeZone};
use std::collections::{HashSet, VecDeque};

type Point = (i32, i32);

/// Milliseconds per snake step — classic arcade pace.
pub const TICK_MS: i64 = 130;
/// The snake (and the RNG driving it) fully restarts every 15 minutes, even
/// if it never grows long enough or corners itself first.
const WINDOW_MS: i64 = 15 * 60 * 1000;

/// A tiny deterministic PRNG (SplitMix64) — good enough for food placement,
/// and lets the whole run be replayed byte-for-byte from a seed.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Starts a fresh run: a compact length-5 snake in the middle of the board, heading
/// right, with one food cell placed somewhere it isn't.
fn spawn(cols: usize, rows: usize, rng: &mut Rng) -> (VecDeque<Point>, HashSet<Point>, Point) {
    let cx = (cols / 2) as i32;
    let cy = (rows / 2) as i32;
    let body: VecDeque<Point> = (0..5).map(|i| (cx - i, cy)).collect();
    let occupied: HashSet<Point> = body.iter().copied().collect();
    let food = spawn_food(cols, rows, &occupied, rng);
    (body, occupied, food)
}

/// Picks a random empty cell for the next food. Falls back to a full scan if
/// the board is nearly packed, so it always terminates.
fn spawn_food(cols: usize, rows: usize, occupied: &HashSet<Point>, rng: &mut Rng) -> Point {
    for _ in 0..200 {
        let p = (rng.below(cols) as i32, rng.below(rows) as i32);
        if !occupied.contains(&p) {
            return p;
        }
    }
    (0..rows as i32)
        .flat_map(|y| (0..cols as i32).map(move |x| (x, y)))
        .find(|p| !occupied.contains(p))
        .unwrap_or((0, 0))
}

fn current_direction(body: &VecDeque<Point>) -> Point {
    let head = body[0];
    let neck = body.get(1).copied().unwrap_or((head.0 - 1, head.1));
    (head.0 - neck.0, head.1 - neck.1)
}

fn manhattan(a: Point, b: Point) -> i32 {
    (a.0 - b.0).abs() + (a.1 - b.1).abs()
}

fn is_safe(point: Point, cols: usize, rows: usize, occupied: &HashSet<Point>) -> bool {
    point.0 >= 0
        && point.1 >= 0
        && point.0 < cols as i32
        && point.1 < rows as i32
        && !occupied.contains(&point)
}

fn clear_run(
    head: Point,
    direction: Point,
    cols: usize,
    rows: usize,
    occupied: &HashSet<Point>,
) -> usize {
    (1..)
        .map(|distance| {
            (
                head.0 + direction.0 * distance,
                head.1 + direction.1 * distance,
            )
        })
        .take_while(|point| is_safe(*point, cols, rows, occupied))
        .count()
}

/// Chooses a calm, human-like move. Once the snake commits to an axis that
/// approaches the food, it stays on that axis until aligned or obstructed.
/// This produces an L-shaped route in open space instead of alternating axes
/// every tick. Detours also favor continuing straight and long clear runs.
fn choose_direction(
    cols: usize,
    rows: usize,
    body: &VecDeque<Point>,
    occupied: &HashSet<Point>,
    food: Point,
) -> Option<Point> {
    const DIRECTIONS: [Point; 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let head = body[0];
    let current = current_direction(body);
    let distance = manhattan(head, food);
    let safe: Vec<Point> = DIRECTIONS
        .into_iter()
        .filter(|direction| {
            is_safe(
                (head.0 + direction.0, head.1 + direction.1),
                cols,
                rows,
                occupied,
            )
        })
        .collect();

    // Holding the current direction is the key to natural motion: finish
    // this straight segment before making the one turn needed to align.
    if safe.contains(&current) {
        let next = (head.0 + current.0, head.1 + current.1);
        if manhattan(next, food) < distance {
            return Some(current);
        }
    }

    // If a turn is required, choose a direction that approaches the target.
    // Prefer the longer remaining axis, producing two deliberate segments
    // rather than many short alternating ones.
    if let Some(direction) = safe
        .iter()
        .copied()
        .filter(|direction| {
            let next = (head.0 + direction.0, head.1 + direction.1);
            manhattan(next, food) < distance
        })
        .max_by_key(|direction| {
            let axis_distance = if direction.0 == 0 {
                (food.1 - head.1).abs()
            } else {
                (food.0 - head.0).abs()
            };
            (
                axis_distance,
                clear_run(head, *direction, cols, rows, occupied),
            )
        })
    {
        return Some(direction);
    }

    // A body segment may temporarily block every direct route. Continue the
    // present line if possible; otherwise make the turn with the longest
    // runway so the avoidance maneuver is broad rather than twitchy.
    if safe.contains(&current) {
        return Some(current);
    }
    safe.into_iter()
        .max_by_key(|direction| clear_run(head, *direction, cols, rows, occupied))
}

/// Advances the game by one step using the turn-aware steering policy.
/// Cornering itself — or growing past `max_len` — restarts the run in place.
fn step(
    cols: usize,
    rows: usize,
    body: &mut VecDeque<Point>,
    occupied: &mut HashSet<Point>,
    food: &mut Point,
    rng: &mut Rng,
    max_len: usize,
) {
    let head = *body.front().expect("snake body is never empty");
    let Some(direction) = choose_direction(cols, rows, body, occupied, *food) else {
        let (b, o, f) = spawn(cols, rows, rng);
        *body = b;
        *occupied = o;
        *food = f;
        return;
    };

    let next = (head.0 + direction.0, head.1 + direction.1);
    body.push_front(next);
    occupied.insert(next);

    if next == *food {
        if body.len() >= max_len {
            let (b, o, f) = spawn(cols, rows, rng);
            *body = b;
            *occupied = o;
            *food = f;
        } else {
            *food = spawn_food(cols, rows, occupied, rng);
        }
    } else if let Some(tail) = body.pop_back() {
        occupied.remove(&tail);
    }
}

/// Replays the current 15-minute window from tick zero up to "now", so the
/// board is always a pure function of the wall clock and the terminal size.
fn simulate(now: DateTime<Local>, cols: usize, rows: usize) -> (VecDeque<Point>, Point) {
    let now_ms = now.timestamp_millis();
    let anchor = now_ms.div_euclid(WINDOW_MS) * WINDOW_MS;
    let target_tick = (now_ms - anchor) / TICK_MS;

    let mut rng = Rng::new(anchor as u64 ^ 0x2545_F491_4F6C_DD1D);
    let max_len = (cols * rows / 3).clamp(8, 60).min(cols * rows - 4);
    let (mut body, mut occupied, mut food) = spawn(cols, rows, &mut rng);

    for _ in 0..target_tick {
        step(
            cols,
            rows,
            &mut body,
            &mut occupied,
            &mut food,
            &mut rng,
            max_len,
        );
    }
    (body, food)
}

pub fn render(now: DateTime<Local>, cfg: &Config, avail_w: usize, avail_h: usize) -> Vec<Line> {
    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);
    let border = color::dim(primary, 0.38);
    let quiet = color::dim(primary, 0.14);

    let (time_text, _, suffix) = crate::faces::digital::time_text(now, cfg);
    let clock = if suffix.is_empty() {
        time_text
    } else {
        format!("{time_text} {suffix}")
    };

    let mut extra: Vec<Line> = Vec::new();
    if cfg.show_date && avail_h >= 12 {
        extra.push(render::blank());
        extra.push(render::line(
            now.format("%A, %B %-d %Y").to_string(),
            color::dim(primary, 0.75),
        ));
    }

    // Two terminal columns by one row produces a physically square game
    // cell on common terminal fonts. Cap the board width so a very wide
    // terminal still reads as a deliberate object rather than an empty field.
    let cols = avail_w.saturating_sub(2) / 2;
    let cols = cols.min(48);
    let rows = avail_h.saturating_sub(extra.len() + 6).min(24);
    if cols < 8 || rows < 4 {
        return vec![render::line(clock, primary)];
    }

    let (body, food) = simulate(now, cols, rows);
    let head = body[0];
    let board_w = cols * 2 + 2;
    let mut lines: Vec<Line> = Vec::with_capacity(rows + 6 + extra.len());

    // A balanced HUD: product name, time, and a compact live metric. Each
    // region is pinned to the board width so nothing jumps as values change.
    let left = "SNAKE";
    let right = format!("LENGTH {:02}", body.len());
    let inner_w = board_w.saturating_sub(2);
    let used = left.chars().count() + clock.chars().count() + right.chars().count();
    if used + 4 <= inner_w {
        let free = inner_w - used;
        let gap_left = free / 2;
        let gap_right = free - gap_left;
        lines.push(vec![
            span(left, color::dim(primary, 0.72)),
            span(" ".repeat(gap_left), primary),
            span(&clock, accent),
            span(" ".repeat(gap_right), primary),
            span(right, color::dim(primary, 0.72)),
        ]);
    } else {
        lines.push(render::line(clock.clone(), accent));
    }
    lines.push(render::blank());

    lines.push(vec![
        span("╭", border),
        span("─".repeat(board_w - 2), border),
        span("╮", border),
    ]);

    for y in 0..rows {
        let mut line: Line = vec![span("│", border)];
        for x in 0..cols {
            let p = (x as i32, y as i32);
            let (tile, tile_color) = if p == head {
                let neck = body.get(1).copied().unwrap_or(head);
                let marker = match (head.0 - neck.0, head.1 - neck.1) {
                    (1, 0) => "▶ ",
                    (-1, 0) => "◀ ",
                    (0, -1) => "▲ ",
                    _ => "▼ ",
                };
                (marker.to_string(), accent)
            } else if let Some(index) = body.iter().position(|cell| *cell == p) {
                let fade = 1.0 - index as f64 / body.len().max(2) as f64 * 0.58;
                ("██".to_string(), color::dim(primary, fade))
            } else if p == food {
                (
                    "● ".to_string(),
                    color::lerp(accent, crossterm::style::Color::White, 0.28),
                )
            } else if (x + y * 3) % 11 == 0 {
                ("· ".to_string(), quiet)
            } else {
                ("  ".to_string(), quiet)
            };
            match line.last_mut() {
                Some(last) if last.color == tile_color => last.text.push_str(&tile),
                _ => line.push(span(tile, tile_color)),
            }
        }
        line.push(span("│", border));
        lines.push(line);
    }

    lines.push(vec![
        span("╰", border),
        span("─".repeat(board_w - 2), border),
        span("╯", border),
    ]);

    let anchor_ms = now.timestamp_millis().div_euclid(WINDOW_MS) * WINDOW_MS;
    let next_reset = Local
        .timestamp_millis_opt(anchor_ms + WINDOW_MS)
        .single()
        .map(|time| time.format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string());
    lines.push(render::blank());
    lines.push(render::line(
        format!("AUTOPILOT  ·  NEXT RUN {next_reset}"),
        color::dim(primary, 0.58),
    ));

    lines.extend(extra);
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn polished_board_uses_square_cells_and_fits() {
        let now = Local.with_ymd_and_hms(2026, 8, 17, 10, 9, 42).unwrap();
        let cfg = Config {
            show_date: true,
            ..Config::default()
        };
        let lines = render(now, &cfg, 80, 24);
        assert!(lines.len() <= 24);
        assert!(lines.iter().all(|line| render::line_width(line) <= 80));
        assert!(lines.iter().any(|line| {
            line.iter().any(|span| {
                span.text.contains('▶')
                    || span.text.contains('◀')
                    || span.text.contains('▲')
                    || span.text.contains('▼')
            })
        }));
        assert!(lines
            .iter()
            .any(|line| line.iter().any(|span| span.text.contains('●'))));
    }

    #[test]
    fn open_route_uses_one_deliberate_turn_instead_of_zigzagging() {
        let mut body: VecDeque<Point> = [(5, 5), (4, 5), (3, 5), (2, 5), (1, 5)].into();
        let mut occupied: HashSet<Point> = body.iter().copied().collect();
        let target = (12, 8);
        let mut food = target;
        let mut rng = Rng::new(7);
        let mut directions = Vec::new();

        for _ in 0..20 {
            let before = body[0];
            step(20, 12, &mut body, &mut occupied, &mut food, &mut rng, 60);
            let after = body[0];
            directions.push((after.0 - before.0, after.1 - before.1));
            if after == target {
                break;
            }
        }

        assert_eq!(body[0], target);
        assert_eq!(directions.len(), manhattan((5, 5), target) as usize);
        let turns = directions
            .windows(2)
            .filter(|pair| pair[0] != pair[1])
            .count();
        assert_eq!(turns, 1, "route was not a clean L shape: {directions:?}");
    }

    #[test]
    fn steering_holds_its_current_axis_until_aligned() {
        let body: VecDeque<Point> = [(5, 5), (4, 5), (3, 5), (2, 5), (1, 5)].into();
        let occupied: HashSet<Point> = body.iter().copied().collect();

        assert_eq!(
            choose_direction(20, 12, &body, &occupied, (12, 8)),
            Some((1, 0))
        );
    }
}
