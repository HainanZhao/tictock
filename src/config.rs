//! Persisted user configuration: which face to draw, colors, and display options.
//!
//! Lives at `$XDG_CONFIG_HOME/tictock/config.toml` (or the platform equivalent
//! via the `dirs` crate). Every field has `#[serde(default)]` so old config
//! files keep loading after new fields are added.

use anyhow::{Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, ValueEnum)]
pub enum Face {
    #[default]
    Digital,
    Analog,
    Binary,
    Word,
    Matrix,
    Flip,
    Waves,
    Rings,
    Roman,
    Lcd,
    Hourglass,
    Blocks,
    Cuckoo,
    Radar,
    Ship,
    Grid,
    Warp,
    Snake,
}

impl<'de> Deserialize<'de> for Face {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_ascii_lowercase().as_str() {
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
            "hourglass" => Face::Hourglass,
            "blocks" => Face::Blocks,
            "cuckoo" => Face::Cuckoo,
            "radar" => Face::Radar,
            "ship" => Face::Ship,
            "grid" => Face::Grid,
            "warp" => Face::Warp,
            "snake" => Face::Snake,
            _ => Face::Digital, // Fallback to the first clock face if variant is unknown
        })
    }
}

impl Face {
    /// All faces, in the order they're cycled through and shown in the picker grid.
    pub const ALL: [Face; 18] = [
        Face::Digital,
        Face::Analog,
        Face::Binary,
        Face::Word,
        Face::Matrix,
        Face::Flip,
        Face::Waves,
        Face::Rings,
        Face::Roman,
        Face::Lcd,
        Face::Hourglass,
        Face::Blocks,
        Face::Cuckoo,
        Face::Radar,
        Face::Ship,
        Face::Grid,
        Face::Warp,
        Face::Snake,
    ];

    fn index(self) -> usize {
        Self::ALL.iter().position(|f| *f == self).unwrap_or(0)
    }

    pub fn next(self) -> Face {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    pub fn prev(self) -> Face {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl fmt::Display for Face {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Face::Digital => write!(f, "digital"),
            Face::Analog => write!(f, "analog"),
            Face::Binary => write!(f, "binary"),
            Face::Word => write!(f, "word"),
            Face::Matrix => write!(f, "matrix"),
            Face::Flip => write!(f, "flip"),
            Face::Waves => write!(f, "waves"),
            Face::Rings => write!(f, "rings"),
            Face::Roman => write!(f, "roman"),
            Face::Lcd => write!(f, "lcd"),
            Face::Hourglass => write!(f, "hourglass"),
            Face::Blocks => write!(f, "blocks"),
            Face::Cuckoo => write!(f, "cuckoo"),
            Face::Radar => write!(f, "radar"),
            Face::Ship => write!(f, "ship"),
            Face::Grid => write!(f, "grid"),
            Face::Warp => write!(f, "warp"),
            Face::Snake => write!(f, "snake"),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_trigger_offset() -> u32 {
    3
}
fn default_alarm_name() -> String {
    "Alarm".to_string()
}
fn default_second_step() -> u32 {
    1
}
/// 0 means "auto": grow the face to fill the terminal.
fn default_scale() -> u8 {
    0
}

/// Upper bound on the `scale` setting (0 stays "auto").
pub const MAX_SCALE: u8 = 9;
/// Upper bound on glyph cap height in sub-cell pixels, so an enormous
/// terminal doesn't render absurdly heavy strokes.
pub const MAX_CAP_PX: f64 = 120.0;
fn default_color() -> String {
    "#38d9e8".to_string()
}
/// By default, accent_color is "none", meaning it falls back to the primary color
/// to avoid unexpected blending/purplish tints. Users can set a custom accent
/// color to explicitly enable dual-color gradients.
fn default_accent() -> String {
    "none".to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Recurrence {
    Once,
    Daily,
    Weekly,
    BiWeekly,
    Weekday,
    Weekend,
}

impl fmt::Display for Recurrence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Recurrence::Once => write!(f, "once"),
            Recurrence::Daily => write!(f, "daily"),
            Recurrence::Weekly => write!(f, "weekly"),
            Recurrence::BiWeekly => write!(f, "bi-weekly"),
            Recurrence::Weekday => write!(f, "weekday"),
            Recurrence::Weekend => write!(f, "weekend"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Alarm {
    #[serde(default = "default_alarm_name")]
    pub name: String,
    pub time: String, // "HH:MM"
    pub recurrence: Recurrence,
    pub start_date: String, // "YYYY-MM-DD"
    #[serde(default)]
    pub day_of_week: Option<String>,
    #[serde(default = "default_trigger_offset")]
    pub trigger_offset_min: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for Alarm {
    fn default() -> Self {
        Self {
            name: "Alarm".to_string(),
            time: "08:00".to_string(),
            recurrence: Recurrence::Once,
            start_date: "".to_string(),
            day_of_week: None,
            trigger_offset_min: default_trigger_offset(),
            enabled: true,
        }
    }
}

impl Alarm {
    pub fn get_start_date(&self) -> Option<chrono::NaiveDate> {
        chrono::NaiveDate::parse_from_str(&self.start_date, "%Y-%m-%d").ok()
    }

    pub fn get_time(&self) -> Option<chrono::NaiveTime> {
        chrono::NaiveTime::parse_from_str(&self.time, "%H:%M").ok()
    }

    pub fn get_day_of_week(&self) -> chrono::Weekday {
        use chrono::Datelike;
        if let Some(ref dow_str) = self.day_of_week {
            match dow_str.to_ascii_lowercase().as_str() {
                "monday" | "mon" => chrono::Weekday::Mon,
                "tuesday" | "tue" => chrono::Weekday::Tue,
                "wednesday" | "wed" => chrono::Weekday::Wed,
                "thursday" | "thu" => chrono::Weekday::Thu,
                "friday" | "fri" => chrono::Weekday::Fri,
                "saturday" | "sat" => chrono::Weekday::Sat,
                "sunday" | "sun" => chrono::Weekday::Sun,
                _ => chrono::Weekday::Mon,
            }
        } else {
            self.get_start_date()
                .map_or(chrono::Weekday::Mon, |d| d.weekday())
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Which clock face to draw.
    pub face: Face,
    /// 12-hour clock with am/pm (digital face only) vs. 24-hour.
    #[serde(default = "default_true")]
    pub hour12: bool,
    /// Show a seconds readout / second hand.
    #[serde(default = "default_true")]
    pub show_seconds: bool,
    /// Show today's date under the clock.
    #[serde(default = "default_true")]
    pub show_date: bool,
    /// Blink the ':' separators once a second (digital face only).
    #[serde(default = "default_true")]
    pub blink_colon: bool,
    /// Draw hour tick marks around the rim (analog face only).
    #[serde(default = "default_true")]
    pub tick_marks: bool,
    /// Size multiplier for the big digits (digital face only), 1-4.
    #[serde(default = "default_scale")]
    pub scale: u8,
    /// Primary color: digit / clock-face color.
    #[serde(default = "default_color")]
    pub color: String,
    /// Accent color: blinking colon / clock hands.
    #[serde(default = "default_accent")]
    pub accent_color: String,
    /// Show the unlit segments faintly on the `lcd` face, the way a real
    /// panel does. Off by default — it reads as clutter at small sizes.
    #[serde(default = "default_false")]
    pub ghost_segments: bool,
    /// Granularity of the displayed seconds. 1 counts every second; 5 shows
    /// :00, :05, :10 and so on, which calms faces whose glyphs change width.
    #[serde(default = "default_second_step")]
    pub second_step: u32,
    /// Optional alarm time in "HH:MM" format (24-hour).
    pub alarm: Option<String>,
    /// Multiple configurable alarms.
    #[serde(default)]
    pub alarms: Vec<Alarm>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            face: Face::default(),
            hour12: true,
            show_seconds: true,
            show_date: true,
            blink_colon: true,
            tick_marks: true,
            scale: default_scale(),
            color: default_color(),
            accent_color: default_accent(),
            ghost_segments: false,
            second_step: default_second_step(),
            alarm: None,
            alarms: Vec::new(),
        }
    }
}

impl Config {
    /// Resolves the legacy alarm setting, returning a valid local time.
    pub fn resolve_alarm(&self) -> Option<String> {
        let val = self.alarm.as_ref()?;
        if val.is_empty() || val.eq_ignore_ascii_case("none") {
            return None;
        }
        chrono::NaiveTime::parse_from_str(val, "%H:%M")
            .ok()
            .map(|time| time.format("%H:%M").to_string())
    }

    /// Where the config file lives on this platform.
    pub fn path() -> Result<PathBuf> {
        let dir =
            dirs::config_dir().context("could not determine the platform config directory")?;
        Ok(dir.join("tictock").join("config.toml"))
    }

    /// Loads the config file, falling back to defaults if it doesn't exist yet.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        let path = if path.exists() {
            path
        } else {
            // Preserve existing preferences from releases that used the old
            // `clock` product name. The next save writes them to `tictock`.
            let legacy = dirs::config_dir()
                .context("could not determine the platform config directory")?
                .join("clock")
                .join("config.toml");
            if legacy.exists() {
                legacy
            } else {
                path
            }
        };
        if !path.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let cfg: Config = toml::from_str(&text)
            .with_context(|| format!("parsing {} (bad TOML?)", path.display()))?;
        Ok(cfg)
    }

    /// Writes the config to disk, creating the parent directory if needed.
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("serializing config")?;
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// The scale to actually draw at: the user's fixed `scale`, or — when
    /// `scale` is 0 ("auto") — the largest size the caller found that fits.
    pub fn resolve_scale(&self, auto_fit: usize) -> usize {
        if self.scale == 0 {
            auto_fit.max(1)
        } else {
            self.scale.min(MAX_SCALE) as usize
        }
    }

    /// Rounds a second count down to the configured step.
    pub fn step_second(&self, s: u32) -> u32 {
        let step = self.second_step.clamp(1, 60);
        s / step * step
    }

    pub fn is_auto_scale(&self) -> bool {
        self.scale == 0
    }

    /// Resolves the accent color, falling back to the primary color if set to "none" or empty.
    pub fn resolve_accent(&self) -> String {
        if self.accent_color.is_empty() || self.accent_color.eq_ignore_ascii_case("none") {
            self.color.clone()
        } else {
            self.accent_color.clone()
        }
    }

    /// Cap height in sub-cell pixels: the auto-fit value, or a fixed size
    /// derived from `scale` when the user pinned one.
    pub fn resolve_height(&self, auto_fit: f64) -> f64 {
        if self.scale == 0 {
            auto_fit
        } else {
            (self.scale.min(MAX_SCALE) as f64) * 6.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_face_deserialization_fallback() {
        // 1. Valid face variant -> Should deserialize correctly
        let valid_toml = "face = \"waves\"";
        let cfg: Config = toml::from_str(valid_toml).unwrap();
        assert_eq!(cfg.face, Face::Waves);

        // 2. Unknown/obsolete face variant -> Should fallback to Digital
        let unknown_toml = "face = \"obsolete_or_typo_face\"";
        let cfg_fallback: Config = toml::from_str(unknown_toml).unwrap();
        assert_eq!(cfg_fallback.face, Face::Digital);
    }

    #[test]
    fn resolve_alarm_rejects_invalid_clock_times() {
        let mut cfg = Config {
            alarm: Some("23:59".to_string()),
            ..Config::default()
        };
        assert_eq!(cfg.resolve_alarm().as_deref(), Some("23:59"));

        cfg.alarm = Some("99:99".to_string());
        assert_eq!(cfg.resolve_alarm(), None);
    }
}
