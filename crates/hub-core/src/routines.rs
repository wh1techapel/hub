//! Rutinitas: recurring blocks such as the school timetable. A routine is a rule, not rows: its
//! occurrences are worked out for the days being shown. One occurrence can be skipped or moved
//! without touching the rest (`routine_exceptions`).

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::error::{invalid, Error, Reason, Result};
use crate::events::Slot;
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, Pigment, RoutineKind, Source, Verb};
use crate::recurrence::Rule;
use crate::time::{clock_text, date_text, instant, minutes_of, parse_clock, parse_date, parse_instant, Day};

const TITLE_MAX: usize = 200;

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RoutineInput {
    pub title: String,
    pub kind: RoutineKind,
    pub rule: Rule,
    /// `HH:MM`, local.
    pub start_time: String,
    pub duration_min: u32,
    pub starts_on: String,
    #[serde(default)]
    pub ends_on: Option<String>,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub pigment: Pigment,
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Routine {
    pub id: String,
    pub title: String,
    pub kind: RoutineKind,
    pub rule: Rule,
    pub start_time: String,
    pub duration_min: u32,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub location: String,
    pub pigment: Pigment,
    pub project_id: Option<String>,
    pub source: Source,
}

/// A routine on one day, after its exceptions.
#[derive(Debug, Clone)]
pub struct Occurrence {
    pub routine: Routine,
    /// The date the rule put it on (the key for its exception).
    pub rule_date: Date,
    /// Where it happens: usually the same day and the routine's own time.
    pub start: Timestamp,
    pub end: Timestamp,
    pub moved: bool,
}

struct Checked {
    title: String,
    rule: Rule,
    project_id: Option<String>,
}

fn check(input: &RoutineInput) -> Result<Checked> {
    let title = input.title.trim();
    if title.is_empty() || title.chars().count() > TITLE_MAX {
        return invalid("title", Reason::Length);
    }
    if input.location.chars().count() > TITLE_MAX {
        return invalid("location", Reason::Length);
    }
    parse_clock("startTime", &input.start_time)?;
    if input.duration_min == 0 || input.duration_min > 24 * 60 {
        return invalid("durationMin", Reason::Format);
    }
    let starts_on = parse_date("startsOn", &input.starts_on)?;
    if let Some(ends) = &input.ends_on
        && parse_date("endsOn", ends)? < starts_on
    {
        return invalid("endsOn", Reason::Order);
    }
    Ok(Checked {
        title: title.to_string(),
        rule: input.rule.clone().validated()?,
        project_id: input.project_id.as_deref().map(|id| ids::parse("projectId", id)).transpose()?,
    })
}

fn check_project(tx: &Transaction<'_>, project_id: &Option<String>) -> Result<()> {
    if let Some(project) = project_id {
        let found = tx
            .query_row("SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL", [project], |_| Ok(()))
            .optional()?;
        if found.is_none() {
            return invalid("projectId", Reason::Unknown);
        }
    }
    Ok(())
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, id: &str, title: &str) -> Result<()> {
    activity::record(
        tx,
        now,
        Record { actor, verb, entity: Entity::Routine, entity_id: Some(id), summary: title, details: serde_json::json!({}) },
    )
}

pub fn create(conn: &mut Connection, input: RoutineInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let checked = check(&input)?;
    let tx = conn.transaction()?;
    check_project(&tx, &checked.project_id)?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO routines (id, title, kind, rrule, start_time, duration_min, starts_on, ends_on, location,
                               pigment, project_id, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        params![
            id,
            checked.title,
            input.kind,
            checked.rule.to_text(),
            input.start_time,
            input.duration_min,
            input.starts_on,
            input.ends_on,
            input.location.trim(),
            input.pigment,
            checked.project_id,
            Source::from(actor),
            stamp,
        ],
    )?;
    record(&tx, now, actor, Verb::Created, &id, &checked.title)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Routine, id)))
}

pub fn update(conn: &mut Connection, id: &str, input: RoutineInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let checked = check(&input)?;
    let tx = conn.transaction()?;
    check_project(&tx, &checked.project_id)?;
    let changed = tx.execute(
        "UPDATE routines SET title = ?2, kind = ?3, rrule = ?4, start_time = ?5, duration_min = ?6, starts_on = ?7,
                ends_on = ?8, location = ?9, pigment = ?10, project_id = ?11, updated_at = ?12
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            id,
            checked.title,
            input.kind,
            checked.rule.to_text(),
            input.start_time,
            input.duration_min,
            input.starts_on,
            input.ends_on,
            input.location.trim(),
            input.pigment,
            checked.project_id,
            instant(now),
        ],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("routine"));
    }
    record(&tx, now, actor, Verb::Updated, &id, &checked.title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Routine, id))
}

pub fn delete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let title: Option<String> = tx
        .query_row("SELECT title FROM routines WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let title = title.ok_or(Error::NotFound("routine"))?;
    let stamp = instant(now);
    tx.execute("UPDATE routines SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    record(&tx, now, actor, Verb::Deleted, &id, &title)?;
    tx.commit()?;
    Ok(Change::all(Entity::Routine))
}

fn routine_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<(Routine, String)> {
    let rule_text: String = row.get(3)?;
    Ok((
        Routine {
            id: row.get(0)?,
            title: row.get(1)?,
            kind: row.get(2)?,
            rule: Rule::weekly(&[]),
            start_time: row.get(4)?,
            duration_min: row.get(5)?,
            starts_on: row.get(6)?,
            ends_on: row.get(7)?,
            location: row.get(8)?,
            pigment: row.get(9)?,
            project_id: row.get(10)?,
            source: row.get(11)?,
        },
        rule_text,
    ))
}

/// Every active routine. A rule this version cannot read is left out rather than failing the list.
pub fn list(conn: &Connection) -> Result<Vec<Routine>> {
    let mut statement = conn.prepare(
        "SELECT id, title, kind, rrule, start_time, duration_min, starts_on, ends_on, location, pigment,
                project_id, source
         FROM routines WHERE deleted_at IS NULL AND active = 1 ORDER BY start_time, title",
    )?;
    let rows = statement.query_map([], routine_row)?;
    let mut out = Vec::new();
    for row in rows {
        let (mut routine, text) = row?;
        if let Ok(rule) = Rule::parse(&text) {
            routine.rule = rule;
            out.push(routine);
        }
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> Result<Routine> {
    let id = ids::parse("id", id)?;
    list(conn)?.into_iter().find(|r| r.id == id).ok_or(Error::NotFound("routine"))
}

/// routine_exceptions: routine id, rule date, kind, moved start and end.
type ExceptionRow = (String, String, String, Option<String>, Option<String>);

/// The occurrences that land on any of `days` (local), exceptions applied: skipped ones are gone,
/// moved ones sit where they were moved to (possibly another day).
pub fn occurrences(conn: &Connection, days: &[Day], tz: &TimeZone) -> Result<Vec<Occurrence>> {
    let (Some(first), Some(last)) = (days.first(), days.last()) else {
        return Ok(Vec::new());
    };
    let routines = list(conn)?;
    let mut statement = conn.prepare("SELECT routine_id, date, kind, moved_start, moved_end FROM routine_exceptions")?;
    let exceptions: Vec<ExceptionRow> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let exception = |routine: &str, date: &str| exceptions.iter().find(|e| e.0 == routine && e.1 == date);

    let mut out = Vec::new();
    for routine in &routines {
        let starts_on = parse_date("startsOn", &routine.starts_on)?;
        let ends_on = routine.ends_on.as_deref().map(|d| parse_date("endsOn", d)).transpose()?;
        let start_minute = minutes_of(parse_clock("startTime", &routine.start_time)?);
        for day in days {
            let date = day.date;
            if date < starts_on || ends_on.is_some_and(|end| date > end) || !routine.rule.occurs_on(starts_on, date) {
                continue;
            }
            let key = date_text(date);
            match exception(&routine.id, &key) {
                Some((_, _, kind, _, _)) if kind == "skipped" => {}
                Some((_, _, _, Some(_), Some(_))) => {} // moved: placed below, wherever it went
                _ => {
                    let start = crate::events::local_instant(date, start_minute, tz)?;
                    let end = crate::events::local_instant(date, start_minute + routine.duration_min as i32, tz)?;
                    out.push(Occurrence { routine: routine.clone(), rule_date: date, start, end, moved: false });
                }
            }
        }
        // Moved occurrences that now fall inside the range, from wherever their rule put them.
        for (routine_id, date, kind, moved_start, moved_end) in &exceptions {
            if routine_id != &routine.id || kind != "moved" {
                continue;
            }
            let (Some(start), Some(end)) = (moved_start, moved_end) else { continue };
            let start = parse_instant("movedStart", start)?;
            let end = parse_instant("movedEnd", end)?;
            if start < last.end && end > first.start {
                out.push(Occurrence {
                    routine: routine.clone(),
                    rule_date: parse_date("date", date)?,
                    start,
                    end,
                    moved: true,
                });
            }
        }
    }
    Ok(out)
}

fn routine_title(tx: &Transaction<'_>, id: &str) -> Result<String> {
    let title: Option<String> = tx
        .query_row("SELECT title FROM routines WHERE id = ?1 AND deleted_at IS NULL", [id], |row| row.get(0))
        .optional()?;
    title.ok_or(Error::NotFound("routine"))
}

/// Skip one occurrence ("libur hari ini").
pub fn skip_occurrence(conn: &mut Connection, id: &str, date: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    parse_date("date", date)?;
    let tx = conn.transaction()?;
    let title = routine_title(&tx, &id)?;
    tx.execute(
        "INSERT INTO routine_exceptions (routine_id, date, kind) VALUES (?1, ?2, 'skipped')
         ON CONFLICT (routine_id, date) DO UPDATE SET kind = 'skipped', moved_start = NULL, moved_end = NULL",
        params![id, date],
    )?;
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Updated, entity: Entity::Routine, entity_id: Some(&id), summary: &title, details: serde_json::json!({ "skipped": date }) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::Routine, id))
}

/// Move one occurrence (a drag on the calendar); the rest of the routine stays.
pub fn move_occurrence(conn: &mut Connection, id: &str, date: &str, to: &Slot, tz: &TimeZone, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    parse_date("date", date)?;
    let (start, end) = to.instants(tz)?;
    let tx = conn.transaction()?;
    let title = routine_title(&tx, &id)?;
    tx.execute(
        "INSERT INTO routine_exceptions (routine_id, date, kind, moved_start, moved_end) VALUES (?1, ?2, 'moved', ?3, ?4)
         ON CONFLICT (routine_id, date) DO UPDATE SET kind = 'moved', moved_start = excluded.moved_start,
           moved_end = excluded.moved_end",
        params![id, date, instant(start), instant(end)],
    )?;
    activity::record(
        &tx,
        now,
        Record { actor, verb: Verb::Moved, entity: Entity::Routine, entity_id: Some(&id), summary: &title, details: serde_json::json!({ "date": date, "start": instant(start) }) },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::Routine, id))
}

/// Put one occurrence back the way the rule has it.
pub fn restore_occurrence(conn: &mut Connection, id: &str, date: &str) -> Result<Change> {
    let id = ids::parse("id", id)?;
    parse_date("date", date)?;
    conn.execute("DELETE FROM routine_exceptions WHERE routine_id = ?1 AND date = ?2", params![id, date])?;
    Ok(Change::one(Entity::Routine, id))
}

/// "HH:MM" plus minutes, for display of an occurrence's end.
pub fn end_time(start: &str, duration: u32) -> Result<String> {
    let start = minutes_of(parse_clock("startTime", start)?);
    let end = (start + duration as i32) % 1440;
    Ok(clock_text(jiff::civil::Time::new((end / 60) as i8, (end % 60) as i8, 0, 0)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::recurrence::Day as WeekDay;

    fn tz() -> TimeZone {
        TimeZone::get("Asia/Jakarta").unwrap()
    }

    fn now() -> Timestamp {
        "2026-10-23T03:00:00Z".parse().unwrap()
    }

    fn day(text: &str) -> Day {
        Day::of(text.parse().unwrap(), &tz()).unwrap()
    }

    fn matematika(conn: &mut Connection) -> String {
        let input = RoutineInput {
            title: "Matematika".into(),
            kind: RoutineKind::Lesson,
            rule: Rule::weekly(&[WeekDay::Mo, WeekDay::Th]),
            start_time: "07:00".into(),
            duration_min: 90,
            starts_on: "2026-10-19".into(),
            ends_on: Some("2026-12-18".into()),
            location: "Ruang 12".into(),
            pigment: Pigment::Lazuardi,
            project_id: None,
        };
        create(conn, input, Actor::Owner, now()).unwrap().0
    }

    #[test]
    fn occurrences_follow_the_rule_and_the_term() {
        let mut conn = open_in_memory().unwrap();
        matematika(&mut conn);
        let week: Vec<Day> = ["2026-10-19", "2026-10-20", "2026-10-21", "2026-10-22", "2026-10-23"].iter().map(|d| day(d)).collect();
        let found = occurrences(&conn, &week, &tz()).unwrap();
        let dates: Vec<String> = found.iter().map(|o| date_text(o.rule_date)).collect();
        assert_eq!(dates, ["2026-10-19", "2026-10-22"]);
        assert_eq!(instant(found[0].start), "2026-10-19T00:00:00Z");
        assert_eq!(instant(found[0].end), "2026-10-19T01:30:00Z");
        assert!(occurrences(&conn, &[day("2026-12-21")], &tz()).unwrap().is_empty(), "after the term");
    }

    #[test]
    fn one_occurrence_can_be_skipped_or_moved() {
        let mut conn = open_in_memory().unwrap();
        let id = matematika(&mut conn);
        skip_occurrence(&mut conn, &id, "2026-10-19", Actor::Owner, now()).unwrap();
        // Thursday's lesson moved to Friday 09.00.
        let to = Slot { date: "2026-10-23".into(), start_minute: 9 * 60, end_minute: 10 * 60 + 30 };
        move_occurrence(&mut conn, &id, "2026-10-22", &to, &tz(), Actor::Owner, now()).unwrap();
        let thursday = occurrences(&conn, &[day("2026-10-22")], &tz()).unwrap();
        assert!(thursday.is_empty());
        let friday = occurrences(&conn, &[day("2026-10-23")], &tz()).unwrap();
        assert_eq!(friday.len(), 1);
        assert!(friday[0].moved);
        assert_eq!(instant(friday[0].start), "2026-10-23T02:00:00Z");
        assert!(occurrences(&conn, &[day("2026-10-19")], &tz()).unwrap().is_empty());
        restore_occurrence(&mut conn, &id, "2026-10-19").unwrap();
        assert_eq!(occurrences(&conn, &[day("2026-10-19")], &tz()).unwrap().len(), 1);
    }

    #[test]
    fn input_is_checked() {
        let mut conn = open_in_memory().unwrap();
        let good = RoutineInput {
            title: "Belajar Cyber".into(),
            kind: RoutineKind::Study,
            rule: Rule::weekly(&[WeekDay::Mo, WeekDay::We, WeekDay::Fr]),
            start_time: "19:00".into(),
            duration_min: 90,
            starts_on: "2026-10-23".into(),
            ends_on: None,
            location: String::new(),
            pigment: Pigment::Lavender,
            project_id: None,
        };
        let bad = [
            RoutineInput { title: " ".into(), ..good.clone() },
            RoutineInput { start_time: "7:00".into(), ..good.clone() },
            RoutineInput { duration_min: 0, ..good.clone() },
            RoutineInput { ends_on: Some("2026-10-01".into()), ..good.clone() },
            RoutineInput { rule: Rule { freq: crate::recurrence::Freq::Daily, interval: 1, by_day: vec![WeekDay::Mo] }, ..good.clone() },
        ];
        for input in bad {
            assert!(create(&mut conn, input, Actor::Owner, now()).is_err());
        }
        let (id, _) = create(&mut conn, good, Actor::Owner, now()).unwrap();
        assert_eq!(get(&conn, &id).unwrap().rule.to_text(), "FREQ=WEEKLY;BYDAY=MO,WE,FR");
        assert_eq!(end_time("19:00", 90).unwrap(), "20:30");
        delete(&mut conn, &id, Actor::Owner, now()).unwrap();
        assert!(list(&conn).unwrap().is_empty());
    }
}
