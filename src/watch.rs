//! Import new label files as they appear in a folder, such as Downloads.
//! Files are imported only after their size has stopped changing, so a download
//! in progress is never opened half-written. Existing files are never imported.
use anyhow::{Context, Result, bail};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

const EXTENSIONS: [&str; 8] = ["pdf", "png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp"];
const MAX_FILE_BYTES: u64 = 50_000_000;
const MAX_ENTRIES: usize = 5_000;
const MAX_PER_SCAN: usize = 20;

pub struct Watcher {
    dir: PathBuf,
    seen: HashSet<PathBuf>,
    /// Size at the previous scan, for files that may still be downloading.
    pending: HashMap<PathBuf, u64>,
}

fn supported(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| !name.to_string_lossy().starts_with('.'))
        && path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

impl Watcher {
    /// Start watching. Files already in the folder are remembered, not imported.
    pub fn new(dir: &Path) -> Result<Self> {
        let mut watcher = Self {
            dir: dir.to_owned(),
            seen: HashSet::new(),
            pending: HashMap::new(),
        };
        for (path, _) in watcher.candidates()? {
            watcher.seen.insert(path);
        }
        Ok(watcher)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Regular, supported, non-empty files. Symbolic links are ignored so a link cannot
    /// pull in a file from somewhere the user did not choose.
    fn candidates(&self) -> Result<Vec<(PathBuf, u64)>> {
        let entries = std::fs::read_dir(&self.dir)
            .with_context(|| format!("Cannot read {}", self.dir.display()))?;
        let mut found = Vec::new();
        for entry in entries.take(MAX_ENTRIES).flatten() {
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_file()
                && supported(&path)
                && (1..=MAX_FILE_BYTES).contains(&metadata.len())
            {
                found.push((path, metadata.len()));
            }
        }
        found.sort();
        Ok(found)
    }

    /// Files that appeared since the last scan and have a stable size. Oldest names first.
    pub fn scan(&mut self) -> Result<Vec<PathBuf>> {
        let mut ready = Vec::new();
        let mut still_present = HashMap::new();
        for (path, length) in self.candidates()? {
            if self.seen.contains(&path) {
                continue;
            }
            if self.pending.get(&path) == Some(&length) && ready.len() < MAX_PER_SCAN {
                self.seen.insert(path.clone());
                ready.push(path);
            } else {
                still_present.insert(path, length);
            }
        }
        self.pending = still_present;
        Ok(ready)
    }
}

/// Explain why a folder may be unreadable from the Snap. `real_home` is the user's actual
/// home folder (`SNAP_REAL_HOME` inside a Snap), which the `home` interface covers except
/// for hidden entries.
pub fn snap_access_hint(
    dir: &Path,
    real_home: Option<&Path>,
    in_snap: bool,
) -> Option<&'static str> {
    if !in_snap {
        return None;
    }
    let Some(relative) = real_home.and_then(|home| dir.strip_prefix(home).ok()) else {
        return Some(
            "The Snap can read your home folder only. For folders on other drives, run: sudo snap connect thermalprint:removable-media",
        );
    };
    relative
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
        .then_some("The Snap cannot read hidden folders. Choose a folder such as Downloads.")
}

pub fn current_snap_hint(dir: &Path) -> Option<&'static str> {
    let home = std::env::var_os("SNAP_REAL_HOME")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    snap_access_hint(dir, home.as_deref(), std::env::var_os("SNAP").is_some())
}

/// Validate a folder before watching it.
pub fn check(dir: &Path) -> Result<()> {
    if !dir.is_dir() {
        bail!("{} is not a folder", dir.display());
    }
    Watcher::new(dir)
        .map(|_| ())
        .map_err(|error| match current_snap_hint(dir) {
            Some(hint) => anyhow::anyhow!("{error:#}. {hint}"),
            None => error,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn existing_files_are_never_imported_and_new_ones_wait_for_a_stable_size() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "old.pdf", b"%PDF old");
        let mut watcher = Watcher::new(dir.path()).unwrap();
        assert!(watcher.scan().unwrap().is_empty());
        let new = write(dir.path(), "label.pdf", b"%PDF-1");
        assert!(
            watcher.scan().unwrap().is_empty(),
            "first sighting only records the size"
        );
        std::fs::write(&new, b"%PDF-1 and more").unwrap();
        assert!(
            watcher.scan().unwrap().is_empty(),
            "a growing file is not stable"
        );
        assert_eq!(watcher.scan().unwrap(), std::slice::from_ref(&new));
        assert!(
            watcher.scan().unwrap().is_empty(),
            "each file is imported once"
        );
    }

    #[test]
    fn unsupported_hidden_empty_and_linked_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let mut watcher = Watcher::new(dir.path()).unwrap();
        write(dir.path(), "notes.txt", b"x");
        write(dir.path(), ".hidden.pdf", b"x");
        write(dir.path(), "empty.pdf", b"");
        write(dir.path(), "part.pdf.crdownload", b"x");
        let outside = tempfile::tempdir().unwrap();
        let target = write(outside.path(), "secret.pdf", b"%PDF");
        std::os::unix::fs::symlink(&target, dir.path().join("link.pdf")).unwrap();
        for _ in 0..3 {
            assert!(watcher.scan().unwrap().is_empty());
        }
    }

    #[test]
    fn extensions_match_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        let mut watcher = Watcher::new(dir.path()).unwrap();
        let png = write(dir.path(), "LABEL.PNG", b"x");
        watcher.scan().unwrap();
        assert_eq!(watcher.scan().unwrap(), [png]);
    }

    #[test]
    fn a_missing_folder_is_an_error_and_is_not_a_folder_is_reported() {
        assert!(Watcher::new(Path::new("/nonexistent/folder")).is_err());
        let dir = tempfile::tempdir().unwrap();
        let file = write(dir.path(), "a.pdf", b"x");
        assert!(
            check(&file)
                .unwrap_err()
                .to_string()
                .contains("not a folder")
        );
        assert!(check(dir.path()).is_ok());
    }

    #[test]
    fn snap_hints_cover_other_drives_and_hidden_folders_only() {
        let home = Path::new("/home/alexis");
        assert_eq!(
            snap_access_hint(Path::new("/home/alexis/Downloads"), Some(home), true),
            None
        );
        assert!(
            snap_access_hint(Path::new("/media/usb"), Some(home), true)
                .unwrap()
                .contains("removable-media")
        );
        assert!(
            snap_access_hint(Path::new("/home/alexis/.cache/x"), Some(home), true)
                .unwrap()
                .contains("hidden")
        );
        assert_eq!(
            snap_access_hint(Path::new("/media/usb"), Some(home), false),
            None
        );
    }
}
