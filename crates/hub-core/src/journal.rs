//! Catatan: dated notes, done activities, and the evening sentence of Tutup Hari.
//!
//! A note may explain a task (`task_id`): a file ULTRON was told about with a deadline becomes a
//! task, and the owner's words stay as a note pointing at it (migrations/0004).

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, JournalKind, Source, Verb};
use crate::time::{instant, parse_date};

const TEXT_MAX: usize = 4000;
const LIST_MAX: u32 = 200;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct JournalEntry {
    pub id: String,
    pub date: String,
    pub text: String,
    pub kind: JournalKind,
    pub source: Source,
    /// The task this note explains, if any.
    pub task_id: Option<String>,
    pub created_at: String,
}

/// A new entry, for the callers that need more than a date, words, and a kind.
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub date: String,
    pub text: String,
    pub kind: JournalKind,
    pub task_id: Option<String>,
}

pub fn create(conn: &mut Connection, date: &str, text: &str, kind: JournalKind, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let entry = NewEntry { date: date.into(), text: text.into(), kind, task_id: None };
    create_with(conn, &entry, actor, Source::from(actor), now)
}

/// An entry with every field, and where it came from (`Source::Import` for ULTRON's old notes).
pub fn create_with(conn: &mut Connection, entry: &NewEntry, actor: Actor, source: Source, now: Timestamp) -> Result<(String, Change)> {
    let tx = conn.transaction()?;
    let id = insert(&tx, entry, actor, source, now)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Journal, id)))
}

pub(crate) fn create_in(tx: &Transaction<'_>, date: &str, text: &str, kind: JournalKind, actor: Actor, now: Timestamp) -> Result<String> {
    let entry = NewEntry { date: date.into(), text: text.into(), kind, task_id: None };
    insert(tx, &entry, actor, Source::from(actor), now)
}

pub(crate) fn insert(tx: &Transaction<'_>, entry: &NewEntry, actor: Actor, source: Source, now: Timestamp) -> Result<String> {
    parse_date("date", &entry.date)?;
    let text = entry.text.trim();
    if text.is_empty() || text.chars().count() > TEXT_MAX {
        return invalid("text", Reason::Length);
    }
    let task_id = entry.task_id.as_deref().map(|id| live_task(tx, id)).transpose()?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO journal (id, date, text, kind, source, task_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, entry.date, text, entry.kind, source, task_id, stamp],
    )?;
    let summary: String = text.chars().take(80).collect();
    activity::record(
        tx,
        now,
        Record {
            actor,
            verb: Verb::Created,
            entity: Entity::Journal,
            entity_id: Some(&id),
            summary: &summary,
            details: serde_json::json!({ "kind": entry.kind }),
        },
    )?;
    Ok(id)
}

fn live_task(conn: &Connection, id: &str) -> Result<String> {
    let id = ids::parse("taskId", id)?;
    let found = conn.query_row("SELECT 1 FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [&id], |_| Ok(())).optional()?;
    if found.is_none() {
        return invalid("taskId", Reason::Unknown);
    }
    Ok(id)
}

const COLUMNS: &str = "id, date, text, kind, source, task_id, created_at";

fn entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<JournalEntry> {
    Ok(JournalEntry {
        id: row.get(0)?,
        date: row.get(1)?,
        text: row.get(2)?,
        kind: row.get(3)?,
        source: row.get(4)?,
        task_id: row.get(5)?,
        created_at: row.get(6)?,
    })
}

pub fn on(conn: &Connection, date: &str) -> Result<Vec<JournalEntry>> {
    parse_date("date", date)?;
    let sql = format!("SELECT {COLUMNS} FROM journal WHERE date = ?1 AND deleted_at IS NULL ORDER BY created_at");
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([date], entry)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn get(conn: &Connection, id: &str) -> Result<JournalEntry> {
    let id = ids::parse("id", id)?;
    let sql = format!("SELECT {COLUMNS} FROM journal WHERE id = ?1 AND deleted_at IS NULL");
    conn.query_row(&sql, [&id], entry).optional()?.ok_or(Error::NotFound("journal"))
}

/// The newest entries, of one kind or all.
pub fn recent(conn: &Connection, kind: Option<JournalKind>, limit: u32) -> Result<Vec<JournalEntry>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM journal WHERE deleted_at IS NULL AND (?1 IS NULL OR kind = ?1)
         ORDER BY date DESC, created_at DESC LIMIT ?2"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params![kind, limit.clamp(1, LIST_MAX)], entry)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The notes that explain a task.
pub fn for_task(conn: &Connection, task_id: &str) -> Result<Vec<JournalEntry>> {
    let task_id = ids::parse("taskId", task_id)?;
    let sql = format!("SELECT {COLUMNS} FROM journal WHERE task_id = ?1 AND deleted_at IS NULL ORDER BY created_at");
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([&task_id], entry)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Point a note at a task, or at none.
pub fn link_task(conn: &mut Connection, id: &str, task_id: Option<&str>, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let task_id = task_id.map(|task| live_task(&tx, task)).transpose()?;
    let text: Option<String> = tx
        .query_row("SELECT text FROM journal WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let text = text.ok_or(Error::NotFound("journal"))?;
    tx.execute("UPDATE journal SET task_id = ?2, updated_at = ?3 WHERE id = ?1", params![id, task_id, instant(now)])?;
    let summary: String = text.chars().take(80).collect();
    activity::record(
        &tx,
        now,
        Record {
            actor,
            verb: Verb::Updated,
            entity: Entity::Journal,
            entity_id: Some(&id),
            summary: &summary,
            details: serde_json::json!({ "taskId": task_id }),
        },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::Journal, id))
}

/// Remove a note (soft, like everything else Hub keeps).
pub fn delete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let text: Option<String> = tx
        .query_row("SELECT text FROM journal WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let text = text.ok_or(Error::NotFound("journal"))?;
    let stamp = instant(now);
    tx.execute("UPDATE journal SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    let summary: String = text.chars().take(80).collect();
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Deleted, entity: Entity::Journal, entity_id: Some(&id), summary: &summary, details: serde_json::json!({}) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::Journal, id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::tasks::{self, TaskInput};

    #[test]
    fn notes_are_kept_by_date() {
        let mut conn = open_in_memory().unwrap();
        let now: Timestamp = "2026-10-23T14:00:00Z".parse().unwrap();
        create(&mut conn, "2026-10-23", "Aku sudah membersihkan kamar", JournalKind::Done, Actor::Owner, now).unwrap();
        assert!(create(&mut conn, "2026-10-23", "   ", JournalKind::Note, Actor::Owner, now).is_err());
        let entries = on(&conn, "2026-10-23").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].kind, JournalKind::Done);
    }

    #[test]
    fn a_note_can_explain_a_task_and_be_removed() {
        let mut conn = open_in_memory().unwrap();
        let now: Timestamp = "2026-10-23T14:00:00Z".parse().unwrap();
        let (task, _) = tasks::create(&mut conn, TaskInput { title: "Tugas jaringan".into(), ..Default::default() }, Actor::Ultron, now).unwrap();
        let entry = NewEntry { date: "2026-10-23".into(), text: "File tugas jaringan".into(), kind: JournalKind::Note, task_id: Some(task.clone()) };
        let (id, _) = create_with(&mut conn, &entry, Actor::Ultron, Source::Import, now).unwrap();
        let kept = get(&conn, &id).unwrap();
        assert_eq!(kept.task_id.as_deref(), Some(task.as_str()));
        assert_eq!(kept.source, Source::Import);
        assert_eq!(for_task(&conn, &task).unwrap().len(), 1);

        let missing = NewEntry { task_id: Some(new_id()), ..entry };
        assert!(matches!(
            create_with(&mut conn, &missing, Actor::Ultron, Source::Ultron, now),
            Err(Error::Invalid { field: "taskId", .. })
        ));

        link_task(&mut conn, &id, None, Actor::Ultron, now).unwrap();
        assert!(for_task(&conn, &task).unwrap().is_empty());
        assert_eq!(recent(&conn, Some(JournalKind::Note), 10).unwrap().len(), 1);
        delete(&mut conn, &id, Actor::Ultron, now).unwrap();
        assert!(recent(&conn, None, 10).unwrap().is_empty());
        assert!(matches!(get(&conn, &id), Err(Error::NotFound(_))));
    }
}
