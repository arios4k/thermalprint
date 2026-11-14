//! Fonts for label text, in three groups:
//! - fonts built into the app (`builtin:…`), which print the same on every computer;
//! - your own fonts: TrueType and OpenType files kept in a `fonts` folder next to the settings;
//! - fonts installed on this computer (`system:…`), found through fontconfig.
//!
//! A design stores only the font's name, so your own and the computer's fonts have to be
//! installed on each computer that prints the design; without them the default font is used.
use anyhow::{Context, Result, bail};
use fontdue::{Font, FontSettings};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

pub const MAX_FONTS: usize = 50;
const MAX_BYTES: u64 = 20_000_000;

pub fn fonts_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
        .map(|dirs| dirs.config_dir().join("fonts"))
}

/// Names of built-in fonts in a design start with this.
pub const BUILTIN_PREFIX: &str = "builtin:";
/// Names of fonts installed on the computer start with this.
pub const SYSTEM_PREFIX: &str = "system:";

/// A font built into the app.
pub struct BuiltIn {
    /// What a design stores, after [`BUILTIN_PREFIX`].
    pub id: &'static str,
    /// What the font picker shows.
    pub name: &'static str,
    regular: &'static [u8],
    bold: Option<&'static [u8]>,
}

/// The built-in fonts besides the default one (DejaVu Sans), chosen for labels.
pub const BUILT_IN: &[BuiltIn] = &[
    BuiltIn {
        id: "condensed",
        name: "Condensed (Noto Sans Condensed)",
        regular: include_bytes!("../assets/fonts/NotoSans-Condensed.ttf"),
        bold: Some(include_bytes!("../assets/fonts/NotoSans-CondensedBold.ttf")),
    },
    BuiltIn {
        id: "monospace",
        name: "Monospace (DejaVu Sans Mono)",
        regular: include_bytes!("../assets/fonts/DejaVuSansMono.ttf"),
        bold: Some(include_bytes!("../assets/fonts/DejaVuSansMono-Bold.ttf")),
    },
    BuiltIn {
        id: "serif",
        name: "Serif (DejaVu Serif)",
        regular: include_bytes!("../assets/fonts/DejaVuSerif.ttf"),
        bold: Some(include_bytes!("../assets/fonts/DejaVuSerif-Bold.ttf")),
    },
    BuiltIn {
        id: "headline",
        name: "Headline (Noto Sans Display ExtraCondensed Black)",
        regular: include_bytes!("../assets/fonts/NotoSansDisplay-ExtraCondensedBlack.ttf"),
        bold: None,
    },
    BuiltIn {
        id: "rounded",
        name: "Rounded (Varela Round)",
        regular: include_bytes!("../assets/fonts/VarelaRound-Regular.ttf"),
        bold: None,
    },
];

impl BuiltIn {
    /// The name a design stores for this font.
    pub fn key(&self) -> String {
        format!("{BUILTIN_PREFIX}{}", self.id)
    }
}

fn builtin(name: &str, bold: bool) -> Option<&'static Font> {
    static CACHE: OnceLock<Mutex<HashMap<(usize, bool), &'static Font>>> = OnceLock::new();
    let id = name.strip_prefix(BUILTIN_PREFIX)?;
    let index = BUILT_IN.iter().position(|font| font.id == id)?;
    let entry = &BUILT_IN[index];
    let (bytes, bold) = match (bold, entry.bold) {
        (true, Some(bytes)) => (bytes, true),
        _ => (entry.regular, false),
    };
    let cache = CACHE.get_or_init(Default::default);
    if let Some(found) = cache.lock().ok()?.get(&(index, bold)) {
        return Some(*found);
    }
    let font: &'static Font = Box::leak(Box::new(
        Font::from_bytes(bytes, FontSettings::default()).ok()?,
    ));
    cache.lock().ok()?.insert((index, bold), font);
    Some(font)
}

/// A font installed on this computer.
#[derive(Clone, Debug, PartialEq)]
pub struct SystemFont {
    /// What the picker shows and, after [`SYSTEM_PREFIX`], what a design stores:
    /// the family, plus the style unless it is the regular one.
    pub name: String,
    pub family: String,
    pub style: String,
    pub path: PathBuf,
}

fn is_regular(style: &str) -> bool {
    matches!(
        style.to_ascii_lowercase().as_str(),
        "" | "regular" | "book" | "normal" | "roman" | "medium"
    )
}

/// Turn fontconfig's `file<TAB>family<TAB>style` lines into a sorted list with one entry per
/// name. A variable font file is listed once per named style, but only its default style is
/// drawn, so each file is offered once, under its family name.
pub fn parse_fc_list(output: &str) -> Vec<SystemFont> {
    let mut by_file: HashMap<PathBuf, SystemFont> = HashMap::new();
    let mut seen_files: HashMap<PathBuf, usize> = HashMap::new();
    // Whether every style listed for a file is slanted: an italic variable font.
    let mut slanted: HashMap<PathBuf, bool> = HashMap::new();
    for line in output.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(file), Some(family), Some(style)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let path = PathBuf::from(file);
        let (family, style) = (family.trim(), style.trim());
        if family.is_empty() || !is_font_file(&path) {
            continue;
        }
        *seen_files.entry(path.clone()).or_default() += 1;
        // A variable font's own entry has no style name, so it says nothing either way.
        if !style.is_empty() {
            let lower = style.to_ascii_lowercase();
            let italic = lower.contains("italic") || lower.contains("oblique");
            slanted
                .entry(path.clone())
                .and_modify(|all| *all &= italic)
                .or_insert(italic);
        }
        let font = SystemFont {
            name: if is_regular(style) {
                family.to_string()
            } else {
                format!("{family} {style}")
            },
            family: family.to_string(),
            style: style.to_string(),
            path: path.clone(),
        };
        by_file
            .entry(path)
            .and_modify(|kept| {
                if is_regular(&font.style) && !is_regular(&kept.style) {
                    *kept = font.clone();
                }
            })
            .or_insert(font);
    }
    let mut fonts: Vec<SystemFont> = by_file
        .into_iter()
        .map(|(path, mut font)| {
            if seen_files.get(&path).copied().unwrap_or(0) > 1 {
                let stem = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
                    .unwrap_or_default();
                let italic = slanted.get(&path).copied().unwrap_or(false)
                    || stem.contains("italic")
                    || stem.contains("oblique");
                font.style = if italic { "Italic" } else { "Regular" }.into();
                font.name = if italic {
                    format!("{} Italic", font.family)
                } else {
                    font.family.clone()
                };
            }
            font
        })
        .filter(|font| font.name.chars().count() <= 72)
        .collect();
    fonts.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    fonts.dedup_by(|a, b| a.name == b.name);
    fonts
}

/// When fontconfig is missing, look through the usual font folders and name fonts after
/// their files.
fn scan_font_folders() -> Vec<SystemFont> {
    let mut roots = vec![
        PathBuf::from("/usr/share/fonts"),
        PathBuf::from("/usr/local/share/fonts"),
    ];
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        roots.push(home.join(".local/share/fonts"));
        roots.push(home.join(".fonts"));
    }
    let mut lines = String::new();
    let mut stack = roots;
    let mut visited = 0;
    while let Some(dir) = stack.pop() {
        visited += 1;
        if visited > 5000 {
            break;
        }
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(path);
            } else if kind.is_file() && is_font_file(&path) {
                let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned());
                if let Some(stem) = stem {
                    lines.push_str(&format!("{}\t{stem}\tRegular\n", path.display()));
                }
            }
        }
    }
    parse_fc_list(&lines)
}

/// The fonts installed on this computer, found once and kept for the life of the program.
pub fn system_fonts() -> &'static [SystemFont] {
    static FONTS: OnceLock<Vec<SystemFont>> = OnceLock::new();
    FONTS.get_or_init(|| {
        std::process::Command::new("fc-list")
            .args(["--format", "%{file}\t%{family[0]}\t%{style[0]}\n"])
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| parse_fc_list(&String::from_utf8_lossy(&output.stdout)))
            .filter(|fonts| !fonts.is_empty())
            .unwrap_or_else(scan_font_folders)
    })
}

fn load_path(path: &Path) -> Option<&'static Font> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, &'static Font>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(found) = cache.lock().ok()?.get(path) {
        return Some(*found);
    }
    let metadata = std::fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let font: &'static Font = Box::leak(Box::new(
        Font::from_bytes(bytes, FontSettings::default()).ok()?,
    ));
    cache.lock().ok()?.insert(path.to_path_buf(), font);
    Some(font)
}

/// Find a font of the computer by the name a design stores. With `bold`, the same family's
/// Bold style is used when the computer has it.
pub fn system_in<'a>(fonts: &'a [SystemFont], name: &str, bold: bool) -> Option<&'a SystemFont> {
    let name = name.strip_prefix(SYSTEM_PREFIX)?;
    let chosen = fonts.iter().find(|font| font.name == name)?;
    if bold && is_regular(&chosen.style) {
        let bold_name = format!("{} Bold", chosen.family);
        if let Some(bold) = fonts.iter().find(|font| font.name == bold_name) {
            return Some(bold);
        }
    }
    Some(chosen)
}

/// Draw `text` in the font a design names, as a grey mask `(width, height, coverage)` for the
/// font chooser, about `px` pixels tall and at most `max_width` wide. A computer font is read
/// for this and let go afterwards, so browsing many fonts does not keep them all in memory.
pub fn sample(name: &str, text: &str, px: f32, max_width: u32) -> Option<(usize, usize, Vec<u8>)> {
    let draw = |font: &Font| draw_sample(font, text, px, max_width);
    if name.is_empty() {
        draw(crate::design_render::default_font(false))
    } else if name.starts_with(SYSTEM_PREFIX) {
        let path = &system_in(system_fonts(), name, false)?.path;
        let metadata = std::fs::metadata(path).ok()?;
        if !metadata.is_file() || metadata.len() > MAX_BYTES {
            return None;
        }
        let font = Font::from_bytes(std::fs::read(path).ok()?, FontSettings::default()).ok()?;
        draw(&font)
    } else {
        draw(lookup(name, false)?)
    }
}

fn draw_sample(
    font: &Font,
    text: &str,
    px: f32,
    max_width: u32,
) -> Option<(usize, usize, Vec<u8>)> {
    let line = font.horizontal_line_metrics(px)?;
    let height = ((line.ascent - line.descent).ceil() as usize + 2).clamp(1, 400);
    let mut glyphs = vec![];
    let mut x = 0_f32;
    for c in text.chars() {
        let (metrics, bitmap) = font.rasterize(c, px);
        glyphs.push((x, metrics, bitmap));
        x += metrics.advance_width;
        if x >= max_width as f32 {
            break;
        }
    }
    let width = (x.ceil() as usize).clamp(1, max_width.max(1) as usize);
    let mut mask = vec![0_u8; width * height];
    for (origin, metrics, bitmap) in glyphs {
        let left = (origin + metrics.xmin as f32).round() as i64;
        let top = (line.ascent - metrics.height as f32 - metrics.ymin as f32).round() as i64 + 1;
        for gy in 0..metrics.height {
            for gx in 0..metrics.width {
                let (x, y) = (left + gx as i64, top + gy as i64);
                if x < 0 || y < 0 || x >= width as i64 || y >= height as i64 {
                    continue;
                }
                let cell = &mut mask[y as usize * width + x as usize];
                *cell = (*cell).max(bitmap[gy * metrics.width + gx]);
            }
        }
    }
    Some((width, height, mask))
}

/// Whether the Bold switch changes this font: the default font and the built-in fonts with a
/// bold face, and the computer's fonts whose family has a Bold style.
pub fn has_bold(name: &str) -> bool {
    if name.is_empty() {
        return true;
    }
    if let Some(id) = name.strip_prefix(BUILTIN_PREFIX) {
        return BUILT_IN
            .iter()
            .any(|font| font.id == id && font.bold.is_some());
    }
    if name.starts_with(SYSTEM_PREFIX) {
        let fonts = system_fonts();
        return match (system_in(fonts, name, false), system_in(fonts, name, true)) {
            (Some(regular), Some(bold)) => regular.path != bold.path,
            _ => false,
        };
    }
    false
}

/// What the font picker shows for the name a design stores.
pub fn display_name(name: &str) -> String {
    if let Some(id) = name.strip_prefix(BUILTIN_PREFIX) {
        return BUILT_IN
            .iter()
            .find(|font| font.id == id)
            .map_or(id, |font| font.name)
            .to_string();
    }
    name.strip_prefix(SYSTEM_PREFIX).unwrap_or(name).to_string()
}

/// A font name that is safe to use as a file name: no folders, no dots at the start.
pub fn is_valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 80
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
}

fn is_font_file(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("ttf") || e.eq_ignore_ascii_case("otf"))
}

pub fn list_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            (metadata.is_file() && is_font_file(&path) && metadata.len() <= MAX_BYTES)
                .then(|| path.file_stem().map(|s| s.to_string_lossy().into_owned()))
                .flatten()
        })
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.truncate(MAX_FONTS);
    names
}

fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    if !is_valid_name(name) {
        return None;
    }
    ["ttf", "otf", "TTF", "OTF"]
        .iter()
        .map(|ext| dir.join(format!("{name}.{ext}")))
        .find(|path| {
            std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file() && m.len() <= MAX_BYTES)
        })
}

/// Load a font by name. Fonts are kept for the life of the program, so a design with the same
/// font on many labels reads the file once.
pub fn load_in(dir: &Path, name: &str) -> Option<&'static Font> {
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, String), &'static Font>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (dir.to_path_buf(), name.to_string());
    if let Some(found) = cache.lock().ok()?.get(&key) {
        return Some(*found);
    }
    let loaded = find_file(dir, name)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| Font::from_bytes(bytes, FontSettings::default()).ok())
        .map(|font| &*Box::leak(Box::new(font)));
    // A missing font is not remembered, so installing it later takes effect at once.
    if let Some(font) = loaded {
        cache.lock().ok()?.insert(key, font);
    }
    loaded
}

/// The font a design names: built in, the computer's, or your own. `bold` picks a bold face
/// where the font has one. None means the default font is used.
pub fn lookup(name: &str, bold: bool) -> Option<&'static Font> {
    if name.is_empty() {
        None
    } else if name.starts_with(BUILTIN_PREFIX) {
        builtin(name, bold)
    } else if name.starts_with(SYSTEM_PREFIX) {
        load_path(&system_in(system_fonts(), name, bold)?.path)
    } else {
        load_in(&fonts_dir()?, name)
    }
}

/// Copy a font file into the fonts folder after checking that it can be read.
pub fn install_in(dir: &Path, source: &Path) -> Result<String> {
    if !is_font_file(source) {
        bail!("Choose a .ttf or .otf font file");
    }
    let metadata = std::fs::metadata(source).context("The font file could not be read")?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        bail!("The font file is larger than 20 MB");
    }
    let bytes = std::fs::read(source).context("The font file could not be read")?;
    Font::from_bytes(bytes.as_slice(), FontSettings::default())
        .map_err(|e| anyhow::anyhow!("This is not a usable font: {e}"))?;
    let name = source
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !is_valid_name(&name) || name.starts_with(BUILTIN_PREFIX) || name.starts_with(SYSTEM_PREFIX)
    {
        bail!("The font file name is not usable");
    }
    std::fs::create_dir_all(dir).context("Could not create the fonts folder")?;
    let target = dir.join(format!(
        "{name}.{}",
        source
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default()
    ));
    if !target.exists() && list_in(dir).len() >= MAX_FONTS {
        bail!("You already have {MAX_FONTS} fonts. Remove one first");
    }
    std::fs::write(&target, bytes).context("Could not store the font")?;
    Ok(name)
}

pub fn install(source: &Path) -> Result<String> {
    install_in(
        &fonts_dir().context("No settings folder is available")?,
        source,
    )
}

pub fn list() -> Vec<String> {
    fonts_dir().map(|dir| list_in(&dir)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLED: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/fonts/DejaVuSans-Bold.ttf"
    );

    #[test]
    fn an_installed_font_is_listed_and_loads() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list_in(dir.path()).is_empty());
        assert!(load_in(dir.path(), "DejaVuSans-Bold").is_none());
        let name = install_in(dir.path(), Path::new(BUNDLED)).unwrap();
        assert_eq!(name, "DejaVuSans-Bold");
        assert_eq!(list_in(dir.path()), ["DejaVuSans-Bold"]);
        let font = load_in(dir.path(), &name).expect("loads");
        assert!(font.lookup_glyph_index('A') != 0);
        assert!(
            std::ptr::eq(font, load_in(dir.path(), &name).unwrap()),
            "cached"
        );
    }

    #[test]
    fn unusable_fonts_and_names_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("Fake.ttf");
        std::fs::write(&fake, b"not a font").unwrap();
        assert!(install_in(dir.path(), &fake).is_err());
        assert!(install_in(dir.path(), Path::new("/etc/hostname")).is_err());
        assert!(
            list_in(dir.path()).len() == 1 && list_in(dir.path())[0] == "Fake",
            "the file in the folder is only listed"
        );
        // Names cannot reach outside the folder.
        for bad in ["", "../x", "a/b", ".hidden", "a\\\\b"] {
            assert!(!is_valid_name(bad), "{bad:?}");
            assert!(load_in(dir.path(), bad).is_none());
        }
        assert!(
            load_in(dir.path(), "Fake").is_none(),
            "a damaged file does not load"
        );
    }
}

#[cfg(test)]
mod font_group_tests {
    use super::*;

    #[test]
    fn every_built_in_font_loads_with_its_bold_face_when_it_has_one() {
        for font in BUILT_IN {
            let regular = lookup(&font.key(), false).expect("regular loads");
            assert!(regular.lookup_glyph_index('A') != 0, "{}", font.name);
            let bold = lookup(&font.key(), true).unwrap();
            assert_eq!(
                std::ptr::eq(regular, bold),
                font.bold.is_none(),
                "{}",
                font.name
            );
            assert_eq!(has_bold(&font.key()), font.bold.is_some());
            assert_eq!(display_name(&font.key()), font.name);
        }
        assert!(lookup("builtin:nope", false).is_none());
        assert!(lookup("", false).is_none() && has_bold(""));
    }

    #[test]
    fn computer_fonts_are_listed_once_with_readable_names() {
        let fc = "/f/Inter-Regular.ttf\tInter\tRegular\n\
                  /f/Inter-Bold.ttf\tInter\tBold\n\
                  /f/Ubuntu[wdth,wght].ttf\tUbuntu\tLight\n\
                  /f/Ubuntu[wdth,wght].ttf\tUbuntu\tBold\n\
                  /f/Ubuntu[wdth,wght].ttf\tUbuntu\tRegular\n\
                  /f/Ubuntu-Italic[wdth,wght].ttf\tUbuntu\t\n\
                  /f/Ubuntu[wdth,wght].ttf\tUbuntu\t\n\
                  /f/Ubuntu-Italic[wdth,wght].ttf\tUbuntu\tLight Italic\n\
                  /f/Ubuntu-Italic[wdth,wght].ttf\tUbuntu\tBold Italic\n\
                  /f/Collection.ttc\tCollected\tRegular\n\
                  /f/bitmap.pcf.gz\tFixed\tRegular\n\
                  /f/Copy/Inter-Regular.ttf\tInter\tRegular\n\
                  broken line\n";
        let fonts = parse_fc_list(fc);
        let names: Vec<&str> = fonts.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["Inter", "Inter Bold", "Ubuntu", "Ubuntu Italic"]);
        // A variable font is drawn in its default style, so it is offered once.
        assert_eq!(fonts[2].path, PathBuf::from("/f/Ubuntu[wdth,wght].ttf"));
        let inter = system_in(&fonts, "system:Inter", false).unwrap();
        let bold = system_in(&fonts, "system:Inter", true).unwrap();
        assert_eq!(
            (inter.style.as_str(), bold.style.as_str()),
            ("Regular", "Bold")
        );
        // Without a Bold style, bold falls back to the font itself.
        assert_eq!(
            system_in(&fonts, "system:Ubuntu", true).unwrap().name,
            "Ubuntu"
        );
        assert!(system_in(&fonts, "system:Missing", false).is_none());
        assert!(system_in(&fonts, "Inter", false).is_none());
        assert_eq!(display_name("system:Inter Bold"), "Inter Bold");
        assert_eq!(display_name("MyFont"), "MyFont");
    }

    #[test]
    fn your_own_fonts_cannot_take_a_reserved_name() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("builtin:serif.ttf");
        std::fs::copy(
            concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/DejaVuSerif.ttf"),
            &source,
        )
        .unwrap();
        let target = dir.path().join("fonts");
        assert!(install_in(&target, &source).is_err());
    }
}

#[cfg(test)]
mod sample_tests {
    use super::*;

    #[test]
    fn font_samples_draw_the_text_in_that_font_within_the_width() {
        let (w, h, mask) = sample("", "Default", 24., 400).expect("default font draws");
        assert_eq!(mask.len(), w * h);
        assert!(w > 20 && w <= 400 && h > 10);
        assert!(mask.iter().any(|&a| a > 200), "some ink");
        let condensed = sample("builtin:condensed", "Default", 24., 400).unwrap();
        assert!(condensed.0 < w, "condensed text is narrower");
        let (narrow, _, _) = sample("", "A very long font name indeed", 24., 60).unwrap();
        assert!(narrow <= 60);
        assert!(sample("system:No Such Font 123", "x", 24., 100).is_none());
        assert!(sample("builtin:nope", "x", 24., 100).is_none());
    }
}
