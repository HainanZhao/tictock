mod app;
mod braille;
mod color;
mod config;
mod faces;
mod render;
mod seg7;
mod vector;

use anyhow::{bail, Result};
use clap::{Args, Parser, Subcommand};
use config::{Config, Face};

/// A beautiful, configurable clock for your terminal.
#[derive(Parser)]
#[command(name = "clock", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    overrides: Overrides,
}

#[derive(Args)]
struct Overrides {
    /// Clock face to draw for this run (doesn't change the saved config).
    #[arg(long)]
    face: Option<Face>,
    /// Show a 12-hour clock with am/pm.
    #[arg(long, conflicts_with = "hour24")]
    hour12: bool,
    /// Show a 24-hour clock.
    #[arg(long)]
    hour24: bool,
    /// Show seconds.
    #[arg(long, conflicts_with = "no_seconds")]
    seconds: bool,
    /// Hide seconds.
    #[arg(long)]
    no_seconds: bool,
    /// Show today's date.
    #[arg(long, conflicts_with = "no_date")]
    date: bool,
    /// Hide today's date.
    #[arg(long)]
    no_date: bool,
    /// Primary color (digits / clock face). See `clock config colors`.
    #[arg(long)]
    color: Option<String>,
    /// Accent color (blinking colon / clock hands).
    #[arg(long)]
    accent_color: Option<String>,
    /// Clock size: 0 auto-fills the terminal, 1-9 pins a size.
    #[arg(long)]
    scale: Option<u8>,
    /// Optional alarm time in "HH:MM" format (24-hour).
    #[arg(long)]
    alarm: Option<String>,
}

impl Overrides {
    fn apply(&self, mut cfg: Config) -> Config {
        if let Some(alarm) = &self.alarm {
            cfg.alarm = Some(alarm.clone());
        }
        if let Some(face) = self.face {
            cfg.face = face;
        }
        if self.hour12 {
            cfg.hour12 = true;
        }
        if self.hour24 {
            cfg.hour12 = false;
        }
        if self.seconds {
            cfg.show_seconds = true;
        }
        if self.no_seconds {
            cfg.show_seconds = false;
        }
        if self.date {
            cfg.show_date = true;
        }
        if self.no_date {
            cfg.show_date = false;
        }
        if let Some(c) = &self.color {
            cfg.color = c.clone();
        }
        if let Some(c) = &self.accent_color {
            cfg.accent_color = c.clone();
        }
        if let Some(s) = self.scale {
            cfg.scale = s;
        }
        cfg
    }
}

#[derive(Subcommand)]
enum Command {
    /// Show the clock (also the default when no subcommand is given).
    Run,
    /// Manage the saved configuration.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Manage alarms.
    Alarm {
        #[command(subcommand)]
        action: AlarmAction,
    },
}

#[derive(Subcommand)]
enum AlarmAction {
    /// List all alarms.
    List,
    /// Add a new alarm.
    Add {
        /// Alarm time in "HH:MM" format (24-hour).
        #[arg(long)]
        time: String,
        /// Optional name or label for the alarm.
        #[arg(long, default_value = "Alarm")]
        name: String,
        /// Recurrence: once, daily, weekly, bi-weekly, weekday, weekend.
        #[arg(long, value_enum, default_value = "once")]
        recurrence: config::Recurrence,
        /// Optional start date in "YYYY-MM-DD" format (defaults to today).
        #[arg(long)]
        start_date: Option<String>,
    },
    /// Remove an alarm by its index.
    Remove { index: usize },
    /// Clear all alarms.
    Clear,
    /// Enable an alarm by its index.
    Enable { index: usize },
    /// Disable an alarm by its index.
    Disable { index: usize },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print the config file path.
    Path,
    /// Print the current config (as TOML).
    Show,
    /// Set a config value and save it, e.g. `clock config set face analog`.
    Set { key: String, value: String },
    /// Reset the config file to defaults.
    Reset,
    /// List the built-in color names.
    Colors,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let base = Config::load()?;

    match cli.command {
        None | Some(Command::Run) => {
            let cfg = cli.overrides.apply(base);
            app::run(cfg)
        }
        Some(Command::Config { action }) => run_config(action, base),
        Some(Command::Alarm { action }) => run_alarm(action, base),
    }
}

fn run_alarm(action: AlarmAction, mut cfg: Config) -> Result<()> {
    match action {
        AlarmAction::List => {
            if cfg.alarms.is_empty() {
                println!("No alarms configured.");
            } else {
                println!(
                    "{:<5} {:<15} {:<10} {:<12} {:<12} {:<8}",
                    "ID", "Name", "Time", "Recurrence", "Start Date", "Status"
                );
                println!("{}", "-".repeat(65));
                for (i, alarm) in cfg.alarms.iter().enumerate() {
                    let status = if alarm.enabled { "Enabled" } else { "Disabled" };
                    println!(
                        "{:<5} {:<15} {:<10} {:<12} {:<12} {:<8}",
                        i,
                        alarm.name,
                        alarm.time,
                        alarm.recurrence.to_string(),
                        alarm.start_date,
                        status
                    );
                }
            }
        }
        AlarmAction::Add {
            time,
            name,
            recurrence,
            start_date,
        } => {
            if time.len() != 5 || !time.contains(':') {
                bail!("Time must be in \"HH:MM\" 24-hour format.");
            }
            let parts: Vec<&str> = time.split(':').collect();
            if parts.len() != 2 {
                bail!("Time must be in \"HH:MM\" format.");
            }
            let h: u32 = parts[0]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid hour"))?;
            let m: u32 = parts[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid minute"))?;
            if h >= 24 || m >= 60 {
                bail!("Hour must be 0-23 and minute 0-59.");
            }

            let s_date = match start_date {
                Some(d) => {
                    if chrono::NaiveDate::parse_from_str(&d, "%Y-%m-%d").is_err() {
                        bail!("Start date must be in \"YYYY-MM-DD\" format.");
                    }
                    d
                }
                None => chrono::Local::now().format("%Y-%m-%d").to_string(),
            };

            let alarm = config::Alarm {
                name,
                time,
                recurrence,
                start_date: s_date,
                day_of_week: None,
                trigger_offset_min: 3,
                enabled: true,
            };

            cfg.alarms.push(alarm);
            cfg.save()?;
            println!("Alarm added successfully.");
        }
        AlarmAction::Remove { index } => {
            if index >= cfg.alarms.len() {
                bail!("Invalid alarm index: {index}.");
            }
            cfg.alarms.remove(index);
            cfg.save()?;
            println!("Alarm removed.");
        }
        AlarmAction::Clear => {
            cfg.alarms.clear();
            cfg.save()?;
            println!("All alarms cleared.");
        }
        AlarmAction::Enable { index } => {
            if index >= cfg.alarms.len() {
                bail!("Invalid alarm index: {index}.");
            }
            cfg.alarms[index].enabled = true;
            cfg.save()?;
            println!("Alarm {index} enabled.");
        }
        AlarmAction::Disable { index } => {
            if index >= cfg.alarms.len() {
                bail!("Invalid alarm index: {index}.");
            }
            cfg.alarms[index].enabled = false;
            cfg.save()?;
            println!("Alarm {index} disabled.");
        }
    }
    Ok(())
}

fn run_config(action: ConfigAction, mut cfg: Config) -> Result<()> {
    match action {
        ConfigAction::Path => {
            println!("{}", Config::path()?.display());
        }
        ConfigAction::Show => {
            print!("{}", toml::to_string_pretty(&cfg)?);
        }
        ConfigAction::Reset => {
            Config::default().save()?;
            println!("reset {} to defaults", Config::path()?.display());
        }
        ConfigAction::Colors => {
            for name in color::NAMES {
                println!("{name}");
            }
        }
        ConfigAction::Set { key, value } => {
            set_field(&mut cfg, &key, &value)?;
            cfg.save()?;
            println!("saved {} to {}", key, Config::path()?.display());
        }
    }
    Ok(())
}

fn set_field(cfg: &mut Config, key: &str, value: &str) -> Result<()> {
    fn parse_bool(v: &str) -> Result<bool> {
        match v.to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Ok(true),
            "false" | "off" | "no" | "0" => Ok(false),
            other => bail!("expected true/false, got '{other}'"),
        }
    }

    match key {
        "face" => {
            cfg.face = match value.to_ascii_lowercase().as_str() {
                "digital" => Face::Digital,
                "analog" => Face::Analog,
                "binary" => Face::Binary,
                "word" => Face::Word,
                "matrix" => Face::Matrix,
                "flip" => Face::Flip,
                "waves" => Face::Waves,
                "rings" => Face::Rings,
                "roman" => Face::Roman,
                "lcd" => Face::Lcd,
                "blocks" => Face::Blocks,
                "hourglass" => Face::Hourglass,
                "cuckoo" => Face::Cuckoo,
                "radar" => Face::Radar,
                "ship" => Face::Ship,
                "grid" => Face::Grid,
                "warp" => Face::Warp,
                "snake" => Face::Snake,
                other => bail!(
                    "unknown face '{other}' (expected one of: digital, analog, binary, word, \
                     matrix, flip, waves, rings, roman, lcd, hourglass, blocks, cuckoo, radar, ship, grid, warp, snake)"
                ),
            }
        }
        "hour12" => cfg.hour12 = parse_bool(value)?,
        "show_seconds" => cfg.show_seconds = parse_bool(value)?,
        "show_date" => cfg.show_date = parse_bool(value)?,
        "blink_colon" => cfg.blink_colon = parse_bool(value)?,
        "tick_marks" => cfg.tick_marks = parse_bool(value)?,
        "ghost_segments" => cfg.ghost_segments = parse_bool(value)?,
        "alarm" => {
            if value.eq_ignore_ascii_case("none") || value.is_empty() {
                cfg.alarm = None;
            } else if chrono::NaiveTime::parse_from_str(value, "%H:%M").is_ok() {
                cfg.alarm = Some(value.to_string());
            } else {
                bail!("alarm must be in \"HH:MM\" 24-hour format (or \"none\" to disable)");
            }
        }
        "second_step" => {
            cfg.second_step = value
                .parse::<u32>()
                .map_err(|_| anyhow::anyhow!("second_step must be a number 1-60"))?
                .clamp(1, 60)
        }
        "scale" => {
            cfg.scale = value
                .parse::<u8>()
                .map_err(|_| anyhow::anyhow!("scale must be 0 (auto) or 1-{}", config::MAX_SCALE))?
                .min(config::MAX_SCALE)
        }
        "color" => cfg.color = value.to_string(),
        "accent_color" => cfg.accent_color = value.to_string(),
        other => bail!(
            "unknown key '{other}' (expected one of: face, hour12, show_seconds, show_date, \
             blink_colon, tick_marks, ghost_segments, alarm, second_step, scale, color, \
             accent_color)"
        ),
    }
    Ok(())
}
