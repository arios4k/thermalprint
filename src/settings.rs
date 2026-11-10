use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Automatic,
    Manual,
    FullPage,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Scale {
    #[default]
    Fit,
    Actual,
    Custom,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}
/// Per-printer settings remembered when you switch queues.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrinterProfile {
    pub media: String,
    pub resolution: String,
    pub dpi: u32,
    pub width_mm: f32,
    pub height_mm: f32,
    pub copies: u16,
    /// Shift printed content to correct a printer that feeds slightly off-center.
    pub offset_x_mm: f32,
    pub offset_y_mm: f32,
}
impl Default for PrinterProfile {
    fn default() -> Self {
        Self {
            media: String::new(),
            resolution: String::new(),
            dpi: 203,
            width_mm: 101.6,
            height_mm: 152.4,
            copies: 1,
            offset_x_mm: 0.,
            offset_y_mm: 0.,
        }
    }
}
/// The largest alignment correction, in millimeters, in either direction.
pub const MAX_OFFSET_MM: f32 = 10.;
const MAX_PROFILES: usize = 50;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub width_mm: f32,
    pub height_mm: f32,
    pub margin_mm: f32,
    pub dpi: u32,
    pub rotation: u16,
    pub scale: Scale,
    pub scale_percent: f32,
    pub mode: Mode,
    pub copies: u16,
    pub printer: String,
    pub media: String,
    pub resolution: String,
    pub enhance: bool,
    pub theme: ThemeChoice,
    pub recent: Vec<PathBuf>,
    pub print_history: Vec<String>,
    /// Whether the navigation sidebar is collapsed to icons.
    pub navigation_collapsed: bool,
    pub offset_x_mm: f32,
    pub offset_y_mm: f32,
    pub profiles: BTreeMap<String, PrinterProfile>,
    /// A folder to watch for new label files, and whether watching is on.
    pub watch_folder: Option<PathBuf>,
    pub watch_enabled: bool,
    /// Move labels from the watch folder into a Processed subfolder once printed.
    pub move_printed: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            width_mm: 101.6,
            height_mm: 152.4,
            margin_mm: 2.0,
            dpi: 203,
            rotation: 0,
            scale: Scale::Fit,
            scale_percent: 100.0,
            mode: Mode::Automatic,
            copies: 1,
            printer: String::new(),
            media: String::new(),
            resolution: String::new(),
            enhance: false,
            theme: ThemeChoice::System,
            recent: vec![],
            print_history: vec![],
            navigation_collapsed: false,
            offset_x_mm: 0.,
            offset_y_mm: 0.,
            profiles: BTreeMap::new(),
            watch_folder: None,
            watch_enabled: false,
            move_printed: false,
        }
    }
}
impl Settings {
    fn path() -> Option<PathBuf> {
        directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
            .map(|p| p.config_dir().join("settings.json"))
    }
    pub fn load() -> Self {
        Self::path()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|v| serde_json::from_slice::<Self>(&v).ok())
            .map(|mut s| {
                // Config files are input, too. Keep allocations and CUPS arguments bounded.
                s.width_mm = finite(s.width_mm, 101.6).clamp(10., 500.);
                s.height_mm = finite(s.height_mm, 152.4).clamp(10., 500.);
                s.margin_mm = finite(s.margin_mm, 2.).clamp(0., 20.);
                s.scale_percent = finite(s.scale_percent, 100.).clamp(10., 400.);
                s.dpi = if [203, 300, 600].contains(&s.dpi) {
                    s.dpi
                } else {
                    203
                };
                s.rotation = (s.rotation / 90 % 4) * 90;
                s.copies = s.copies.clamp(1, 999);
                s.recent.truncate(8);
                s.print_history.truncate(50);
                s.offset_x_mm = finite(s.offset_x_mm, 0.).clamp(-MAX_OFFSET_MM, MAX_OFFSET_MM);
                s.offset_y_mm = finite(s.offset_y_mm, 0.).clamp(-MAX_OFFSET_MM, MAX_OFFSET_MM);
                while s.profiles.len() > MAX_PROFILES {
                    s.profiles.pop_last();
                }
                for profile in s.profiles.values_mut() {
                    profile.clean();
                }
                s
            })
            .unwrap_or_default()
    }
    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(p) = Self::path() {
            std::fs::create_dir_all(p.parent().unwrap())?;
            let mut file = tempfile::NamedTempFile::new_in(p.parent().unwrap())?;
            serde_json::to_writer_pretty(file.as_file_mut(), self)?;
            file.as_file().sync_all()?;
            file.persist(p)?;
        }
        Ok(())
    }
    /// Capture the values that belong to the selected printer.
    pub fn current_profile(&self) -> PrinterProfile {
        PrinterProfile {
            media: self.media.clone(),
            resolution: self.resolution.clone(),
            dpi: self.dpi,
            width_mm: self.width_mm,
            height_mm: self.height_mm,
            copies: self.copies,
            offset_x_mm: self.offset_x_mm,
            offset_y_mm: self.offset_y_mm,
        }
    }
    /// Remember the current values for the selected printer.
    pub fn store_profile(&mut self) {
        if !self.printer.is_empty()
            && (self.profiles.len() < MAX_PROFILES || self.profiles.contains_key(&self.printer))
        {
            self.profiles
                .insert(self.printer.clone(), self.current_profile());
        }
    }
    /// Switch to a printer, saving the old one's values and restoring the new one's.
    /// Returns whether a saved profile was applied.
    pub fn switch_printer(&mut self, name: &str) -> bool {
        self.store_profile();
        self.printer = name.to_owned();
        let Some(profile) = self.profiles.get(name).cloned() else {
            self.offset_x_mm = 0.;
            self.offset_y_mm = 0.;
            return false;
        };
        self.media = profile.media;
        self.resolution = profile.resolution;
        self.dpi = profile.dpi;
        self.width_mm = profile.width_mm;
        self.height_mm = profile.height_mm;
        self.copies = profile.copies;
        self.offset_x_mm = profile.offset_x_mm;
        self.offset_y_mm = profile.offset_y_mm;
        true
    }
    pub fn remember(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(8);
    }
}
impl PrinterProfile {
    fn clean(&mut self) {
        if ![203, 300, 600].contains(&self.dpi) {
            self.dpi = 203;
        }
        self.width_mm = finite(self.width_mm, 101.6).clamp(10., 500.);
        self.height_mm = finite(self.height_mm, 152.4).clamp(10., 500.);
        self.copies = self.copies.clamp(1, 999);
        self.offset_x_mm = finite(self.offset_x_mm, 0.).clamp(-MAX_OFFSET_MM, MAX_OFFSET_MM);
        self.offset_y_mm = finite(self.offset_y_mm, 0.).clamp(-MAX_OFFSET_MM, MAX_OFFSET_MM);
    }
}
fn finite(v: f32, default: f32) -> f32 {
    if v.is_finite() { v } else { default }
}

#[cfg(test)]
mod sidebar_tests {
    use super::*;

    #[test]
    fn sidebar_state_defaults_to_expanded_and_round_trips() {
        assert!(!Settings::default().navigation_collapsed);
        // Preferences saved before this setting existed still load.
        let old: Settings = serde_json::from_str(r#"{"copies": 2}"#).unwrap();
        assert!(!old.navigation_collapsed);
        let saved = serde_json::to_string(&Settings {
            navigation_collapsed: true,
            ..Settings::default()
        })
        .unwrap();
        assert!(
            serde_json::from_str::<Settings>(&saved)
                .unwrap()
                .navigation_collapsed
        );
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn switching_printers_restores_each_printers_own_values() {
        let mut s = Settings {
            printer: "A".into(),
            media: "w4h6".into(),
            dpi: 300,
            offset_x_mm: 1.5,
            ..Settings::default()
        };
        assert!(!s.switch_printer("B"), "unknown printers start clean");
        assert_eq!(s.offset_x_mm, 0.);
        s.media = "LB06".into();
        s.offset_y_mm = -2.;
        assert!(s.switch_printer("A"));
        assert_eq!((s.media.as_str(), s.dpi, s.offset_x_mm), ("w4h6", 300, 1.5));
        assert!(s.switch_printer("B"));
        assert_eq!((s.media.as_str(), s.offset_y_mm), ("LB06", -2.));
    }

    #[test]
    fn profiles_round_trip_and_older_files_still_load() {
        let old: Settings = serde_json::from_str(r#"{"copies": 3}"#).unwrap();
        assert!(old.profiles.is_empty() && old.offset_x_mm == 0.);
        let mut s = Settings {
            printer: "A".into(),
            offset_x_mm: 2.,
            ..Settings::default()
        };
        s.store_profile();
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back.profiles["A"].offset_x_mm, 2.);
    }

    #[test]
    fn bad_profile_values_are_clamped() {
        let mut profile = PrinterProfile {
            offset_x_mm: 999.,
            dpi: 7,
            copies: 0,
            ..Default::default()
        };
        profile.clean();
        assert_eq!(
            (profile.offset_x_mm, profile.dpi, profile.copies),
            (MAX_OFFSET_MM, 203, 1)
        );
    }
}
