//! Tasks, subtasks, the lists the Tugas page shows, and the day's Fokus.
//!
//! A task has a do date (when to work on it) separate from its deadline. One level of subtasks.
//! Lists keep the owner's own order (`order_key`), except Mendatang (by date) and Selesai (newest
//! first). A task whose do date passed comes back on Hari Ini until it is done or moved (owner's
//! choice, CONTEXT.md).

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, Record};
use crate::cards::{task_card, TaskCard, TASK_COLUMNS, TASK_JOINS};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, Source, TaskStatus, Verb};
use crate::order;
use crate::time::{instant, parse_clock, parse_date, Day};

const TITLE_MAX: usize = 300;
const NOTES_MAX: usize = 10_000;
pub const FOCUS_MAX: usize = 3;
const DONE_LIST_MAX: u32 = 200;

/// A task's editable fields: what a new task gets, and what an edit replaces as a whole.
#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct TaskInput {
    pub title: String,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub priority: u8,
    #[serde(default)]
    pub do_date: Option<String>,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub due_time: Option<String>,
    #[serde(default)]
    pub estimate_min: Option<u32>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub area_id: Option<String>,
    /// Only on creation: the task this one is a step of.
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// The name used before CP3; kept for the seed and older tests.
pub type NewTask = TaskInput;

struct Checked {
    title: String,
    notes: String,
    project_id: Option<String>,
    area_id: Option<String>,
    parent_id: Option<String>,
}

fn check(input: &TaskInput) -> Result<Checked> {
    let title = input.title.trim().to_string();
    if title.is_empty() || title.chars().count() > TITLE_MAX {
        return invalid("title", Reason::Length);
    }
    if input.notes.chars().count() > NOTES_MAX {
        return invalid("notes", Reason::Length);
    }
    if input.priority > 3 {
        return invalid("priority", Reason::Format);
    }
    if let Some(date) = &input.do_date {
        parse_date("doDate", date)?;
    }
    if let Some(date) = &input.due_date {
        parse_date("dueDate", date)?;
    }
    if let Some(time) = &input.due_time {
        if input.due_date.is_none() {
            return invalid("dueTime", Reason::Order);
        }
        parse_clock("dueTime", time)?;
    }
    if input.estimate_min == Some(0) || input.estimate_min.is_some_and(|m| m > 24 * 60) {
        return invalid("estimateMin", Reason::Format);
    }
    let parse = |field, value: &Option<String>| value.as_deref().map(|id| ids::parse(field, id)).transpose();
    Ok(Checked {
        title,
        notes: input.notes.trim_end().to_string(),
        project_id: parse("projectId", &input.project_id)?,
        area_id: parse("areaId", &input.area_id)?,
        parent_id: parse("parentId", &input.parent_id)?,
    })
}

fn exists(tx: &Transaction<'_>, table: &str, id: &str) -> Result<bool> {
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1 AND deleted_at IS NULL");
    Ok(tx.query_row(&sql, [id], |_| Ok(())).optional()?.is_some())
}

fn check_links(tx: &Transaction<'_>, checked: &Checked) -> Result<()> {
    if let Some(project) = &checked.project_id
        && !exists(tx, "projects", project)?
    {
        return invalid("projectId", Reason::Unknown);
    }
    if let Some(area) = &checked.area_id
        && !exists(tx, "areas", area)?
    {
        return invalid("areaId", Reason::Unknown);
    }
    if let Some(parent) = &checked.parent_id {
        // One level of steps: a step cannot have steps of its own.
        let top: Option<bool> = tx
            .query_row(
                "SELECT parent_id IS NULL FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
                [parent],
                |row| row.get(0),
            )
            .optional()?;
        if top != Some(true) {
            return invalid("parentId", Reason::Unknown);
        }
    }
    Ok(())
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, id: &str, title: &str) -> Result<()> {
    activity::record(
        tx,
        now,
        Record { actor, verb, entity: Entity::Task, entity_id: Some(id), summary: title, details: serde_json::json!({}) },
    )
}

pub fn create(conn: &mut Connection, input: TaskInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    create_as(conn, input, actor, Source::from(actor), now)
}

/// A new task that came from somewhere other than its actor (`Source::Import`: ULTRON's old list).
pub fn create_as(conn: &mut Connection, input: TaskInput, actor: Actor, source: Source, now: Timestamp) -> Result<(String, Change)> {
    let checked = check(&input)?;
    let tx = conn.transaction()?;
    check_links(&tx, &checked)?;
    let last: Option<String> = match &checked.parent_id {
        Some(parent) => tx.query_row("SELECT max(order_key) FROM tasks WHERE parent_id = ?1", [parent], |row| row.get(0))?,
        None => tx.query_row("SELECT max(order_key) FROM tasks WHERE parent_id IS NULL", [], |row| row.get(0))?,
    };
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO tasks (id, title, notes, priority, do_date, due_date, due_time, estimate_min,
                            project_id, area_id, parent_id, order_key, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
        params![
            id,
            checked.title,
            checked.notes,
            input.priority,
            input.do_date,
            input.due_date,
            input.due_time,
            input.estimate_min,
            checked.project_id,
            checked.area_id,
            checked.parent_id,
            order::after(last.as_deref()),
            source,
            stamp,
        ],
    )?;
    record(&tx, now, actor, Verb::Created, &id, &checked.title)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Task, id)))
}

/// Replace a task's editable fields. Its place, status, and parent stay.
pub fn update(conn: &mut Connection, id: &str, input: TaskInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let mut checked = check(&input)?;
    checked.parent_id = None;
    let tx = conn.transaction()?;
    check_links(&tx, &checked)?;
    let changed = tx.execute(
        "UPDATE tasks SET title = ?2, notes = ?3, priority = ?4, do_date = ?5, due_date = ?6, due_time = ?7,
                estimate_min = ?8, project_id = ?9, area_id = ?10, updated_at = ?11
         WHERE id = ?1 AND deleted_at IS NULL",
        params![
            id,
            checked.title,
            checked.notes,
            input.priority,
            input.do_date,
            input.due_date,
            input.due_time,
            input.estimate_min,
            checked.project_id,
            checked.area_id,
            instant(now),
        ],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("task"));
    }
    record(&tx, now, actor, Verb::Updated, &id, &checked.title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Task, id))
}

/// Mark a task done. Completing a done task changes nothing and records nothing.
pub fn complete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    set_done(conn, id, true, actor, now)
}

/// Undo a completion.
pub fn reopen(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    set_done(conn, id, false, actor, now)
}

fn set_done(conn: &mut Connection, id: &str, done: bool, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let row: Option<(String, TaskStatus)> = tx
        .query_row(
            "SELECT title, status FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (title, status) = row.ok_or(Error::NotFound("task"))?;
    if (status == TaskStatus::Done) == done {
        return Ok(None);
    }
    let stamp = instant(now);
    if done {
        tx.execute(
            "UPDATE tasks SET status = 'done', completed_at = ?2, updated_at = ?2 WHERE id = ?1",
            params![id, stamp],
        )?;
    } else {
        tx.execute(
            "UPDATE tasks SET status = 'todo', completed_at = NULL, updated_at = ?2 WHERE id = ?1",
            params![id, stamp],
        )?;
    }
    record(&tx, now, actor, if done { Verb::Completed } else { Verb::Reopened }, &id, &title)?;
    tx.commit()?;
    Ok(Some(Change::one(Entity::Task, id)))
}

/// Remove a task and its steps (soft: `restore` brings them back).
pub fn delete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let title: Option<String> = tx
        .query_row("SELECT title FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let title = title.ok_or(Error::NotFound("task"))?;
    let stamp = instant(now);
    tx.execute(
        "UPDATE tasks SET deleted_at = ?2, updated_at = ?2 WHERE (id = ?1 OR parent_id = ?1) AND deleted_at IS NULL",
        params![id, stamp],
    )?;
    record(&tx, now, actor, Verb::Deleted, &id, &title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Task, id))
}

/// Undo a removal: the task and the steps removed with it.
pub fn restore(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT title, deleted_at FROM tasks WHERE id = ?1 AND deleted_at IS NOT NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (title, deleted_at) = row.ok_or(Error::NotFound("task"))?;
    tx.execute(
        "UPDATE tasks SET deleted_at = NULL, updated_at = ?3 WHERE (id = ?1 OR parent_id = ?1) AND deleted_at = ?2",
        params![id, deleted_at, instant(now)],
    )?;
    record(&tx, now, actor, Verb::Restored, &id, &title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Task, id))
}

/// Move a task between two neighbours in a hand-sorted list (either may be missing at an end).
pub fn reorder(conn: &mut Connection, id: &str, before: Option<&str>, after: Option<&str>, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let key = |conn: &Connection, other: Option<&str>| -> Result<Option<String>> {
        match other {
            Some(other) => {
                let other = ids::parse("neighbour", other)?;
                conn.query_row("SELECT order_key FROM tasks WHERE id = ?1", [&other], |row| row.get(0))
                    .optional()?
                    .ok_or(Error::NotFound("task"))
                    .map(Some)
            }
            None => Ok(None),
        }
    };
    let low = key(conn, before)?;
    let high = key(conn, after)?;
    if low.is_none() && high.is_none() {
        return Ok(Change::one(Entity::Task, id));
    }
    if let (Some(low), Some(high)) = (&low, &high)
        && low >= high
    {
        return invalid("neighbour", Reason::Order);
    }
    let new_key = order::between(low.as_deref(), high.as_deref());
    let changed = conn.execute(
        "UPDATE tasks SET order_key = ?2, updated_at = ?3 WHERE id = ?1 AND deleted_at IS NULL",
        params![id, new_key, instant(now)],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("task"));
    }
    // Reordering is not news for Jejak: no activity row.
    Ok(Change::one(Entity::Task, id))
}

/// Put a task on another day, or take its do date away (Tutup Hari's "lepaskan").
pub fn set_do_date(conn: &mut Connection, id: &str, date: Option<&str>, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    if let Some(date) = date {
        parse_date("doDate", date)?;
    }
    let tx = conn.transaction()?;
    let title: Option<String> = tx
        .query_row("SELECT title FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let title = title.ok_or(Error::NotFound("task"))?;
    tx.execute(
        "UPDATE tasks SET do_date = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, date, instant(now)],
    )?;
    activity::record(
        &tx,
        now,
        Record {
            actor,
            verb: Verb::Moved,
            entity: Entity::Task,
            entity_id: Some(&id),
            summary: &title,
            details: serde_json::json!({ "doDate": date }),
        },
    )?;
    tx.commit()?;
    Ok(Change::one(Entity::Task, id))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum TaskView {
    /// Caught but not placed: no project, area, do date, or deadline.
    Inbox,
    /// Do date or deadline today or earlier, and what was finished today.
    Today,
    /// Do date (or, without one, deadline) after today.
    Upcoming,
    /// Every open task.
    Anytime,
    /// Finished, newest first.
    Done,
}

/// One of the Tugas lists. `day` is today in the owner's zone.
pub fn list(conn: &Connection, view: TaskView, day: &Day) -> Result<Vec<TaskCard>> {
    let date = crate::time::date_text(day.date);
    let filter = match view {
        TaskView::Inbox => {
            "t.status IN ('todo', 'doing') AND t.project_id IS NULL AND t.area_id IS NULL
             AND t.do_date IS NULL AND t.due_date IS NULL ORDER BY t.order_key"
        }
        TaskView::Today => {
            "((t.status IN ('todo', 'doing') AND (t.do_date <= ?1 OR t.due_date <= ?1))
              OR (t.status = 'done' AND t.completed_at >= ?2 AND t.completed_at < ?3)) ORDER BY t.order_key"
        }
        TaskView::Upcoming => {
            "t.status IN ('todo', 'doing') AND (t.do_date > ?1 OR (t.do_date IS NULL AND t.due_date > ?1))
             ORDER BY coalesce(t.do_date, t.due_date), t.order_key"
        }
        TaskView::Anytime => "t.status IN ('todo', 'doing') ORDER BY t.order_key",
        TaskView::Done => "t.status = 'done' ORDER BY t.completed_at DESC LIMIT ?4",
    };
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks t {TASK_JOINS}
         WHERE t.deleted_at IS NULL AND t.parent_id IS NULL AND {filter}"
    );
    let mut statement = conn.prepare(&sql)?;
    let start = instant(day.start);
    let end = instant(day.end);
    let rows = match view {
        TaskView::Inbox | TaskView::Upcoming | TaskView::Anytime => statement.query_map([&date], task_card)?,
        TaskView::Today => statement.query_map(params![date, start, end], task_card)?,
        TaskView::Done => statement.query_map(params![date, start, end, DONE_LIST_MAX], task_card)?,
    };
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskCounts {
    pub inbox: u32,
    pub today: u32,
    pub upcoming: u32,
    pub anytime: u32,
}

/// Open tasks per list, for the badges beside the list names.
pub fn counts(conn: &Connection, date: &str) -> Result<TaskCounts> {
    Ok(conn.query_row(
        "SELECT
           count(*) FILTER (WHERE project_id IS NULL AND area_id IS NULL AND do_date IS NULL AND due_date IS NULL),
           count(*) FILTER (WHERE do_date <= ?1 OR due_date <= ?1),
           count(*) FILTER (WHERE do_date > ?1 OR (do_date IS NULL AND due_date > ?1)),
           count(*)
         FROM tasks WHERE deleted_at IS NULL AND parent_id IS NULL AND status IN ('todo', 'doing')",
        [date],
        |row| Ok(TaskCounts { inbox: row.get(0)?, today: row.get(1)?, upcoming: row.get(2)?, anytime: row.get(3)? }),
    )?)
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TimeBlock {
    pub event_id: String,
    pub start: String,
    pub end: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskDetail {
    pub card: TaskCard,
    pub notes: String,
    pub subtasks: Vec<TaskCard>,
    pub blocks: Vec<TimeBlock>,
    pub created_at: String,
    pub files: Vec<crate::files::FileCard>,
}

pub fn detail(conn: &Connection, id: &str, today: &str) -> Result<TaskDetail> {
    let id = ids::parse("id", id)?;
    let sql = format!("SELECT {TASK_COLUMNS} FROM tasks t {TASK_JOINS} WHERE t.id = ?2 AND t.deleted_at IS NULL");
    let card = conn.query_row(&sql, params![today, id], task_card).optional()?.ok_or(Error::NotFound("task"))?;
    let (notes, created_at): (String, String) =
        conn.query_row("SELECT notes, created_at FROM tasks WHERE id = ?1", [&id], |row| Ok((row.get(0)?, row.get(1)?)))?;
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks t {TASK_JOINS}
         WHERE t.parent_id = ?2 AND t.deleted_at IS NULL AND t.status <> 'cancelled' ORDER BY t.order_key"
    );
    let mut statement = conn.prepare(&sql)?;
    let subtasks = statement.query_map(params![today, id], task_card)?.collect::<rusqlite::Result<Vec<_>>>()?;
    let mut statement = conn.prepare(
        "SELECT id, start_at, end_at FROM events WHERE task_id = ?1 AND deleted_at IS NULL AND all_day = 0 ORDER BY start_at",
    )?;
    let blocks = statement
        .query_map([&id], |row| Ok(TimeBlock { event_id: row.get(0)?, start: row.get(1)?, end: row.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let target = crate::files::LinkTarget { kind: crate::model::LinkKind::Task, id: id.clone() };
    let files = crate::files::list(conn, &crate::files::FileQuery { target: Some(target), limit: Some(100), ..Default::default() })?;
    Ok(TaskDetail { card, notes, subtasks, blocks, created_at, files })
}

/// A task's editable fields as stored (not as shown: a card's area may come from its project), so
/// a partial edit (the API's PATCH) can change some and keep the rest.
pub fn input_of(conn: &Connection, id: &str) -> Result<TaskInput> {
    let id = ids::parse("id", id)?;
    conn.query_row(
        "SELECT title, notes, priority, do_date, due_date, due_time, estimate_min, project_id, area_id
         FROM tasks WHERE id = ?1 AND deleted_at IS NULL",
        [&id],
        |row| {
            Ok(TaskInput {
                title: row.get(0)?,
                notes: row.get(1)?,
                priority: row.get(2)?,
                do_date: row.get(3)?,
                due_date: row.get(4)?,
                due_time: row.get(5)?,
                estimate_min: row.get(6)?,
                project_id: row.get(7)?,
                area_id: row.get(8)?,
                parent_id: None,
            })
        },
    )
    .optional()?
    .ok_or(Error::NotFound("task"))
}

/// Choose the Fokus for a date: up to three tasks, in order. Replaces the earlier choice.
pub fn set_focus(conn: &mut Connection, date: &str, task_ids: &[&str], actor: Actor, now: Timestamp) -> Result<Change> {
    let tx = conn.transaction()?;
    let change = set_focus_in(&tx, date, task_ids, actor, now)?;
    tx.commit()?;
    Ok(change)
}

pub(crate) fn set_focus_in(tx: &Transaction<'_>, date: &str, task_ids: &[&str], actor: Actor, now: Timestamp) -> Result<Change> {
    parse_date("date", date)?;
    if task_ids.len() > FOCUS_MAX {
        return invalid("taskIds", Reason::TooMany);
    }
    let parsed: Vec<String> = task_ids.iter().map(|id| ids::parse("taskIds", id)).collect::<Result<_>>()?;
    tx.execute("DELETE FROM focus WHERE date = ?1", [date])?;
    let mut titles = Vec::new();
    for (slot, id) in parsed.iter().enumerate() {
        let title: Option<String> = tx
            .query_row("SELECT title FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [id], |row| row.get(0))
            .optional()?;
        let Some(title) = title else {
            return invalid("taskIds", Reason::Unknown);
        };
        tx.execute("INSERT INTO focus (date, slot, task_id) VALUES (?1, ?2, ?3)", params![date, slot as i64 + 1, id])?;
        titles.push(title);
    }
    activity::record(
        tx,
        now,
        Record {
            actor,
            verb: Verb::Focused,
            entity: Entity::Focus,
            entity_id: None,
            summary: date,
            details: serde_json::json!({ "tasks": parsed, "titles": titles }),
        },
    )?;
    Ok(Change::all(Entity::Focus))
}

/// Up to three open tasks Hub would put in focus for `date`: overdue first, then the nearest
/// deadline (within two days), then what is planned for the day, by priority.
pub fn suggest_focus(conn: &Connection, date: &str) -> Result<Vec<TaskCard>> {
    let horizon = parse_date("date", date)?
        .checked_add(jiff::Span::new().days(2))
        .map(crate::time::date_text)
        .unwrap_or_else(|_| date.to_string());
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM tasks t {TASK_JOINS}
         WHERE t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status IN ('todo', 'doing')
           AND t.id NOT IN (SELECT task_id FROM focus WHERE date = ?1)
           AND (t.do_date <= ?1 OR t.due_date <= ?2 OR t.priority >= 2)
         ORDER BY (t.due_date IS NOT NULL AND t.due_date < ?1) DESC,
                  (t.due_date IS NOT NULL AND t.due_date <= ?2) DESC, t.due_date,
                  t.priority DESC, (t.do_date IS NOT NULL AND t.do_date <= ?1) DESC, t.order_key
         LIMIT 3"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map(params![date, horizon], task_card)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// The tasks in focus for a date, in slot order (done ones included).
pub fn focus_of(conn: &Connection, date: &str) -> Result<Vec<TaskCard>> {
    let sql = format!(
        "SELECT {TASK_COLUMNS} FROM focus f JOIN tasks t ON t.id = f.task_id AND t.deleted_at IS NULL
         {TASK_JOINS} WHERE f.date = ?1 ORDER BY f.slot"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([date], task_card)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::activity::recent;
    use crate::db::open_in_memory;
    use crate::model::Verb;
    use jiff::tz::TimeZone;

    fn now() -> Timestamp {
        "2026-10-22T05:00:00Z".parse().unwrap()
    }

    fn day() -> Day {
        Day::containing(now(), &TimeZone::get("Asia/Jakarta").unwrap()).unwrap()
    }

    fn task(conn: &mut Connection, title: &str) -> String {
        create(conn, TaskInput { title: title.into(), ..Default::default() }, Actor::Owner, now()).unwrap().0
    }

    fn with(conn: &mut Connection, input: TaskInput) -> String {
        create(conn, input, Actor::Owner, now()).unwrap().0
    }

    fn titles(cards: &[TaskCard]) -> Vec<&str> {
        cards.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn complete_and_reopen_record_themselves_once() {
        let mut conn = open_in_memory().unwrap();
        let id = task(&mut conn, "Laporan Praktikum 5");
        assert!(complete(&mut conn, &id, Actor::Owner, now()).unwrap().is_some());
        assert!(complete(&mut conn, &id, Actor::Owner, now()).unwrap().is_none());
        let (status, completed): (TaskStatus, Option<String>) = conn
            .query_row("SELECT status, completed_at FROM tasks WHERE id = ?1", [&id], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap();
        assert_eq!(status, TaskStatus::Done);
        assert_eq!(completed.as_deref(), Some("2026-10-22T05:00:00Z"));
        assert!(reopen(&mut conn, &id, Actor::Ultron, now()).unwrap().is_some());
        let verbs: Vec<(Verb, Actor)> = recent(&conn, 10, false).unwrap().into_iter().map(|i| (i.verb, i.actor)).collect();
        assert_eq!(verbs.len(), 3);
        assert!(verbs.contains(&(Verb::Reopened, Actor::Ultron)));
    }

    #[test]
    fn unknown_or_malformed_ids_are_refused() {
        let mut conn = open_in_memory().unwrap();
        assert!(matches!(complete(&mut conn, &new_id(), Actor::Owner, now()), Err(Error::NotFound("task"))));
        assert!(matches!(complete(&mut conn, "' OR 1=1 --", Actor::Owner, now()), Err(Error::Invalid { field: "id", .. })));
    }

    #[test]
    fn input_is_checked_before_anything_is_written() {
        let mut conn = open_in_memory().unwrap();
        let bad = [
            TaskInput { title: "  ".into(), ..Default::default() },
            TaskInput { title: "x".into(), due_time: Some("19:00".into()), ..Default::default() },
            TaskInput { title: "x".into(), do_date: Some("besok".into()), ..Default::default() },
            TaskInput { title: "x".into(), project_id: Some(new_id()), ..Default::default() },
            TaskInput { title: "x".into(), priority: 4, ..Default::default() },
            TaskInput { title: "x".into(), estimate_min: Some(0), ..Default::default() },
            TaskInput { title: "x".into(), parent_id: Some(new_id()), ..Default::default() },
        ];
        for input in bad {
            assert!(create(&mut conn, input, Actor::Owner, now()).is_err());
        }
        let count: i64 = conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get(0)).unwrap();
        assert_eq!(count, 0);
        assert!(recent(&conn, 10, true).unwrap().is_empty());
    }

    #[test]
    fn steps_are_one_level_and_go_with_their_task() {
        let mut conn = open_in_memory().unwrap();
        let parent = task(&mut conn, "Laporan");
        let step = with(&mut conn, TaskInput { title: "Topologi".into(), parent_id: Some(parent.clone()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Trunk".into(), parent_id: Some(parent.clone()), ..Default::default() });
        assert!(create(&mut conn, TaskInput { title: "Terlalu dalam".into(), parent_id: Some(step.clone()), ..Default::default() }, Actor::Owner, now()).is_err());
        complete(&mut conn, &step, Actor::Owner, now()).unwrap();
        let detail = detail(&conn, &parent, "2026-10-22").unwrap();
        assert_eq!(titles(&detail.subtasks), ["Topologi", "Trunk"]);
        assert_eq!((detail.card.subtasks_done, detail.card.subtasks_total), (1, 2));
        // Steps never show as tasks of their own in the lists.
        assert_eq!(titles(&list(&conn, TaskView::Inbox, &day()).unwrap()), ["Laporan"]);

        delete(&mut conn, &parent, Actor::Owner, now()).unwrap();
        assert!(list(&conn, TaskView::Anytime, &day()).unwrap().is_empty());
        assert!(detail_exists(&conn, &step).is_none());
        restore(&mut conn, &parent, Actor::Owner, now()).unwrap();
        assert_eq!(super::detail(&conn, &parent, "2026-10-22").unwrap().subtasks.len(), 2);
    }

    fn detail_exists(conn: &Connection, id: &str) -> Option<()> {
        conn.query_row("SELECT 1 FROM tasks WHERE id = ?1 AND deleted_at IS NULL", [id], |_| Ok(())).optional().unwrap()
    }

    #[test]
    fn lists_sort_tasks_into_their_places() {
        let mut conn = open_in_memory().unwrap();
        let (project, _) = crate::projects::create(
            &mut conn,
            crate::projects::ProjectInput { name: "Lab".into(), ..Default::default() },
            Actor::Owner,
            now(),
        )
        .unwrap();
        task(&mut conn, "Ide liar");
        with(&mut conn, TaskInput { title: "Kemarin".into(), do_date: Some("2026-10-21".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Hari ini".into(), do_date: Some("2026-10-22".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Tenggat hari ini".into(), due_date: Some("2026-10-22".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Jumat".into(), do_date: Some("2026-10-23".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Tenggat Senin".into(), due_date: Some("2026-10-26".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Di proyek".into(), project_id: Some(project), ..Default::default() });
        let done = with(&mut conn, TaskInput { title: "Selesai".into(), do_date: Some("2026-10-22".into()), ..Default::default() });
        complete(&mut conn, &done, Actor::Owner, now()).unwrap();

        assert_eq!(titles(&list(&conn, TaskView::Inbox, &day()).unwrap()), ["Ide liar"]);
        assert_eq!(titles(&list(&conn, TaskView::Today, &day()).unwrap()), ["Kemarin", "Hari ini", "Tenggat hari ini", "Selesai"]);
        assert_eq!(titles(&list(&conn, TaskView::Upcoming, &day()).unwrap()), ["Jumat", "Tenggat Senin"]);
        assert_eq!(list(&conn, TaskView::Anytime, &day()).unwrap().len(), 7);
        assert_eq!(titles(&list(&conn, TaskView::Done, &day()).unwrap()), ["Selesai"]);
        let counts = counts(&conn, "2026-10-22").unwrap();
        assert_eq!((counts.inbox, counts.today, counts.upcoming, counts.anytime), (1, 3, 2, 7));
    }

    #[test]
    fn edits_replace_fields_and_reorder_moves_one_row() {
        let mut conn = open_in_memory().unwrap();
        let a = task(&mut conn, "A");
        let b = task(&mut conn, "B");
        let c = task(&mut conn, "C");
        reorder(&mut conn, &c, None, Some(&a), now()).unwrap();
        assert_eq!(titles(&list(&conn, TaskView::Inbox, &day()).unwrap()), ["C", "A", "B"]);
        reorder(&mut conn, &c, Some(&a), Some(&b), now()).unwrap();
        assert_eq!(titles(&list(&conn, TaskView::Inbox, &day()).unwrap()), ["A", "C", "B"]);
        assert!(reorder(&mut conn, &c, Some(&b), Some(&a), now()).is_err());

        update(
            &mut conn,
            &b,
            TaskInput { title: "B baru".into(), due_date: Some("2026-10-30".into()), due_time: Some("23:59".into()), priority: 2, ..Default::default() },
            Actor::Owner,
            now(),
        )
        .unwrap();
        let card = detail(&conn, &b, "2026-10-22").unwrap().card;
        assert_eq!((card.title.as_str(), card.due_time.as_deref(), card.priority), ("B baru", Some("23:59"), 2));
        set_do_date(&mut conn, &b, Some("2026-10-23"), Actor::Owner, now()).unwrap();
        assert_eq!(detail(&conn, &b, "2026-10-22").unwrap().card.do_date.as_deref(), Some("2026-10-23"));
    }

    #[test]
    fn focus_holds_at_most_three_and_suggestions_lead_with_urgency() {
        let mut conn = open_in_memory().unwrap();
        let ids: Vec<String> = (0..4).map(|i| task(&mut conn, &format!("t{i}"))).collect();
        let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
        assert!(set_focus(&mut conn, "2026-10-22", &refs, Actor::Owner, now()).is_err());
        set_focus(&mut conn, "2026-10-22", &[refs[3], refs[0]], Actor::Owner, now()).unwrap();
        assert_eq!(titles(&focus_of(&conn, "2026-10-22").unwrap()), ["t3", "t0"]);

        let mut conn = open_in_memory().unwrap();
        task(&mut conn, "Tanpa tanggal");
        with(&mut conn, TaskInput { title: "Penting".into(), priority: 3, ..Default::default() });
        with(&mut conn, TaskInput { title: "Hari ini".into(), do_date: Some("2026-10-22".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Lewat".into(), due_date: Some("2026-10-20".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Lusa".into(), due_date: Some("2026-10-24".into()), ..Default::default() });
        with(&mut conn, TaskInput { title: "Bulan depan".into(), due_date: Some("2026-11-30".into()), ..Default::default() });
        assert_eq!(titles(&suggest_focus(&conn, "2026-10-22").unwrap()), ["Lewat", "Lusa", "Penting"]);
    }
}
