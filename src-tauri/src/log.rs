//! Errors with no window to show them in go to `hub.log` in the data folder. One previous file is
//! kept when it grows past a megabyte. No owner data is written here: only what failed.

use std::fs::{self, OpenOptions};
use std::io::Write;

use hub_core::Paths;

const MAX_BYTES: u64 = 1024 * 1024;

pub fn error(paths: &Paths, message: &str) {
    eprintln!("[hub] {message}");
    if fs::metadata(&paths.log).is_ok_and(|meta| meta.len() > MAX_BYTES) {
        let _ = fs::rename(&paths.log, paths.log.with_extension("log.1"));
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&paths.log) {
        let _ = writeln!(file, "{} {message}", hub_core::time::instant(jiff::Timestamp::now()));
    }
}
