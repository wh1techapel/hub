//! Clients of the local API (ULTRON): a name, the hash of its token, and what it may do.
//!
//! The token itself never reaches the database. The app writes it to a file in the data folder,
//! which only the owner's account can read, and keeps its SHA-256 here; a request's token is
//! hashed and looked up. Rotating replaces the hash, so the old token stops working at once.

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Reason, Result};
use crate::ids::new_id;
use crate::model::{Actor, Entity, Verb};
use crate::time::{instant, parse_instant};

/// What a client may do (docs/architecture.md, "Local API"). Reading is one scope; each kind of
/// write is its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Scope {
    #[serde(rename = "read")]
    Read,
    #[serde(rename = "write:tasks")]
    WriteTasks,
    #[serde(rename = "write:reminders")]
    WriteReminders,
    #[serde(rename = "write:events")]
    WriteEvents,
    #[serde(rename = "write:files")]
    WriteFiles,
    #[serde(rename = "write:journal")]
    WriteJournal,
    /// Studio, for Claude Code: read the projects it was given access to.
    #[serde(rename = "studio:read")]
    StudioRead,
    /// Studio, for Claude Code: start a checkpoint, update its tasks, ask, report.
    #[serde(rename = "studio:report")]
    StudioReport,
}

impl Scope {
    /// ULTRON's scopes: the day, tasks, reminders, files, notes. No Studio.
    pub const ULTRON: &'static [Scope] =
        &[Scope::Read, Scope::WriteTasks, Scope::WriteReminders, Scope::WriteEvents, Scope::WriteFiles, Scope::WriteJournal];
    /// Claude Code's scopes: Studio only (docs/studio.md, "Access").
    pub const CLAUDE: &'static [Scope] = &[Scope::StudioRead, Scope::StudioReport];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ApiClient {
    pub id: String,
    pub name: String,
    pub scopes: Vec<Scope>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

impl ApiClient {
    pub fn may(&self, scope: Scope) -> bool {
        self.scopes.contains(&scope)
    }
}

const NAME_MAX: usize = 40;
/// `last_used_at` is written at most this often per client, so reading does not mean writing.
const TOUCH_EVERY_SECONDS: i64 = 60;

/// The hash kept for a token: SHA-256, lowercase hex.
pub fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn client(row: &rusqlite::Row<'_>) -> rusqlite::Result<ApiClient> {
    let scopes: String = row.get(2)?;
    Ok(ApiClient {
        id: row.get(0)?,
        name: row.get(1)?,
        // A scope this version does not know is dropped, never widened.
        scopes: serde_json::from_str::<Vec<serde_json::Value>>(&scopes)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| serde_json::from_value(value).ok())
            .collect(),
        created_at: row.get(3)?,
        last_used_at: row.get(4)?,
    })
}

const COLUMNS: &str = "id, name, scopes, created_at, last_used_at";

/// The live client whose token hashes to `hash`.
pub fn find_by_hash(conn: &Connection, hash: &str) -> Result<Option<ApiClient>> {
    let sql = format!("SELECT {COLUMNS} FROM api_clients WHERE token_hash = ?1 AND revoked_at IS NULL");
    Ok(conn.query_row(&sql, [hash], client).optional()?)
}

pub fn by_name(conn: &Connection, name: &str) -> Result<Option<ApiClient>> {
    let sql = format!("SELECT {COLUMNS} FROM api_clients WHERE name = ?1 AND revoked_at IS NULL");
    Ok(conn.query_row(&sql, [name], client).optional()?)
}

pub fn list(conn: &Connection) -> Result<Vec<ApiClient>> {
    let sql = format!("SELECT {COLUMNS} FROM api_clients WHERE revoked_at IS NULL ORDER BY created_at");
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([], client)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Give `name` a new token hash (and these scopes), creating the client when it is new. The old
/// token stops working in the same transaction. Recorded in Jejak as a settings change by `actor`
/// (Hub on first start, the owner when rotating from Pengaturan); the hash is never recorded.
pub fn set_token(conn: &mut Connection, name: &str, hash: &str, scopes: &[Scope], actor: Actor, now: Timestamp) -> Result<ApiClient> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return invalid("name", Reason::Length);
    }
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return invalid("tokenHash", Reason::Format);
    }
    let scopes_json = serde_json::to_string(scopes)?;
    let stamp = instant(now);
    let tx = conn.transaction()?;
    let existing: Option<String> = tx.query_row("SELECT id FROM api_clients WHERE name = ?1", [name], |row| row.get(0)).optional()?;
    let verb = match &existing {
        Some(id) => {
            tx.execute(
                "UPDATE api_clients SET token_hash = ?2, scopes = ?3, revoked_at = NULL WHERE id = ?1",
                params![id, hash, scopes_json],
            )?;
            Verb::Updated
        }
        None => {
            tx.execute(
                "INSERT INTO api_clients (id, name, token_hash, scopes, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![new_id(), name, hash, scopes_json, stamp],
            )?;
            Verb::Created
        }
    };
    activity::record(
        &tx,
        now,
        Record {
            actor,
            verb,
            entity: Entity::Settings,
            entity_id: None,
            summary: "api_token",
            details: serde_json::json!({ "client": name }),
        },
    )?;
    tx.commit()?;
    Ok(by_name(conn, name)?.expect("the client was just written"))
}

/// Note that a client was just used (at most once a minute).
pub fn touch(conn: &Connection, client: &ApiClient, now: Timestamp) -> Result<()> {
    let recent = client
        .last_used_at
        .as_deref()
        .and_then(|at| parse_instant("lastUsedAt", at).ok())
        .is_some_and(|at| now.as_second() - at.as_second() < TOUCH_EVERY_SECONDS);
    if !recent {
        conn.execute("UPDATE api_clients SET last_used_at = ?2 WHERE id = ?1", params![client.id, instant(now)])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn now() -> Timestamp {
        "2026-10-23T14:00:00Z".parse().unwrap()
    }

    #[test]
    fn a_token_is_found_by_its_hash_until_it_is_replaced() {
        let mut conn = open_in_memory().unwrap();
        let first = hash_token("first-token");
        let client = set_token(&mut conn, "ULTRON", &first, &[Scope::Read, Scope::WriteTasks], Actor::Hub, now()).unwrap();
        assert!(client.may(Scope::WriteTasks));
        assert!(!client.may(Scope::WriteFiles));
        assert_eq!(find_by_hash(&conn, &first).unwrap().unwrap().name, "ULTRON");

        let second = hash_token("second-token");
        set_token(&mut conn, "ULTRON", &second, Scope::ULTRON, Actor::Owner, now()).unwrap();
        assert!(find_by_hash(&conn, &first).unwrap().is_none(), "the old token stops working");
        assert!(find_by_hash(&conn, &second).unwrap().unwrap().may(Scope::WriteFiles));
        assert_eq!(list(&conn).unwrap().len(), 1);
    }

    #[test]
    fn the_hash_is_sha256_hex_and_never_recorded() {
        let mut conn = open_in_memory().unwrap();
        let hash = hash_token("abc");
        assert_eq!(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert!(set_token(&mut conn, "ULTRON", "not-a-hash", Scope::ULTRON, Actor::Hub, now()).is_err());
        set_token(&mut conn, "ULTRON", &hash, Scope::ULTRON, Actor::Hub, now()).unwrap();
        let recorded: String = conn.query_row("SELECT group_concat(details || summary) FROM activity", [], |row| row.get(0)).unwrap();
        assert!(!recorded.contains(&hash));
    }

    #[test]
    fn last_use_is_written_at_most_once_a_minute() {
        let mut conn = open_in_memory().unwrap();
        let client = set_token(&mut conn, "ULTRON", &hash_token("t"), Scope::ULTRON, Actor::Hub, now()).unwrap();
        touch(&conn, &client, now()).unwrap();
        let used = by_name(&conn, "ULTRON").unwrap().unwrap();
        assert_eq!(used.last_used_at.as_deref(), Some("2026-10-23T14:00:00Z"));
        let later: Timestamp = "2026-10-23T14:00:30Z".parse().unwrap();
        touch(&conn, &used, later).unwrap();
        assert_eq!(by_name(&conn, "ULTRON").unwrap().unwrap().last_used_at.as_deref(), Some("2026-10-23T14:00:00Z"));
    }
}
