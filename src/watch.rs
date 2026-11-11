//! Import new label files as they appear in a folder, such as Downloads.
//! Files are imported only after their size has stopped changing, so a download
//! in progress is never opened half-written. Existing files are never imported.
use anyhow::{Context, Result, bail};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

const EXTENSIONS: [&str; 8] = ["pdf", "png", "jpg", "jpeg", "tif", "tiff", "bmp", "webp"];
const MAX_FILE_BYTES: u64 = 50_000_000;
const MAX_ENTRIES: usize = 5_000;
const MAX_PER_SCAN: usize = 20;

/// Size and modification time. A file is "the same" only while both are unchanged, so a
/// new file saved under a name that was used (and imported) before is still noticed.
type Stamp = (u64, Option<SystemTime>);

pub struct Watcher {
    dir: PathBuf,
    /// Files already imported or present at start, forgotten once deleted or replaced.
    seen: HashMap<PathBuf, Stamp>,
    /// Stamp at the previous scan, for files that may still be downloading.
    pending: HashMap<PathBuf, Stamp>,
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
            seen: HashMap::new(),
            pending: HashMap::new(),
        };
        for (path, stamp) in watcher.candidates()?.0 {
            watcher.seen.insert(path, stamp);
        }
        Ok(watcher)
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Remember a file as already handled, so putting it back in the folder (for example
    /// after an undo) does not import it a second time.
    pub fn mark_seen(&mut self, path: &Path) {
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            self.seen
                .insert(path.to_owned(), (metadata.len(), metadata.modified().ok()));
        }
    }

    /// Regular, supported, non-empty files. Symbolic links are ignored so a link cannot
    /// pull in a file from somewhere the user did not choose. The flag is true when the
    /// folder was too large to list completely.
    fn candidates(&self) -> Result<(Vec<(PathBuf, Stamp)>, bool)> {
        let entries = std::fs::read_dir(&self.dir)
            .with_context(|| format!("Cannot read {}", self.dir.display()))?;
        let mut found = Vec::new();
        let mut count = 0;
        for entry in entries.flatten() {
            count += 1;
            if count > MAX_ENTRIES {
                found.sort();
                return Ok((found, true));
            }
            let path = entry.path();
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_file()
                && supported(&path)
                && (1..=MAX_FILE_BYTES).contains(&metadata.len())
            {
                found.push((path, (metadata.len(), metadata.modified().ok())));
            }
        }
        found.sort();
        Ok((found, false))
    }

    /// Files that appeared since the last scan and have a stable size. Oldest names first.
    pub fn scan(&mut self) -> Result<Vec<PathBuf>> {
        let mut ready = Vec::new();
        let mut still_present = HashMap::new();
        let (candidates, truncated) = self.candidates()?;
        if !truncated {
            // Forget files that were deleted, moved or replaced, so a new file reusing
            // an old name is treated as new.
            let current: HashMap<&PathBuf, &Stamp> =
                candidates.iter().map(|(p, s)| (p, s)).collect();
            self.seen
                .retain(|path, stamp| current.get(path).is_some_and(|now| *now == stamp));
        }
        for (path, stamp) in candidates {
            if self.seen.contains_key(&path) {
                continue;
            }
            if self.pending.get(&path) == Some(&stamp) && ready.len() < MAX_PER_SCAN {
                self.seen.insert(path.clone(), stamp);
                ready.push(path);
            } else {
                still_present.insert(path, stamp);
            }
        }
        self.pending = still_present;
        Ok(ready)
    }
}

/// Name of the subfolder, inside the watch folder, that printed labels are moved to.
pub const PROCESSED_DIR: &str = "Processed";

/// Move a printed label from the watch folder into its `Processed` subfolder. Only files
/// directly inside the watch folder are moved, so labels imported from elsewhere are never
/// touched. An existing file with the same name is kept: the new one gets " (2)", " (3)", ...
/// Returns the new path, or `None` when the file is not in the watch folder.
pub fn archive(file: &Path, watch_dir: &Path) -> Result<Option<PathBuf>> {
    if file.parent() != Some(watch_dir) {
        return Ok(None);
    }
    let Some(name) = file.file_name() else {
        return Ok(None);
    };
    let processed = watch_dir.join(PROCESSED_DIR);
    std::fs::create_dir_all(&processed)
        .with_context(|| format!("Cannot create {}", processed.display()))?;
    let stem = file
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = file
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut target = processed.join(name);
    let mut number = 2;
    while target.symlink_metadata().is_ok() {
        target = processed.join(format!("{stem} ({number}){extension}"));
        number += 1;
    }
    std::fs::rename(file, &target)
        .with_context(|| format!("Cannot move {} to {}", file.display(), target.display()))?;
    Ok(Some(target))
}

/// Put a file moved by [`archive`] back where it came from. Nothing is overwritten: if a
/// file with the original name exists again, the archived one stays in `Processed`.
/// Returns whether the file was restored.
pub fn restore(original: &Path, archived: &Path) -> Result<bool> {
    if original.symlink_metadata().is_ok() || archived.symlink_metadata().is_err() {
        return Ok(false);
    }
    std::fs::rename(archived, original).with_context(|| {
        format!(
            "Cannot move {} back to {}",
            archived.display(),
            original.display()
        )
    })?;
    Ok(true)
}

/// Explain why a folder may be unreadable from the Snap. `real_home` is the user's actual
/// home folder (`SNAP_REAL_HOME` inside a Snap), which the `home` interface covers except
/// for hidden entries.
pub fn snap_access_hint(
    dir: &Path,
    real_home: Option<&Path>,
    in_snap: bool,
) -> Option<&'static str> {
    // Folders chosen in the file picker arrive as document-portal paths, which the Snap can read.
    if !in_snap || dir.starts_with("/run/user") {
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
    fn a_new_file_reusing_an_imported_name_is_imported_again() {
        let dir = tempfile::tempdir().unwrap();
        let mut watcher = Watcher::new(dir.path()).unwrap();
        let label = write(dir.path(), "label.pdf", b"%PDF-1");
        watcher.scan().unwrap();
        assert_eq!(watcher.scan().unwrap(), std::slice::from_ref(&label));
        std::fs::remove_file(&label).unwrap();
        assert!(watcher.scan().unwrap().is_empty());
        write(dir.path(), "label.pdf", b"%PDF-2 different");
        watcher.scan().unwrap();
        assert_eq!(watcher.scan().unwrap(), [label]);
    }

    #[test]
    fn archive_moves_watch_folder_files_only_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let first = write(dir.path(), "label.pdf", b"one");
        let moved = archive(&first, dir.path()).unwrap().unwrap();
        assert_eq!(moved, dir.path().join(PROCESSED_DIR).join("label.pdf"));
        assert!(!first.exists());
        let second = write(dir.path(), "label.pdf", b"two");
        let moved_again = archive(&second, dir.path()).unwrap().unwrap();
        assert_eq!(
            moved_again,
            dir.path().join(PROCESSED_DIR).join("label (2).pdf")
        );
        assert_eq!(std::fs::read(&moved).unwrap(), b"one");
        let other = tempfile::tempdir().unwrap();
        let elsewhere = write(other.path(), "x.pdf", b"x");
        assert_eq!(archive(&elsewhere, dir.path()).unwrap(), None);
        assert!(elsewhere.exists());
        let mut watcher = Watcher::new(dir.path()).unwrap();
        assert!(
            watcher.scan().unwrap().is_empty(),
            "Processed is not scanned"
        );
    }

    #[test]
    fn restore_puts_a_label_back_without_overwriting_and_is_not_reimported() {
        let dir = tempfile::tempdir().unwrap();
        let original = write(dir.path(), "label.pdf", b"one");
        let archived = archive(&original, dir.path()).unwrap().unwrap();
        let mut watcher = Watcher::new(dir.path()).unwrap();
        assert!(restore(&original, &archived).unwrap());
        assert!(original.exists() && !archived.exists());
        watcher.mark_seen(&original);
        for _ in 0..3 {
            assert!(watcher.scan().unwrap().is_empty());
        }
        let again = archive(&original, dir.path()).unwrap().unwrap();
        write(dir.path(), "label.pdf", b"newer");
        assert!(!restore(&original, &again).unwrap(), "never overwrites");
        assert_eq!(std::fs::read(&original).unwrap(), b"newer");
        assert!(again.exists());
        assert!(!restore(&original, &dir.path().join("missing.pdf")).unwrap());
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
        let portal = Path::new("/run/user/1000/doc/ab12/Downloads");
        assert_eq!(snap_access_hint(portal, Some(home), true), None);
    }
}
