//! The Vault: Hub's own copies of files the owner chose to keep (or that came from a drive that
//! will be unplugged). `vault\<yyyy>\<uuid>-<name>`, hashed with SHA-256 while copying, so a
//! missing original still has its copy and the copy can be checked.
//!
//! These functions touch only the disk. The caller copies without holding the database, then
//! records the result with `files::set_vault`.

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use jiff::Timestamp;
use sha2::{Digest, Sha256};

use crate::error::{invalid, Reason, Result};
use crate::hub::Paths;
use crate::ids::new_id;

/// What a copy left behind: its place relative to the Vault folder, and its hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copy {
    pub relative: String,
    pub sha256: String,
}

/// Characters Windows refuses in a file name, and control characters, become "_".
fn safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    let mut short: String = trimmed.chars().take(120).collect();
    if short.is_empty() {
        short = "file".into();
    }
    short
}

/// Copy `source` into the Vault. Folders are not copied.
pub fn copy_in(paths: &Paths, source: &Path, name: &str, now: Timestamp) -> Result<Copy> {
    if !source.is_file() {
        return invalid("path", Reason::Format);
    }
    let year = now.to_zoned(jiff::tz::TimeZone::UTC).year();
    let relative = format!("{year}\\{}-{}", new_id(), safe_name(name));
    let target = paths.vault.join(&relative);
    if let Some(folder) = target.parent() {
        fs::create_dir_all(folder)?;
    }
    let result = copy_hashed(source, &target);
    if result.is_err() {
        // Never leave half a copy behind.
        let _ = fs::remove_file(&target);
    }
    Ok(Copy { relative, sha256: result? })
}

fn copy_hashed(source: &Path, target: &Path) -> Result<String> {
    let mut reader = BufReader::new(File::open(source)?);
    let mut writer = BufWriter::new(File::create_new(target)?);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 256 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        writer.write_all(&buffer[..read])?;
    }
    writer.flush()?;
    writer.get_ref().sync_all()?;
    Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Where a Vault copy is on disk. Only relative paths Hub made itself are accepted.
pub fn resolve(paths: &Paths, relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    let plain = path.components().all(|part| matches!(part, std::path::Component::Normal(_)));
    plain.then(|| paths.vault.join(path))
}

/// Delete a copy (the owner asked to). A copy already gone is fine.
pub fn remove(paths: &Paths, relative: &str) -> Result<()> {
    let Some(path) = resolve(paths, relative) else {
        return invalid("vaultPath", Reason::Format);
    };
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_copy_lands_in_the_year_folder_with_its_hash() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path().join("hub"));
        let source = dir.path().join("writeup.md");
        fs::write(&source, b"abc").unwrap();
        let now: Timestamp = "2026-10-23T05:00:00Z".parse().unwrap();
        let copy = copy_in(&paths, &source, "writeup.md", now).unwrap();
        assert!(copy.relative.starts_with("2026\\"));
        assert!(copy.relative.ends_with("-writeup.md"));
        // SHA-256 of "abc".
        assert_eq!(copy.sha256, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        let stored = resolve(&paths, &copy.relative).unwrap();
        assert_eq!(fs::read(&stored).unwrap(), b"abc");
        remove(&paths, &copy.relative).unwrap();
        assert!(!stored.exists());
        remove(&paths, &copy.relative).unwrap();
    }

    #[test]
    fn names_are_made_safe_and_folders_refused() {
        assert_eq!(safe_name("a<b>:c?.txt"), "a_b__c_.txt");
        assert_eq!(safe_name("..."), "file");
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::under(dir.path().join("hub"));
        let now: Timestamp = "2026-10-23T05:00:00Z".parse().unwrap();
        assert!(copy_in(&paths, dir.path(), "folder", now).is_err());
        assert_eq!(resolve(&paths, "..\\hub.db"), None);
        assert_eq!(resolve(&paths, "C:\\Windows\\win.ini"), None);
    }
}
