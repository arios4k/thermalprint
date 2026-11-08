//! Your own label templates: designs saved in a `templates` folder next to the settings.
//! A template is an ordinary `.thermalprint` file; opening one starts an unsaved copy.
use crate::design::LabelDocument;
use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

pub const MAX_TEMPLATES: usize = 100;
const MAX_FILE_BYTES: u64 = 2_000_000;

#[derive(Clone, Debug, PartialEq)]
pub struct UserTemplate {
    pub name: String,
    pub path: PathBuf,
}

pub fn templates_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
        .map(|dirs| dirs.config_dir().join("templates"))
}

/// A safe file name for a label title: letters, digits, spaces, and `-_.`.
pub fn file_stem(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    let short: String = cleaned.chars().take(60).collect();
    if short.is_empty() {
        "Template".into()
    } else {
        short
    }
}

pub fn list_in(dir: &Path) -> Vec<UserTemplate> {
    let mut found: Vec<UserTemplate> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path).ok()?;
            let is_design = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("thermalprint"));
            if !(metadata.is_file() && is_design && metadata.len() <= MAX_FILE_BYTES) {
                return None;
            }
            let name = path.file_stem()?.to_string_lossy().into_owned();
            Some(UserTemplate { name, path })
        })
        .collect();
    found.sort_by_key(|t| t.name.to_lowercase());
    found.truncate(MAX_TEMPLATES);
    found
}

/// Save a design as a template named after its label name. Saving the same name again
/// replaces that template. Returns the path and whether an older one was replaced.
pub fn save_in(dir: &Path, doc: &LabelDocument) -> Result<(PathBuf, bool)> {
    doc.validate()?;
    std::fs::create_dir_all(dir).context("Could not create the templates folder")?;
    let path = dir.join(format!("{}.thermalprint", file_stem(&doc.title)));
    let replaced = path.exists();
    if !replaced && list_in(dir).len() >= MAX_TEMPLATES {
        bail!("You already have {MAX_TEMPLATES} templates. Remove one first");
    }
    let mut copy = doc.clone();
    copy.title = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    copy.save(&path)?;
    Ok((path, replaced))
}

/// Delete a template; only files inside the templates folder can be removed.
pub fn remove_in(dir: &Path, path: &Path) -> Result<()> {
    let inside = path.parent().is_some_and(|parent| parent == dir);
    if !inside
        || path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("thermalprint"))
    {
        bail!("That file is not one of your templates");
    }
    std::fs::remove_file(path).context("Could not remove the template")
}

pub fn list() -> Vec<UserTemplate> {
    templates_dir().map(|dir| list_in(&dir)).unwrap_or_default()
}
pub fn save(doc: &LabelDocument) -> Result<(PathBuf, bool)> {
    save_in(
        &templates_dir().context("No settings folder is available")?,
        doc,
    )
}
pub fn remove(path: &Path) -> Result<()> {
    remove_in(
        &templates_dir().context("No settings folder is available")?,
        path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::design::{Element, TextAlign};

    fn design(title: &str) -> LabelDocument {
        let mut doc = LabelDocument::blank(50., 30.);
        doc.title = title.into();
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
    fn titles_become_safe_file_names() {
        assert_eq!(file_stem("Shelf tag 2×1"), "Shelf tag 2_1");
        let traversal = file_stem("../../etc/passwd");
        assert!(
            !traversal.contains('/') && !traversal.starts_with('.'),
            "{traversal}"
        );
        assert_eq!(file_stem("   "), "Template");
        assert_eq!(file_stem(&"x".repeat(200)).len(), 60);
    }

    #[test]
    fn templates_are_saved_listed_reloaded_and_replaced_by_name() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list_in(dir.path()).is_empty());
        let (path, replaced) = save_in(dir.path(), &design("Price tag")).unwrap();
        assert!(!replaced && path.exists());
        save_in(dir.path(), &design("Asset tag")).unwrap();
        let names: Vec<_> = list_in(dir.path()).into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["Asset tag", "Price tag"], "sorted by name");
        let loaded = LabelDocument::load(&path).unwrap();
        assert_eq!(loaded.title, "Price tag");
        assert_eq!(loaded.objects.len(), 1);
        let (_, replaced) = save_in(dir.path(), &design("Price tag")).unwrap();
        assert!(replaced);
        assert_eq!(list_in(dir.path()).len(), 2);
    }

    #[test]
    fn removing_only_works_inside_the_templates_folder() {
        let dir = tempfile::tempdir().unwrap();
        let (path, _) = save_in(dir.path(), &design("Keep")).unwrap();
        let outside = tempfile::tempdir().unwrap();
        let stray = outside.path().join("stray.thermalprint");
        std::fs::write(&stray, "{}").unwrap();
        assert!(remove_in(dir.path(), &stray).is_err());
        assert!(stray.exists());
        let note = dir.path().join("notes.txt");
        std::fs::write(&note, "x").unwrap();
        assert!(remove_in(dir.path(), &note).is_err());
        remove_in(dir.path(), &path).unwrap();
        assert!(list_in(dir.path()).is_empty());
    }

    #[test]
    fn invalid_designs_and_a_full_folder_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut bad = design("Bad");
        bad.width_mm = -1.;
        assert!(save_in(dir.path(), &bad).is_err());
        for n in 0..MAX_TEMPLATES {
            save_in(dir.path(), &design(&format!("T{n}"))).unwrap();
        }
        assert!(
            save_in(dir.path(), &design("One too many"))
                .unwrap_err()
                .to_string()
                .contains("Remove one")
        );
        assert!(
            save_in(dir.path(), &design("T3")).is_ok(),
            "replacing an existing name is still allowed"
        );
    }
}
