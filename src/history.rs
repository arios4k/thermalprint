//! A short list of recent prints so a label can be reopened and printed again.
//! Reopening never prints on its own: you review the label first.
use crate::design::LabelDocument;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_ENTRIES: usize = 50;
/// Unsaved designs are kept as a snapshot only when small enough for the history file.
const MAX_SNAPSHOT_BYTES: usize = 256_000;
const MAX_TEXT: usize = 200;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A PDF or image label, reopened in Import & Convert.
    #[default]
    Import,
    /// A label design, reopened in the Label Designer.
    Design,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub time: String,
    pub printer: String,
    pub title: String,
    pub copies: u16,
    pub kind: Kind,
    /// The imported file, or the saved design file.
    pub source: Option<PathBuf>,
    pub page: usize,
    /// The design as printed, for designs that were never saved.
    pub design: Option<LabelDocument>,
    pub settings: Option<crate::settings::Settings>,
    pub crop: Option<crate::detection::Crop>,
    pub fingerprint: Option<String>,
    pub raw_zpl: bool,
}

impl Entry {
    pub fn with_raw_zpl(mut self, raw: bool) -> Self {
        self.raw_zpl = raw;
        self
    }
    pub fn with_settings(
        mut self,
        settings: &crate::settings::Settings,
        crop: Option<crate::detection::Crop>,
    ) -> Self {
        let mut saved = settings.clone();
        saved.recent.clear();
        saved.saved_queue.clear();
        saved.profiles.clear();
        saved.stock_profiles.clear();
        saved.print_history.clear();
        self.settings = Some(saved);
        self.crop = crop;
        self.fingerprint = self
            .source
            .as_ref()
            .and_then(|p| crate::duplicates::fingerprint(p, self.page).ok());
        self
    }
}
pub fn now() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

pub fn import_entry(path: &Path, page: usize, printer: &str, copies: u16) -> Entry {
    Entry {
        time: now(),
        printer: printer.into(),
        title: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Imported label".into()),
        copies,
        kind: Kind::Import,
        source: Some(path.to_owned()),
        page,
        design: None,
        ..Default::default()
    }
}

pub fn design_entry(doc: &LabelDocument, path: Option<&Path>, printer: &str, copies: u16) -> Entry {
    let snapshot = serde_json::to_vec(doc)
        .is_ok_and(|bytes| bytes.len() <= MAX_SNAPSHOT_BYTES)
        .then(|| doc.clone());
    Entry {
        time: now(),
        printer: printer.into(),
        title: doc.title.clone(),
        copies,
        kind: Kind::Design,
        source: path.map(Path::to_owned),
        page: 0,
        design: snapshot,
        ..Default::default()
    }
}

pub fn history_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
        .map(|dirs| dirs.config_dir().join("history.json"))
}

fn clean(mut entries: Vec<Entry>) -> Vec<Entry> {
    entries.retain(|e| e.design.as_ref().is_none_or(|d| d.validate().is_ok()));
    for entry in &mut entries {
        entry.title.truncate(MAX_TEXT);
        entry.printer.truncate(MAX_TEXT);
        entry.time.truncate(32);
        entry.copies = entry.copies.clamp(1, 999);
    }
    entries.truncate(MAX_ENTRIES);
    entries
}

pub fn load_from(path: &Path) -> Vec<Entry> {
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<Entry>>(&bytes).ok())
        .map(clean)
        .unwrap_or_default()
}

pub fn load() -> Vec<Entry> {
    history_path().map(|p| load_from(&p)).unwrap_or_default()
}

/// Add an entry at the top, replacing an older entry for the same label.
pub fn record_to(path: &Path, entry: Entry) -> Result<()> {
    let mut entries = load_from(path);
    entries.retain(|e| {
        !(e.kind == entry.kind
            && e.source == entry.source
            && e.page == entry.page
            && e.title == entry.title)
    });
    entries.insert(0, entry);
    let entries = clean(entries);
    save_to(path, &entries)
}
fn save_to(path: &Path, entries: &[Entry]) -> Result<()> {
    let parent = path.parent().context("History path has no folder")?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer(file.as_file_mut(), entries)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

/// Best effort: a history write problem must never fail a print that already succeeded.
pub fn record(entry: Entry) {
    if let Some(fingerprint) = &entry.fingerprint
        && let Err(error) = crate::duplicates::submitted(fingerprint)
    {
        tracing::warn!(%error,"Cannot remember duplicate protection after printing");
    }
    if let Some(path) = history_path()
        && let Err(error) = record_to(&path, entry)
    {
        tracing::warn!(%error, "Could not save print history");
    }
}

/// Keep reprints working when a watched file moves to or from Processed.
pub fn relocate(old: &Path, new: &Path) -> Result<()> {
    let path = history_path().context("No history folder")?;
    relocate_in(&path, old, new)
}
fn relocate_in(path: &Path, old: &Path, new: &Path) -> Result<()> {
    let mut entries = load_from(path);
    let mut changed = false;
    for entry in &mut entries {
        if entry.source.as_deref() == Some(old) {
            entry.source = Some(new.to_owned());
            changed = true;
        }
    }
    if changed {
        save_to(path, &entries)?;
    }
    Ok(())
}
pub fn clear() {
    if let Some(path) = history_path() {
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str) -> Entry {
        import_entry(Path::new(&format!("/tmp/{title}.pdf")), 0, "ORGSTA", 1)
    }

    #[test]
    fn newest_first_and_the_same_label_is_not_listed_twice() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        for title in ["a", "b", "a"] {
            record_to(&path, entry(title)).unwrap();
        }
        let titles: Vec<_> = load_from(&path).into_iter().map(|e| e.title).collect();
        assert_eq!(titles, ["a.pdf", "b.pdf"]);
    }

    #[test]
    fn the_list_is_capped_and_survives_a_missing_or_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        assert!(load_from(&path).is_empty());
        std::fs::write(&path, b"not json").unwrap();
        assert!(load_from(&path).is_empty());
        for i in 0..(MAX_ENTRIES + 5) {
            record_to(&path, entry(&i.to_string())).unwrap();
        }
        assert_eq!(load_from(&path).len(), MAX_ENTRIES);
    }

    #[test]
    fn unsaved_designs_keep_a_snapshot_and_big_ones_do_not() {
        let mut doc = LabelDocument::blank(50., 30.);
        assert!(design_entry(&doc, None, "P", 2).design.is_some());
        doc.add(
            crate::design::Element::Image {
                data_base64: "A".repeat(MAX_SNAPSHOT_BYTES + 1),
            },
            1.,
            1.,
            10.,
            10.,
        );
        let big = design_entry(&doc, Some(Path::new("/tmp/x.thermalprint")), "P", 1);
        assert!(big.design.is_none());
        assert_eq!(
            big.source.as_deref(),
            Some(Path::new("/tmp/x.thermalprint"))
        );
    }

    #[test]
    fn invalid_snapshots_and_out_of_range_values_are_cleaned_on_load() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.json");
        let mut bad = design_entry(&LabelDocument::blank(50., 30.), None, "P", 0);
        bad.design.as_mut().unwrap().version = 99;
        let mut loud = entry("x");
        loud.copies = 60_000;
        loud.title = "t".repeat(1000);
        std::fs::write(&path, serde_json::to_vec(&[bad, loud]).unwrap()).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.len(), 1);
        assert_eq!((loaded[0].copies, loaded[0].title.len()), (999, MAX_TEXT));
    }
}

#[cfg(test)]
mod recipe_tests {
    use super::*;
    #[test]
    fn records_page_crop_and_rendering_settings_without_queue_state() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("label.pdf");
        std::fs::write(&source, b"sample").unwrap();
        let settings = crate::settings::Settings {
            rotation: 90,
            dpi: 300,
            scale_percent: 80.,
            ..Default::default()
        };
        let crop = crate::detection::Crop {
            x: 0.1,
            y: 0.2,
            w: 0.6,
            h: 0.7,
        };
        let entry = import_entry(&source, 2, "printer", 1).with_settings(&settings, Some(crop));
        let restored: Entry = serde_json::from_slice(&serde_json::to_vec(&entry).unwrap()).unwrap();
        assert_eq!(restored.page, 2);
        assert_eq!(restored.crop, Some(crop));
        assert_eq!(restored.settings.unwrap().rotation, 90);
        assert_eq!(
            restored.fingerprint,
            Some(crate::duplicates::fingerprint(&source, 2).unwrap())
        );
    }
}

#[cfg(test)]
mod relocation_tests {
    use super::*;
    #[test]
    fn archived_labels_and_undo_keep_the_reprint_source() {
        let dir = tempfile::tempdir().unwrap();
        let history = dir.path().join("history.json");
        let a = dir.path().join("label.png");
        let b = dir.path().join("Processed/label.png");
        record_to(&history, import_entry(&a, 0, "printer", 1)).unwrap();
        relocate_in(&history, &a, &b).unwrap();
        assert_eq!(load_from(&history)[0].source, Some(b.clone()));
        relocate_in(&history, &b, &a).unwrap();
        assert_eq!(load_from(&history)[0].source, Some(a));
    }
}
