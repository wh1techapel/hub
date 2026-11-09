//! Kalender: what fills each local day. Events, routine occurrences, and time blocks become
//! items with minutes from that day's midnight (an item over midnight is cut in two). Each day
//! also says how full it is, so an over-planned day can be named before it starts.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use crate::error::Result;
use crate::model::{Pigment, ReminderState, Source};
use crate::routines;
use crate::time::{date_text, instant, parse_date, parse_instant, Day};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ItemKind {
    Event,
    Routine,
    /// An event that is a task's time block.
    Block,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarItem {
    /// Unique among the day's items: the event id, or the routine id and the rule's date.
    pub key: String,
    pub kind: ItemKind,
    pub event_id: Option<String>,
    pub routine_id: Option<String>,
    /// The date the routine's rule put this occurrence on (its exception key).
    pub rule_date: Option<String>,
    pub task_id: Option<String>,
    pub task_done: bool,
    pub title: String,
    pub location: String,
    pub all_day: bool,
    /// Minutes from this day's midnight, clamped to it.
    pub start_minute: i32,
    pub end_minute: i32,
    /// The item began the day before / goes on into the next day.
    pub continues_before: bool,
    pub continues_after: bool,
    /// A routine occurrence moved away from its usual time.
    pub moved: bool,
    pub pigment: Pigment,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayReminder {
    pub id: String,
    pub text: String,
    pub minute: i32,
    pub state: ReminderState,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayLoad {
    /// Minutes of timed items inside waking hours.
    pub scheduled_min: u32,
    /// Estimates of the day's tasks that have no time block yet.
    pub planned_min: u32,
    pub waking_min: u32,
    /// Scheduled plus planned is more than 85 % of the waking day.
    pub over: bool,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CalendarDay {
    pub date: String,
    pub items: Vec<CalendarItem>,
    pub reminders: Vec<DayReminder>,
    pub load: DayLoad,
    /// Closed with Tutup Hari.
    pub closed: bool,
}

/// The waking window, minutes from midnight (settings' wake and sleep times).
#[derive(Debug, Clone, Copy)]
pub struct Waking {
    pub wake: i32,
    pub sleep: i32,
}

const OVER_PERCENT: u32 = 85;

/// `count` consecutive days from `first`.
pub fn range(conn: &Connection, first: Date, count: u32, tz: &TimeZone, waking: Waking) -> Result<Vec<CalendarDay>> {
    let days: Vec<Day> = (0..count)
        .map(|n| Day::of(first.checked_add(Span::new().days(i64::from(n)))?, tz))
        .collect::<Result<_>>()?;
    let (Some(first_day), Some(last_day)) = (days.first(), days.last()) else {
        return Ok(Vec::new());
    };

    struct Raw {
        key: String,
        kind: ItemKind,
        event_id: Option<String>,
        routine_id: Option<String>,
        rule_date: Option<String>,
        task_id: Option<String>,
        task_done: bool,
        title: String,
        location: String,
        pigment: Pigment,
        source: Source,
        moved: bool,
        span: Span2,
    }
    enum Span2 {
        Timed(Timestamp, Timestamp),
        AllDay(Date, Date),
    }

    let mut raws = Vec::new();
    let mut statement = conn.prepare(
        "SELECT e.id, e.title, e.location, e.all_day, e.start_at, e.end_at, e.start_date, e.end_date, e.pigment,
                e.source, t.id, t.status = 'done'
         FROM events e LEFT JOIN tasks t ON t.id = e.task_id AND t.deleted_at IS NULL
         WHERE e.deleted_at IS NULL AND (
           (e.all_day = 0 AND e.start_at < ?2 AND e.end_at > ?1)
           OR (e.all_day = 1 AND e.start_date <= ?4 AND e.end_date >= ?3))
         ORDER BY e.all_day DESC, e.start_at, e.title",
    )?;
    let rows = statement.query_map(
        params![instant(first_day.start), instant(last_day.end), date_text(first_day.date), date_text(last_day.date)],
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
                row.get::<_, Source>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<bool>>(11)?,
            ))
        },
    )?;
    for row in rows {
        let (id, title, location, all_day, start_at, end_at, start_date, end_date, pigment, source, task_id, done) = row?;
        let span = match (all_day, start_at, end_at, start_date, end_date) {
            (false, Some(start), Some(end), _, _) => Span2::Timed(parse_instant("startAt", &start)?, parse_instant("endAt", &end)?),
            (true, _, _, Some(start), Some(end)) => Span2::AllDay(parse_date("startDate", &start)?, parse_date("endDate", &end)?),
            _ => continue,
        };
        raws.push(Raw {
            key: id.clone(),
            kind: if task_id.is_some() { ItemKind::Block } else { ItemKind::Event },
            event_id: Some(id),
            routine_id: None,
            rule_date: None,
            task_done: done.unwrap_or(false),
            task_id,
            title,
            location,
            pigment,
            source,
            moved: false,
            span,
        });
    }
    for occurrence in routines::occurrences(conn, &days, tz)? {
        let rule_date = date_text(occurrence.rule_date);
        raws.push(Raw {
            key: format!("{}:{rule_date}", occurrence.routine.id),
            kind: ItemKind::Routine,
            event_id: None,
            routine_id: Some(occurrence.routine.id.clone()),
            rule_date: Some(rule_date),
            task_id: None,
            task_done: false,
            title: occurrence.routine.title.clone(),
            location: occurrence.routine.location.clone(),
            pigment: occurrence.routine.pigment,
            source: occurrence.routine.source,
            moved: occurrence.moved,
            span: Span2::Timed(occurrence.start, occurrence.end),
        });
    }

    let mut reminders = conn.prepare(
        "SELECT id, text, due_at, state FROM reminders
         WHERE deleted_at IS NULL AND state IN ('scheduled', 'snoozed', 'fired') AND due_at >= ?1 AND due_at < ?2
         ORDER BY due_at",
    )?;
    let reminder_rows: Vec<(String, String, String, ReminderState)> = reminders
        .query_map(params![instant(first_day.start), instant(last_day.end)], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::new();
    for day in &days {
        let date = date_text(day.date);
        let mut items = Vec::new();
        for raw in &raws {
            let (all_day, start_minute, end_minute, before, after) = match raw.span {
                Span2::Timed(start, end) => {
                    if !(start < day.end && end > day.start) {
                        continue;
                    }
                    (false, day.minute(start), day.minute(end), start < day.start, end > day.end)
                }
                Span2::AllDay(start, end) => {
                    if !(start <= day.date && end >= day.date) {
                        continue;
                    }
                    (true, 0, day.minute(day.end), start < day.date, end > day.date)
                }
            };
            items.push(CalendarItem {
                key: raw.key.clone(),
                kind: raw.kind,
                event_id: raw.event_id.clone(),
                routine_id: raw.routine_id.clone(),
                rule_date: raw.rule_date.clone(),
                task_id: raw.task_id.clone(),
                task_done: raw.task_done,
                title: raw.title.clone(),
                location: raw.location.clone(),
                all_day,
                start_minute,
                end_minute,
                continues_before: before,
                continues_after: after,
                moved: raw.moved,
                pigment: raw.pigment,
                source: raw.source,
            });
        }
        items.sort_by(|a, b| {
            b.all_day.cmp(&a.all_day).then(a.start_minute.cmp(&b.start_minute)).then(a.title.cmp(&b.title))
        });

        let day_reminders = reminder_rows
            .iter()
            .filter_map(|(id, text, due, state)| {
                let due = parse_instant("dueAt", due).ok()?;
                (due >= day.start && due < day.end).then(|| DayReminder {
                    id: id.clone(),
                    text: text.clone(),
                    minute: day.minute(due),
                    state: *state,
                })
            })
            .collect();

        let scheduled: i32 = items
            .iter()
            .filter(|item| !item.all_day)
            .map(|item| (item.end_minute.min(waking.sleep) - item.start_minute.max(waking.wake)).max(0))
            .sum();
        let planned: i64 = conn.query_row(
            "SELECT coalesce(sum(estimate_min), 0) FROM tasks t
             WHERE t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status IN ('todo', 'doing') AND t.do_date = ?1
               AND NOT EXISTS (SELECT 1 FROM events e WHERE e.task_id = t.id AND e.deleted_at IS NULL)",
            [&date],
            |row| row.get(0),
        )?;
        let waking_min = (waking.sleep - waking.wake).max(0) as u32;
        let scheduled_min = scheduled as u32;
        let planned_min = planned.max(0) as u32;
        let closed = conn
            .query_row("SELECT 1 FROM day_closures WHERE date = ?1", [&date], |_| Ok(()))
            .map(|_| true)
            .or_else(|error| if error == rusqlite::Error::QueryReturnedNoRows { Ok(false) } else { Err(error) })?;
        out.push(CalendarDay {
            date,
            items,
            reminders: day_reminders,
            load: DayLoad {
                scheduled_min,
                planned_min,
                waking_min,
                over: waking_min > 0 && (scheduled_min + planned_min) * 100 > waking_min * OVER_PERCENT,
            },
            closed,
        });
    }
    Ok(out)
}

/// One day.
pub fn day(conn: &Connection, date: Date, tz: &TimeZone, waking: Waking) -> Result<CalendarDay> {
    Ok(range(conn, date, 1, tz, waking)?.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::events::{self, EventInput};
    use crate::model::{Actor, RoutineKind};
    use crate::recurrence::{Day as WeekDay, Rule};
    use crate::routines::{self, RoutineInput};
    use crate::tasks::{self, TaskInput};

    fn tz() -> TimeZone {
        TimeZone::get("Asia/Jakarta").unwrap()
    }

    fn now() -> Timestamp {
        "2026-10-23T03:00:00Z".parse().unwrap()
    }

    const WAKING: Waking = Waking { wake: 5 * 60, sleep: 22 * 60 + 30 };

    fn timed(title: &str, date: &str, start: i32, end: i32) -> EventInput {
        EventInput { title: title.into(), date: date.into(), start_minute: start, end_minute: end, ..Default::default() }
    }

    #[test]
    fn a_week_holds_events_routines_blocks_and_reminders() {
        let mut conn = open_in_memory().unwrap();
        events::create(&mut conn, timed("Rapat OSIS", "2026-10-23", 13 * 60, 14 * 60), &tz(), Actor::Owner, now()).unwrap();
        // Over midnight: 23.00 Friday to 01.00 Saturday.
        events::create(&mut conn, timed("Begadang CTF", "2026-10-23", 23 * 60, 25 * 60), &tz(), Actor::Owner, now()).unwrap();
        let all_day = EventInput { title: "Ujian".into(), all_day: true, date: "2026-10-26".into(), end_date: Some("2026-10-27".into()), ..Default::default() };
        events::create(&mut conn, all_day, &tz(), Actor::Owner, now()).unwrap();
        let (task, _) = tasks::create(&mut conn, TaskInput { title: "Laporan Lab".into(), ..Default::default() }, Actor::Owner, now()).unwrap();
        events::create(&mut conn, EventInput { task_id: Some(task.clone()), ..timed("Laporan Lab", "2026-10-24", 16 * 60, 17 * 60) }, &tz(), Actor::Owner, now()).unwrap();
        routines::create(
            &mut conn,
            RoutineInput {
                title: "Matematika".into(),
                kind: RoutineKind::Lesson,
                rule: Rule::weekly(&[WeekDay::Fr]),
                start_time: "07:00".into(),
                duration_min: 90,
                starts_on: "2026-10-01".into(),
                ends_on: None,
                location: String::new(),
                pigment: Pigment::Lazuardi,
                project_id: None,
            },
            Actor::Owner,
            now(),
        )
        .unwrap();
        let due = events::local_instant("2026-10-23".parse().unwrap(), 21 * 60, &tz()).unwrap();
        crate::reminders::create(&mut conn, "Kumpulkan laporan", due, Actor::Owner, now()).unwrap();

        let week = range(&conn, "2026-10-23".parse().unwrap(), 5, &tz(), WAKING).unwrap();
        let friday = &week[0];
        let titles: Vec<(&str, i32, i32, ItemKind)> =
            friday.items.iter().map(|i| (i.title.as_str(), i.start_minute, i.end_minute, i.kind)).collect();
        assert_eq!(
            titles,
            [
                ("Matematika", 420, 510, ItemKind::Routine),
                ("Rapat OSIS", 780, 840, ItemKind::Event),
                ("Begadang CTF", 1380, 1440, ItemKind::Event),
            ]
        );
        assert!(friday.items[2].continues_after);
        assert_eq!(friday.reminders[0].minute, 21 * 60);
        // Waking hours 05.00 to 22.30: lesson 90 + meeting 60 + CTF until 22.30 = 0.
        assert_eq!(friday.load.scheduled_min, 150);

        let saturday = &week[1];
        assert_eq!(saturday.items[0].title, "Begadang CTF");
        assert!(saturday.items[0].continues_before);
        assert_eq!((saturday.items[0].start_minute, saturday.items[0].end_minute), (0, 60));
        let block = saturday.items.iter().find(|i| i.kind == ItemKind::Block).unwrap();
        assert_eq!(block.task_id.as_deref(), Some(task.as_str()));
        tasks::complete(&mut conn, &task, Actor::Owner, now()).unwrap();
        let saturday = day(&conn, "2026-10-24".parse().unwrap(), &tz(), WAKING).unwrap();
        assert!(saturday.items.iter().any(|i| i.kind == ItemKind::Block && i.task_done));

        let monday = &week[3];
        assert!(monday.items[0].all_day && monday.items[0].continues_after);
        let tuesday = &week[4];
        assert!(tuesday.items[0].all_day && tuesday.items[0].continues_before && !tuesday.items[0].continues_after);
    }

    #[test]
    fn an_over_full_day_is_named() {
        let mut conn = open_in_memory().unwrap();
        events::create(&mut conn, timed("Sekolah", "2026-10-23", 7 * 60, 15 * 60), &tz(), Actor::Owner, now()).unwrap();
        let today = day(&conn, "2026-10-23".parse().unwrap(), &tz(), WAKING).unwrap();
        assert!(!today.load.over);
        for title in ["Laporan", "Presentasi", "Belajar"] {
            tasks::create(
                &mut conn,
                TaskInput { title: title.into(), do_date: Some("2026-10-23".into()), estimate_min: Some(180), ..Default::default() },
                Actor::Owner,
                now(),
            )
            .unwrap();
        }
        let today = day(&conn, "2026-10-23".parse().unwrap(), &tz(), WAKING).unwrap();
        assert_eq!((today.load.scheduled_min, today.load.planned_min, today.load.waking_min), (480, 540, 1050));
        assert!(today.load.over);
    }
}
