//! Belajar: tracks (TryHackMe, Hack The Box, CTF, Lab Sekolah, Homelab), the items learned in
//! them, the time spent (sessions, timed or logged by hand), CTF events, and the week against the
//! owner's goal in active days (CONTEXT.md, "Belajar goal"). Nothing resets: a missed day is only a
//! lighter square in the heatmap.

use std::collections::{BTreeMap, HashMap, HashSet};

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::events::local_instant;
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Difficulty, Domain, Entity, LearningKind, LearningStatus, Pigment, Source, TrackKind, Verb};
use crate::order;
use crate::settings;
use crate::time::{date_text, instant, parse_date, parse_instant};

const TITLE_MAX: usize = 200;
const NOTES_MAX: usize = 20_000;
const URL_MAX: usize = 2000;
/// A timer left running longer than this (forgotten overnight) counts this much.
const TIMER_CAP_MIN: i64 = 8 * 60;
/// The heatmap: 20 weeks, Monday to Sunday.
pub const HEATMAP_WEEKS: i64 = 20;

fn record(tx: &Connection, now: Timestamp, actor: Actor, verb: Verb, entity: Entity, id: &str, summary: &str) -> Result<()> {
    activity::record(tx, now, Record { actor, verb, entity, entity_id: Some(id), summary, details: serde_json::json!({}) })
}

fn checked_text(field: &'static str, text: &str, max: usize) -> Result<String> {
    let text = text.trim();
    if text.is_empty() || text.chars().count() > max {
        return invalid(field, Reason::Length);
    }
    Ok(text.to_string())
}

/// Only web links: a learning item points at a room or a challenge page.
fn checked_url(url: Option<&str>) -> Result<Option<String>> {
    let Some(url) = url.map(str::trim).filter(|u| !u.is_empty()) else { return Ok(None) };
    let lower = url.to_ascii_lowercase();
    if url.chars().count() > URL_MAX || !(lower.starts_with("https://") || lower.starts_with("http://")) || url.contains(char::is_whitespace) {
        return invalid("url", Reason::Format);
    }
    Ok(Some(url.to_string()))
}

fn local_date(ts: Timestamp, tz: &TimeZone) -> Date {
    ts.to_zoned(tz.clone()).date()
}

fn monday_of(date: Date) -> Result<Date> {
    let back = i64::from(date.weekday().to_monday_zero_offset());
    Ok(date.checked_sub(back.days())?)
}

// ---------- tracks ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Track {
    pub id: String,
    pub name: String,
    pub kind: TrackKind,
    pub url: Option<String>,
    pub pigment: Pigment,
    pub items: u32,
    pub done: u32,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TrackInput {
    pub name: String,
    pub kind: TrackKind,
    #[serde(default)]
    pub url: Option<String>,
    pub pigment: Pigment,
}

pub fn tracks(conn: &Connection) -> Result<Vec<Track>> {
    let mut statement = conn.prepare(
        "SELECT t.id, t.name, t.kind, t.url, t.pigment,
                (SELECT count(*) FROM learning_items i WHERE i.track_id = t.id AND i.deleted_at IS NULL),
                (SELECT count(*) FROM learning_items i WHERE i.track_id = t.id AND i.deleted_at IS NULL AND i.status = 'done')
         FROM tracks t WHERE t.deleted_at IS NULL ORDER BY t.order_key",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(Track {
            id: row.get(0)?,
            name: row.get(1)?,
            kind: row.get(2)?,
            url: row.get(3)?,
            pigment: row.get(4)?,
            items: row.get(5)?,
            done: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn create_track(conn: &mut Connection, input: TrackInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let name = checked_text("name", &input.name, 80)?;
    let url = checked_url(input.url.as_deref())?;
    let tx = conn.transaction()?;
    let id = insert_track(&tx, &name, input.kind, url.as_deref(), input.pigment, now)?;
    record(&tx, now, actor, Verb::Created, Entity::Track, &id, &name)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Track, id)))
}

fn insert_track(tx: &Connection, name: &str, kind: TrackKind, url: Option<&str>, pigment: Pigment, now: Timestamp) -> Result<String> {
    let last: Option<String> = tx.query_row("SELECT max(order_key) FROM tracks", [], |row| row.get(0))?;
    let id = new_id();
    tx.execute(
        "INSERT INTO tracks (id, name, kind, url, pigment, order_key, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, name, kind, url, pigment, order::after(last.as_deref()), instant(now)],
    )?;
    Ok(id)
}

pub fn update_track(conn: &mut Connection, id: &str, input: TrackInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let name = checked_text("name", &input.name, 80)?;
    let url = checked_url(input.url.as_deref())?;
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE tracks SET name = ?2, kind = ?3, url = ?4, pigment = ?5, updated_at = ?6 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, name, input.kind, url, input.pigment, instant(now)],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("track"));
    }
    record(&tx, now, actor, Verb::Updated, Entity::Track, &id, &name)?;
    tx.commit()?;
    Ok(Change::one(Entity::Track, id))
}

/// Remove a track and, with it, its items (all soft, so Urungkan brings them back).
pub fn delete_track(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    set_track_deleted(conn, id, true, actor, now)
}

pub fn restore_track(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    set_track_deleted(conn, id, false, actor, now)
}

fn set_track_deleted(conn: &mut Connection, id: &str, deleted: bool, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let (name, was): (String, Option<String>) = tx
        .query_row("SELECT name, deleted_at FROM tracks WHERE id = ?1", [&id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?
        .ok_or(Error::NotFound("track"))?;
    let stamp = instant(now);
    if deleted && was.is_none() {
        tx.execute("UPDATE tracks SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
        tx.execute("UPDATE learning_items SET deleted_at = ?2, updated_at = ?2 WHERE track_id = ?1 AND deleted_at IS NULL", params![id, stamp])?;
        record(&tx, now, actor, Verb::Deleted, Entity::Track, &id, &name)?;
    } else if !deleted && let Some(was) = was {
        tx.execute("UPDATE tracks SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
        tx.execute("UPDATE learning_items SET deleted_at = NULL, updated_at = ?2 WHERE track_id = ?1 AND deleted_at = ?3", params![id, stamp, was])?;
        record(&tx, now, actor, Verb::Restored, Entity::Track, &id, &name)?;
    }
    tx.commit()?;
    Ok(Change::all(Entity::Track))
}

/// The tracks the owner named (CONTEXT.md, "Cyber learning"), made once on the first run.
pub fn ensure_default_tracks(conn: &mut Connection, now: Timestamp) -> Result<bool> {
    if settings::is_stored(conn, "tracks_seeded")? {
        return Ok(false);
    }
    let tx = conn.transaction()?;
    let any: i64 = tx.query_row("SELECT count(*) FROM tracks", [], |row| row.get(0))?;
    if any == 0 {
        for (name, kind, url, pigment) in [
            ("TryHackMe", TrackKind::Platform, Some("https://tryhackme.com"), Pigment::Lumut),
            ("Hack The Box", TrackKind::Platform, Some("https://app.hackthebox.com"), Pigment::Toska),
            ("CTF", TrackKind::Ctf, None, Pigment::Terakota),
            ("Lab Sekolah", TrackKind::Lab, None, Pigment::Lazuardi),
            ("Homelab", TrackKind::Lab, None, Pigment::Kunyit),
        ] {
            insert_track(&tx, name, kind, url, pigment, now)?;
        }
    }
    settings::store_initial(&tx, "tracks_seeded", serde_json::json!(true), now)?;
    tx.commit()?;
    Ok(any == 0)
}

// ---------- items ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LearningCard {
    pub id: String,
    pub track_id: String,
    pub track_name: String,
    pub pigment: Pigment,
    pub ctf_event_id: Option<String>,
    pub title: String,
    pub kind: LearningKind,
    pub domain: Domain,
    pub difficulty: Option<Difficulty>,
    pub status: LearningStatus,
    pub url: Option<String>,
    pub points: Option<u32>,
    pub progress: Option<u8>,
    pub notes: String,
    /// All the time logged on it.
    pub minutes: u32,
    pub sessions: u32,
    pub last_session_at: Option<String>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub files: u32,
    pub source: Source,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
#[ts(export)]
pub struct LearningInput {
    pub track_id: String,
    pub title: String,
    pub kind: Option<LearningKind>,
    pub domain: Option<Domain>,
    pub difficulty: Option<Difficulty>,
    pub status: Option<LearningStatus>,
    pub url: Option<String>,
    pub points: Option<u32>,
    pub progress: Option<u8>,
    pub notes: String,
    pub ctf_event_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct LearningQuery {
    #[ts(optional)]
    pub track_id: Option<String>,
    #[ts(optional)]
    pub status: Option<LearningStatus>,
    #[ts(optional)]
    pub ctf_event_id: Option<String>,
}

const CARD_SQL: &str = "SELECT i.id, i.track_id, t.name, t.pigment, i.ctf_event_id, i.title, i.kind, i.domain, i.difficulty, i.status,
        i.url, i.points, i.progress, i.notes,
        coalesce((SELECT sum(s.duration_min) FROM learning_sessions s WHERE s.item_id = i.id), 0),
        (SELECT count(*) FROM learning_sessions s WHERE s.item_id = i.id),
        (SELECT max(s.started_at) FROM learning_sessions s WHERE s.item_id = i.id),
        i.started_at, i.completed_at,
        (SELECT count(*) FROM file_links l JOIN files f ON f.id = l.file_id AND f.deleted_at IS NULL
          WHERE l.target_type = 'learning_item' AND l.target_id = i.id),
        i.source
     FROM learning_items i JOIN tracks t ON t.id = i.track_id";

fn card_from(row: &rusqlite::Row<'_>) -> rusqlite::Result<LearningCard> {
    let domain: String = row.get(7)?;
    Ok(LearningCard {
        id: row.get(0)?,
        track_id: row.get(1)?,
        track_name: row.get(2)?,
        pigment: row.get(3)?,
        ctf_event_id: row.get(4)?,
        title: row.get(5)?,
        kind: row.get(6)?,
        domain: Domain::parse(&domain).unwrap_or(Domain::Other),
        difficulty: row.get(8)?,
        status: row.get(9)?,
        url: row.get(10)?,
        points: row.get(11)?,
        progress: row.get(12)?,
        notes: row.get(13)?,
        minutes: row.get(14)?,
        sessions: row.get(15)?,
        last_session_at: row.get(16)?,
        started_at: row.get(17)?,
        completed_at: row.get(18)?,
        files: row.get(19)?,
        source: row.get(20)?,
    })
}

pub fn items(conn: &Connection, query: &LearningQuery) -> Result<Vec<LearningCard>> {
    let track = query.track_id.as_deref().map(|id| ids::parse("trackId", id)).transpose()?;
    let ctf = query.ctf_event_id.as_deref().map(|id| ids::parse("ctfEventId", id)).transpose()?;
    let sql = format!(
        "{CARD_SQL}
         WHERE i.deleted_at IS NULL AND t.deleted_at IS NULL
           AND (?1 IS NULL OR i.track_id = ?1) AND (?2 IS NULL OR i.status = ?2) AND (?3 IS NULL OR i.ctf_event_id = ?3)
         ORDER BY CASE i.status WHEN 'doing' THEN 0 WHEN 'todo' THEN 1 WHEN 'done' THEN 2 ELSE 3 END, i.order_key"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params![track, query.status, ctf], card_from)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn item(conn: &Connection, id: &str) -> Result<LearningCard> {
    let id = ids::parse("id", id)?;
    let sql = format!("{CARD_SQL} WHERE i.id = ?1 AND i.deleted_at IS NULL");
    conn.query_row(&sql, [&id], card_from).optional()?.ok_or(Error::NotFound("learning item"))
}

struct CheckedItem {
    track_id: String,
    title: String,
    kind: LearningKind,
    domain: Domain,
    status: LearningStatus,
    url: Option<String>,
    progress: Option<u8>,
    notes: String,
    ctf_event_id: Option<String>,
}

fn check_item(conn: &Connection, input: &LearningInput) -> Result<CheckedItem> {
    let track_id = ids::parse("trackId", &input.track_id)?;
    let known: bool = conn.query_row("SELECT count(*) > 0 FROM tracks WHERE id = ?1 AND deleted_at IS NULL", [&track_id], |row| row.get(0))?;
    if !known {
        return invalid("trackId", Reason::Unknown);
    }
    let ctf_event_id = input.ctf_event_id.as_deref().map(|id| ids::parse("ctfEventId", id)).transpose()?;
    if let Some(ctf) = &ctf_event_id {
        let known: bool =
            conn.query_row("SELECT count(*) > 0 FROM ctf_events WHERE id = ?1 AND deleted_at IS NULL", [ctf], |row| row.get(0))?;
        if !known {
            return invalid("ctfEventId", Reason::Unknown);
        }
    }
    if input.progress.is_some_and(|p| p > 100) {
        return invalid("progress", Reason::Format);
    }
    let notes = input.notes.trim().to_string();
    if notes.chars().count() > NOTES_MAX {
        return invalid("notes", Reason::Length);
    }
    Ok(CheckedItem {
        track_id,
        title: checked_text("title", &input.title, TITLE_MAX)?,
        kind: input.kind.unwrap_or(LearningKind::Room),
        domain: input.domain.unwrap_or(Domain::Other),
        status: input.status.unwrap_or(LearningStatus::Todo),
        url: checked_url(input.url.as_deref())?,
        progress: input.progress,
        notes,
        ctf_event_id,
    })
}

pub fn create_item(conn: &mut Connection, input: LearningInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let checked = check_item(conn, &input)?;
    let tx = conn.transaction()?;
    let last: Option<String> = tx.query_row(
        "SELECT max(order_key) FROM learning_items WHERE track_id = ?1",
        [&checked.track_id],
        |row| row.get(0),
    )?;
    let id = new_id();
    let stamp = instant(now);
    let started = matches!(checked.status, LearningStatus::Doing | LearningStatus::Done).then(|| stamp.clone());
    let completed = (checked.status == LearningStatus::Done).then(|| stamp.clone());
    tx.execute(
        "INSERT INTO learning_items (id, track_id, ctf_event_id, title, kind, domain, difficulty, status, url, points, progress,
                                     started_at, completed_at, notes, order_key, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?17)",
        params![
            id,
            checked.track_id,
            checked.ctf_event_id,
            checked.title,
            checked.kind,
            checked.domain,
            input.difficulty,
            checked.status,
            checked.url,
            input.points,
            checked.progress,
            started,
            completed,
            checked.notes,
            order::after(last.as_deref()),
            Source::from(actor),
            stamp
        ],
    )?;
    record(&tx, now, actor, Verb::Created, Entity::LearningItem, &id, &checked.title)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::LearningItem, id)))
}

pub fn update_item(conn: &mut Connection, id: &str, input: LearningInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let checked = check_item(conn, &input)?;
    let tx = conn.transaction()?;
    let before: LearningStatus = tx
        .query_row("SELECT status FROM learning_items WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("learning item"))?;
    tx.execute(
        "UPDATE learning_items SET track_id = ?2, ctf_event_id = ?3, title = ?4, kind = ?5, domain = ?6, difficulty = ?7, url = ?8,
                points = ?9, progress = ?10, notes = ?11, updated_at = ?12 WHERE id = ?1",
        params![
            id,
            checked.track_id,
            checked.ctf_event_id,
            checked.title,
            checked.kind,
            checked.domain,
            input.difficulty,
            checked.url,
            input.points,
            checked.progress,
            checked.notes,
            instant(now)
        ],
    )?;
    if before != checked.status {
        apply_status(&tx, &id, checked.status, now)?;
    }
    let verb = if before != LearningStatus::Done && checked.status == LearningStatus::Done { Verb::Completed } else { Verb::Updated };
    record(&tx, now, actor, verb, Entity::LearningItem, &id, &checked.title)?;
    tx.commit()?;
    Ok(Change::one(Entity::LearningItem, id))
}

/// Status with its dates: doing and done start the item, done finishes it, anything else clears
/// the finish.
fn apply_status(tx: &Transaction<'_>, id: &str, status: LearningStatus, now: Timestamp) -> Result<()> {
    let stamp = instant(now);
    tx.execute(
        "UPDATE learning_items SET status = ?2,
                started_at = CASE WHEN ?2 IN ('doing', 'done') THEN coalesce(started_at, ?3) ELSE started_at END,
                completed_at = CASE WHEN ?2 = 'done' THEN coalesce(completed_at, ?3) ELSE NULL END,
                progress = CASE WHEN ?2 = 'done' THEN 100 ELSE progress END,
                updated_at = ?3
         WHERE id = ?1",
        params![id, status, stamp],
    )?;
    Ok(())
}

pub fn set_status(conn: &mut Connection, id: &str, status: LearningStatus, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let (title, before): (String, LearningStatus) = tx
        .query_row("SELECT title, status FROM learning_items WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| Ok((row.get(0)?, row.get(1)?)))
        .optional()?
        .ok_or(Error::NotFound("learning item"))?;
    if before == status {
        return Ok(Change::one(Entity::LearningItem, id));
    }
    apply_status(&tx, &id, status, now)?;
    let verb = match status {
        LearningStatus::Done => Verb::Completed,
        _ if before == LearningStatus::Done => Verb::Reopened,
        _ => Verb::Updated,
    };
    record(&tx, now, actor, verb, Entity::LearningItem, &id, &title)?;
    tx.commit()?;
    Ok(Change::one(Entity::LearningItem, id))
}

pub fn delete_item(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    set_item_deleted(conn, id, true, actor, now)
}

pub fn restore_item(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    set_item_deleted(conn, id, false, actor, now)
}

fn set_item_deleted(conn: &mut Connection, id: &str, deleted: bool, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let title: String = tx
        .query_row("SELECT title FROM learning_items WHERE id = ?1", [&id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("learning item"))?;
    let stamp = instant(now);
    let changed = if deleted {
        tx.execute("UPDATE learning_items SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NULL", params![id, stamp])?
    } else {
        tx.execute("UPDATE learning_items SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL", params![id, stamp])?
    };
    if deleted {
        tx.execute("DELETE FROM learning_timer WHERE item_id = ?1", [&id])?;
    }
    if changed > 0 {
        record(&tx, now, actor, if deleted { Verb::Deleted } else { Verb::Restored }, Entity::LearningItem, &id, &title)?;
    }
    tx.commit()?;
    Ok(Change::one(Entity::LearningItem, id))
}

// ---------- sessions and the timer ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Session {
    pub id: String,
    pub item_id: String,
    pub item_title: String,
    pub started_at: String,
    pub duration_min: u32,
    pub note: String,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Timer {
    pub item_id: String,
    pub item_title: String,
    pub track_name: String,
    pub pigment: Pigment,
    pub started_at: String,
}

/// A session typed in afterwards: on a local date, from a minute of that day, for so long.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct SessionInput {
    pub item_id: String,
    pub date: String,
    pub start_minute: i32,
    pub minutes: u32,
    #[serde(default)]
    pub note: String,
}

pub fn sessions(conn: &Connection, item_id: &str) -> Result<Vec<Session>> {
    let item_id = ids::parse("itemId", item_id)?;
    let mut statement = conn.prepare(
        "SELECT s.id, s.item_id, i.title, s.started_at, s.duration_min, s.note, s.source FROM learning_sessions s
         JOIN learning_items i ON i.id = s.item_id WHERE s.item_id = ?1 ORDER BY s.started_at DESC LIMIT 200",
    )?;
    let rows = statement.query_map([&item_id], |row| {
        Ok(Session {
            id: row.get(0)?,
            item_id: row.get(1)?,
            item_title: row.get(2)?,
            started_at: row.get(3)?,
            duration_min: row.get(4)?,
            note: row.get(5)?,
            source: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn timer(conn: &Connection) -> Result<Option<Timer>> {
    Ok(conn
        .query_row(
            "SELECT lt.item_id, i.title, t.name, t.pigment, lt.started_at FROM learning_timer lt
             JOIN learning_items i ON i.id = lt.item_id JOIN tracks t ON t.id = i.track_id",
            [],
            |row| Ok(Timer { item_id: row.get(0)?, item_title: row.get(1)?, track_name: row.get(2)?, pigment: row.get(3)?, started_at: row.get(4)? }),
        )
        .optional()?)
}

fn insert_session(tx: &Connection, item_id: &str, started: Timestamp, minutes: u32, note: &str, actor: Actor, now: Timestamp) -> Result<String> {
    let id = new_id();
    tx.execute(
        "INSERT INTO learning_sessions (id, item_id, started_at, duration_min, note, source, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, item_id, instant(started), minutes, note, Source::from(actor), instant(now)],
    )?;
    let title: String = tx.query_row("SELECT title FROM learning_items WHERE id = ?1", [item_id], |row| row.get(0))?;
    activity::record(
        tx,
        now,
        Record {
            actor,
            verb: Verb::Created,
            entity: Entity::LearningSession,
            entity_id: Some(item_id),
            summary: &title,
            details: serde_json::json!({ "minutes": minutes, "session": id }),
        },
    )?;
    Ok(id)
}

/// Stop the running timer into a session. Returns what it wrote, if a timer ran.
fn stop_in(tx: &Transaction<'_>, actor: Actor, now: Timestamp) -> Result<Option<(String, u32)>> {
    let running: Option<(String, String)> =
        tx.query_row("SELECT item_id, started_at FROM learning_timer", [], |row| Ok((row.get(0)?, row.get(1)?))).optional()?;
    let Some((item_id, started_at)) = running else { return Ok(None) };
    let started = parse_instant("startedAt", &started_at)?;
    let seconds = (now.as_second() - started.as_second()).max(0);
    let minutes = ((seconds + 59) / 60).clamp(1, TIMER_CAP_MIN) as u32;
    tx.execute("DELETE FROM learning_timer", [])?;
    insert_session(tx, &item_id, started, minutes, "", actor, now)?;
    Ok(Some((item_id, minutes)))
}

/// Start timing an item. A timer already running on another item is stopped into its session
/// first; the item becomes "doing".
pub fn start_timer(conn: &mut Connection, item_id: &str, actor: Actor, now: Timestamp) -> Result<Vec<Change>> {
    let item_id = ids::parse("itemId", item_id)?;
    let tx = conn.transaction()?;
    let status: LearningStatus = tx
        .query_row("SELECT status FROM learning_items WHERE id = ?1 AND deleted_at IS NULL", [&item_id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("learning item"))?;
    let running: Option<String> = tx.query_row("SELECT item_id FROM learning_timer", [], |row| row.get(0)).optional()?;
    if running.as_deref() == Some(item_id.as_str()) {
        return Ok(Vec::new());
    }
    stop_in(&tx, actor, now)?;
    tx.execute("INSERT INTO learning_timer (id, item_id, started_at) VALUES (1, ?1, ?2)", params![item_id, instant(now)])?;
    if status == LearningStatus::Todo {
        apply_status(&tx, &item_id, LearningStatus::Doing, now)?;
    }
    tx.commit()?;
    Ok(vec![Change::one(Entity::LearningItem, item_id), Change::all(Entity::LearningSession)])
}

pub fn stop_timer(conn: &mut Connection, actor: Actor, now: Timestamp) -> Result<Option<(u32, Vec<Change>)>> {
    let tx = conn.transaction()?;
    let stopped = stop_in(&tx, actor, now)?;
    tx.commit()?;
    Ok(stopped.map(|(item_id, minutes)| (minutes, vec![Change::one(Entity::LearningItem, item_id), Change::all(Entity::LearningSession)])))
}

/// Throw the running timer away (started by mistake).
pub fn discard_timer(conn: &mut Connection) -> Result<Change> {
    conn.execute("DELETE FROM learning_timer", [])?;
    Ok(Change::all(Entity::LearningSession))
}

pub fn add_session(conn: &mut Connection, input: SessionInput, tz: &TimeZone, actor: Actor, now: Timestamp) -> Result<(String, Vec<Change>)> {
    let item_id = ids::parse("itemId", &input.item_id)?;
    let date = parse_date("date", &input.date)?;
    if !(0..1440).contains(&input.start_minute) {
        return invalid("startMinute", Reason::Format);
    }
    if input.minutes == 0 || i64::from(input.minutes) > 16 * 60 {
        return invalid("minutes", Reason::Format);
    }
    let note = input.note.trim();
    if note.chars().count() > 1000 {
        return invalid("note", Reason::Length);
    }
    let started = local_instant(date, input.start_minute, tz)?;
    let tx = conn.transaction()?;
    let known: bool =
        tx.query_row("SELECT count(*) > 0 FROM learning_items WHERE id = ?1 AND deleted_at IS NULL", [&item_id], |row| row.get(0))?;
    if !known {
        return Err(Error::NotFound("learning item"));
    }
    let id = insert_session(&tx, &item_id, started, input.minutes, note, actor, now)?;
    tx.commit()?;
    Ok((id, vec![Change::one(Entity::LearningItem, item_id), Change::all(Entity::LearningSession)]))
}

pub fn delete_session(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Vec<Change>> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let (item_id, minutes, title): (String, u32, String) = tx
        .query_row(
            "SELECT s.item_id, s.duration_min, i.title FROM learning_sessions s JOIN learning_items i ON i.id = s.item_id WHERE s.id = ?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(Error::NotFound("session"))?;
    tx.execute("DELETE FROM learning_sessions WHERE id = ?1", [&id])?;
    activity::record(
        &tx,
        now,
        Record {
            actor,
            verb: Verb::Deleted,
            entity: Entity::LearningSession,
            entity_id: Some(&item_id),
            summary: &title,
            details: serde_json::json!({ "minutes": minutes, "session": id }),
        },
    )?;
    tx.commit()?;
    Ok(vec![Change::one(Entity::LearningItem, item_id), Change::all(Entity::LearningSession)])
}

// ---------- CTF events ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CtfEvent {
    pub id: String,
    pub name: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub team: Option<String>,
    pub rank: Option<u32>,
    pub participants: Option<u32>,
    pub url: Option<String>,
    pub notes: String,
    pub challenges: u32,
    pub solved: u32,
    pub points: u32,
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields, default)]
#[ts(export)]
pub struct CtfInput {
    pub name: String,
    pub starts_on: Option<String>,
    pub ends_on: Option<String>,
    pub team: Option<String>,
    pub rank: Option<u32>,
    pub participants: Option<u32>,
    pub url: Option<String>,
    pub notes: String,
}

pub fn ctf_events(conn: &Connection) -> Result<Vec<CtfEvent>> {
    let mut statement = conn.prepare(
        "SELECT c.id, c.name, c.starts_on, c.ends_on, c.team, c.rank, c.participants, c.url, c.notes,
                (SELECT count(*) FROM learning_items i WHERE i.ctf_event_id = c.id AND i.deleted_at IS NULL),
                (SELECT count(*) FROM learning_items i WHERE i.ctf_event_id = c.id AND i.deleted_at IS NULL AND i.status = 'done'),
                coalesce((SELECT sum(i.points) FROM learning_items i WHERE i.ctf_event_id = c.id AND i.deleted_at IS NULL AND i.status = 'done'), 0)
         FROM ctf_events c WHERE c.deleted_at IS NULL ORDER BY coalesce(c.starts_on, c.created_at) DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(CtfEvent {
            id: row.get(0)?,
            name: row.get(1)?,
            starts_on: row.get(2)?,
            ends_on: row.get(3)?,
            team: row.get(4)?,
            rank: row.get(5)?,
            participants: row.get(6)?,
            url: row.get(7)?,
            notes: row.get(8)?,
            challenges: row.get(9)?,
            solved: row.get(10)?,
            points: row.get(11)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

struct CheckedCtf {
    name: String,
    starts_on: Option<String>,
    ends_on: Option<String>,
    team: Option<String>,
    url: Option<String>,
    notes: String,
}

fn check_ctf(input: &CtfInput) -> Result<CheckedCtf> {
    let starts_on = input.starts_on.as_deref().filter(|d| !d.is_empty()).map(|d| parse_date("startsOn", d).map(date_text)).transpose()?;
    let ends_on = input.ends_on.as_deref().filter(|d| !d.is_empty()).map(|d| parse_date("endsOn", d).map(date_text)).transpose()?;
    if let (Some(start), Some(end)) = (&starts_on, &ends_on)
        && end < start
    {
        return invalid("endsOn", Reason::Order);
    }
    if input.rank == Some(0) || input.participants == Some(0) {
        return invalid("rank", Reason::Format);
    }
    let team = input.team.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    if team.as_ref().is_some_and(|t| t.chars().count() > 80) {
        return invalid("team", Reason::Length);
    }
    let notes = input.notes.trim().to_string();
    if notes.chars().count() > NOTES_MAX {
        return invalid("notes", Reason::Length);
    }
    Ok(CheckedCtf { name: checked_text("name", &input.name, 120)?, starts_on, ends_on, team, url: checked_url(input.url.as_deref())?, notes })
}

pub fn create_ctf(conn: &mut Connection, input: CtfInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let checked = check_ctf(&input)?;
    let tx = conn.transaction()?;
    let id = new_id();
    tx.execute(
        "INSERT INTO ctf_events (id, name, starts_on, ends_on, team, rank, participants, url, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
        params![id, checked.name, checked.starts_on, checked.ends_on, checked.team, input.rank, input.participants, checked.url, checked.notes, instant(now)],
    )?;
    record(&tx, now, actor, Verb::Created, Entity::CtfEvent, &id, &checked.name)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::CtfEvent, id)))
}

pub fn update_ctf(conn: &mut Connection, id: &str, input: CtfInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let checked = check_ctf(&input)?;
    let tx = conn.transaction()?;
    let changed = tx.execute(
        "UPDATE ctf_events SET name = ?2, starts_on = ?3, ends_on = ?4, team = ?5, rank = ?6, participants = ?7, url = ?8, notes = ?9,
                updated_at = ?10 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, checked.name, checked.starts_on, checked.ends_on, checked.team, input.rank, input.participants, checked.url, checked.notes, instant(now)],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("ctf event"));
    }
    record(&tx, now, actor, Verb::Updated, Entity::CtfEvent, &id, &checked.name)?;
    tx.commit()?;
    Ok(Change::one(Entity::CtfEvent, id))
}

/// Remove an event; its challenges stay in the CTF track, without the event.
pub fn delete_ctf(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name: String = tx
        .query_row("SELECT name FROM ctf_events WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("ctf event"))?;
    let stamp = instant(now);
    tx.execute("UPDATE ctf_events SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    tx.execute("UPDATE learning_items SET ctf_event_id = NULL, updated_at = ?2 WHERE ctf_event_id = ?1", params![id, stamp])?;
    record(&tx, now, actor, Verb::Deleted, Entity::CtfEvent, &id, &name)?;
    tx.commit()?;
    Ok(Change::all(Entity::CtfEvent))
}

// ---------- the week and the heatmap ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayMinutes {
    pub date: String,
    pub minutes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DomainStat {
    pub domain: Domain,
    pub minutes: u32,
    pub done: u32,
    pub items: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LearningOverview {
    /// Monday of this week.
    pub week_start: String,
    pub goal_days: u8,
    pub active_days: u8,
    pub minutes_week: u32,
    /// Monday to Sunday of this week.
    pub days: Vec<DayMinutes>,
    /// 20 weeks, oldest Monday first, ending with this week's Sunday.
    pub heatmap: Vec<DayMinutes>,
    pub domains: Vec<DomainStat>,
    /// What to pick up again: the timed item, else the last one worked on that is not done.
    pub current: Option<LearningCard>,
    pub timer: Option<Timer>,
}

/// Minutes per local day from `from` (a Monday) through today.
fn minutes_by_day(conn: &Connection, from: Date, tz: &TimeZone) -> Result<BTreeMap<Date, u32>> {
    let start = from.to_zoned(tz.clone())?.timestamp();
    let mut statement = conn.prepare(
        "SELECT s.started_at, s.duration_min FROM learning_sessions s
         JOIN learning_items i ON i.id = s.item_id AND i.deleted_at IS NULL
         WHERE s.started_at >= ?1",
    )?;
    let rows = statement.query_map([instant(start)], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?)))?;
    let mut by_day = BTreeMap::new();
    for row in rows {
        let (at, minutes) = row?;
        let Ok(at) = at.parse::<Timestamp>() else { continue };
        *by_day.entry(local_date(at, tz)).or_insert(0) += minutes;
    }
    Ok(by_day)
}

pub fn overview(conn: &Connection, now: Timestamp, tz: &TimeZone) -> Result<LearningOverview> {
    let goal_days = settings::load(conn)?.learning_goal_days;
    let today = local_date(now, tz);
    let monday = monday_of(today)?;
    let first = monday.checked_sub((7 * (HEATMAP_WEEKS - 1)).days())?;
    let by_day = minutes_by_day(conn, first, tz)?;
    let day_list = |from: Date, count: i64| -> Result<Vec<DayMinutes>> {
        (0..count)
            .map(|offset| {
                let date = from.checked_add(offset.days())?;
                Ok(DayMinutes { date: date_text(date), minutes: by_day.get(&date).copied().unwrap_or(0) })
            })
            .collect()
    };
    let days = day_list(monday, 7)?;
    let heatmap = day_list(first, HEATMAP_WEEKS * 7)?;
    let active_days = days.iter().filter(|d| d.minutes > 0).count() as u8;
    let minutes_week = days.iter().map(|d| d.minutes).sum();

    let mut domains: HashMap<Domain, DomainStat> = HashMap::new();
    let mut statement = conn.prepare(
        "SELECT i.domain, i.status = 'done', coalesce((SELECT sum(s.duration_min) FROM learning_sessions s WHERE s.item_id = i.id), 0)
         FROM learning_items i JOIN tracks t ON t.id = i.track_id AND t.deleted_at IS NULL
         WHERE i.deleted_at IS NULL AND i.status <> 'dropped'",
    )?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, bool>(1)?, row.get::<_, u32>(2)?)))?;
    for row in rows {
        let (domain, done, minutes) = row?;
        let domain = Domain::parse(&domain).unwrap_or(Domain::Other);
        let stat = domains.entry(domain).or_insert(DomainStat { domain, minutes: 0, done: 0, items: 0 });
        stat.items += 1;
        stat.minutes += minutes;
        stat.done += u32::from(done);
    }
    let domains: Vec<DomainStat> = Domain::ALL.iter().filter_map(|d| domains.remove(d)).collect();

    let timer = timer(conn)?;
    let current_id: Option<String> = match &timer {
        Some(timer) => Some(timer.item_id.clone()),
        None => conn
            .query_row(
                "SELECT i.id FROM learning_items i JOIN tracks t ON t.id = i.track_id AND t.deleted_at IS NULL
                 WHERE i.deleted_at IS NULL AND i.status = 'doing'
                 ORDER BY coalesce((SELECT max(s.started_at) FROM learning_sessions s WHERE s.item_id = i.id), i.started_at, i.updated_at) DESC
                 LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()?,
    };
    let current = current_id.map(|id| item(conn, &id)).transpose()?;
    Ok(LearningOverview {
        week_start: date_text(monday),
        goal_days,
        active_days,
        minutes_week,
        days,
        heatmap,
        domains,
        current,
        timer,
    })
}

/// Distinct local days with learning in [from, to), for Jejak's weekly comparison.
pub fn active_days_between(conn: &Connection, from: Timestamp, to: Timestamp, tz: &TimeZone) -> Result<(u32, u32)> {
    let mut statement = conn.prepare(
        "SELECT s.started_at, s.duration_min FROM learning_sessions s
         JOIN learning_items i ON i.id = s.item_id AND i.deleted_at IS NULL
         WHERE s.started_at >= ?1 AND s.started_at < ?2",
    )?;
    let rows = statement.query_map([instant(from), instant(to)], |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?)))?;
    let mut days = HashSet::new();
    let mut minutes = 0;
    for row in rows {
        let (at, duration) = row?;
        if let Ok(at) = at.parse::<Timestamp>() {
            days.insert(local_date(at, tz));
        }
        minutes += duration;
    }
    Ok((days.len() as u32, minutes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn jakarta() -> TimeZone {
        TimeZone::get("Asia/Jakarta").unwrap()
    }

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    fn thm(conn: &mut Connection) -> String {
        ensure_default_tracks(conn, at("2026-10-01T00:00:00Z")).unwrap();
        tracks(conn).unwrap().into_iter().find(|t| t.name == "TryHackMe").unwrap().id
    }

    fn room(conn: &mut Connection, track: &str, title: &str, domain: Domain) -> String {
        let input = LearningInput { track_id: track.into(), title: title.into(), domain: Some(domain), ..Default::default() };
        create_item(conn, input, Actor::Owner, at("2026-10-01T00:00:00Z")).unwrap().0
    }

    #[test]
    fn the_owners_tracks_are_made_once() {
        let mut conn = open_in_memory().unwrap();
        assert!(ensure_default_tracks(&mut conn, at("2026-10-01T00:00:00Z")).unwrap());
        let names: Vec<String> = tracks(&conn).unwrap().into_iter().map(|t| t.name).collect();
        assert_eq!(names, ["TryHackMe", "Hack The Box", "CTF", "Lab Sekolah", "Homelab"]);
        let id = tracks(&conn).unwrap()[4].id.clone();
        delete_track(&mut conn, &id, Actor::Owner, at("2026-10-01T00:00:00Z")).unwrap();
        assert!(!ensure_default_tracks(&mut conn, at("2026-10-02T00:00:00Z")).unwrap());
        assert_eq!(tracks(&conn).unwrap().len(), 4);
    }

    #[test]
    fn a_timer_becomes_a_session_and_starting_another_stops_it() {
        let mut conn = open_in_memory().unwrap();
        let track = thm(&mut conn);
        let nmap = room(&mut conn, &track, "Nmap Live Host Discovery", Domain::Network);
        let web = room(&mut conn, &track, "OWASP Top 10", Domain::Web);
        start_timer(&mut conn, &nmap, Actor::Owner, at("2026-10-23T12:00:00Z")).unwrap();
        assert_eq!(item(&conn, &nmap).unwrap().status, LearningStatus::Doing);
        assert_eq!(timer(&conn).unwrap().unwrap().item_id, nmap);
        // 44 min 10 s rounds up to 45.
        start_timer(&mut conn, &web, Actor::Owner, at("2026-10-23T12:44:10Z")).unwrap();
        assert_eq!(item(&conn, &nmap).unwrap().minutes, 45);
        let (minutes, _) = stop_timer(&mut conn, Actor::Owner, at("2026-10-23T13:00:00Z")).unwrap().unwrap();
        assert_eq!(minutes, 16);
        assert!(timer(&conn).unwrap().is_none());
        assert!(stop_timer(&mut conn, Actor::Owner, at("2026-10-23T13:00:00Z")).unwrap().is_none());

        // Forgotten overnight: capped.
        start_timer(&mut conn, &web, Actor::Owner, at("2026-10-23T14:00:00Z")).unwrap();
        let (minutes, _) = stop_timer(&mut conn, Actor::Owner, at("2026-10-24T09:00:00Z")).unwrap().unwrap();
        assert_eq!(minutes as i64, TIMER_CAP_MIN);
    }

    #[test]
    fn the_week_counts_active_local_days_against_the_goal() {
        let mut conn = open_in_memory().unwrap();
        let track = thm(&mut conn);
        let nmap = room(&mut conn, &track, "Nmap", Domain::Network);
        let tz = jakarta();
        let log = |conn: &mut Connection, date: &str, minute: i32, minutes: u32| {
            let input = SessionInput { item_id: nmap.clone(), date: date.into(), start_minute: minute, minutes, note: String::new() };
            add_session(conn, input, &tz, Actor::Owner, at("2026-10-23T12:00:00Z")).unwrap();
        };
        // Monday 19 and Wednesday 21 (twice); 00:30 WIB on Thursday 22 is still Thursday locally.
        log(&mut conn, "2026-10-19", 19 * 60, 60);
        log(&mut conn, "2026-10-21", 19 * 60, 30);
        log(&mut conn, "2026-10-21", 21 * 60, 30);
        log(&mut conn, "2026-10-22", 30, 20);
        log(&mut conn, "2026-10-12", 19 * 60, 90); // last week
        let view = overview(&conn, at("2026-10-23T05:00:00Z"), &tz).unwrap();
        assert_eq!(view.week_start, "2026-10-19");
        assert_eq!((view.active_days, view.goal_days, view.minutes_week), (3, 4, 140));
        assert_eq!(view.days.iter().map(|d| d.minutes).collect::<Vec<_>>(), [60, 0, 60, 20, 0, 0, 0]);
        assert_eq!(view.heatmap.len(), 140);
        assert_eq!(view.heatmap.last().unwrap().date, "2026-10-25");
        assert_eq!(view.heatmap.iter().map(|d| d.minutes).sum::<u32>(), 230);
        assert_eq!(view.domains, [DomainStat { domain: Domain::Network, minutes: 230, done: 0, items: 1 }]);
        assert_eq!(view.current, None, "todo items are not picked up again");
    }

    #[test]
    fn status_carries_its_dates_and_links_are_checked() {
        let mut conn = open_in_memory().unwrap();
        let track = thm(&mut conn);
        let id = room(&mut conn, &track, "Linux Fundamentals", Domain::Linux);
        set_status(&mut conn, &id, LearningStatus::Done, Actor::Owner, at("2026-10-23T05:00:00Z")).unwrap();
        let done = item(&conn, &id).unwrap();
        assert_eq!((done.progress, done.completed_at.is_some(), done.started_at.is_some()), (Some(100), true, true));
        set_status(&mut conn, &id, LearningStatus::Doing, Actor::Owner, at("2026-10-23T06:00:00Z")).unwrap();
        assert_eq!(item(&conn, &id).unwrap().completed_at, None);
        assert_eq!(activity::recent(&conn, 1, false).unwrap()[0].verb, Verb::Reopened);

        let bad_url = LearningInput { track_id: track.clone(), title: "x".into(), url: Some("javascript:alert(1)".into()), ..Default::default() };
        assert!(create_item(&mut conn, bad_url, Actor::Owner, at("2026-10-23T05:00:00Z")).is_err());
        let bad_track = LearningInput { track_id: new_id(), title: "x".into(), ..Default::default() };
        assert!(create_item(&mut conn, bad_track, Actor::Owner, at("2026-10-23T05:00:00Z")).is_err());
    }

    #[test]
    fn a_ctf_counts_its_solved_challenges() {
        let mut conn = open_in_memory().unwrap();
        ensure_default_tracks(&mut conn, at("2026-10-01T00:00:00Z")).unwrap();
        let ctf_track = tracks(&conn).unwrap().into_iter().find(|t| t.kind == TrackKind::Ctf).unwrap().id;
        let input = CtfInput { name: "Cyber Jawara 2026".into(), starts_on: Some("2026-10-24".into()), ends_on: Some("2026-10-25".into()), ..Default::default() };
        let (event, _) = create_ctf(&mut conn, input, Actor::Owner, at("2026-10-23T00:00:00Z")).unwrap();
        for (title, points, done) in [("baby-web", 100, true), ("rev-me", 300, false)] {
            let input = LearningInput {
                track_id: ctf_track.clone(),
                title: title.into(),
                kind: Some(LearningKind::Challenge),
                points: Some(points),
                status: Some(if done { LearningStatus::Done } else { LearningStatus::Todo }),
                ctf_event_id: Some(event.clone()),
                ..Default::default()
            };
            create_item(&mut conn, input, Actor::Owner, at("2026-10-24T03:00:00Z")).unwrap();
        }
        let events = ctf_events(&conn).unwrap();
        assert_eq!((events[0].challenges, events[0].solved, events[0].points), (2, 1, 100));
        let backwards = CtfInput { name: "x".into(), starts_on: Some("2026-10-25".into()), ends_on: Some("2026-10-24".into()), ..Default::default() };
        assert!(create_ctf(&mut conn, backwards, Actor::Owner, at("2026-10-23T00:00:00Z")).is_err());
    }
}
