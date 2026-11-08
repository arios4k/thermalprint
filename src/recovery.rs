//! Crash recovery: while a design has unsaved changes, a copy is kept next to the settings.
//! It is removed when the design is saved or deliberately discarded, so a copy that is still
//! there at the next start means the app ended unexpectedly.
use crate::design::LabelDocument;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const FILE: &str = "recovery.thermalprint";
const MAX_BYTES: u64 = 20_000_000;

pub fn recovery_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
        .map(|dirs| dirs.data_local_dir().to_path_buf())
}

pub fn write_in(dir: &Path, doc: &LabelDocument) -> Result<()> {
    std::fs::create_dir_all(dir).context("Could not create the recovery folder")?;
    // Write to a side file and rename it, so a crash while writing keeps the older copy.
    let target = dir.join(FILE);
    let partial = dir.join(format!("{FILE}.part"));
    doc.save(&partial)?;
    std::fs::rename(&partial, &target).context("Could not store the recovery copy")
}

pub fn read_in(dir: &Path) -> Option<LabelDocument> {
    let path = dir.join(FILE);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() || metadata.len() > MAX_BYTES {
        return None;
    }
    LabelDocument::load(&path).ok()
}

pub fn clear_in(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(FILE));
    let _ = std::fs::remove_file(dir.join(format!("{FILE}.part")));
}

pub fn write(doc: &LabelDocument) {
    if let Some(dir) = recovery_dir() {
        let _ = write_in(&dir, doc);
    }
}
pub fn read() -> Option<LabelDocument> {
    read_in(&recovery_dir()?)
}
pub fn clear() {
    if let Some(dir) = recovery_dir() {
        clear_in(&dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Element, TextAlign};

    fn design() -> LabelDocument {
        let mut doc = LabelDocument::blank(50., 30.);
        doc.title = "Unsaved work".into();
        doc.add(
            Element::Text {
                text: "Hello".into(),
                font_pt: 12.,
                bold: false,
                align: TextAlign::Left,
            },
            2.,
            2.,
            30.,
            8.,
        );
        doc
    }

    #[test]
    fn a_recovery_copy_round_trips_and_is_cleared() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_in(dir.path()).is_none());
        let doc = design();
        write_in(dir.path(), &doc).unwrap();
        assert_eq!(read_in(dir.path()), Some(doc.clone()));
        let mut newer = doc;
        newer.title = "Newer".into();
        write_in(dir.path(), &newer).unwrap();
        assert_eq!(read_in(dir.path()).unwrap().title, "Newer");
        clear_in(dir.path());
        assert!(read_in(dir.path()).is_none());
        clear_in(dir.path());
    }

    #[test]
    fn a_damaged_recovery_copy_is_ignored_not_fatal() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), "{ not json").unwrap();
        assert!(read_in(dir.path()).is_none());
    }

    #[test]
    fn a_half_written_copy_never_replaces_the_good_one() {
        let dir = tempfile::tempdir().unwrap();
        write_in(dir.path(), &design()).unwrap();
        std::fs::write(dir.path().join(format!("{FILE}.part")), "{ half").unwrap();
        assert_eq!(read_in(dir.path()).unwrap().title, "Unsaved work");
    }
}
