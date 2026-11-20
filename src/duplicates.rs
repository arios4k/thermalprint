//! Durable content-based reservations for automatic printing.
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io::Read, path::Path};

#[derive(Serialize, Deserialize)]
struct Reservation {
    fingerprint: String,
    submitted: bool,
}
fn path() -> Result<std::path::PathBuf> {
    Ok(crate::background::directory()?.join("automatic-prints.json"))
}

pub fn fingerprint(file: &Path, page: usize) -> Result<String> {
    let mut file = std::fs::File::open(file)?;
    if file.metadata()?.len() > 50_000_000 {
        bail!("Label exceeds 50 MB");
    }
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    hash.update((page as u64).to_le_bytes());
    Ok(format!("{:x}", hash.finalize()))
}
fn read(path: &Path) -> Result<Vec<Reservation>> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).context("Cannot read automatic print reservations")
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(e.into()),
    }
}
fn write(path: &Path, entries: &[Reservation]) -> Result<()> {
    let dir = path.parent().context("No reservation folder")?;
    std::fs::create_dir_all(dir)?;
    let mut file = tempfile::NamedTempFile::new_in(dir)?;
    serde_json::to_writer(file.as_file_mut(), entries)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}
pub fn contains(fingerprint: &str) -> Result<bool> {
    Ok(read(&path()?)?.iter().any(|r| r.fingerprint == fingerprint))
}
/// Single-instance ownership serializes claims. Persist before the external side effect.
/// Failed/uncertain submissions remain reserved so a restart cannot double-dispatch.
pub fn reserve(fingerprint: &str) -> Result<()> {
    reserve_in(&path()?, fingerprint)
}
fn reserve_in(path: &Path, fingerprint: &str) -> Result<()> {
    let mut entries = read(path)?;
    if entries.iter().any(|r| r.fingerprint == fingerprint) {
        bail!(
            "This label was already submitted or needs review after an interrupted print. Review it and use Print manually if another copy is wanted."
        );
    }
    if entries.len() >= 10_000 {
        bail!(
            "Automatic print history is full. Review and clear duplicate protection in Print Center."
        );
    }
    entries.push(Reservation {
        fingerprint: fingerprint.into(),
        submitted: false,
    });
    write(path, &entries)
}
pub fn submitted(fingerprint: &str) -> Result<()> {
    let path = path()?;
    let mut entries = read(&path)?;
    if let Some(entry) = entries.iter_mut().find(|r| r.fingerprint == fingerprint) {
        entry.submitted = true;
    } else if entries.len() < 10_000 {
        entries.push(Reservation {
            fingerprint: fingerprint.into(),
            submitted: true,
        });
    } else {
        bail!("Automatic print history is full");
    }
    write(&path, &entries)
}
pub fn clear() -> Result<()> {
    write(&path()?, &[])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renamed_files_match_and_changed_content_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.pdf");
        let b = dir.path().join("download (1).pdf");
        std::fs::write(&a, b"same label").unwrap();
        std::fs::copy(&a, &b).unwrap();
        assert_eq!(fingerprint(&a, 0).unwrap(), fingerprint(&b, 0).unwrap());
        assert_ne!(fingerprint(&a, 0).unwrap(), fingerprint(&b, 1).unwrap());
        std::fs::write(&b, b"new label").unwrap();
        assert_ne!(fingerprint(&a, 0).unwrap(), fingerprint(&b, 0).unwrap());
    }
    #[test]
    fn interrupted_reservations_survive_reload_and_bad_journals_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prints.json");
        reserve_in(&path, "one").unwrap();
        assert!(reserve_in(&path, "one").is_err());
        std::fs::write(&path, b"broken").unwrap();
        assert!(reserve_in(&path, "two").is_err());
    }
}
