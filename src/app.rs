//! Terminal setup, the render loop, and live keybindings.

use crate::color;
use crate::config::{Config, Face, MAX_SCALE};
use crate::faces;
use crate::render::{self, span, Line};
use anyhow::Result;
use chrono::{DateTime, Datelike, Local, Timelike};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{Stdout, Write};
use std::time::Duration;

const HELP_ITEMS: &[&str] = &[
    "\u{2190}\u{2192} face",
    "tab picker",
    "c color",
    "t 12/24h",
    "+/- size",
    "a alarms",
    "q quit",
];
const PICKER_COLS: usize = 3;
/// Rows reserved at the bottom for the status line.
const CHROME_H: u16 = 2;

pub fn run(mut cfg: Config) -> Result<()> {
    let started_with = cfg.clone();

    terminal::enable_raw_mode()?;
    let mut out = std::io::stdout();
    execute!(out, EnterAlternateScreen, Hide, EnableMouseCapture)?;

    let result = event_loop(&mut out, &mut cfg);

    execute!(out, Show, LeaveAlternateScreen, DisableMouseCapture)?;
    terminal::disable_raw_mode()?;

    // Whatever the user switched to during the session becomes the default
    // for next time, so restarting resumes the face they were last looking
    // at. Written onto the *stored* config so one-off CLI overrides don't
    // leak into it, and only when something actually changed on screen.
    persist_session(&started_with, &cfg);

    result
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveAlarm {
    index: usize,
    candidate_date: chrono::NaiveDate,
}

fn legacy_alarm_active(cfg: &Config, now: DateTime<Local>) -> bool {
    cfg.resolve_alarm()
        .is_some_and(|alarm_time| now.format("%H:%M").to_string() == alarm_time)
}

fn active_alarm_still_valid(cfg: &Config, active: &ActiveAlarm, now: DateTime<Local>) -> bool {
    let Some(alarm) = cfg.alarms.get(active.index) else {
        return false;
    };
    if !alarm.enabled {
        return false;
    }
    let Some(alarm_time) = alarm.get_time() else {
        return false;
    };

    let scheduled_dt = active.candidate_date.and_time(alarm_time);
    let trigger_start = scheduled_dt - chrono::Duration::minutes(alarm.trigger_offset_min as i64);
    let now_naive = now.naive_local();
    now_naive >= trigger_start && now_naive < scheduled_dt + chrono::Duration::minutes(5)
}

fn check_active_alarm(
    cfg: &Config,
    now: DateTime<Local>,
    dismissed: &[(usize, chrono::NaiveDate)],
) -> Option<ActiveAlarm> {
    let now_naive = now.naive_local();
    for (i, alarm) in cfg.alarms.iter().enumerate() {
        if !alarm.enabled {
            continue;
        }
        let alarm_time = match alarm.get_time() {
            Some(t) => t,
            None => continue,
        };
        let start_date = match alarm.get_start_date() {
            Some(d) => d,
            None => continue,
        };

        for day_offset in 0..=1 {
            let candidate_date = now.date_naive() + chrono::Duration::days(day_offset);
            if candidate_date < start_date {
                continue;
            }

            if dismissed
                .iter()
                .any(|(idx, date)| *idx == i && *date == candidate_date)
            {
                continue;
            }

            let matches_recurrence = match alarm.recurrence {
                crate::config::Recurrence::Once => candidate_date == start_date,
                crate::config::Recurrence::Daily => true,
                crate::config::Recurrence::Weekly => {
                    candidate_date.weekday() == alarm.get_day_of_week()
                }
                crate::config::Recurrence::BiWeekly => {
                    let diff = (candidate_date - start_date).num_days();
                    diff >= 0 && diff % 14 == 0
                }
                crate::config::Recurrence::Weekday => {
                    let wd = candidate_date.weekday();
                    wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun
                }
                crate::config::Recurrence::Weekend => {
                    let wd = candidate_date.weekday();
                    wd == chrono::Weekday::Sat || wd == chrono::Weekday::Sun
                }
            };

            if matches_recurrence {
                let scheduled_dt = candidate_date.and_time(alarm_time);
                let trigger_start =
                    scheduled_dt - chrono::Duration::minutes(alarm.trigger_offset_min as i64);
                if now_naive >= trigger_start
                    && now_naive < scheduled_dt + chrono::Duration::minutes(5)
                {
                    return Some(ActiveAlarm {
                        index: i,
                        candidate_date,
                    });
                }
            }
        }
    }
    None
}

fn duration_until_next_trigger(cfg: &Config, now: DateTime<Local>) -> Option<Duration> {
    let mut min_dur: Option<Duration> = None;
    let now_naive = now.naive_local();

    for alarm in &cfg.alarms {
        if !alarm.enabled {
            continue;
        }
        let alarm_time = match alarm.get_time() {
            Some(t) => t,
            None => continue,
        };
        let start_date = match alarm.get_start_date() {
            Some(d) => d,
            None => continue,
        };

        for day_offset in 0..=2 {
            let candidate_date = now.date_naive() + chrono::Duration::days(day_offset);
            if candidate_date < start_date {
                continue;
            }

            let matches_recurrence = match alarm.recurrence {
                crate::config::Recurrence::Once => candidate_date == start_date,
                crate::config::Recurrence::Daily => true,
                crate::config::Recurrence::Weekly => {
                    candidate_date.weekday() == alarm.get_day_of_week()
                }
                crate::config::Recurrence::BiWeekly => {
                    let diff = (candidate_date - start_date).num_days();
                    diff >= 0 && diff % 14 == 0
                }
                crate::config::Recurrence::Weekday => {
                    let wd = candidate_date.weekday();
                    wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun
                }
                crate::config::Recurrence::Weekend => {
                    let wd = candidate_date.weekday();
                    wd == chrono::Weekday::Sat || wd == chrono::Weekday::Sun
                }
            };

            if matches_recurrence {
                let scheduled_dt = candidate_date.and_time(alarm_time);
                let trigger_start =
                    scheduled_dt - chrono::Duration::minutes(alarm.trigger_offset_min as i64);
                if trigger_start > now_naive {
                    let dur = trigger_start - now_naive;
                    if let Ok(std_dur) = dur.to_std() {
                        min_dur = Some(min_dur.map_or(std_dur, |m| m.min(std_dur)));
                    }
                }
            }
        }
    }
    min_dur
}

/// Saves the settings the user can change from the keyboard. Failures are
/// deliberately silent: a read-only config directory shouldn't take the
/// clock down after it has already exited cleanly.
fn persist_session(before: &Config, after: &Config) {
    let changed = before.face != after.face
        || before.hour12 != after.hour12
        || before.show_seconds != after.show_seconds
        || before.scale != after.scale
        || before.color != after.color;
    if !changed {
        return;
    }
    if let Ok(mut stored) = Config::load() {
        stored.face = after.face;
        stored.hour12 = after.hour12;
        stored.show_seconds = after.show_seconds;
        stored.scale = after.scale;
        stored.color = after.color.clone();
        let _ = stored.save();
    }
}

/// How long we can sleep before the on-screen display would go stale.
///
/// The clock does no polling or busy-waiting: `event::poll` parks the thread
/// (backed by kqueue/epoll/IOCP under crossterm) until either a key arrives
/// or this deadline passes, so idle CPU usage is effectively zero. We only
/// wake as often as the display can actually change: every 500ms to blink
/// a colon, every second to advance a seconds readout, or — with seconds
/// hidden — only once a minute.
fn next_wake(cfg: &Config, now: DateTime<Local>, active_alarm: bool) -> Duration {
    let animated = matches!(
        cfg.face,
        Face::Analog
            | Face::Rings
            | Face::Hourglass
            | Face::Cuckoo
            | Face::Radar
            | Face::Ship
            | Face::Warp
    );
    let blinks = cfg.blink_colon
        && matches!(
            cfg.face,
            Face::Digital | Face::Matrix | Face::Flip | Face::Lcd | Face::Grid
        );

    if cfg.face == Face::Warp {
        // High-smoothness continuous 60 FPS animation loop!
        // Sleeping for exactly 16ms between frames ensures buttery-smooth, uniform pacing.
        return Duration::from_millis(16);
    }

    if cfg.face == Face::Snake {
        // Classic arcade snake speed — fast enough to read as movement, slow
        // enough that each step is legible in a terminal grid.
        return Duration::from_millis(crate::faces::snake::TICK_MS as u64);
    }

    let period_ms: i64 = if active_alarm {
        250 // Pulse smoothly multiple times a second during the alarm!
    } else if blinks {
        500
    } else if cfg.show_seconds || animated {
        250 // Wake up 4 times a second for buttery smooth seconds!
    } else {
        60_000
    };
    let ms = now.timestamp_millis();
    let remainder = period_ms - ms.rem_euclid(period_ms);
    let mut sleep_dur = Duration::from_millis(remainder.clamp(10, period_ms) as u64);

    if let Some(until_trigger) = duration_until_next_trigger(cfg, now) {
        if until_trigger < sleep_dur {
            sleep_dur = until_trigger.max(Duration::from_millis(10));
        }
    }
    sleep_dur
}

/// Moves the picker's grid selection by (dcol, drow), clamping at the grid
/// edges and refusing to land on a trailing empty cell (the face count
/// doesn't evenly divide the column count).
fn move_selection(selected: usize, dcol: i32, drow: i32) -> usize {
    let n = Face::ALL.len();
    let rows = n.div_ceil(PICKER_COLS);
    let row = selected / PICKER_COLS;
    let col = selected % PICKER_COLS;
    let new_col = (col as i32 + dcol).clamp(0, PICKER_COLS as i32 - 1) as usize;
    let new_row = (row as i32 + drow).clamp(0, rows as i32 - 1) as usize;
    let idx = new_row * PICKER_COLS + new_col;
    if idx < n {
        idx
    } else {
        selected
    }
}

fn event_loop(out: &mut Stdout, cfg: &mut Config) -> Result<()> {
    let mut needs_clear = true;
    let mut picker: Option<usize> = None;
    let mut alarm_manager: Option<usize> = None;
    let mut selected_col: usize = 0;
    let mut active_alarm: Option<ActiveAlarm> = None;
    let mut dismissed_occurrences: Vec<(usize, chrono::NaiveDate)> = Vec::new();
    let mut dismissed_legacy_occurrence: Option<(chrono::NaiveDate, u32, u32)> = None;
    let mut last_beep_second: Option<u32> = None;

    loop {
        let now = Local::now();
        if active_alarm.is_none() {
            active_alarm = check_active_alarm(cfg, now, &dismissed_occurrences);
        } else if let Some(ref active) = active_alarm {
            if !active_alarm_still_valid(cfg, active, now) {
                active_alarm = None;
                needs_clear = true;
            }
        }

        let legacy_occurrence = (now.date_naive(), now.hour(), now.minute());
        let legacy_active =
            legacy_alarm_active(cfg, now) && dismissed_legacy_occurrence != Some(legacy_occurrence);
        let alarm_is_active = active_alarm.is_some() || legacy_active;

        let current_second = now.second();
        if alarm_is_active && current_second % 2 == 0 && last_beep_second != Some(current_second) {
            last_beep_second = Some(current_second);
            let _ = out.write_all(b"\x07");
            let _ = out.flush();
        }

        if needs_clear {
            queue!(out, Clear(ClearType::All))?;
            needs_clear = false;
        }
        if let Some(selected) = alarm_manager {
            draw_alarm_manager(out, cfg, selected, selected_col)?;
        } else {
            match picker {
                Some(selected) => draw_picker(out, cfg, selected)?,
                None => draw(out, cfg, alarm_is_active)?,
            }
        }
        let _ = out.write_all(b"\0");
        out.flush()?;

        let wait = next_wake(cfg, Local::now(), alarm_is_active).min(Duration::from_millis(100));
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if alarm_is_active {
                        if let Some(active) = active_alarm.take() {
                            dismissed_occurrences.push((active.index, active.candidate_date));
                            if cfg.alarms[active.index].recurrence
                                == crate::config::Recurrence::Once
                            {
                                cfg.alarms[active.index].enabled = false;
                                let _ = cfg.save();
                            }
                        }
                        if legacy_active {
                            dismissed_legacy_occurrence = Some(legacy_occurrence);
                        }
                        needs_clear = true;
                        continue;
                    }

                    if let Some(selected) = alarm_manager {
                        let fields = if cfg.alarms.is_empty() {
                            vec![]
                        } else {
                            active_fields(&cfg.alarms[selected])
                        };
                        let max_cols = fields.len().max(1);

                        match k.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                alarm_manager = None;
                                needs_clear = true;
                            }
                            KeyCode::Up => {
                                if !cfg.alarms.is_empty() {
                                    let new_selected = selected.saturating_sub(1);
                                    alarm_manager = Some(new_selected);
                                    let new_max_cols =
                                        active_fields(&cfg.alarms[new_selected]).len();
                                    selected_col = selected_col.min(new_max_cols - 1);
                                }
                            }
                            KeyCode::Down => {
                                if !cfg.alarms.is_empty() {
                                    let new_selected = (selected + 1).min(cfg.alarms.len() - 1);
                                    alarm_manager = Some(new_selected);
                                    let new_max_cols =
                                        active_fields(&cfg.alarms[new_selected]).len();
                                    selected_col = selected_col.min(new_max_cols - 1);
                                }
                            }
                            KeyCode::Left => {
                                if !cfg.alarms.is_empty() {
                                    selected_col = if selected_col == 0 {
                                        max_cols - 1
                                    } else {
                                        selected_col - 1
                                    };
                                }
                            }
                            KeyCode::Right => {
                                if !cfg.alarms.is_empty() {
                                    selected_col = (selected_col + 1) % max_cols;
                                }
                            }
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                if !cfg.alarms.is_empty() && selected < cfg.alarms.len() {
                                    if let Some(&field) = fields.get(selected_col) {
                                        adjust_field(&mut cfg.alarms[selected], field, 1);
                                        let _ = cfg.save();
                                    }
                                }
                            }
                            KeyCode::Char('-') => {
                                if !cfg.alarms.is_empty() && selected < cfg.alarms.len() {
                                    if let Some(&field) = fields.get(selected_col) {
                                        adjust_field(&mut cfg.alarms[selected], field, -1);
                                        let _ = cfg.save();
                                    }
                                }
                            }
                            KeyCode::Char(' ') | KeyCode::Enter => {
                                if !cfg.alarms.is_empty() && selected < cfg.alarms.len() {
                                    if let Some(&field) = fields.get(selected_col) {
                                        if field == AlarmField::Name {
                                            let _ = terminal::disable_raw_mode();
                                            let _ = execute!(out, Show);

                                            let (_, term_h) = terminal::size().unwrap_or((80, 24));
                                            let _ =
                                                execute!(out, MoveTo(0, term_h.saturating_sub(1)));
                                            print!("\r\nEnter new alarm name (or leave empty to cancel): ");
                                            let _ = std::io::stdout().flush();

                                            let mut input = String::new();
                                            if std::io::stdin().read_line(&mut input).is_ok() {
                                                let trimmed = input.trim();
                                                if !trimmed.is_empty() {
                                                    cfg.alarms[selected].name = trimmed.to_string();
                                                    let _ = cfg.save();
                                                }
                                            }

                                            let _ = terminal::enable_raw_mode();
                                            let _ = execute!(out, Hide);
                                            needs_clear = true;
                                        } else if field == AlarmField::Enabled {
                                            cfg.alarms[selected].enabled =
                                                !cfg.alarms[selected].enabled;
                                            let _ = cfg.save();
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('a') => {
                                let new_alarm = crate::config::Alarm {
                                    name: "Alarm".to_string(),
                                    time: "08:00".to_string(),
                                    recurrence: crate::config::Recurrence::Once,
                                    start_date: Local::now().format("%Y-%m-%d").to_string(),
                                    day_of_week: None,
                                    trigger_offset_min: 3,
                                    enabled: true,
                                };
                                cfg.alarms.push(new_alarm);
                                let _ = cfg.save();
                                alarm_manager = Some(cfg.alarms.len() - 1);
                                selected_col = 0;
                                needs_clear = true;
                            }
                            KeyCode::Char('d')
                            | KeyCode::Char('x')
                            | KeyCode::Backspace
                            | KeyCode::Delete => {
                                if selected < cfg.alarms.len() {
                                    cfg.alarms.remove(selected);
                                    let _ = cfg.save();
                                    if cfg.alarms.is_empty() {
                                        alarm_manager = Some(0);
                                        selected_col = 0;
                                    } else {
                                        let new_selected = selected.min(cfg.alarms.len() - 1);
                                        alarm_manager = Some(new_selected);
                                        let new_max_cols =
                                            active_fields(&cfg.alarms[new_selected]).len();
                                        selected_col = selected_col.min(new_max_cols - 1);
                                    }
                                    needs_clear = true;
                                }
                            }
                            _ => {}
                        }
                    } else if let Some(selected) = picker {
                        match k.code {
                            KeyCode::Esc | KeyCode::Char('q') => {
                                picker = None;
                                needs_clear = true;
                            }
                            KeyCode::Enter => {
                                cfg.face = Face::ALL[selected];
                                picker = None;
                                needs_clear = true;
                            }
                            KeyCode::Left => picker = Some(move_selection(selected, -1, 0)),
                            KeyCode::Right => picker = Some(move_selection(selected, 1, 0)),
                            KeyCode::Up => picker = Some(move_selection(selected, 0, -1)),
                            KeyCode::Down => picker = Some(move_selection(selected, 0, 1)),
                            _ => {}
                        }
                    } else {
                        match k.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('c') if k.modifiers.contains(KeyModifiers::CONTROL) => {
                                break
                            }
                            KeyCode::Char('a') => {
                                alarm_manager = Some(0);
                                needs_clear = true;
                            }
                            KeyCode::Left => {
                                cfg.face = cfg.face.prev();
                                needs_clear = true;
                            }
                            KeyCode::Right => {
                                cfg.face = cfg.face.next();
                                needs_clear = true;
                            }
                            KeyCode::Tab => {
                                let idx =
                                    Face::ALL.iter().position(|f| *f == cfg.face).unwrap_or(0);
                                picker = Some(idx);
                                needs_clear = true;
                            }
                            KeyCode::Char('t') => cfg.hour12 = !cfg.hour12,
                            KeyCode::Char('c') => {
                                const PRESETS: &[&str] = &[
                                    "#38d9e8", // Cyan
                                    "#10b981", // Emerald Green
                                    "#f59e0b", // Amber
                                    "#ef4444", // Red
                                    "#a855f7", // Purple
                                    "#3b82f6", // Blue
                                    "#ffffff", // White
                                ];
                                let cur_color = cfg.color.trim().to_ascii_lowercase();
                                let idx = PRESETS.iter().position(|&p| p == cur_color).unwrap_or(0);
                                let next_idx = (idx + 1) % PRESETS.len();
                                cfg.color = PRESETS[next_idx].to_string();
                                needs_clear = true;
                            }
                            KeyCode::Char('s') => {
                                cfg.show_seconds = !cfg.show_seconds;
                                needs_clear = true;
                            }
                            KeyCode::Char('0') => {
                                cfg.scale = 0;
                                needs_clear = true;
                            }
                            KeyCode::Char('+') | KeyCode::Char('=') => {
                                // Leaving auto starts from the size on screen.
                                let cur = current_scale(cfg)?;
                                cfg.scale = (cur + 1).min(MAX_SCALE);
                                needs_clear = true;
                            }
                            KeyCode::Char('-') => {
                                let cur = current_scale(cfg)?;
                                cfg.scale = cur.saturating_sub(1).max(1);
                                needs_clear = true;
                            }
                            _ => {}
                        }
                    }
                }
                Event::Mouse(mouse_event) => {
                    if let Some(selected) = picker {
                        if let event::MouseEventKind::Down(event::MouseButton::Left) =
                            mouse_event.kind
                        {
                            let (term_w, term_h) = terminal::size()?;
                            let n = Face::ALL.len();
                            let grid_rows = n.div_ceil(PICKER_COLS);
                            let gap_x: u16 = 2;
                            let gap_y: u16 = 0;
                            let label_h: u16 = 1;

                            let box_w = ((term_w.saturating_sub(4)
                                - gap_x * (PICKER_COLS as u16 - 1))
                                / PICKER_COLS as u16)
                                .clamp(16, 46);
                            let box_h = ((term_h.saturating_sub(4)) / grid_rows as u16)
                                .saturating_sub(label_h + gap_y)
                                .clamp(6, 14);

                            let total_w =
                                PICKER_COLS as u16 * box_w + (PICKER_COLS as u16 - 1) * gap_x;
                            let total_h = grid_rows as u16 * (box_h + label_h + gap_y);
                            let start_col = term_w.saturating_sub(total_w) / 2;
                            let start_row = term_h.saturating_sub(total_h + 1) / 2;

                            let cx = mouse_event.column;
                            let cy = mouse_event.row;

                            for i in 0..n {
                                let col = (i % PICKER_COLS) as u16;
                                let row = (i / PICKER_COLS) as u16;
                                let x0 = start_col + col * (box_w + gap_x);
                                let y0 = start_row + row * (box_h + label_h + gap_y);

                                if cx >= x0
                                    && cx < x0 + box_w
                                    && cy >= y0
                                    && cy < y0 + box_h + label_h
                                {
                                    if selected == i {
                                        cfg.face = Face::ALL[i];
                                        picker = None;
                                        needs_clear = true;
                                    } else {
                                        picker = Some(i);
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                Event::Resize(_, _) => needs_clear = true,
                _ => {}
            }
        }
    }
    Ok(())
}

/// The scale currently on screen, so `+`/`-` continue from what the user sees
/// rather than jumping when leaving auto-scale.
fn current_scale(cfg: &Config) -> Result<u8> {
    if !cfg.is_auto_scale() {
        return Ok(cfg.scale);
    }
    let (w, h) = terminal::size()?;
    let (text, _, suffix) = faces::digital::time_text(Local::now(), cfg);
    let mut reserved = 0;
    if !suffix.is_empty() {
        reserved += 2;
    }
    if cfg.show_date {
        reserved += 2;
    }
    let fit_len = if cfg.show_seconds {
        text.chars().count()
    } else {
        text.chars().count() + 3
    };
    let cap = crate::vector::fit_height(
        fit_len,
        w as usize,
        (h.saturating_sub(CHROME_H) as usize).saturating_sub(reserved),
        crate::config::MAX_CAP_PX,
    );
    // `scale` counts in 6px cap-height steps; round to the nearest one.
    Ok(((cap / 6.0).round() as u8).clamp(1, MAX_SCALE))
}

fn render_face(face: Face, now: DateTime<Local>, cfg: &Config, w: usize, h: usize) -> Vec<Line> {
    let mut resolved_cfg = cfg.clone();
    resolved_cfg.accent_color = cfg.resolve_accent();
    let cfg = &resolved_cfg;
    let lines = match face {
        Face::Digital => faces::digital::render(now, cfg, w, h),
        Face::Analog => faces::analog::render(now, cfg, w, h),
        Face::Binary => faces::binary::render(now, cfg, w, h),
        Face::Word => faces::word::render(now, cfg, w, h),
        Face::Matrix => faces::matrix::render(now, cfg, w, h),
        Face::Flip => faces::flip::render(now, cfg, w, h),
        Face::Waves => faces::waves::render(now, cfg, w, h),
        Face::Rings => faces::rings::render(now, cfg, w, h),
        Face::Roman => faces::roman::render(now, cfg, w, h),
        Face::Lcd => faces::lcd::render(now, cfg, w, h),
        Face::Hourglass => faces::hourglass::render(now, cfg, w, h),
        Face::Blocks => faces::blocks::render(now, cfg, w, h),
        Face::Cuckoo => faces::cuckoo::render(now, cfg, w, h),
        Face::Radar => faces::radar::render(now, cfg, w, h),
        Face::Ship => faces::ship::render(now, cfg, w, h),
        Face::Grid => faces::grid::render(now, cfg, w, h),
        Face::Warp => faces::warp::render(now, cfg, w, h),
        Face::Snake => faces::snake::render(now, cfg, w, h),
    };

    let rendered_w = lines.iter().map(render::line_width).max().unwrap_or(0);
    if lines.len() <= h && rendered_w <= w {
        lines
    } else {
        compact_face(face, now, cfg, w, h)
    }
}

/// A small but fully composed fallback for a face whose minimum geometric
/// footprint cannot fit the available canvas. This is preferable to clipping:
/// picker cards and narrow terminals retain hierarchy, identity, and time.
fn compact_face(face: Face, now: DateTime<Local>, cfg: &Config, w: usize, h: usize) -> Vec<Line> {
    if w == 0 || h == 0 {
        return Vec::new();
    }

    let primary = color::parse(&cfg.color);
    let accent = color::parse(&cfg.accent_color);
    let (full_time, _, suffix) = faces::digital::time_text(now, cfg);
    let mut clock = if suffix.is_empty() {
        full_time.clone()
    } else {
        format!("{full_time} {suffix}")
    };
    if clock.chars().count() + 4 > w {
        let short: String = full_time.chars().take(5).collect();
        clock = if suffix.is_empty() || short.chars().count() + suffix.chars().count() + 5 > w {
            short
        } else {
            format!("{short} {suffix}")
        };
    }

    if h < 3 || w < 6 {
        return vec![render::line(
            clock.chars().take(w).collect::<String>(),
            accent,
        )];
    }

    let box_w = w.clamp(6, 30);
    let inner_w = box_w - 2;
    let label = face.to_string().to_uppercase();
    let label: String = label.chars().take(inner_w.saturating_sub(2)).collect();
    let mut top_inner = format!("─ {label} ");
    top_inner.push_str(&"─".repeat(inner_w.saturating_sub(top_inner.chars().count())));

    let clock: String = clock.chars().take(inner_w).collect();
    let clock_w = clock.chars().count();
    let left = (inner_w - clock_w) / 2;
    let right = inner_w - clock_w - left;
    let border = color::dim(primary, 0.42);
    let mut lines = vec![
        vec![
            span("╭", border),
            span(top_inner, border),
            span("╮", border),
        ],
        vec![
            span("│", border),
            span(" ".repeat(left), primary),
            span(clock, accent),
            span(" ".repeat(right), primary),
            span("│", border),
        ],
        vec![
            span("╰", border),
            span("─".repeat(inner_w), border),
            span("╯", border),
        ],
    ];

    if cfg.show_date && h >= 5 {
        let date = now.format("%a, %b %-d").to_string();
        if date.chars().count() <= w {
            lines.push(render::blank());
            lines.push(render::line(date, color::dim(primary, 0.68)));
        }
    }
    lines
}

fn draw(out: &mut Stdout, cfg: &Config, is_alarm_active: bool) -> Result<()> {
    let (term_w, term_h) = terminal::size()?;
    if term_w < 20 || term_h < 6 {
        queue!(out, Clear(ClearType::All), MoveTo(0, 0))?;
        queue!(out, Print("terminal too small"))?;
        return Ok(());
    }

    let now = Local::now();
    let avail_h = term_h.saturating_sub(CHROME_H) as usize;

    let mut current_cfg = cfg.clone();
    let mut bg_color = Color::Reset;

    if is_alarm_active && now.second() % 2 == 0 {
        bg_color = Color::Red;
        current_cfg.color = "black".to_string();
        current_cfg.accent_color = "black".to_string();
    }

    let lines = render_face(
        current_cfg.face,
        now,
        &current_cfg,
        term_w as usize,
        avail_h,
    );
    draw_block(out, term_w, avail_h as u16, &lines, bg_color)?;

    // Draw the next upcoming alarm or blank space in the row between the clock area and status.
    let alarm_str = next_upcoming_alarm(cfg, now);
    for row in avail_h..term_h.saturating_sub(1) as usize {
        queue!(out, MoveTo(0, row as u16), SetBackgroundColor(bg_color))?;
        if let Some(ref text) = alarm_str {
            let text_len = text.chars().count();
            if text_len < term_w as usize {
                let pad = (term_w as usize - text_len) / 2;
                queue!(
                    out,
                    SetForegroundColor(Color::DarkGrey),
                    Print(" ".repeat(pad)),
                    Print(text),
                    Print(" ".repeat(term_w as usize - pad - text_len)),
                    ResetColor
                )?;
                continue;
            }
        }
        queue!(out, Print(" ".repeat(term_w as usize)))?;
    }

    draw_status(out, term_w, term_h, bg_color)?;
    Ok(())
}

/// Centered hint bar on the bottom row. The row is cleared first so a wider
/// previous frame can't leave characters stranded past the new text.
fn draw_status(out: &mut Stdout, term_w: u16, term_h: u16, bg: Color) -> Result<()> {
    let sep = "  \u{00b7}  ";
    let mut text = HELP_ITEMS.join(sep);
    if text.chars().count() > term_w as usize {
        // Narrow terminal: drop the separators' padding, then trailing items.
        text = HELP_ITEMS.join(" \u{00b7} ");
        while text.chars().count() > term_w as usize && text.contains('\u{00b7}') {
            let cut = text.rfind('\u{00b7}').unwrap();
            text.truncate(cut);
            text = text.trim_end().to_string();
        }
    }
    let pad = (term_w as usize).saturating_sub(text.chars().count()) / 2;
    let left_pad = " ".repeat(pad);
    let mut right_pad = " ".repeat((term_w as usize).saturating_sub(pad + text.chars().count()));

    // Active spinner character at the bottom right corner forces
    // GPU-accelerated terminals like Warp to redraw the frame instead of freezing.
    let now = Local::now();
    let spinners = ['|', '/', '-', '\\'];
    let spinner_char = spinners[(now.timestamp_subsec_millis() / 250) as usize % 4];
    if !right_pad.is_empty() {
        right_pad.pop();
        right_pad.push(spinner_char);
    }

    queue!(
        out,
        MoveTo(0, term_h.saturating_sub(1)),
        SetBackgroundColor(bg),
        SetForegroundColor(Color::DarkGrey),
        Print(format!("{left_pad}{text}{right_pad}")),
        ResetColor
    )?;
    Ok(())
}

/// Centers a block of styled lines in the given area and prints it.
///
/// Every cell of the area is written every frame, including the blank ones.
/// Painting only the glyphs would leave the previous frame behind wherever
/// the block shrank or shifted — the block is centered, so any change in its
/// width or height (a shorter word-clock phrase, a bar value going 9 -> 10,
/// the am/pm tag appearing) moves it and strands the old pixels.
fn draw_block(out: &mut Stdout, area_w: u16, area_h: u16, lines: &[Line], bg: Color) -> Result<()> {
    let aw = area_w as usize;
    let ah = area_h as usize;
    let block_w = render::block_width(lines).min(aw);
    let block_h = lines.len().min(ah);
    let start_row = (ah - block_h) / 2;
    let start_col = (aw - block_w) / 2;

    for row in 0..ah {
        queue!(out, MoveTo(0, row as u16), SetBackgroundColor(bg))?;

        let line = row
            .checked_sub(start_row)
            .filter(|i| *i < block_h)
            .map(|i| &lines[i]);

        let Some(l) = line else {
            queue!(out, Print(" ".repeat(aw)))?;
            continue;
        };

        let lw = render::line_width(l).min(block_w);
        let left = start_col + (block_w - lw) / 2;
        queue!(out, Print(" ".repeat(left)))?;

        let mut used = 0usize;
        for s in l {
            let room = aw - left - used;
            if room == 0 {
                break;
            }
            let text: String = s.text.chars().take(room).collect();
            used += text.chars().count();
            queue!(
                out,
                SetForegroundColor(s.color),
                SetBackgroundColor(bg),
                Print(&text)
            )?;
        }
        queue!(
            out,
            SetBackgroundColor(bg),
            Print(" ".repeat(aw - left - used))
        )?;
    }
    queue!(out, ResetColor)?;
    Ok(())
}

/// A small preview of `face` for the picker grid: no date clutter, and forced
/// to a size that fits inside one grid cell.
fn mini_render(face: Face, now: DateTime<Local>, cfg: &Config, w: usize, h: usize) -> Vec<Line> {
    let mut preview = cfg.clone();
    preview.scale = 0;
    preview.show_date = false;
    preview.show_seconds = false;
    preview.blink_colon = false;
    render_face(face, now, &preview, w, h)
}

fn draw_box(out: &mut Stdout, x0: u16, y0: u16, w: u16, h: u16, color: Color) -> Result<()> {
    let inner = w.saturating_sub(2) as usize;
    let mut top = String::from("\u{250c}");
    top.extend(std::iter::repeat_n('\u{2500}', inner));
    top.push('\u{2510}');
    let mut bottom = String::from("\u{2514}");
    bottom.extend(std::iter::repeat_n('\u{2500}', inner));
    bottom.push('\u{2518}');

    queue!(out, MoveTo(x0, y0), SetForegroundColor(color), Print(&top))?;
    for r in 1..h.saturating_sub(1) {
        queue!(
            out,
            MoveTo(x0, y0 + r),
            Print('\u{2502}'),
            MoveTo(x0 + w.saturating_sub(1), y0 + r),
            Print('\u{2502}')
        )?;
    }
    queue!(
        out,
        MoveTo(x0, y0 + h.saturating_sub(1)),
        Print(&bottom),
        ResetColor
    )?;
    Ok(())
}

fn draw_picker(out: &mut Stdout, cfg: &Config, selected: usize) -> Result<()> {
    let (term_w, term_h) = terminal::size()?;
    let now = Local::now();
    let accent = color::parse(&cfg.resolve_accent());

    let n = Face::ALL.len();
    let grid_rows = n.div_ceil(PICKER_COLS);
    let gap_x: u16 = 2;
    let gap_y: u16 = 0;
    let label_h: u16 = 1;

    // Size cells to the terminal so the grid fills the screen too.
    let box_w = ((term_w.saturating_sub(4) - gap_x * (PICKER_COLS as u16 - 1))
        / PICKER_COLS as u16)
        .clamp(16, 46);
    let box_h = ((term_h.saturating_sub(4)) / grid_rows as u16)
        .saturating_sub(label_h + gap_y)
        .clamp(6, 14);
    let cell_w = box_w.saturating_sub(2);
    let cell_h = box_h.saturating_sub(2);

    let total_w = PICKER_COLS as u16 * box_w + (PICKER_COLS as u16 - 1) * gap_x;
    let total_h = grid_rows as u16 * (box_h + label_h + gap_y);
    let start_col = term_w.saturating_sub(total_w) / 2;
    let start_row = term_h.saturating_sub(total_h + 1) / 2;

    for (i, face) in Face::ALL.iter().enumerate() {
        let col = (i % PICKER_COLS) as u16;
        let row = (i / PICKER_COLS) as u16;
        let x0 = start_col + col * (box_w + gap_x);
        let y0 = start_row + row * (box_h + label_h + gap_y);
        let is_selected = i == selected;
        let border = if is_selected { accent } else { Color::DarkGrey };

        draw_box(out, x0, y0, box_w, box_h, border)?;

        // Previews are live, so their size changes as the time does. Write
        // the whole cell interior every frame or the last frame shows through.
        let lines = mini_render(*face, now, cfg, cell_w as usize, cell_h as usize);
        let cw = cell_w as usize;
        let chh = cell_h as usize;
        let shown = lines.len().min(chh);
        let top_pad = (chh - shown) / 2;
        let inner_w = render::block_width(&lines).min(cw);

        for ri in 0..chh {
            queue!(out, MoveTo(x0 + 1, y0 + 1 + ri as u16), ResetColor)?;

            let line = ri
                .checked_sub(top_pad)
                .filter(|i| *i < shown)
                .map(|i| &lines[i]);

            let Some(l) = line else {
                queue!(out, Print(" ".repeat(cw)))?;
                continue;
            };

            let lw = render::line_width(l).min(inner_w);
            let left = (cw - inner_w) / 2 + (inner_w - lw) / 2;
            queue!(out, Print(" ".repeat(left)))?;

            let mut used = 0usize;
            for s in l {
                let room = cw - left - used;
                if room == 0 {
                    break;
                }
                let text: String = s.text.chars().take(room).collect();
                used += text.chars().count();
                queue!(out, SetForegroundColor(s.color), Print(&text))?;
            }
            queue!(out, ResetColor, Print(" ".repeat(cw - left - used)))?;
        }

        let label = face.to_string().to_uppercase();
        let lpad = (box_w as usize).saturating_sub(label.chars().count()) / 2;
        queue!(
            out,
            MoveTo(x0 + lpad as u16, y0 + box_h),
            SetForegroundColor(border),
            Print(&label),
            ResetColor
        )?;
    }

    let hint = "\u{2190}\u{2192}\u{2191}\u{2193} move   enter select   esc/q cancel";
    let hint_len = hint.chars().count() as u16;
    queue!(
        out,
        MoveTo(
            term_w.saturating_sub(hint_len) / 2,
            (start_row + total_h).min(term_h.saturating_sub(1))
        ),
        SetForegroundColor(Color::DarkGrey),
        Print(hint),
        ResetColor
    )?;
    Ok(())
}

pub fn next_upcoming_alarm(cfg: &Config, now: DateTime<Local>) -> Option<String> {
    let mut next_alarm: Option<(chrono::NaiveDateTime, &crate::config::Alarm)> = None;
    let now_naive = now.naive_local();

    for alarm in &cfg.alarms {
        if !alarm.enabled {
            continue;
        }
        let alarm_time = match alarm.get_time() {
            Some(t) => t,
            None => continue,
        };
        let start_date = match alarm.get_start_date() {
            Some(d) => d,
            None => continue,
        };

        for day_offset in 0..=15 {
            let candidate_date = now.date_naive() + chrono::Duration::days(day_offset);
            if candidate_date < start_date {
                continue;
            }

            let matches_recurrence = match alarm.recurrence {
                crate::config::Recurrence::Once => candidate_date == start_date,
                crate::config::Recurrence::Daily => true,
                crate::config::Recurrence::Weekly => {
                    candidate_date.weekday() == alarm.get_day_of_week()
                }
                crate::config::Recurrence::BiWeekly => {
                    let diff = (candidate_date - start_date).num_days();
                    diff >= 0 && diff % 14 == 0
                }
                crate::config::Recurrence::Weekday => {
                    let wd = candidate_date.weekday();
                    wd != chrono::Weekday::Sat && wd != chrono::Weekday::Sun
                }
                crate::config::Recurrence::Weekend => {
                    let wd = candidate_date.weekday();
                    wd == chrono::Weekday::Sat || wd == chrono::Weekday::Sun
                }
            };

            if matches_recurrence {
                let scheduled_dt = candidate_date.and_time(alarm_time);
                if scheduled_dt >= now_naive {
                    match next_alarm {
                        None => next_alarm = Some((scheduled_dt, alarm)),
                        Some((best_dt, _)) if scheduled_dt < best_dt => {
                            next_alarm = Some((scheduled_dt, alarm))
                        }
                        _ => {}
                    }
                    break;
                }
            }
        }
    }

    next_alarm.map(|(dt, alarm)| {
        let day_prefix = dt.format("%a ").to_string(); // e.g. "Mon "
        format!(
            "Alarm: {}{} ({}) [{}]",
            day_prefix, alarm.time, alarm.name, alarm.recurrence
        )
    })
}

fn has_detail_field(rec: &crate::config::Recurrence) -> bool {
    matches!(
        rec,
        crate::config::Recurrence::Once
            | crate::config::Recurrence::Weekly
            | crate::config::Recurrence::BiWeekly
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlarmField {
    Enabled,
    Hour,
    Minute,
    Recurrence,
    Detail,
    WarningOffset,
    Name,
}

pub fn active_fields(alarm: &crate::config::Alarm) -> Vec<AlarmField> {
    let mut fields = vec![
        AlarmField::Enabled,
        AlarmField::Hour,
        AlarmField::Minute,
        AlarmField::Recurrence,
    ];
    if has_detail_field(&alarm.recurrence) {
        fields.push(AlarmField::Detail);
    }
    fields.push(AlarmField::WarningOffset);
    fields.push(AlarmField::Name);
    fields
}

fn adjust_field(alarm: &mut crate::config::Alarm, field: AlarmField, delta: i32) {
    use chrono::Timelike;
    match field {
        AlarmField::Enabled => {
            alarm.enabled = !alarm.enabled;
        }
        AlarmField::Hour => {
            if let Some(time) = alarm.get_time() {
                let new_h = (time.hour() as i32 + delta).rem_euclid(24) as u32;
                alarm.time = format!("{:02}:{:02}", new_h, time.minute());
            }
        }
        AlarmField::Minute => {
            if let Some(time) = alarm.get_time() {
                let new_m = (time.minute() as i32 + delta).rem_euclid(60) as u32;
                alarm.time = format!("{:02}:{:02}", time.hour(), new_m);
            }
        }
        AlarmField::Recurrence => {
            let current = alarm.recurrence;
            let variants = [
                crate::config::Recurrence::Once,
                crate::config::Recurrence::Daily,
                crate::config::Recurrence::Weekly,
                crate::config::Recurrence::BiWeekly,
                crate::config::Recurrence::Weekday,
                crate::config::Recurrence::Weekend,
            ];
            let pos = variants.iter().position(|&v| v == current).unwrap_or(0);
            let new_pos = (pos as i32 + delta).rem_euclid(variants.len() as i32) as usize;
            alarm.recurrence = variants[new_pos];

            if alarm.recurrence == crate::config::Recurrence::Weekly && alarm.day_of_week.is_none()
            {
                alarm.day_of_week = Some("Monday".to_string());
            }
        }
        AlarmField::Detail => match alarm.recurrence {
            crate::config::Recurrence::Weekly => {
                let current_dow = alarm.get_day_of_week();
                let days = [
                    chrono::Weekday::Mon,
                    chrono::Weekday::Tue,
                    chrono::Weekday::Wed,
                    chrono::Weekday::Thu,
                    chrono::Weekday::Fri,
                    chrono::Weekday::Sat,
                    chrono::Weekday::Sun,
                ];
                let pos = days.iter().position(|&d| d == current_dow).unwrap_or(0);
                let new_pos = (pos as i32 + delta).rem_euclid(7) as usize;
                let next_dow = days[new_pos];
                let dow_str = match next_dow {
                    chrono::Weekday::Mon => "Monday",
                    chrono::Weekday::Tue => "Tuesday",
                    chrono::Weekday::Wed => "Wednesday",
                    chrono::Weekday::Thu => "Thursday",
                    chrono::Weekday::Fri => "Friday",
                    chrono::Weekday::Sat => "Saturday",
                    chrono::Weekday::Sun => "Sunday",
                };
                alarm.day_of_week = Some(dow_str.to_string());
            }
            crate::config::Recurrence::Once | crate::config::Recurrence::BiWeekly => {
                if let Some(start) = alarm.get_start_date() {
                    let new_date = start + chrono::Duration::days(delta as i64);
                    alarm.start_date = new_date.format("%Y-%m-%d").to_string();
                }
            }
            _ => {}
        },
        AlarmField::WarningOffset => {
            alarm.trigger_offset_min =
                (alarm.trigger_offset_min as i32 + delta).clamp(0, 60) as u32;
        }
        AlarmField::Name => {}
    }
}

fn draw_alarm_manager(
    out: &mut Stdout,
    cfg: &Config,
    selected_row: usize,
    selected_col: usize,
) -> Result<()> {
    let (term_w, term_h) = terminal::size()?;
    queue!(out, Clear(ClearType::All))?;

    let accent = color::parse(&cfg.resolve_accent());

    // Title
    let title = "=== ALARM MANAGER ===";
    let title_x = term_w.saturating_sub(title.chars().count() as u16) / 2;
    queue!(
        out,
        MoveTo(title_x, 2),
        SetForegroundColor(accent),
        Print(title),
        ResetColor
    )?;

    // List of alarms
    let list_start_y = 4;
    let mut current_y = list_start_y;

    if cfg.alarms.is_empty() {
        let empty_msg = "No alarms configured. Press 'a' to add one.";
        let empty_x = term_w.saturating_sub(empty_msg.chars().count() as u16) / 2;
        queue!(
            out,
            MoveTo(empty_x, current_y),
            SetForegroundColor(Color::DarkGrey),
            Print(empty_msg),
            ResetColor
        )?;
    } else {
        for (i, alarm) in cfg.alarms.iter().enumerate() {
            if current_y >= term_h.saturating_sub(6) {
                break; // Don't overflow the screen
            }

            let is_row_selected = i == selected_row;

            let status_str = if alarm.enabled { " ON " } else { "OFF" };
            let status_color = if alarm.enabled {
                Color::Green
            } else {
                Color::Red
            };

            let hh = format!("{:02}", alarm.get_time().map_or(0, |t| t.hour()));
            let mm = format!("{:02}", alarm.get_time().map_or(0, |t| t.minute()));

            let rec_str = format!("{:<10}", alarm.recurrence.to_string());

            let has_detail = has_detail_field(&alarm.recurrence);
            let detail_str = if has_detail {
                match alarm.recurrence {
                    crate::config::Recurrence::Weekly => {
                        format!("{:<12}", alarm.day_of_week.as_deref().unwrap_or("Monday"))
                    }
                    crate::config::Recurrence::Once | crate::config::Recurrence::BiWeekly => {
                        format!("{:<12}", alarm.start_date)
                    }
                    _ => "".to_string(),
                }
            } else {
                "".to_string()
            };

            let offset_str = if alarm.trigger_offset_min == 0 {
                "At Event    ".to_string()
            } else {
                format!("{:>2}m before  ", alarm.trigger_offset_min)
            };

            let mut line_len = 3 + 6 + 2 + 5 + 2 + 10 + 2 + 12 + 2 + alarm.name.chars().count();
            if has_detail {
                line_len += 12 + 2;
            }

            let start_x = term_w.saturating_sub(line_len as u16) / 2;
            queue!(out, MoveTo(start_x, current_y))?;

            // 1. Prefix (Row indicator)
            let prefix = if is_row_selected { ">  " } else { "   " };
            queue!(
                out,
                SetForegroundColor(if is_row_selected {
                    accent
                } else {
                    Color::DarkGrey
                }),
                Print(prefix),
                ResetColor
            )?;

            let active_f = active_fields(alarm);
            let current_field = active_f.get(selected_col).copied();

            // 2. Col 0: Status Toggle
            let highlight_0 = is_row_selected && current_field == Some(AlarmField::Enabled);
            let status_text = format!("[{status_str}]");
            if highlight_0 {
                queue!(
                    out,
                    SetBackgroundColor(status_color),
                    SetForegroundColor(Color::Black),
                    Print(&status_text),
                    ResetColor
                )?;
            } else {
                queue!(
                    out,
                    SetForegroundColor(status_color),
                    Print(&status_text),
                    ResetColor
                )?;
            }
            queue!(out, Print("  "))?;

            // 3. Col 1 & 2: Time (HH:MM)
            let highlight_1 = is_row_selected && current_field == Some(AlarmField::Hour);
            let highlight_2 = is_row_selected && current_field == Some(AlarmField::Minute);

            if highlight_1 {
                queue!(
                    out,
                    SetBackgroundColor(accent),
                    SetForegroundColor(Color::Black),
                    Print(&hh),
                    ResetColor
                )?;
            } else {
                queue!(
                    out,
                    SetForegroundColor(if is_row_selected {
                        Color::White
                    } else {
                        Color::Reset
                    }),
                    Print(&hh),
                    ResetColor
                )?;
            }

            queue!(
                out,
                SetForegroundColor(if is_row_selected {
                    accent
                } else {
                    Color::DarkGrey
                }),
                Print(":"),
                ResetColor
            )?;

            if highlight_2 {
                queue!(
                    out,
                    SetBackgroundColor(accent),
                    SetForegroundColor(Color::Black),
                    Print(&mm),
                    ResetColor
                )?;
            } else {
                queue!(
                    out,
                    SetForegroundColor(if is_row_selected {
                        Color::White
                    } else {
                        Color::Reset
                    }),
                    Print(&mm),
                    ResetColor
                )?;
            }
            queue!(out, Print("  "))?;

            // 4. Col 3: Recurrence
            let highlight_3 = is_row_selected && current_field == Some(AlarmField::Recurrence);
            if highlight_3 {
                queue!(
                    out,
                    SetBackgroundColor(accent),
                    SetForegroundColor(Color::Black),
                    Print(&rec_str),
                    ResetColor
                )?;
            } else {
                queue!(
                    out,
                    SetForegroundColor(if is_row_selected {
                        Color::White
                    } else {
                        Color::Reset
                    }),
                    Print(&rec_str),
                    ResetColor
                )?;
            }

            // 5. Col 4: Detail (Date or Day of Week)
            if has_detail {
                queue!(out, Print("  "))?;
                let highlight_4 = is_row_selected && current_field == Some(AlarmField::Detail);
                if highlight_4 {
                    queue!(
                        out,
                        SetBackgroundColor(accent),
                        SetForegroundColor(Color::Black),
                        Print(&detail_str),
                        ResetColor
                    )?;
                } else {
                    queue!(
                        out,
                        SetForegroundColor(if is_row_selected {
                            Color::White
                        } else {
                            Color::Reset
                        }),
                        Print(&detail_str),
                        ResetColor
                    )?;
                }
            }

            // 6. Col 4/5: Warning Offset
            queue!(out, Print("  "))?;
            let highlight_offset =
                is_row_selected && current_field == Some(AlarmField::WarningOffset);
            if highlight_offset {
                queue!(
                    out,
                    SetBackgroundColor(accent),
                    SetForegroundColor(Color::Black),
                    Print(&offset_str),
                    ResetColor
                )?;
            } else {
                queue!(
                    out,
                    SetForegroundColor(if is_row_selected {
                        Color::White
                    } else {
                        Color::Reset
                    }),
                    Print(&offset_str),
                    ResetColor
                )?;
            }

            // 7. Name
            queue!(out, Print("  "))?;
            let highlight_name = is_row_selected && current_field == Some(AlarmField::Name);
            if highlight_name {
                queue!(
                    out,
                    SetBackgroundColor(accent),
                    SetForegroundColor(Color::Black),
                    Print(&alarm.name),
                    ResetColor
                )?;
            } else {
                queue!(
                    out,
                    SetForegroundColor(if is_row_selected {
                        accent
                    } else {
                        Color::DarkGrey
                    }),
                    Print(&alarm.name),
                    ResetColor
                )?;
            }

            current_y += 1;
        }
    }

    // Help box at the bottom
    let help_box_y = term_h.saturating_sub(6);
    let separator = "-".repeat(term_w.min(50) as usize);
    let sep_x = term_w.saturating_sub(separator.chars().count() as u16) / 2;
    queue!(
        out,
        MoveTo(sep_x, help_box_y),
        SetForegroundColor(Color::DarkGrey),
        Print(&separator),
        ResetColor
    )?;

    let help_lines = &[
        "\u{2191}\u{2193} Row Selection   \u{2190}\u{2192} Move Highlighted Field",
        "+/- Adjust Focused Value   Space Toggle Status   a Add   d Delete",
        "esc/q Back to Clock",
    ];

    for (offset, line) in help_lines.iter().enumerate() {
        let x = term_w.saturating_sub(line.chars().count() as u16) / 2;
        queue!(
            out,
            MoveTo(x, help_box_y + 1 + offset as u16),
            SetForegroundColor(Color::DarkGrey),
            Print(line),
            ResetColor
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Alarm, Recurrence};
    use chrono::{NaiveDate, TimeZone};

    #[test]
    fn test_check_active_alarm_once() {
        let mut cfg = Config::default();
        // Set up a Once alarm
        let alarm = Alarm {
            name: "Once Alarm".to_string(),
            time: "08:00".to_string(),
            recurrence: Recurrence::Once,
            start_date: "2026-08-17".to_string(),
            day_of_week: None,
            trigger_offset_min: 1,
            enabled: true,
        };
        cfg.alarms.push(alarm);

        // 1. Right on the triggering time (1 min before scheduled)
        // 2026-08-17 07:59:00 Local
        let now = Local.with_ymd_and_hms(2026, 8, 17, 7, 59, 0).unwrap();
        let active = check_active_alarm(&cfg, now, &[]);
        assert!(active.is_some());
        let active = active.unwrap();
        assert_eq!(active.index, 0);
        assert_eq!(
            active.candidate_date,
            NaiveDate::from_ymd_opt(2026, 8, 17).unwrap()
        );

        // 2. Already dismissed
        let active_dismissed = check_active_alarm(
            &cfg,
            now,
            &[(0, NaiveDate::from_ymd_opt(2026, 8, 17).unwrap())],
        );
        assert!(active_dismissed.is_none());

        // 3. Before triggering window
        let now_before = Local.with_ymd_and_hms(2026, 8, 17, 7, 58, 59).unwrap();
        assert!(check_active_alarm(&cfg, now_before, &[]).is_none());

        // 4. After triggering window (5 min after scheduled time)
        let now_after = Local.with_ymd_and_hms(2026, 8, 17, 8, 5, 0).unwrap();
        assert!(check_active_alarm(&cfg, now_after, &[]).is_none());
    }

    #[test]
    fn test_active_alarm_stays_valid_for_configured_offset() {
        let mut cfg = Config::default();
        cfg.alarms.push(Alarm {
            name: "Early warning".to_string(),
            time: "08:00".to_string(),
            recurrence: Recurrence::Once,
            start_date: "2026-08-17".to_string(),
            day_of_week: None,
            trigger_offset_min: 3,
            enabled: true,
        });

        let now = Local.with_ymd_and_hms(2026, 8, 17, 7, 57, 30).unwrap();
        let active = check_active_alarm(&cfg, now, &[]).unwrap();
        assert!(active_alarm_still_valid(&cfg, &active, now));
    }

    #[test]
    fn test_legacy_alarm_enters_active_state_only_in_matching_minute() {
        let cfg = Config {
            alarm: Some("08:00".to_string()),
            ..Config::default()
        };

        let during = Local.with_ymd_and_hms(2026, 8, 17, 8, 0, 30).unwrap();
        let after = Local.with_ymd_and_hms(2026, 8, 17, 8, 1, 0).unwrap();
        assert!(legacy_alarm_active(&cfg, during));
        assert!(!legacy_alarm_active(&cfg, after));
    }

    #[test]
    fn test_check_active_alarm_daily() {
        let mut cfg = Config::default();
        let alarm = Alarm {
            name: "Daily Alarm".to_string(),
            time: "12:30".to_string(),
            recurrence: Recurrence::Daily,
            start_date: "2026-08-15".to_string(),
            day_of_week: None,
            trigger_offset_min: 1,
            enabled: true,
        };
        cfg.alarms.push(alarm);

        // 1. On start_date, triggering window: 12:29:00
        let now1 = Local.with_ymd_and_hms(2026, 8, 15, 12, 29, 0).unwrap();
        assert!(check_active_alarm(&cfg, now1, &[]).is_some());

        // 2. On a subsequent day, triggering window
        let now2 = Local.with_ymd_and_hms(2026, 8, 20, 12, 29, 30).unwrap();
        assert!(check_active_alarm(&cfg, now2, &[]).is_some());

        // 3. On a day before start_date
        let now_before = Local.with_ymd_and_hms(2026, 8, 14, 12, 29, 0).unwrap();
        assert!(check_active_alarm(&cfg, now_before, &[]).is_none());
    }

    #[test]
    fn test_check_active_alarm_weekday_weekend() {
        let mut cfg = Config::default();
        let wd_alarm = Alarm {
            name: "Weekday".to_string(),
            time: "09:00".to_string(),
            recurrence: Recurrence::Weekday,
            start_date: "2026-08-17".to_string(), // Mon
            day_of_week: None,
            trigger_offset_min: 1,
            enabled: true,
        };
        let we_alarm = Alarm {
            name: "Weekend".to_string(),
            time: "10:00".to_string(),
            recurrence: Recurrence::Weekend,
            start_date: "2026-08-17".to_string(),
            day_of_week: None,
            trigger_offset_min: 1,
            enabled: true,
        };
        cfg.alarms.push(wd_alarm);
        cfg.alarms.push(we_alarm);

        // 2026-08-17 is Monday. Weekday alarm should trigger.
        let mon = Local.with_ymd_and_hms(2026, 8, 17, 8, 59, 0).unwrap();
        let act = check_active_alarm(&cfg, mon, &[]);
        assert!(act.is_some());
        assert_eq!(act.unwrap().index, 0);

        // Monday 10:00 weekend alarm should NOT trigger
        let mon_we = Local.with_ymd_and_hms(2026, 8, 17, 9, 59, 0).unwrap();
        assert!(check_active_alarm(&cfg, mon_we, &[]).is_none());

        // 2026-08-22 is Saturday. Weekend alarm should trigger.
        let sat = Local.with_ymd_and_hms(2026, 8, 22, 9, 59, 0).unwrap();
        let act = check_active_alarm(&cfg, sat, &[]);
        assert!(act.is_some());
        assert_eq!(act.unwrap().index, 1);
    }

    #[test]
    fn test_check_active_alarm_weekly_custom_day() {
        let mut cfg = Config::default();
        let weekly_alarm = Alarm {
            name: "Weekly Mon".to_string(),
            time: "09:00".to_string(),
            recurrence: Recurrence::Weekly,
            start_date: "2026-08-17".to_string(),
            day_of_week: Some("Monday".to_string()),
            trigger_offset_min: 1,
            enabled: true,
        };
        cfg.alarms.push(weekly_alarm);

        // Monday trigger -> Should succeed
        let mon = Local.with_ymd_and_hms(2026, 8, 17, 8, 59, 0).unwrap();
        assert!(check_active_alarm(&cfg, mon, &[]).is_some());

        // Tuesday trigger -> Should fail
        let tue = Local.with_ymd_and_hms(2026, 8, 18, 8, 59, 0).unwrap();
        assert!(check_active_alarm(&cfg, tue, &[]).is_none());
    }

    #[test]
    fn test_duration_until_next_trigger() {
        let mut cfg = Config::default();
        let alarm = Alarm {
            name: "Once Alarm".to_string(),
            time: "08:00".to_string(),
            recurrence: Recurrence::Once,
            start_date: "2026-08-17".to_string(),
            day_of_week: None,
            trigger_offset_min: 1,
            enabled: true,
        };
        cfg.alarms.push(alarm);

        // 10 minutes before trigger (which is at 07:59:00)
        let now = Local.with_ymd_and_hms(2026, 8, 17, 7, 49, 0).unwrap();
        let dur = duration_until_next_trigger(&cfg, now);
        assert!(dur.is_some());
        assert_eq!(dur.unwrap(), Duration::from_secs(600));
    }

    #[test]
    fn test_next_upcoming_alarm_formatting() {
        let mut cfg = Config::default();
        let alarm = Alarm {
            name: "My Alarm".to_string(),
            time: "15:00".to_string(),
            recurrence: Recurrence::Weekly,
            start_date: "2026-08-17".to_string(),
            day_of_week: Some("Tuesday".to_string()),
            trigger_offset_min: 3,
            enabled: true,
        };
        cfg.alarms.push(alarm);

        // Monday 15:00. The next trigger should be Tuesday 15:00.
        let now = Local.with_ymd_and_hms(2026, 8, 17, 15, 0, 0).unwrap();
        let summary = next_upcoming_alarm(&cfg, now);
        assert!(summary.is_some());
        assert_eq!(summary.unwrap(), "Alarm: Tue 15:00 (My Alarm) [weekly]");
    }

    #[test]
    fn every_auto_scaled_face_fits_its_canvas() {
        let now = Local.with_ymd_and_hms(2026, 8, 17, 10, 9, 42).unwrap();
        let cfg = Config {
            scale: 0,
            show_date: true,
            show_seconds: true,
            blink_colon: false,
            ..Config::default()
        };

        let mut failures = Vec::new();
        for (width, height) in [(16, 6), (40, 12), (80, 24), (160, 48)] {
            for face in Face::ALL {
                let lines = render_face(face, now, &cfg, width, height);
                let rendered_width = lines.iter().map(render::line_width).max().unwrap_or(0);
                if lines.len() > height || rendered_width > width {
                    failures.push(format!(
                        "{face}: {rendered_width}x{} into {width}x{height}",
                        lines.len()
                    ));
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
