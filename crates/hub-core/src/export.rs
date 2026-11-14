//! "Ekspor data": everything the owner keeps, as one JSON document they can read or move.
//!
//! Every table of the owner's data, row by row, column by column, as stored. Left out: the search
//! index (made again from the data), `api_clients` (token hashes are not the owner's data and
//! have no use outside this Hub), and `studio_claude_sessions` (how Claude Code was connected, kept
//! 30 days; not the owner's data either). Files are references (paths), as in the database; Vault copies
//! stay in the Vault folder.

use std::fs;
use std::io::Write;
use std::path::Path;

use jiff::Timestamp;
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde_json::{json, Map, Value};

use crate::db;
use crate::error::Result;
use crate::time::instant;

/// The owner's data, in an order that reads well (things before what points at them).
pub const TABLES: &[&str] = &[
    "settings",
    "areas",
    "projects",
    "milestones",
    "tasks",
    "focus",
    "routines",
    "routine_exceptions",
    "events",
    "reminders",
    "collections",
    "files",
    "file_links",
    "file_collections",
    "tracks",
    "ctf_events",
    "learning_items",
    "learning_sessions",
    "learning_timer",
    "focus_run",
    "focus_blocks",
    "journal",
    "day_closures",
    "tags",
    "taggings",
    "imports",
    "studio_projects",
    "studio_messages",
    "studio_ledger",
    "studio_stacks",
    "studio_prd_versions",
    "studio_roadmaps",
    "studio_checkpoints",
    "studio_tasks",
    "studio_reports",
    "studio_requests",
    "activity",
];

fn value(cell: ValueRef<'_>) -> Value {
    match cell {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(number) => json!(number),
        ValueRef::Real(number) => json!(number),
        ValueRef::Text(text) => json!(String::from_utf8_lossy(text)),
        ValueRef::Blob(bytes) => json!(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()),
    }
}

fn rows(conn: &Connection, table: &str) -> Result<Vec<Value>> {
    // The name comes from TABLES above, never from outside.
    let mut statement = conn.prepare(&format!("SELECT * FROM {table}"))?;
    let names: Vec<String> = statement.column_names().iter().map(|name| name.to_string()).collect();
    let mut found = statement.query([])?;
    let mut out = Vec::new();
    while let Some(row) = found.next()? {
        let mut object = Map::new();
        for (index, name) in names.iter().enumerate() {
            object.insert(name.clone(), value(row.get_ref(index)?));
        }
        out.push(Value::Object(object));
    }
    Ok(out)
}

/// The whole export as one JSON value.
pub fn export(conn: &Connection, version: &str, now: Timestamp) -> Result<Value> {
    let mut tables = Map::new();
    for table in TABLES {
        tables.insert((*table).to_string(), Value::Array(rows(conn, table)?));
    }
    Ok(json!({
        "app": "Hub",
        "version": version,
        "schemaVersion": db::schema_version(conn)?,
        "exportedAt": instant(now),
        "tables": tables,
    }))
}

/// Write the export to `path`: beside it first, then moved over, so a failed write never leaves
/// half a file under the owner's chosen name.
pub fn write(conn: &Connection, path: &Path, version: &str, now: Timestamp) -> Result<usize> {
    let document = export(conn, version, now)?;
    let text = serde_json::to_string_pretty(&document)?;
    let partial = path.with_extension("json.partial");
    {
        let mut file = fs::File::create(&partial)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    fs::rename(&partial, path)?;
    Ok(text.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::model::Actor;
    use crate::tasks::{self, TaskInput};

    #[test]
    fn every_table_of_the_owner_s_data_is_exported_and_secrets_are_not() {
        let mut conn = open_in_memory().unwrap();
        let now: Timestamp = "2026-10-24T08:00:00Z".parse().unwrap();
        tasks::create(&mut conn, TaskInput { title: "Belajar subnetting".into(), ..Default::default() }, Actor::Owner, now).unwrap();
        crate::api_clients::set_token(&mut conn, "ULTRON", &crate::api_clients::hash_token("secret"), crate::api_clients::Scope::ULTRON, Actor::Hub, now)
            .unwrap();
        let document = export(&conn, "0.1.0", now).unwrap();
        assert_eq!(document["schemaVersion"], db::latest_version());
        assert_eq!(document["tables"]["tasks"][0]["title"], "Belajar subnetting");
        assert!(!document["tables"]["activity"].as_array().unwrap().is_empty());
        let text = document.to_string();
        assert!(!text.contains(&crate::api_clients::hash_token("secret")), "no token hash");
        assert!(document["tables"].get("api_clients").is_none() && document["tables"].get("search").is_none());

        // Every table in the schema is either exported or left out on purpose.
        let mut statement = conn.prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'search_%'").unwrap();
        let names: Vec<String> = statement.query_map([], |row| row.get(0)).unwrap().collect::<rusqlite::Result<_>>().unwrap();
        for name in names {
            assert!(TABLES.contains(&name.as_str()) || ["api_clients", "search", "studio_claude_sessions"].contains(&name.as_str()), "{name} is neither exported nor left out");
        }
    }

    #[test]
    fn the_file_is_written_whole() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open_in_memory().unwrap();
        let path = dir.path().join("Hub-ekspor.json");
        let bytes = write(&conn, &path, "0.1.0", "2026-10-24T08:00:00Z".parse().unwrap()).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len() as usize, bytes);
        assert!(!path.with_extension("json.partial").exists());
        let back: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(back["app"], "Hub");
    }
}
