//! Your own fonts: TrueType and OpenType files kept in a `fonts` folder next to the settings.
//! A design stores only the font's name, so the font file has to be installed on each computer
//! that prints the design; without it the built-in font is used.
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
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, String), Option<&'static Font>>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (dir.to_path_buf(), name.to_string());
    if let Some(Some(found)) = cache.lock().ok()?.get(&key) {
        return Some(found);
    }
    let loaded = find_file(dir, name)
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| Font::from_bytes(bytes, FontSettings::default()).ok())
        .map(|font| &*Box::leak(Box::new(font)));
    // A missing font is not remembered, so installing it later takes effect at once.
    if loaded.is_some() {
        cache.lock().ok()?.insert(key, loaded);
    }
    loaded
}

pub fn lookup(name: &str) -> Option<&'static Font> {
    if name.is_empty() {
        return None;
    }
    load_in(&fonts_dir()?, name)
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
    if !is_valid_name(&name) {
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
