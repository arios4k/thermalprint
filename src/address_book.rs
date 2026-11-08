//! A small address book: names and multi-line addresses kept in a file next to the settings,
//! so a ship-to or return address can be added to any label in one click.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const MAX_ENTRIES: usize = 500;
const MAX_NAME: usize = 100;
const MAX_ADDRESS: usize = 600;
const FILE: &str = "addresses.json";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub address: String,
}

pub fn book_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "ThermalPrint", "ThermalPrint")
        .map(|dirs| dirs.config_dir().to_path_buf())
}

pub fn load_in(dir: &Path) -> Vec<Entry> {
    let path = dir.join(FILE);
    let Ok(metadata) = std::fs::symlink_metadata(&path) else {
        return Vec::new();
    };
    if !metadata.is_file() || metadata.len() > 2_000_000 {
        return Vec::new();
    }
    let mut entries: Vec<Entry> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    entries.retain(|e| !e.name.trim().is_empty());
    entries.truncate(MAX_ENTRIES);
    entries
}

fn write_in(dir: &Path, entries: &[Entry]) -> Result<()> {
    std::fs::create_dir_all(dir).context("Could not create the settings folder")?;
    let partial = dir.join(format!("{FILE}.part"));
    std::fs::write(&partial, serde_json::to_string_pretty(entries)?)?;
    std::fs::rename(&partial, dir.join(FILE)).context("Could not save the address book")
}

/// Add an address, or replace the one with the same name (ignoring case). Returns the new list.
pub fn add_in(dir: &Path, name: &str, address: &str) -> Result<Vec<Entry>> {
    let name = name.trim();
    let address = address.trim_end();
    if name.is_empty() || address.trim().is_empty() {
        bail!("An address needs a name and some text");
    }
    if name.chars().count() > MAX_NAME || address.chars().count() > MAX_ADDRESS {
        bail!("The name can have {MAX_NAME} characters and the address {MAX_ADDRESS}");
    }
    let mut entries = load_in(dir);
    let entry = Entry {
        name: name.to_string(),
        address: address.to_string(),
    };
    match entries
        .iter_mut()
        .find(|e| e.name.eq_ignore_ascii_case(name))
    {
        Some(existing) => *existing = entry,
        None => {
            if entries.len() >= MAX_ENTRIES {
                bail!("The address book is full ({MAX_ENTRIES}). Remove an entry first");
            }
            entries.push(entry);
        }
    }
    entries.sort_by_key(|e| e.name.to_lowercase());
    write_in(dir, &entries)?;
    Ok(entries)
}

pub fn remove_in(dir: &Path, name: &str) -> Result<Vec<Entry>> {
    let mut entries = load_in(dir);
    entries.retain(|e| e.name != name);
    write_in(dir, &entries)?;
    Ok(entries)
}

pub fn load() -> Vec<Entry> {
    book_dir().map(|dir| load_in(&dir)).unwrap_or_default()
}
pub fn add(name: &str, address: &str) -> Result<Vec<Entry>> {
    add_in(
        &book_dir().context("No settings folder is available")?,
        name,
        address,
    )
}
pub fn remove(name: &str) -> Result<Vec<Entry>> {
    remove_in(
        &book_dir().context("No settings folder is available")?,
        name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_added_sorted_replaced_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_in(dir.path()).is_empty());
        add_in(dir.path(), "Zed Shop", "1 Main St\nTown").unwrap();
        let list = add_in(dir.path(), "alice", "2 Side Rd\nCity  \n").unwrap();
        assert_eq!(
            list.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["alice", "Zed Shop"]
        );
        assert_eq!(
            list[0].address, "2 Side Rd\nCity",
            "trailing blanks are dropped"
        );
        let list = add_in(dir.path(), "ALICE", "New address").unwrap();
        assert_eq!(list.len(), 2, "the same name replaces, ignoring case");
        assert_eq!(load_in(dir.path())[0].address, "New address");
        let list = remove_in(dir.path(), "ALICE").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(remove_in(dir.path(), "missing").unwrap().len(), 1);
    }

    #[test]
    fn bad_input_and_damaged_files_are_handled() {
        let dir = tempfile::tempdir().unwrap();
        assert!(add_in(dir.path(), " ", "x").is_err());
        assert!(add_in(dir.path(), "Name", "  \n ").is_err());
        assert!(add_in(dir.path(), &"n".repeat(101), "x").is_err());
        assert!(add_in(dir.path(), "Name", &"a".repeat(601)).is_err());
        std::fs::write(dir.path().join(FILE), "{ broken").unwrap();
        assert!(load_in(dir.path()).is_empty());
        assert!(
            add_in(dir.path(), "Fresh", "Start over").is_ok(),
            "a damaged file is replaced"
        );
        for n in 0..MAX_ENTRIES {
            let _ = add_in(dir.path(), &format!("Person {n}"), "x");
        }
        assert!(add_in(dir.path(), "One too many", "x").is_err());
    }
}
