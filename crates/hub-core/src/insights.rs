//! Jejak: the timeline of what changed, by whom, filtered and paged; and this week against the
//! owner's four-week average (docs/design.md, "Jejak"). Plain counts from the rows themselves,
//! never a score.

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, ToSpan};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::ActivityItem;
use crate::error::{invalid, Reason, Result};
use crate::learning;
use crate::model::Actor;
use crate::time::{date_text, instant};

/// The timeline's filter groups, closer to how the owner thinks than the tables are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TrailGroup {
    Tasks,
    Projects,
    Calendar,
    Files,
    Learning,
    Notes,
    /// Studio: projects, PRDs, roadmaps, checkpoints, Claude's tasks, reports, requests.
    Studio,
}

impl TrailGroup {
    fn entities(self) -> &'static [&'static str] {
        match self {
            TrailGroup::Tasks => &["task", "focus"],
            TrailGroup::Projects => &["project", "milestone", "area"],
            TrailGroup::Calendar => &["event", "routine", "reminder"],
            TrailGroup::Files => &["file", "collection"],
            TrailGroup::Learning => &["track", "learning_item", "learning_session", "ctf_event"],
            TrailGroup::Notes => &["journal"],
            TrailGroup::Studio => &[
                "studio_project",
                "studio_ledger",
                "studio_stack",
                "studio_prd",
                "studio_roadmap",
                "studio_checkpoint",
                "studio_task",
                "studio_report",
                "studio_request",
            ],
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TrailQuery {
    #[ts(optional)]
    pub actor: Option<Actor>,
    #[ts(optional)]
    pub group: Option<TrailGroup>,
    /// The `next` of the previous page.
    #[ts(optional)]
    pub before: Option<String>,
    #[ts(optional)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrailPage {
    pub items: Vec<ActivityItem>,
    /// Where the next page starts, or None at the beginning of time.
    pub next: Option<String>,
}

/// Newest first; settings changes are not part of the owner's history.
pub fn trail(conn: &Connection, query: &TrailQuery) -> Result<TrailPage> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    // The cursor is "<at>|<id>" of the last row shown.
    let (before_at, before_id) = match query.before.as_deref() {
        None => (None, None),
        Some(cursor) => match cursor.split_once('|') {
            Some((at, id)) if at.len() == 20 && id.len() == 36 => (Some(at.to_string()), Some(id.to_string())),
            _ => return invalid("before", Reason::Format),
        },
    };
    let entities = serde_json::to_string(&query.group.map(|g| g.entities()).unwrap_or(&[]))?;
    let mut statement = conn.prepare(
        "SELECT id, at, actor, verb, entity_type, entity_id, summary FROM activity
         WHERE entity_type <> 'settings'
           AND (?1 IS NULL OR actor = ?1)
           AND (?2 = '[]' OR entity_type IN (SELECT value FROM json_each(?2)))
           AND (?3 IS NULL OR at < ?3 OR (at = ?3 AND id < ?4))
         ORDER BY at DESC, id DESC LIMIT ?5",
    )?;
    let rows = statement.query_map(params![query.actor, entities, before_at, before_id, limit + 1], |row| {
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
    let mut items = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    let next = if items.len() as u32 > limit {
        items.truncate(limit as usize);
        items.last().map(|last| format!("{}|{}", last.at, last.id))
    } else {
        None
    };
    Ok(TrailPage { items, next })
}

/// One measure: this week, the average of the four weeks before, and eight weeks for a sparkline.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Measure {
    pub value: u32,
    pub average: f64,
    /// Oldest first; the last is this week.
    pub weeks: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WeekInsight {
    /// Monday of this week.
    pub week_start: String,
    pub tasks_done: Measure,
    pub learning_minutes: Measure,
    pub active_days: Measure,
    pub files_added: Measure,
    pub projects_moved: Measure,
}

const WEEKS: usize = 8;
const AVERAGE_OF: usize = 4;

fn measure(weeks: Vec<u32>) -> Measure {
    let value = weeks.last().copied().unwrap_or(0);
    let before = &weeks[weeks.len().saturating_sub(AVERAGE_OF + 1)..weeks.len().saturating_sub(1)];
    let average = if before.is_empty() { 0.0 } else { f64::from(before.iter().sum::<u32>()) / before.len() as f64 };
    Measure { value, average: (average * 10.0).round() / 10.0, weeks }
}

fn count_between(conn: &Connection, sql: &str, from: &str, to: &str) -> Result<u32> {
    Ok(conn.query_row(sql, params![from, to], |row| row.get(0))?)
}

pub fn week(conn: &Connection, now: Timestamp, tz: &TimeZone) -> Result<WeekInsight> {
    let today = now.to_zoned(tz.clone()).date();
    let monday = today.checked_sub(i64::from(today.weekday().to_monday_zero_offset()).days())?;
    let first: Date = monday.checked_sub((7 * (WEEKS as i64 - 1)).days())?;
    let mut tasks = Vec::with_capacity(WEEKS);
    let mut minutes = Vec::with_capacity(WEEKS);
    let mut days = Vec::with_capacity(WEEKS);
    let mut files = Vec::with_capacity(WEEKS);
    let mut projects = Vec::with_capacity(WEEKS);
    for index in 0..WEEKS as i64 {
        let start_date = first.checked_add((7 * index).days())?;
        let start = start_date.to_zoned(tz.clone())?.timestamp();
        let end = start_date.checked_add(7.days())?.to_zoned(tz.clone())?.timestamp();
        let (from, to) = (instant(start), instant(end));
        tasks.push(count_between(
            conn,
            "SELECT count(*) FROM tasks WHERE deleted_at IS NULL AND status = 'done' AND completed_at >= ?1 AND completed_at < ?2",
            &from,
            &to,
        )?);
        files.push(count_between(conn, "SELECT count(*) FROM files WHERE deleted_at IS NULL AND added_at >= ?1 AND added_at < ?2", &from, &to)?);
        projects.push(count_between(
            conn,
            "SELECT count(*) FROM (
               SELECT project_id FROM tasks WHERE project_id IS NOT NULL AND deleted_at IS NULL AND status = 'done'
                 AND completed_at >= ?1 AND completed_at < ?2
               UNION
               SELECT project_id FROM milestones WHERE deleted_at IS NULL AND done_at >= ?1 AND done_at < ?2)",
            &from,
            &to,
        )?);
        let (active, spent) = learning::active_days_between(conn, start, end, tz)?;
        days.push(active);
        minutes.push(spent);
    }
    Ok(WeekInsight {
        week_start: date_text(monday),
        tasks_done: measure(tasks),
        learning_minutes: measure(minutes),
        active_days: measure(days),
        files_added: measure(files),
        projects_moved: measure(projects),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::{self, Record};
    use crate::db::open_in_memory;
    use crate::model::{Entity, Verb};
    use crate::tasks::{self, TaskInput};

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    #[test]
    fn the_week_is_held_against_the_four_before_it() {
        let mut conn = open_in_memory().unwrap();
        let tz = TimeZone::get("Asia/Jakarta").unwrap();
        // One task done in each of the four weeks before, three this week (Monday 19 October).
        for done in ["2026-09-22T03:00:00Z", "2026-09-29T03:00:00Z", "2026-10-06T03:00:00Z", "2026-10-13T03:00:00Z"] {
            let (id, _) = tasks::create(&mut conn, TaskInput { title: "lama".into(), ..Default::default() }, Actor::Owner, at(done)).unwrap();
            tasks::complete(&mut conn, &id, Actor::Owner, at(done)).unwrap();
        }
        for done in ["2026-10-19T03:00:00Z", "2026-10-20T03:00:00Z", "2026-10-22T03:00:00Z"] {
            let (id, _) = tasks::create(&mut conn, TaskInput { title: "baru".into(), ..Default::default() }, Actor::Owner, at(done)).unwrap();
            tasks::complete(&mut conn, &id, Actor::Owner, at(done)).unwrap();
        }
        let insight = week(&conn, at("2026-10-23T05:00:00Z"), &tz).unwrap();
        assert_eq!(insight.week_start, "2026-10-19");
        assert_eq!(insight.tasks_done.value, 3);
        assert_eq!(insight.tasks_done.average, 1.0);
        assert_eq!(insight.tasks_done.weeks, [0, 0, 0, 1, 1, 1, 1, 3]);
        assert_eq!(insight.files_added.value, 0);
    }

    #[test]
    fn the_trail_pages_and_filters() {
        let conn = open_in_memory().unwrap();
        for (index, (actor, entity)) in
            [(Actor::Owner, Entity::Task), (Actor::Ultron, Entity::Task), (Actor::Owner, Entity::File), (Actor::Owner, Entity::Settings)]
                .into_iter()
                .enumerate()
        {
            activity::record(
                &conn,
                at(&format!("2026-10-23T0{index}:00:00Z")),
                Record { actor, verb: Verb::Created, entity, entity_id: None, summary: "x", details: serde_json::json!({}) },
            )
            .unwrap();
        }
        let first = trail(&conn, &TrailQuery { limit: Some(2), ..Default::default() }).unwrap();
        assert_eq!(first.items.len(), 2);
        assert_eq!(first.items[0].entity, Entity::File, "settings are left out, newest first");
        let second = trail(&conn, &TrailQuery { limit: Some(2), before: first.next.clone(), ..Default::default() }).unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.next, None);
        let ultron = trail(&conn, &TrailQuery { actor: Some(Actor::Ultron), ..Default::default() }).unwrap();
        assert_eq!(ultron.items.len(), 1);
        let files = trail(&conn, &TrailQuery { group: Some(TrailGroup::Files), ..Default::default() }).unwrap();
        assert_eq!(files.items.len(), 1);
        assert!(trail(&conn, &TrailQuery { before: Some("x".into()), ..Default::default() }).is_err());
    }
}
