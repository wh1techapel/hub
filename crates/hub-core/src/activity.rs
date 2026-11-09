//! Jejak: every write leaves one row here, in the same transaction as the write itself.
//!
//! `summary` is a snapshot of the thing's name at that moment (a task's title), not a sentence:
//! the UI builds the sentence from actor, verb, and entity in the owner's language.

use jiff::Timestamp;
use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use crate::error::Result;
use crate::ids::new_id;
use crate::model::{Actor, Entity, Verb};
use crate::time::instant;

pub struct Record<'a> {
    pub actor: Actor,
    pub verb: Verb,
    pub entity: Entity,
    pub entity_id: Option<&'a str>,
    pub summary: &'a str,
    pub details: serde_json::Value,
}

pub fn record(conn: &Connection, at: Timestamp, record: Record<'_>) -> Result<()> {
    conn.execute(
        "INSERT INTO activity (id, at, actor, verb, entity_type, entity_id, summary, details)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            new_id(),
            instant(at),
            record.actor,
            record.verb,
            record.entity,
            record.entity_id,
            record.summary,
            record.details.to_string(),
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivityItem {
    pub id: String,
    pub at: String,
    pub actor: Actor,
    pub verb: Verb,
    pub entity: Entity,
    pub entity_id: Option<String>,
    pub summary: String,
}

/// The newest rows first. Settings changes are left out unless asked for: they are not part of
/// the owner's day.
pub fn recent(conn: &Connection, limit: u32, include_settings: bool) -> Result<Vec<ActivityItem>> {
    let mut statement = conn.prepare(
        "SELECT id, at, actor, verb, entity_type, entity_id, summary FROM activity
         WHERE ?1 OR entity_type <> 'settings'
         ORDER BY at DESC, id DESC LIMIT ?2",
    )?;
    let rows = statement.query_map(params![include_settings, limit], |row| {
        Ok(ActivityItem {
            id: row.get(0)?,
            at: row.get(1)?,
            actor: row.get(2)?,
            verb: row.get(3)?,
            entity: row.get(4)?,
            entity_id: row.get(5)?,
            summary: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn newest_first_and_settings_hidden_by_default() {
        let conn = open_in_memory().unwrap();
        let t0: Timestamp = "2026-10-22T01:00:00Z".parse().unwrap();
        let t1: Timestamp = "2026-10-22T02:00:00Z".parse().unwrap();
        record(
            &conn,
            t0,
            Record {
                actor: Actor::Owner,
                verb: Verb::Completed,
                entity: Entity::Task,
                entity_id: Some("x"),
                summary: "Laporan Lab",
                details: serde_json::json!({}),
            },
        )
        .unwrap();
        record(
            &conn,
            t1,
            Record {
                actor: Actor::Owner,
                verb: Verb::Updated,
                entity: Entity::Settings,
                entity_id: None,
                summary: "ink",
                details: serde_json::json!({ "keys": ["ink"] }),
            },
        )
        .unwrap();
        let items = recent(&conn, 10, false).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].summary, "Laporan Lab");
        let all = recent(&conn, 10, true).unwrap();
        assert_eq!(all[0].entity, Entity::Settings);
    }
}
