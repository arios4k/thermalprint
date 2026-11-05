use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    pub fn remember(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(8);
    }
}
fn finite(v: f32, default: f32) -> f32 {
    if v.is_finite() { v } else { default }
}
