//! One open Hub: its folders and its single database connection.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;

use crate::db;
use crate::error::{Error, Result};

/// Where a Hub keeps its data (docs/architecture.md, "Location").
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
    pub db: PathBuf,
    pub backups: PathBuf,
    pub vault: PathBuf,
    /// Thumbnails and other things Hub can make again.
    pub cache: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    pub fn under(root: impl Into<PathBuf>) -> Paths {
        let root = root.into();
        Paths {
            db: root.join("hub.db"),
            backups: root.join("backups"),
            vault: root.join("vault"),
            cache: root.join("cache"),
            log: root.join("hub.log"),
            root,
        }
    }
}

/// The database behind one lock. One owner, short statements: a single connection is enough, and
/// it keeps writes strictly one after another.
pub struct Hub {
    conn: Mutex<Connection>,
    paths: Paths,
}

impl Hub {
    pub fn open(paths: Paths) -> Result<Hub> {
        fs::create_dir_all(&paths.root)?;
        let conn = db::open_with_backup(&paths.db, &paths.backups)?;
        Ok(Hub {
            conn: Mutex::new(conn),
            paths,
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        let conn = self.conn.lock().map_err(|_| Error::Poisoned)?;
        f(&conn)
    }

    pub fn write<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut conn = self.conn.lock().map_err(|_| Error::Poisoned)?;
        f(&mut conn)
    }
}
