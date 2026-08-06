//! User settings, stored as JSON at
//! `~/Library/Application Support/Ichi/config.json`.
//!
//! Every field has a default via `#[serde(default)]`, so a config that is
//! missing keys — or written by an older version — still loads. A config that
//! fails to parse entirely falls back to defaults rather than preventing
//! startup: a typo in a shortcut should not leave you with no window manager.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The nine grid positions, named as they appear in the config file.
/// The numbering matches the numpad layout the Windows build uses.
pub const POSITIONS: [(&str, u32); 9] = [
    ("bottom_left", 1),
    ("bottom", 2),
    ("bottom_right", 3),
    ("left", 4),
    ("center", 5),
    ("right", 6),
    ("top_left", 7),
    ("top", 8),
    ("top_right", 9),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Hotkeys {
    pub bottom_left: String,
    pub bottom: String,
    pub bottom_right: String,
    pub left: String,
    pub center: String,
    pub right: String,
    pub top_left: String,
    pub top: String,
    pub top_right: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        // The Windows bindings: Ctrl+Alt+Numpad, laid out as a 3x3 grid.
        Self {
            bottom_left: "ctrl+alt+keypad1".into(),
            bottom: "ctrl+alt+keypad2".into(),
            bottom_right: "ctrl+alt+keypad3".into(),
            left: "ctrl+alt+keypad4".into(),
            center: "ctrl+alt+keypad5".into(),
            right: "ctrl+alt+keypad6".into(),
            top_left: "ctrl+alt+keypad7".into(),
            top: "ctrl+alt+keypad8".into(),
            top_right: "ctrl+alt+keypad9".into(),
        }
    }
}

impl Hotkeys {
    /// Pair each configured shortcut with the grid position it drives.
    pub fn bindings(&self) -> Vec<(&'static str, u32, &str)> {
        POSITIONS
            .iter()
            .map(|(name, position)| {
                let shortcut = match *name {
                    "bottom_left" => &self.bottom_left,
                    "bottom" => &self.bottom,
                    "bottom_right" => &self.bottom_right,
                    "left" => &self.left,
                    "center" => &self.center,
                    "right" => &self.right,
                    "top_left" => &self.top_left,
                    "top" => &self.top,
                    _ => &self.top_right,
                };
                (*name, *position, shortcut.as_str())
            })
            .collect()
    }
}

/// Both default to zero, which keeps the Windows build's flush-tiling
/// behaviour: windows meet edge to edge and sit flush against the screen.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Gaps {
    /// Space between a snapped window and the edge of the usable screen area.
    pub screen_edge: i32,
    /// Space between two adjacent snapped windows. Each window gives up half.
    pub between_windows: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub hotkeys: Hotkeys,
    pub gaps: Gaps,
    /// Multi-tap sequence for halves and corners. Any length; taps wrap around.
    pub cycle_ratios: Vec<f64>,
    /// Multi-tap sequence for the centre position.
    pub center_ratios: Vec<f64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkeys: Hotkeys::default(),
            gaps: Gaps::default(),
            cycle_ratios: vec![0.5, 1.0 / 3.0, 2.0 / 3.0],
            center_ratios: vec![1.0, 0.8, 0.6],
        }
    }
}

impl Config {
    /// Drop ratios outside `(0.0, 1.0]`, falling back to the default sequence
    /// if that leaves nothing. A zero or negative ratio would collapse the
    /// window to nothing and look like a crash.
    fn sanitise(mut self) -> Self {
        let defaults = Config::default();

        self.cycle_ratios.retain(|r| *r > 0.0 && *r <= 1.0);
        if self.cycle_ratios.is_empty() {
            self.cycle_ratios = defaults.cycle_ratios;
        }

        self.center_ratios.retain(|r| *r > 0.0 && *r <= 1.0);
        if self.center_ratios.is_empty() {
            self.center_ratios = defaults.center_ratios;
        }

        self.gaps.screen_edge = self.gaps.screen_edge.max(0);
        self.gaps.between_windows = self.gaps.between_windows.max(0);

        self
    }
}

/// `~/Library/Application Support/Ichi/config.json`
pub fn config_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library/Application Support/Ichi")
            .join("config.json"),
    )
}

/// Load the config, writing a default one on first run.
///
/// Returns the config plus any warnings worth surfacing — a malformed file is
/// reported rather than silently discarded, because the user will otherwise
/// wonder why their edits did nothing.
pub fn load() -> (Config, Vec<String>) {
    let mut warnings = Vec::new();

    let Some(path) = config_path() else {
        warnings.push("could not determine HOME; using default settings".into());
        return (Config::default(), warnings);
    };

    if !path.exists() {
        let config = Config::default();
        if let Err(error) = save(&config) {
            warnings.push(format!("could not write default config: {error}"));
        }
        return (config, warnings);
    }

    match fs::read_to_string(&path) {
        Ok(contents) => match serde_json::from_str::<Config>(&contents) {
            Ok(config) => (config.sanitise(), warnings),
            Err(error) => {
                warnings.push(format!(
                    "{} is invalid ({error}); using default settings. \
                     Fix the file and choose Reload Config.",
                    path.display()
                ));
                (Config::default(), warnings)
            }
        },
        Err(error) => {
            warnings.push(format!("could not read {}: {error}", path.display()));
            (Config::default(), warnings)
        }
    }
}

pub fn save(config: &Config) -> std::io::Result<()> {
    let Some(path) = config_path() else {
        return Err(std::io::Error::other("no HOME directory"));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json =
        serde_json::to_string_pretty(config).map_err(|e| std::io::Error::other(e.to_string()))?;
    fs::write(path, format!("{json}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_windows_bindings() {
        let hotkeys = Hotkeys::default();
        assert_eq!(hotkeys.left, "ctrl+alt+keypad4");
        assert_eq!(hotkeys.center, "ctrl+alt+keypad5");
        assert_eq!(hotkeys.top_right, "ctrl+alt+keypad9");
    }

    #[test]
    fn bindings_cover_all_nine_positions_once() {
        let hotkeys = Hotkeys::default();
        let bindings = hotkeys.bindings();
        assert_eq!(bindings.len(), 9);

        let mut positions: Vec<u32> = bindings.iter().map(|(_, p, _)| *p).collect();
        positions.sort_unstable();
        assert_eq!(positions, (1..=9).collect::<Vec<_>>());

        // Each field must be wired to its own shortcut, not accidentally
        // aliased to a neighbour by the match in `bindings`.
        let mut shortcuts: Vec<&str> = bindings.iter().map(|(_, _, s)| *s).collect();
        shortcuts.sort_unstable();
        shortcuts.dedup();
        assert_eq!(shortcuts.len(), 9, "two positions share a shortcut");
    }

    #[test]
    fn bindings_map_each_name_to_its_own_field() {
        let hotkeys = Hotkeys {
            left: "ctrl+a".into(),
            right: "ctrl+b".into(),
            ..Hotkeys::default()
        };
        let bindings = hotkeys.bindings();
        let find = |name: &str| {
            bindings
                .iter()
                .find(|(n, _, _)| *n == name)
                .map(|(_, _, s)| *s)
                .unwrap()
        };
        assert_eq!(find("left"), "ctrl+a");
        assert_eq!(find("right"), "ctrl+b");
    }

    #[test]
    fn partial_config_fills_in_defaults() {
        let json = r#"{"gaps": {"screen_edge": 12}}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.gaps.screen_edge, 12);
        assert_eq!(config.gaps.between_windows, 0);
        // Untouched sections still get their defaults.
        assert_eq!(config.hotkeys, Hotkeys::default());
        assert_eq!(config.cycle_ratios, Config::default().cycle_ratios);
    }

    #[test]
    fn sanitise_drops_out_of_range_ratios() {
        let config = Config {
            cycle_ratios: vec![0.5, 0.0, -1.0, 2.5, 0.25],
            ..Config::default()
        }
        .sanitise();
        assert_eq!(config.cycle_ratios, vec![0.5, 0.25]);
    }

    #[test]
    fn sanitise_restores_defaults_when_every_ratio_is_invalid() {
        let config = Config {
            cycle_ratios: vec![0.0, -3.0],
            center_ratios: vec![9.9],
            ..Config::default()
        }
        .sanitise();
        assert_eq!(config.cycle_ratios, Config::default().cycle_ratios);
        assert_eq!(config.center_ratios, Config::default().center_ratios);
    }

    #[test]
    fn sanitise_clamps_negative_gaps() {
        let config = Config {
            gaps: Gaps {
                screen_edge: -20,
                between_windows: -1,
            },
            ..Config::default()
        }
        .sanitise();
        assert_eq!(config.gaps.screen_edge, 0);
        assert_eq!(config.gaps.between_windows, 0);
    }

    #[test]
    fn round_trips_through_json() {
        let original = Config {
            gaps: Gaps {
                screen_edge: 8,
                between_windows: 4,
            },
            cycle_ratios: vec![0.5, 0.25],
            ..Config::default()
        };
        let json = serde_json::to_string_pretty(&original).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }

    #[test]
    fn unknown_fields_are_rejected_rather_than_silently_ignored() {
        // A typo like "gap" instead of "gaps" should surface as a parse error
        // (which load() reports), not vanish leaving the user confused.
        let json = r#"{"gap": {"screen_edge": 12}}"#;
        assert!(serde_json::from_str::<Config>(json).is_err());
    }
}
