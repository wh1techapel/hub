//! Pengingat: one-time alerts at a time. Never a task by itself (a reminder may point at one).
//!
//! States: `scheduled` until due, `fired` once Hub has said so (until the owner acts), `snoozed`
//! with a new due time, then `done` or `cancelled`. The scheduler in the app asks `next_due` and
//! calls `fire_due`; everything else here is the owner's (or, in CP5, ULTRON's) doing.

use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::events::local_instant;
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, ReminderState, Source, Verb};
use crate::time::{instant, parse_date, parse_instant};

const TEXT_MAX: usize = 300;
/// Longer than this past its time when Hub fires it: "while you were away".
const LATE_AFTER_SECONDS: i64 = 120;
const SNOOZE_MAX_MINUTES: u32 = 24 * 60;

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ReminderInput {
    pub text: String,
    /// Local date and minute from its midnight.
    pub date: String,
    pub minute: i32,
    #[serde(default)]
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReminderItem {
    pub id: String,
    pub text: String,
    pub due_at: String,
    pub state: ReminderState,
    pub fired_at: Option<String>,
    pub task_id: Option<String>,
    pub source: Source,
}

/// A reminder the scheduler just fired.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Fired {
    pub id: String,
    pub text: String,
    pub due_at: String,
    /// It came due while Hub was not running (or the computer slept).
    pub late: bool,
}

fn checked_text(text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > TEXT_MAX {
        return invalid("text", Reason::Length);
    }
    Ok(text.to_string())
}

fn due_of(input: &ReminderInput, tz: &TimeZone) -> Result<Timestamp> {
    let date = parse_date("date", &input.date)?;
    if !(0..1440).contains(&input.minute) {
        return invalid("minute", Reason::Format);
    }
    local_instant(date, input.minute, tz)
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, id: &str, text: &str) -> Result<()> {
    activity::record(
        tx,
        now,
        Record { actor, verb, entity: Entity::Reminder, entity_id: Some(id), summary: text, details: serde_json::json!({}) },
    )
}

/// A reminder at an instant.
pub fn create(conn: &mut Connection, text: &str, due: Timestamp, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    insert(conn, text, due, None, actor, Source::from(actor), now)
}

/// A reminder at an instant, maybe for a task, from a given source (the API: ULTRON, or an import
/// of ULTRON's old reminders).
pub fn create_as(
    conn: &mut Connection,
    text: &str,
    due: Timestamp,
    task_id: Option<&str>,
    actor: Actor,
    source: Source,
    now: Timestamp,
) -> Result<(String, Change)> {
    let task = task_id.map(|id| ids::parse("taskId", id)).transpose()?;
    insert(conn, text, due, task, actor, source, now)
}

/// A reminder at a local date and time.
pub fn create_at(conn: &mut Connection, input: ReminderInput, tz: &TimeZone, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let due = due_of(&input, tz)?;
    let task = input.task_id.as_deref().map(|id| ids::parse("taskId", id)).transpose()?;
    insert(conn, &input.text, due, task, actor, Source::from(actor), now)
}

/// New words, a new instant, or both (the API); what is not given stays. Only a reminder still
/// to come or waiting for the owner can change. One moved into the future is scheduled again.
pub fn change(conn: &mut Connection, id: &str, text: Option<&str>, due: Option<Timestamp>, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let text = text.map(checked_text).transpose()?;
    let tx = conn.transaction()?;
    let row: Option<(String, String, ReminderState)> = tx
        .query_row("SELECT text, due_at, state FROM reminders WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .optional()?;
    let (old_text, old_due, state) = row.ok_or(Error::NotFound("reminder"))?;
    if matches!(state, ReminderState::Done | ReminderState::Cancelled) {
        return invalid("state", Reason::Order);
    }
    let due = match due {
        Some(due) => due,
        None => parse_instant("dueAt", &old_due)?,
    };
    let text = text.unwrap_or(old_text);
    let next = if due > now { ReminderState::Scheduled } else { state };
    tx.execute(
        "UPDATE reminders SET text = ?2, due_at = ?3, state = ?4, fired_at = CASE WHEN ?4 = 'scheduled' THEN NULL ELSE fired_at END,
                updated_at = ?5 WHERE id = ?1",
        params![id, text, instant(due), next, instant(now)],
    )?;
    record(&tx, now, actor, Verb::Updated, &id, &text)?;
    tx.commit()?;
    Ok(Change::one(Entity::Reminder, id))
}

/// One reminder, whatever its state.
pub fn get(conn: &Connection, id: &str) -> Result<ReminderItem> {
    let id = ids::parse("id", id)?;
    let sql = format!("SELECT {ITEM_COLUMNS} FROM reminders WHERE id = ?1 AND deleted_at IS NULL");
    conn.query_row(&sql, [&id], item).optional()?.ok_or(Error::NotFound("reminder"))
}

fn insert(
    conn: &mut Connection,
    text: &str,
    due: Timestamp,
    task: Option<String>,
    actor: Actor,
    source: Source,
    now: Timestamp,
) -> Result<(String, Change)> {
    let text = checked_text(text)?;
    let tx = conn.transaction()?;
    if let Some(task) = &task {
        let found = tx.query_row("SELECT 1 FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [task], |_| Ok(())).optional()?;
        if found.is_none() {
            return invalid("taskId", Reason::Unknown);
        }
    }
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO reminders (id, text, due_at, task_id, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![id, text, instant(due), task, source, stamp],
    )?;
    record(&tx, now, actor, Verb::Created, &id, &text)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Reminder, id)))
}

/// New words or a new time. A reminder moved into the future is scheduled again.
pub fn update(conn: &mut Connection, id: &str, input: ReminderInput, tz: &TimeZone, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let text = checked_text(&input.text)?;
    let due = due_of(&input, tz)?;
    let tx = conn.transaction()?;
    let state: Option<ReminderState> = tx
        .query_row("SELECT state FROM reminders WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let state = state.ok_or(Error::NotFound("reminder"))?;
    let next = if due > now && matches!(state, ReminderState::Fired | ReminderState::Snoozed | ReminderState::Scheduled) {
        ReminderState::Scheduled
    } else {
        state
    };
    tx.execute(
        "UPDATE reminders SET text = ?2, due_at = ?3, state = ?4, updated_at = ?5 WHERE id = ?1",
        params![id, text, instant(due), next, instant(now)],
    )?;
    record(&tx, now, actor, Verb::Updated, &id, &text)?;
    tx.commit()?;
    Ok(Change::one(Entity::Reminder, id))
}

fn transition(
    conn: &mut Connection,
    id: &str,
    actor: Actor,
    now: Timestamp,
    verb: Verb,
    apply: impl FnOnce(&Transaction<'_>, &str, ReminderState) -> Result<bool>,
) -> Result<Option<Change>> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let row: Option<(String, ReminderState)> = tx
        .query_row("SELECT text, state FROM reminders WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    let (text, state) = row.ok_or(Error::NotFound("reminder"))?;
    if !apply(&tx, &id, state)? {
        return Ok(None);
    }
    record(&tx, now, actor, verb, &id, &text)?;
    tx.commit()?;
    Ok(Some(Change::one(Entity::Reminder, id)))
}

/// Handled ("Selesai"). Done or cancelled reminders stay as they are.
pub fn done(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    let stamp = instant(now);
    transition(conn, id, actor, now, Verb::Completed, |tx, id, state| {
        if matches!(state, ReminderState::Done | ReminderState::Cancelled) {
            return Ok(false);
        }
        tx.execute("UPDATE reminders SET state = 'done', updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
        Ok(true)
    })
}

/// No longer wanted.
pub fn cancel(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    let stamp = instant(now);
    transition(conn, id, actor, now, Verb::Cancelled, |tx, id, state| {
        if matches!(state, ReminderState::Done | ReminderState::Cancelled) {
            return Ok(false);
        }
        tx.execute("UPDATE reminders SET state = 'cancelled', updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
        Ok(true)
    })
}

/// Again in `minutes` from now ("Tunda 10 m").
pub fn snooze(conn: &mut Connection, id: &str, minutes: u32, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    if minutes == 0 || minutes > SNOOZE_MAX_MINUTES {
        return invalid("minutes", Reason::Format);
    }
    let due = now.checked_add(Span::new().minutes(i64::from(minutes)))?;
    let stamp = instant(now);
    transition(conn, id, actor, now, Verb::Snoozed, |tx, id, state| {
        if matches!(state, ReminderState::Done | ReminderState::Cancelled) {
            return Ok(false);
        }
        tx.execute(
            "UPDATE reminders SET state = 'snoozed', due_at = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, instant(due), stamp],
        )?;
        Ok(true)
    })
}

/// When the next reminder comes due, if any.
pub fn next_due(conn: &Connection) -> Result<Option<Timestamp>> {
    let due: Option<String> = conn.query_row(
        "SELECT min(due_at) FROM reminders WHERE deleted_at IS NULL AND state IN ('scheduled', 'snoozed')",
        [],
        |row| row.get(0),
    )?;
    due.map(|due| parse_instant("dueAt", &due)).transpose()
}

/// Fire everything due by `now`, oldest first, each with an activity row by Hub.
pub fn fire_due(conn: &mut Connection, now: Timestamp) -> Result<Vec<Fired>> {
    let tx = conn.transaction()?;
    let due: Vec<(String, String, String)> = {
        let mut statement = tx.prepare(
            "SELECT id, text, due_at FROM reminders
             WHERE deleted_at IS NULL AND state IN ('scheduled', 'snoozed') AND due_at <= ?1 ORDER BY due_at",
        )?;
        statement
            .query_map([instant(now)], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<rusqlite::Result<_>>()?
    };
    let mut fired = Vec::new();
    for (id, text, due_at) in due {
        tx.execute(
            "UPDATE reminders SET state = 'fired', fired_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, instant(now)],
        )?;
        record(&tx, now, Actor::Hub, Verb::Reminded, &id, &text)?;
        let late = now.as_second() - parse_instant("dueAt", &due_at)?.as_second() > LATE_AFTER_SECONDS;
        fired.push(Fired { id, text, due_at, late });
    }
    tx.commit()?;
    Ok(fired)
}

const ITEM_COLUMNS: &str = "id, text, due_at, state, fired_at, task_id, source";

fn item(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReminderItem> {
    Ok(ReminderItem {
        id: row.get(0)?,
        text: row.get(1)?,
        due_at: row.get(2)?,
        state: row.get(3)?,
        fired_at: row.get(4)?,
        task_id: row.get(5)?,
        source: row.get(6)?,
    })
}

/// Reminders still to act on (fired) and still to come, by time; then the last week's handled ones.
pub fn list(conn: &Connection, now: Timestamp) -> Result<Vec<ReminderItem>> {
    let week_ago = now.checked_sub(Span::new().hours(24 * 7))?;
    let sql = format!(
        "SELECT {ITEM_COLUMNS} FROM reminders
         WHERE deleted_at IS NULL AND (state IN ('scheduled', 'snoozed', 'fired')
               OR (state IN ('done', 'cancelled') AND updated_at >= ?1))
         ORDER BY CASE state WHEN 'fired' THEN 0 WHEN 'scheduled' THEN 1 WHEN 'snoozed' THEN 1 ELSE 2 END,
                  CASE WHEN state IN ('done', 'cancelled') THEN NULL ELSE due_at END, updated_at DESC"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([instant(week_ago)], item)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Fired and not yet handled: what the window shows as alerts.
pub fn alerts(conn: &Connection) -> Result<Vec<ReminderItem>> {
    let sql = format!("SELECT {ITEM_COLUMNS} FROM reminders WHERE deleted_at IS NULL AND state = 'fired' ORDER BY due_at");
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([], item)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    fn tz() -> TimeZone {
        TimeZone::get("Asia/Jakarta").unwrap()
    }

    #[test]
    fn a_reminder_fires_once_and_waits_for_the_owner() {
        let mut conn = open_in_memory().unwrap();
        let now = at("2026-10-23T03:00:00Z");
        let input = ReminderInput { text: "Kumpulkan laporan".into(), date: "2026-10-23".into(), minute: 21 * 60, task_id: None };
        let (id, _) = create_at(&mut conn, input, &tz(), Actor::Owner, now).unwrap();
        assert_eq!(next_due(&conn).unwrap(), Some(at("2026-10-23T14:00:00Z")));
        assert!(fire_due(&mut conn, at("2026-10-23T13:59:59Z")).unwrap().is_empty());

        let fired = fire_due(&mut conn, at("2026-10-23T14:00:30Z")).unwrap();
        assert_eq!(fired.len(), 1);
        assert!(!fired[0].late);
        assert!(fire_due(&mut conn, at("2026-10-23T14:05:00Z")).unwrap().is_empty(), "fired once");
        assert_eq!(next_due(&conn).unwrap(), None);
        assert_eq!(alerts(&conn).unwrap().len(), 1);

        snooze(&mut conn, &id, 10, Actor::Owner, at("2026-10-23T14:01:00Z")).unwrap();
        assert!(alerts(&conn).unwrap().is_empty());
        assert_eq!(next_due(&conn).unwrap(), Some(at("2026-10-23T14:11:00Z")));
        let fired = fire_due(&mut conn, at("2026-10-23T14:11:00Z")).unwrap();
        assert_eq!(fired.len(), 1);
        done(&mut conn, &id, Actor::Owner, at("2026-10-23T14:12:00Z")).unwrap();
        assert!(done(&mut conn, &id, Actor::Owner, at("2026-10-23T14:12:00Z")).unwrap().is_none());
        assert!(snooze(&mut conn, &id, 10, Actor::Owner, at("2026-10-23T14:13:00Z")).unwrap().is_none(), "done stays done");
        assert!(alerts(&conn).unwrap().is_empty());
        let listed = list(&conn, at("2026-10-23T15:00:00Z")).unwrap();
        assert_eq!(listed[0].state, ReminderState::Done);
    }

    #[test]
    fn missed_reminders_fire_late_on_start() {
        let mut conn = open_in_memory().unwrap();
        create(&mut conn, "Minum obat", at("2026-10-23T01:00:00Z"), Actor::Owner, at("2026-10-22T00:00:00Z")).unwrap();
        create(&mut conn, "Telepon ibu", at("2026-10-23T02:00:00Z"), Actor::Owner, at("2026-10-22T00:00:00Z")).unwrap();
        let fired = fire_due(&mut conn, at("2026-10-23T06:00:00Z")).unwrap();
        let texts: Vec<(&str, bool)> = fired.iter().map(|f| (f.text.as_str(), f.late)).collect();
        assert_eq!(texts, [("Minum obat", true), ("Telepon ibu", true)]);
        let recent = crate::activity::recent(&conn, 10, false).unwrap();
        assert!(recent.iter().any(|item| item.actor == Actor::Hub && item.verb == Verb::Reminded));
    }

    #[test]
    fn a_fired_reminder_moved_to_later_is_scheduled_again() {
        let mut conn = open_in_memory().unwrap();
        let now = at("2026-10-23T03:00:00Z");
        let (id, _) = create(&mut conn, "Belajar", at("2026-10-23T02:00:00Z"), Actor::Owner, now).unwrap();
        fire_due(&mut conn, now).unwrap();
        let input = ReminderInput { text: "Belajar subnetting".into(), date: "2026-10-23".into(), minute: 19 * 60, task_id: None };
        update(&mut conn, &id, input, &tz(), Actor::Owner, now).unwrap();
        assert!(alerts(&conn).unwrap().is_empty());
        assert_eq!(next_due(&conn).unwrap(), Some(at("2026-10-23T12:00:00Z")));
        cancel(&mut conn, &id, Actor::Owner, now).unwrap();
        assert_eq!(next_due(&conn).unwrap(), None);
    }

    #[test]
    fn input_is_checked() {
        let mut conn = open_in_memory().unwrap();
        let now = at("2026-10-23T03:00:00Z");
        let bad = [
            ReminderInput { text: " ".into(), date: "2026-10-23".into(), minute: 60, task_id: None },
            ReminderInput { text: "x".into(), date: "besok".into(), minute: 60, task_id: None },
            ReminderInput { text: "x".into(), date: "2026-10-23".into(), minute: 1440, task_id: None },
            ReminderInput { text: "x".into(), date: "2026-10-23".into(), minute: 60, task_id: Some(new_id()) },
        ];
        for input in bad {
            assert!(create_at(&mut conn, input, &tz(), Actor::Owner, now).is_err());
        }
        let (id, _) = create(&mut conn, "x", now, Actor::Owner, now).unwrap();
        assert!(snooze(&mut conn, &id, 0, Actor::Owner, now).is_err());
    }
}
