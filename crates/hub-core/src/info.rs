//! What Settings shows about this Hub itself.

use serde::Serialize;
use ts_rs::TS;

use crate::error::Result;
use crate::time::date_text;
use crate::{backup, db, Hub};

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Info {
    pub version: String,
    pub data_dir: String,
    pub schema_version: i32,
    /// The date of the newest backup, `YYYY-MM-DD`.
    pub last_backup: Option<String>,
}

pub fn info(hub: &Hub, version: &str) -> Result<Info> {
    let schema_version = hub.read(db::schema_version)?;
    let last_backup = backup::existing(&hub.paths().backups)?
        .first()
        .map(|date| date_text(*date));
    Ok(Info {
        version: version.to_string(),
        data_dir: hub.paths().root.display().to_string(),
        schema_version: schema_version as i32,
        last_backup,
    })
}
