//! Jadwal: things at a time on the calendar, and time blocks (an event linked to a task).
//!
//! The UI thinks in the owner's day: a date and minutes from midnight. This module turns those
//! into instants in the owner's time zone, so a time block stays at 19.00 even if the zone in
//! settings changes how UTC lines up.

use jiff::civil::{Date, DateTime, Time};
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, Pigment, Source, Verb};
use crate::time::{date_text, instant, parse_date};

const TITLE_MAX: usize = 200;
const TEXT_MAX: usize = 2000;
/// A timed event may run past midnight, up to the end of the next day.
const MINUTE_MAX: i32 = 48 * 60;

#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct EventInput {
    pub title: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub all_day: bool,
    /// The (first) local date.
    pub date: String,
    /// All-day only: the last date, inclusive. Defaults to `date`.
    #[serde(default)]
    pub end_date: Option<String>,
    /// Timed only: minutes from local midnight of `date`. `end_minute` may pass 1440.
    #[serde(default)]
    pub start_minute: i32,
    #[serde(default)]
    pub end_minute: i32,
    #[serde(default)]
    pub pigment: Pigment,
    #[serde(default)]
    pub project_id: Option<String>,
    /// A time block for this task.
    #[serde(default)]
    pub task_id: Option<String>,
}

/// The NewEvent of CP2 (instants), kept for the seed and tests.
#[derive(Debug, Clone)]
pub struct NewEvent {
    pub title: String,
    pub location: String,
    pub start: Timestamp,
    pub end: Timestamp,
    pub pigment: Pigment,
    pub project_id: Option<String>,
}

/// A local date and minute (possibly past 1440) as an instant.
pub fn local_instant(date: Date, minute: i32, tz: &TimeZone) -> Result<Timestamp> {
    let start = DateTime::from_parts(date, Time::midnight());
    Ok(start.checked_add(Span::new().minutes(i64::from(minute)))?.to_zoned(tz.clone())?.timestamp())
}

enum When {
    Timed { start: Timestamp, end: Timestamp },
    AllDay { start: String, end: String },
}

struct Checked {
    title: String,
    location: String,
    notes: String,
    when: When,
    project_id: Option<String>,
    task_id: Option<String>,
}

fn check(input: &EventInput, tz: &TimeZone) -> Result<Checked> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > TITLE_MAX {
        return invalid("title", Reason::Length);
    }
    if input.location.chars().count() > TITLE_MAX {
        return invalid("location", Reason::Length);
    }
    if input.notes.chars().count() > TEXT_MAX {
        return invalid("notes", Reason::Length);
    }
    let date = parse_date("date", &input.date)?;
    let when = if input.all_day {
        let end = match &input.end_date {
            Some(end) => parse_date("endDate", end)?,
            None => date,
        };
        if end < date {
            return invalid("endDate", Reason::Order);
        }
        When::AllDay { start: date_text(date), end: date_text(end) }
    } else {
        if !(0..1440).contains(&input.start_minute) {
            return invalid("startMinute", Reason::Format);
        }
        if input.end_minute <= input.start_minute || input.end_minute > MINUTE_MAX {
            return invalid("endMinute", Reason::Order);
        }
        When::Timed {
            start: local_instant(date, input.start_minute, tz)?,
            end: local_instant(date, input.end_minute, tz)?,
        }
    };
    let parse = |field, value: &Option<String>| value.as_deref().map(|id| ids::parse(field, id)).transpose();
    Ok(Checked {
        title: title.to_string(),
        location: input.location.trim().to_string(),
        notes: input.notes.trim_end().to_string(),
        when,
        project_id: parse("projectId", &input.project_id)?,
        task_id: parse("taskId", &input.task_id)?,
    })
}

fn check_links(tx: &Transaction<'_>, checked: &Checked) -> Result<()> {
    for (field, table, id) in [("projectId", "projects", &checked.project_id), ("taskId", "tasks", &checked.task_id)] {
        if let Some(id) = id {
            let sql = format!("SELECT 1 FROM {table} WHERE id = ?1 AND deleted_at IS NULL");
            if tx.query_row(&sql, [id], |_| Ok(())).optional()?.is_none() {
                return invalid(field, Reason::Unknown);
            }
        }
    }
    Ok(())
}

fn columns(when: &When) -> (bool, Option<String>, Option<String>, Option<String>, Option<String>) {
    match when {
        When::Timed { start, end } => (false, Some(instant(*start)), Some(instant(*end)), None, None),
        When::AllDay { start, end } => (true, None, None, Some(start.clone()), Some(end.clone())),
    }
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, id: &str, title: &str) -> Result<()> {
    activity::record(
        tx,
        now,
        Record { actor, verb, entity: Entity::Event, entity_id: Some(id), summary: title, details: serde_json::json!({}) },
    )
}

pub fn create(conn: &mut Connection, input: EventInput, tz: &TimeZone, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let checked = check(&input, tz)?;
    let tx = conn.transaction()?;
    check_links(&tx, &checked)?;
    let id = new_id();
    let (all_day, start_at, end_at, start_date, end_date) = columns(&checked.when);
    tx.execute(
        "INSERT INTO events (id, title, notes, location, all_day, start_at, end_at, start_date, end_date,
                             pigment, project_id, task_id, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
        params![
            id,
            checked.title,
            checked.notes,
            checked.location,
            all_day,
            start_at,
            end_at,
            start_date,
            end_date,
            input.pigment,
            checked.project_id,
            checked.task_id,
            Source::from(actor),
            instant(now),
        ],
    )?;
    record(&tx, now, actor, Verb::Created, &id, &checked.title)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Event, id)))
}

/// Replace an event's fields (title, time, place, colour, links).
pub fn update(conn: &mut Connection, id: &str, input: EventInput, tz: &TimeZone, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let checked = check(&input, tz)?;
    let tx = conn.transaction()?;
    check_links(&tx, &checked)?;
    let (all_day, start_at, end_at, start_date, end_date) = columns(&checked.when);
    let changed = tx.execute(
        "UPDATE events SET title = ?2, notes = ?3, location = ?4, all_day = ?5, start_at = ?6, end_at = ?7,
                start_date = ?8, end_date = ?9, pigment = ?10, project_id = ?11, task_id = ?12, updated_at = ?13
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            id,
            checked.title,
            checked.notes,
            checked.location,
            all_day,
            start_at,
            end_at,
            start_date,
            end_date,
            input.pigment,
            checked.project_id,
            checked.task_id,
            instant(now),
        ],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("event"));
    }
    record(&tx, now, actor, Verb::Updated, &id, &checked.title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Event, id))
}

/// A place in the owner's day: a local date and minutes from its midnight (the end may pass 1440).
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Slot {
    pub date: String,
    pub start_minute: i32,
    pub end_minute: i32,
}

impl Slot {
    /// Checked, as two instants.
    pub fn instants(&self, tz: &TimeZone) -> Result<(Timestamp, Timestamp)> {
        let day = parse_date("date", &self.date)?;
        if !(0..1440).contains(&self.start_minute) {
            return invalid("startMinute", Reason::Format);
        }
        if self.end_minute <= self.start_minute || self.end_minute > MINUTE_MAX {
            return invalid("endMinute", Reason::Order);
        }
        Ok((local_instant(day, self.start_minute, tz)?, local_instant(day, self.end_minute, tz)?))
    }
}

/// Move or resize a timed event (a drag on the calendar): only its time changes.
pub fn reschedule(conn: &mut Connection, id: &str, slot: &Slot, tz: &TimeZone, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let (start, end) = slot.instants(tz)?;
    let tx = conn.transaction()?;
    let title: Option<String> = tx
        .query_row("SELECT title FROM events WHERE id = ?1 AND deleted_at IS NULL AND all_day = 0", [&id], |row| row.get(0))
        .optional()?;
    let title = title.ok_or(Error::NotFound("event"))?;
    tx.execute(
        "UPDATE events SET start_at = ?2, end_at = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, instant(start), instant(end), instant(now)],
    )?;
    activity::record(
        &tx,
        now,
        Record {
            actor,
            verb: Verb::Moved,
            entity: Entity::Event,
            entity_id: Some(&id),
            summary: &title,
            details: serde_json::json!({ "start": instant(start), "end": instant(end) }),
        },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::Event, id))
}

pub fn delete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let title: Option<String> = tx
        .query_row("SELECT title FROM events WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let title = title.ok_or(Error::NotFound("event"))?;
    let stamp = instant(now);
    tx.execute("UPDATE events SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    record(&tx, now, actor, Verb::Deleted, &id, &title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Event, id))
}

pub fn restore(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let title: Option<String> = tx
        .query_row("SELECT title FROM events WHERE id = ?1 AND deleted_at IS NOT NULL", [&id], |row| row.get(0))
        .optional()?;
    let title = title.ok_or(Error::NotFound("event"))?;
    tx.execute("UPDATE events SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1", params![id, instant(now)])?;
    record(&tx, now, actor, Verb::Restored, &id, &title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Event, id))
}

/// An event as its editor needs it: local date and minutes (the end may pass 1440).
pub fn get(conn: &Connection, id: &str, tz: &TimeZone) -> Result<EventInput> {
    let id = ids::parse("id", id)?;
    let row = conn
        .query_row(
            "SELECT title, location, notes, all_day, start_at, end_at, start_date, end_date, pigment, project_id, task_id
             FROM events WHERE id = ?1 AND deleted_at IS NULL",
            [&id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Pigment>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                ))
            },
        )
        .optional()?
        .ok_or(Error::NotFound("event"))?;
    let (title, location, notes, all_day, start_at, end_at, start_date, end_date, pigment, project_id, task_id) = row;
    let mut input = EventInput { title, location, notes, all_day, pigment, project_id, task_id, ..Default::default() };
    if all_day {
        input.date = start_date.unwrap_or_default();
        input.end_date = end_date;
    } else {
        let start = crate::time::parse_instant("startAt", start_at.as_deref().unwrap_or_default())?;
        let end = crate::time::parse_instant("endAt", end_at.as_deref().unwrap_or_default())?;
        let day = crate::time::Day::containing(start, tz)?;
        input.date = date_text(day.date);
        input.start_minute = day.minute(start);
        input.end_minute = ((end.as_second() - day.start.as_second()) / 60) as i32;
    }
    Ok(input)
}

/// CP2's instant-based creation, used by the demo seed.
pub fn create_timed(conn: &mut Connection, new: NewEvent, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let title = new.title.trim();
    if title.is_empty() || title.chars().count() > TITLE_MAX {
        return invalid("title", Reason::Length);
    }
    if new.end <= new.start {
        return invalid("end", Reason::Order);
    }
    let tx = conn.transaction()?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO events (id, title, location, all_day, start_at, end_at, pigment, project_id,
                             source, created_at, updated_at)
         VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![id, title, new.location.trim(), instant(new.start), instant(new.end), new.pigment, new.project_id, Source::from(actor), stamp],
    )?;
    record(&tx, now, actor, Verb::Created, &id, title)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Event, id)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn tz() -> TimeZone {
        TimeZone::get("Asia/Jakarta").unwrap()
    }

    fn now() -> Timestamp {
        "2026-10-23T03:00:00Z".parse().unwrap()
    }

    fn timed(title: &str, date: &str, start: i32, end: i32) -> EventInput {
        EventInput { title: title.into(), date: date.into(), start_minute: start, end_minute: end, ..Default::default() }
    }

    #[test]
    fn local_minutes_become_instants_in_the_owners_zone() {
        let mut conn = open_in_memory().unwrap();
        let (id, _) = create(&mut conn, timed("Rapat OSIS", "2026-10-23", 13 * 60, 14 * 60 + 30), &tz(), Actor::Owner, now()).unwrap();
        let (start, end): (String, String) =
            conn.query_row("SELECT start_at, end_at FROM events WHERE id = ?1", [&id], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!((start.as_str(), end.as_str()), ("2026-10-23T06:00:00Z", "2026-10-23T07:30:00Z"));
        reschedule(&mut conn, &id, &Slot { date: "2026-10-24".into(), start_minute: 23 * 60, end_minute: 25 * 60 }, &tz(), Actor::Owner, now()).unwrap();
        let back = get(&conn, &id, &tz()).unwrap();
        assert_eq!((back.date.as_str(), back.start_minute, back.end_minute), ("2026-10-24", 23 * 60, 25 * 60));
        let end: String = conn.query_row("SELECT end_at FROM events WHERE id = ?1", [&id], |row| row.get(0)).unwrap();
        assert_eq!(end, "2026-10-24T18:00:00Z", "past midnight, 01.00 WIB on the 25th");
    }

    #[test]
    fn bad_times_and_links_are_refused() {
        let mut conn = open_in_memory().unwrap();
        let bad = [
            timed("x", "2026-10-23", 600, 600),
            timed("x", "2026-10-23", -5, 60),
            timed("x", "2026-10-23", 1440, 1500),
            timed("x", "23/10/2026", 600, 660),
            timed(" ", "2026-10-23", 600, 660),
            EventInput { task_id: Some(new_id()), ..timed("x", "2026-10-23", 600, 660) },
            EventInput { all_day: true, end_date: Some("2026-10-22".into()), ..timed("x", "2026-10-23", 0, 0) },
        ];
        for input in bad {
            assert!(create(&mut conn, input, &tz(), Actor::Owner, now()).is_err());
        }
        let count: i64 = conn.query_row("SELECT count(*) FROM events", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn all_day_events_keep_dates_and_removal_can_be_undone() {
        let mut conn = open_in_memory().unwrap();
        let input = EventInput { title: "Ujian tengah semester".into(), all_day: true, date: "2026-10-26".into(), end_date: Some("2026-10-30".into()), ..Default::default() };
        let (id, _) = create(&mut conn, input, &tz(), Actor::Owner, now()).unwrap();
        let dates: (String, String) =
            conn.query_row("SELECT start_date, end_date FROM events WHERE id = ?1", [&id], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
        assert_eq!(dates, ("2026-10-26".to_string(), "2026-10-30".to_string()));
        assert!(reschedule(&mut conn, &id, &Slot { date: "2026-10-26".into(), start_minute: 60, end_minute: 120 }, &tz(), Actor::Owner, now()).is_err(), "all-day events are not dragged in time");
        delete(&mut conn, &id, Actor::Owner, now()).unwrap();
        restore(&mut conn, &id, Actor::Owner, now()).unwrap();
        assert!(delete(&mut conn, &id, Actor::Owner, now()).is_ok());
    }
}
