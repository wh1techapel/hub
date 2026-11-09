//! Tutup Hari: the evening review. What got done, what slipped (and where it goes), one optional
//! sentence, tomorrow's focus, then the day is closed. All of it is one transaction.

use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::calendar::{self, ItemKind, Waking};
use crate::cards::{task_card, TaskCard, TASK_COLUMNS, TASK_JOINS};
use crate::error::{invalid, Reason, Result};
use crate::ids;
use crate::journal;
use crate::model::{Actor, Change, Entity, JournalKind, Verb};
use crate::tasks::{self, FOCUS_MAX};
use crate::time::{date_text, instant, parse_date, Day};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DoneKind {
    Task,
    Event,
    Routine,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DoneItem {
    pub title: String,
    pub kind: DoneKind,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DayReview {
    pub date: String,
    pub tomorrow: String,
    pub closed: bool,
    pub done: Vec<DoneItem>,
    /// Open tasks planned for the day or earlier (focus first).
    pub slipped: Vec<TaskCard>,
    /// What Hub would put in focus tomorrow, slipped tasks included.
    pub suggestions: Vec<TaskCard>,
    /// Tomorrow's focus if it was already chosen.
    pub tomorrow_focus: Vec<TaskCard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case", tag = "to", content = "date")]
#[ts(export)]
pub enum Destination {
    Tomorrow,
    Date(String),
    /// Take the do date away: it goes back to the lists, not to a day.
    LetGo,
    /// Leave it where it is.
    Keep,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Slip {
    pub task_id: String,
    pub destination: Destination,
}

#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CloseInput {
    pub date: String,
    #[serde(default)]
    pub slips: Vec<Slip>,
    #[serde(default)]
    pub reflection: Option<String>,
    #[serde(default)]
    pub tomorrow_focus: Vec<String>,
}

fn tomorrow_of(date: &str) -> Result<String> {
    Ok(date_text(parse_date("date", date)?.checked_add(Span::new().days(1))?))
}

pub fn is_closed(conn: &Connection, date: &str) -> Result<bool> {
    Ok(conn.query_row("SELECT 1 FROM day_closures WHERE date = ?1", [date], |_| Ok(())).optional()?.is_some())
}

pub fn review(conn: &Connection, date: &str, tz: &TimeZone, waking: Waking, now: Timestamp) -> Result<DayReview> {
    let day_date = parse_date("date", date)?;
    let day = Day::of(day_date, tz)?;
    let tomorrow = tomorrow_of(date)?;

    let mut done = Vec::new();
    let mut statement = conn.prepare(
        "SELECT title FROM tasks WHERE deleted_at IS NULL AND parent_id IS NULL AND status = 'done'
           AND completed_at >= ?1 AND completed_at < ?2 ORDER BY completed_at",
    )?;
    for title in statement.query_map(params![instant(day.start), instant(day.end)], |row| row.get::<_, String>(0))? {
        done.push(DoneItem { title: title?, kind: DoneKind::Task });
    }
    let past_minute = if now >= day.end { i32::MAX } else { day.minute(now) };
    for item in calendar::day(conn, day_date, tz, waking)?.items {
        if item.all_day || item.end_minute > past_minute || item.kind == ItemKind::Block {
            continue;
        }
        let kind = if item.kind == ItemKind::Routine { DoneKind::Routine } else { DoneKind::Event };
        done.push(DoneItem { title: item.title, kind });
    }

    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks t {TASK_JOINS}
         LEFT JOIN focus f ON f.task_id = t.id AND f.date = ?1
         WHERE t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status IN ('todo', 'doing')
           AND (f.task_id IS NOT NULL OR t.do_date <= ?1)
         ORDER BY f.slot IS NULL, f.slot, t.order_key"
    );
    let mut statement = conn.prepare(&sql)?;
    let slipped = statement.query_map([date], task_card)?.collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(DayReview {
        closed: is_closed(conn, date)?,
        suggestions: tasks::suggest_focus(conn, &tomorrow)?,
        tomorrow_focus: tasks::focus_of(conn, &tomorrow)?,
        date: date.to_string(),
        tomorrow,
        done,
        slipped,
    })
}

/// Close the day: move what slipped, keep the sentence, set tomorrow's focus, remember the close.
pub fn close(conn: &mut Connection, input: CloseInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let tomorrow = tomorrow_of(&input.date)?;
    if input.tomorrow_focus.len() > FOCUS_MAX {
        return invalid("tomorrowFocus", Reason::TooMany);
    }
    let reflection = input.reflection.as_deref().map(str::trim).filter(|text| !text.is_empty());
    let tx = conn.transaction()?;
    if tx.query_row("SELECT 1 FROM day_closures WHERE date = ?1", [&input.date], |_| Ok(())).optional()?.is_some() {
        return invalid("date", Reason::Order);
    }
    for slip in &input.slips {
        let id = ids::parse("taskId", &slip.task_id)?;
        let target = match &slip.destination {
            Destination::Keep => continue,
            Destination::Tomorrow => Some(tomorrow.clone()),
            Destination::Date(date) => Some(date_text(parse_date("destination", date)?)),
            Destination::LetGo => None,
        };
        let changed = tx.execute(
            "UPDATE tasks SET do_date = ?2, updated_at = ?3 WHERE id = ?1 AND deleted_at IS NULL",
            params![id, target, instant(now)],
        )?;
        if changed == 0 {
            return invalid("taskId", Reason::Unknown);
        }
    }
    let reflection_id = match reflection {
        Some(text) => Some(journal::create_in(&tx, &input.date, text, JournalKind::Reflection, actor, now)?),
        None => None,
    };
    if !input.tomorrow_focus.is_empty() {
        let ids: Vec<&str> = input.tomorrow_focus.iter().map(String::as_str).collect();
        tasks::set_focus_in(&tx, &tomorrow, &ids, actor, now)?;
    }
    tx.execute(
        "INSERT INTO day_closures (date, closed_at, reflection_id) VALUES (?1, ?2, ?3)",
        params![input.date, instant(now), reflection_id],
    )?;
    let moved = input.slips.iter().filter(|slip| slip.destination != Destination::Keep).count();
    activity::record(
        &tx,
        now,
        Record {
            actor,
            verb: Verb::Closed,
            entity: Entity::Focus,
            entity_id: None,
            summary: &input.date,
            details: serde_json::json!({ "moved": moved, "reflection": reflection.is_some(), "tomorrowFocus": input.tomorrow_focus.len() }),
        },
    )?;
    tx.commit()?;
    Ok(Change::all(Entity::Focus))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::events::{self, EventInput};
    use crate::tasks::TaskInput;

    fn tz() -> TimeZone {
        TimeZone::get("Asia/Jakarta").unwrap()
    }

    const WAKING: Waking = Waking { wake: 300, sleep: 1350 };

    fn task(conn: &mut Connection, title: &str, do_date: Option<&str>, now: Timestamp) -> String {
        tasks::create(conn, TaskInput { title: title.into(), do_date: do_date.map(Into::into), ..Default::default() }, Actor::Owner, now)
            .unwrap()
            .0
    }

    #[test]
    fn the_evening_review_and_close() {
        let mut conn = open_in_memory().unwrap();
        let morning: Timestamp = "2026-10-23T00:00:00Z".parse().unwrap(); // 07.00 WIB
        let evening: Timestamp = "2026-10-23T14:00:00Z".parse().unwrap(); // 21.00 WIB
        let lab = task(&mut conn, "Laporan Lab", Some("2026-10-23"), morning);
        let nmap = task(&mut conn, "THM Nmap", Some("2026-10-22"), morning);
        let slides = task(&mut conn, "Slide KIK", None, morning);
        let clean = task(&mut conn, "Rapikan meja", Some("2026-10-23"), morning);
        tasks::set_focus(&mut conn, "2026-10-23", &[&lab, &slides], Actor::Owner, morning).unwrap();
        tasks::complete(&mut conn, &clean, Actor::Owner, "2026-10-23T10:00:00Z".parse().unwrap()).unwrap();
        events::create(
            &mut conn,
            EventInput { title: "Rapat OSIS".into(), date: "2026-10-23".into(), start_minute: 13 * 60, end_minute: 14 * 60, ..Default::default() },
            &tz(),
            Actor::Owner,
            morning,
        )
        .unwrap();

        let review = review(&conn, "2026-10-23", &tz(), WAKING, evening).unwrap();
        assert!(!review.closed);
        let done: Vec<(&str, DoneKind)> = review.done.iter().map(|d| (d.title.as_str(), d.kind)).collect();
        assert_eq!(done, [("Rapikan meja", DoneKind::Task), ("Rapat OSIS", DoneKind::Event)]);
        let slipped: Vec<&str> = review.slipped.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(slipped, ["Laporan Lab", "Slide KIK", "THM Nmap"], "focus first, then the rest");
        assert_eq!(review.tomorrow, "2026-10-24");

        let input = CloseInput {
            date: "2026-10-23".into(),
            slips: vec![
                Slip { task_id: lab.clone(), destination: Destination::Tomorrow },
                Slip { task_id: nmap.clone(), destination: Destination::Date("2026-10-26".into()) },
                Slip { task_id: slides.clone(), destination: Destination::LetGo },
            ],
            reflection: Some("  Praktikum lancar, laporan belum.  ".into()),
            tomorrow_focus: vec![lab.clone()],
        };
        close(&mut conn, input.clone(), Actor::Owner, evening).unwrap();
        assert!(close(&mut conn, input, Actor::Owner, evening).is_err(), "a day closes once");

        let do_date = |conn: &Connection, id: &str| -> Option<String> {
            conn.query_row("SELECT do_date FROM tasks WHERE id = ?1", [id], |row| row.get(0)).unwrap()
        };
        assert_eq!(do_date(&conn, &lab).as_deref(), Some("2026-10-24"));
        assert_eq!(do_date(&conn, &nmap).as_deref(), Some("2026-10-26"));
        assert_eq!(do_date(&conn, &slides), None);
        let review = super::review(&conn, "2026-10-23", &tz(), WAKING, evening).unwrap();
        assert!(review.closed);
        assert_eq!(review.tomorrow_focus.len(), 1);
        let journal = journal::on(&conn, "2026-10-23").unwrap();
        assert_eq!(journal[0].text, "Praktikum lancar, laporan belum.");
        assert_eq!(journal[0].kind, JournalKind::Reflection);
    }

    #[test]
    fn a_failed_close_changes_nothing() {
        let mut conn = open_in_memory().unwrap();
        let now: Timestamp = "2026-10-23T14:00:00Z".parse().unwrap();
        let lab = task(&mut conn, "Laporan Lab", Some("2026-10-23"), now);
        let input = CloseInput {
            date: "2026-10-23".into(),
            slips: vec![
                Slip { task_id: lab.clone(), destination: Destination::Tomorrow },
                Slip { task_id: crate::ids::new_id(), destination: Destination::Tomorrow },
            ],
            reflection: None,
            tomorrow_focus: vec![],
        };
        assert!(close(&mut conn, input, Actor::Owner, now).is_err());
        assert!(!is_closed(&conn, "2026-10-23").unwrap());
        let do_date: Option<String> = conn.query_row("SELECT do_date FROM tasks WHERE id = ?1", [&lab], |row| row.get(0)).unwrap();
        assert_eq!(do_date.as_deref(), Some("2026-10-23"));
    }
}
