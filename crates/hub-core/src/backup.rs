//! Daily copies of the database: `backups/hub-YYYY-MM-DD.db`, 7 daily and 4 weekly kept.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use jiff::civil::Date;
use rusqlite::Connection;

use crate::error::Result;
use crate::time::date_text;

const KEEP_DAILY: usize = 7;
const KEEP_WEEKLY: usize = 4;
const PREFIX: &str = "hub-";
const SUFFIX: &str = ".db";

fn file_name(date: Date) -> String {
    format!("{PREFIX}{}{SUFFIX}", date_text(date))
}

/// The dates that have a backup in `dir`, newest first. Other files are ignored.
pub fn existing(dir: &Path) -> Result<Vec<Date>> {
    let mut dates = BTreeSet::new();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    for entry in fs::read_dir(dir)? {
        let name = entry?.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(date) = name.strip_prefix(PREFIX).and_then(|rest| rest.strip_suffix(SUFFIX)) else {
            continue;
        };
        if let Ok(date) = date.parse::<Date>() {
            dates.insert(date);
        }
    }
    Ok(dates.into_iter().rev().collect())
}

/// Copy the database for `today` unless that copy exists, then prune old copies. Returns the new
/// file. The copy is written under a temporary name and renamed, so a half-written file never
/// looks like a backup.
pub fn run_if_due(conn: &Connection, dir: &Path, today: Date) -> Result<Option<PathBuf>> {
    fs::create_dir_all(dir)?;
    let target = dir.join(file_name(today));
    if target.exists() {
        return Ok(None);
    }
    let partial = dir.join(format!("{}.partial", file_name(today)));
    if partial.exists() {
        fs::remove_file(&partial)?;
    }
    conn.execute("VACUUM INTO ?1", [partial.to_string_lossy()])?;
    fs::rename(&partial, &target)?;
    for date in to_prune(&existing(dir)?) {
        let _ = fs::remove_file(dir.join(file_name(date)));
    }
    Ok(Some(target))
}

/// Which backups to delete: keep the newest 7, then the newest of each older ISO week, for 4
/// weeks. `dates` must be newest first.
pub fn to_prune(dates: &[Date]) -> Vec<Date> {
    let mut weeks = Vec::new();
    let mut prune = Vec::new();
    for (index, date) in dates.iter().enumerate() {
        if index < KEEP_DAILY {
            continue;
        }
        let week = date.iso_week_date();
        let key = (week.year(), week.week());
        if !weeks.contains(&key) && weeks.len() < KEEP_WEEKLY {
            weeks.push(key);
        } else {
            prune.push(*date);
        }
    }
    prune
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use jiff::ToSpan;

    fn day(text: &str) -> Date {
        text.parse().unwrap()
    }

    #[test]
    fn keeps_seven_days_and_four_weeks() {
        // 60 consecutive days, newest first.
        let newest = day("2026-10-22");
        let dates: Vec<Date> = (0..60).map(|n| newest.checked_sub(n.days()).unwrap()).collect();
        let prune = to_prune(&dates);
        let kept: Vec<Date> = dates.iter().filter(|d| !prune.contains(d)).copied().collect();
        assert_eq!(kept.len(), 11);
        assert_eq!(&kept[..7], &dates[..7]);
        // The four weekly copies are each the newest of a distinct older week.
        let weeks: BTreeSet<_> = kept[7..]
            .iter()
            .map(|d| (d.iso_week_date().year(), d.iso_week_date().week()))
            .collect();
        assert_eq!(weeks.len(), 4);
    }

    #[test]
    fn a_backup_is_a_readable_database_made_once_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let conn = db::open(&dir.path().join("hub.db")).unwrap();
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('ink', '\"sepia\"', 'x')",
            [],
        )
        .unwrap();
        let backups = dir.path().join("backups");
        let first = run_if_due(&conn, &backups, day("2026-10-22")).unwrap().unwrap();
        assert!(run_if_due(&conn, &backups, day("2026-10-22")).unwrap().is_none());
        let copy = Connection::open(&first).unwrap();
        let ink: String = copy
            .query_row("SELECT value FROM settings WHERE key = 'ink'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(ink, "\"sepia\"");
        fs::write(backups.join("notes.txt"), "not a backup").unwrap();
        assert_eq!(existing(&backups).unwrap(), vec![day("2026-10-22")]);
    }
}
