//! Proyek: outcomes with tasks, milestones, and a history. Progress is derived, never typed in, and
//! always the same way: done tasks over tasks (top-level, not cancelled). A milestone is a dated
//! group of the project's tasks: done once all of them are, or ticked by hand while it has none.

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::activity::{self, ActivityItem, Record};
use crate::cards::{task_card, AreaRef, TaskCard, TASK_COLUMNS, TASK_JOINS};
use crate::error::{invalid, Error, Reason, Result};
use crate::ids::{self, new_id};
use crate::model::{Actor, Change, Entity, Pigment, ProjectStatus, Source, Verb};
use crate::order;
use crate::time::{instant, parse_date};

const NAME_MAX: usize = 120;
const DESCRIPTION_MAX: usize = 2000;
const MILESTONE_MAX: usize = 200;

#[derive(Debug, Clone, Default, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ProjectInput {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub area_id: Option<String>,
    #[serde(default)]
    pub pigment: Pigment,
    #[serde(default)]
    pub target_date: Option<String>,
}

/// Done tasks over tasks.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
    pub percent: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NextStep {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub pigment: Pigment,
    pub area: Option<AreaRef>,
    pub status: ProjectStatus,
    pub target_date: Option<String>,
    pub progress: Progress,
    /// The first open task, in the project's own order.
    pub next_step: Option<NextStep>,
    pub open_tasks: u32,
    pub source: Source,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Milestone {
    pub id: String,
    pub title: String,
    pub due_date: Option<String>,
    /// All its tasks are done; with no tasks, the owner's own tick.
    pub done: bool,
    /// Its tasks (top-level, not cancelled), and how many of them are done. With any, `done`
    /// follows them and cannot be ticked by hand.
    pub tasks_done: u32,
    pub tasks_total: u32,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectDetail {
    pub summary: ProjectSummary,
    pub milestones: Vec<Milestone>,
    /// Open tasks in order, then the ten most recently finished.
    pub tasks: Vec<TaskCard>,
    pub activity: Vec<ActivityItem>,
    /// Files attached to the project or to one of its tasks, newest first.
    pub files: Vec<crate::files::FileCard>,
}

fn checked(input: &ProjectInput) -> Result<(String, String, Option<String>, Option<String>)> {
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > NAME_MAX {
        return invalid("name", Reason::Length);
    }
    let description = input.description.trim();
    if description.chars().count() > DESCRIPTION_MAX {
        return invalid("description", Reason::Length);
    }
    let area_id = input.area_id.as_deref().map(|id| ids::parse("areaId", id)).transpose()?;
    if let Some(date) = &input.target_date {
        parse_date("targetDate", date)?;
    }
    Ok((name.to_string(), description.to_string(), area_id, input.target_date.clone()))
}

fn area_exists(tx: &Transaction<'_>, area_id: &Option<String>) -> Result<()> {
    if let Some(area) = area_id {
        let found = tx
            .query_row("SELECT 1 FROM areas WHERE id = ?1 AND deleted_at IS NULL", [area], |_| Ok(()))
            .optional()?;
        if found.is_none() {
            return invalid("areaId", Reason::Unknown);
        }
    }
    Ok(())
}

fn record(tx: &Transaction<'_>, now: Timestamp, actor: Actor, verb: Verb, entity: Entity, id: &str, summary: &str) -> Result<()> {
    activity::record(
        tx,
        now,
        Record { actor, verb, entity, entity_id: Some(id), summary, details: serde_json::json!({}) },
    )
}

pub fn create(conn: &mut Connection, input: ProjectInput, actor: Actor, now: Timestamp) -> Result<(String, Change)> {
    let tx = conn.transaction()?;
    let id = insert(&tx, &input, actor, now)?;
    tx.commit()?;
    Ok((id.clone(), Change::one(Entity::Project, id)))
}

/// A new project inside a write that does more (Studio links its project in the same write).
pub(crate) fn insert(tx: &Transaction<'_>, input: &ProjectInput, actor: Actor, now: Timestamp) -> Result<String> {
    let (name, description, area_id, target_date) = checked(input)?;
    area_exists(tx, &area_id)?;
    let last: Option<String> = tx.query_row("SELECT max(order_key) FROM projects", [], |row| row.get(0))?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO projects (id, area_id, name, description, pigment, target_date, order_key, source,
                               created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![id, area_id, name, description, input.pigment, target_date, order::after(last.as_deref()), Source::from(actor), stamp],
    )?;
    record(tx, now, actor, Verb::Created, Entity::Project, &id, &name)?;
    Ok(id)
}

pub fn update(conn: &mut Connection, id: &str, input: ProjectInput, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let (name, description, area_id, target_date) = checked(&input)?;
    let tx = conn.transaction()?;
    area_exists(&tx, &area_id)?;
    let changed = tx.execute(
        "UPDATE projects SET name = ?2, description = ?3, area_id = ?4, pigment = ?5, target_date = ?6,
                updated_at = ?7
         WHERE id = ?1 AND deleted_at IS NULL",
        params![id, name, description, area_id, input.pigment, target_date, instant(now)],
    )?;
    if changed == 0 {
        return Err(Error::NotFound("project"));
    }
    record(&tx, now, actor, Verb::Updated, Entity::Project, &id, &name)?;
    tx.commit()?;
    Ok(Change::one(Entity::Project, id))
}

pub fn set_status(conn: &mut Connection, id: &str, status: ProjectStatus, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name: Option<String> = tx
        .query_row("SELECT name FROM projects WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let name = name.ok_or(Error::NotFound("project"))?;
    tx.execute(
        "UPDATE projects SET status = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, status, instant(now)],
    )?;
    let verb = if status == ProjectStatus::Done { Verb::Completed } else { Verb::Updated };
    record(&tx, now, actor, verb, Entity::Project, &id, &name)?;
    tx.commit()?;
    Ok(Change::one(Entity::Project, id))
}

/// Remove a project. Its tasks stay, without a project.
pub fn delete(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let name: Option<String> = tx
        .query_row("SELECT name FROM projects WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let name = name.ok_or(Error::NotFound("project"))?;
    let stamp = instant(now);
    tx.execute("UPDATE projects SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    tx.execute(
        "UPDATE tasks SET project_id = NULL, milestone_id = NULL, updated_at = ?2 WHERE project_id = ?1",
        params![id, stamp],
    )?;
    record(&tx, now, actor, Verb::Deleted, Entity::Project, &id, &name)?;
    tx.commit()?;
    Ok(Change::all(Entity::Project))
}

const SUMMARY_COLUMNS: &str = "p.id, p.name, p.description, p.pigment, a.id, a.name, a.pigment, p.status,
     p.target_date, p.source,
     (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status <> 'cancelled'),
     (SELECT count(*) FROM tasks t WHERE t.project_id = p.id AND t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status = 'done'),
     (SELECT t.id FROM tasks t WHERE t.project_id = p.id AND t.deleted_at IS NULL AND t.parent_id IS NULL
        AND t.status IN ('todo', 'doing') ORDER BY t.order_key LIMIT 1),
     (SELECT t.title FROM tasks t WHERE t.project_id = p.id AND t.deleted_at IS NULL AND t.parent_id IS NULL
        AND t.status IN ('todo', 'doing') ORDER BY t.order_key LIMIT 1)";

fn summary(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProjectSummary> {
    let area = match row.get::<_, Option<String>>(4)? {
        Some(id) => Some(AreaRef { id, name: row.get(5)?, pigment: row.get(6)? }),
        None => None,
    };
    let tasks: u32 = row.get(10)?;
    let tasks_done: u32 = row.get(11)?;
    let next_step = match row.get::<_, Option<String>>(12)? {
        Some(id) => Some(NextStep { id, title: row.get(13)? }),
        None => None,
    };
    Ok(ProjectSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        pigment: row.get(3)?,
        area,
        status: row.get(7)?,
        target_date: row.get(8)?,
        source: row.get(9)?,
        progress: Progress { done: tasks_done, total: tasks, percent: (tasks_done * 100).checked_div(tasks).unwrap_or(0) },
        next_step,
        open_tasks: tasks - tasks_done,
    })
}

/// Every project not removed, active ones first, in the owner's order.
pub fn list(conn: &Connection) -> Result<Vec<ProjectSummary>> {
    let sql = format!(
        "SELECT {SUMMARY_COLUMNS} FROM projects p
         LEFT JOIN areas a ON a.id = p.area_id AND a.deleted_at IS NULL
         WHERE p.deleted_at IS NULL
         ORDER BY CASE p.status WHEN 'active' THEN 0 WHEN 'paused' THEN 1 WHEN 'done' THEN 2 ELSE 3 END, p.order_key"
    );
    let mut statement = conn.prepare(&sql)?;
    let rows = statement.query_map([], summary)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn detail(conn: &Connection, id: &str, today: &str) -> Result<ProjectDetail> {
    let id = ids::parse("id", id)?;
    let sql = format!(
        "SELECT {SUMMARY_COLUMNS} FROM projects p
         LEFT JOIN areas a ON a.id = p.area_id AND a.deleted_at IS NULL
         WHERE p.id = ?1 AND p.deleted_at IS NULL"
    );
    let summary = conn.query_row(&sql, [&id], summary).optional()?.ok_or(Error::NotFound("project"))?;

    let mut statement = conn.prepare(&format!(
        "SELECT m.id, m.title, m.due_date, m.done_at IS NOT NULL, {MILESTONE_TASKS} FROM milestones m
         WHERE m.project_id = ?1 AND m.deleted_at IS NULL ORDER BY m.order_key"
    ))?;
    let milestones = statement
        .query_map([&id], |row| {
            let ticked: bool = row.get(3)?;
            let tasks_total: u32 = row.get(4)?;
            let tasks_done: u32 = row.get(5)?;
            Ok(Milestone {
                id: row.get(0)?,
                title: row.get(1)?,
                due_date: row.get(2)?,
                done: if tasks_total > 0 { tasks_done == tasks_total } else { ticked },
                tasks_done,
                tasks_total,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let sql = format!(
        "SELECT * FROM (
           SELECT {TASK_COLUMNS}, 0 AS grp, t.order_key AS sort FROM tasks t {TASK_JOINS}
           WHERE t.project_id = ?2 AND t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status IN ('todo', 'doing')
           UNION ALL
           SELECT * FROM (
             SELECT {TASK_COLUMNS}, 1 AS grp, t.completed_at AS sort FROM tasks t {TASK_JOINS}
             WHERE t.project_id = ?2 AND t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status = 'done'
             ORDER BY t.completed_at DESC LIMIT 10))
         ORDER BY grp, CASE WHEN grp = 0 THEN sort END, sort DESC"
    );
    let mut statement = conn.prepare(&sql)?;
    let tasks = statement.query_map(params![today, id], task_card)?.collect::<rusqlite::Result<Vec<_>>>()?;

    let mut statement = conn.prepare(
        "SELECT id, at, actor, verb, entity_type, entity_id, summary FROM activity
         WHERE (entity_type = 'project' AND entity_id = ?1)
            OR (entity_type = 'task' AND entity_id IN (SELECT id FROM tasks WHERE project_id = ?1))
            OR (entity_type = 'milestone' AND entity_id IN (SELECT id FROM milestones WHERE project_id = ?1))
         ORDER BY at DESC, id DESC LIMIT 20",
    )?;
    let activity = statement
        .query_map([&id], |row| {
            Ok(ActivityItem {
                id: row.get(0)?,
                at: row.get(1)?,
                actor: row.get(2)?,
                verb: row.get(3)?,
                entity: row.get(4)?,
                entity_id: row.get(5)?,
                summary: row.get(6)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let files = crate::files::list(conn, &crate::files::FileQuery { project_id: Some(id.clone()), limit: Some(100), ..Default::default() })?;
    Ok(ProjectDetail { summary, milestones, tasks, activity, files })
}

/// A milestone's tasks (`m` is the milestone): how many, then how many are done.
const MILESTONE_TASKS: &str = "
     (SELECT count(*) FROM tasks t WHERE t.milestone_id = m.id AND t.project_id = m.project_id
        AND t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status <> 'cancelled'),
     (SELECT count(*) FROM tasks t WHERE t.milestone_id = m.id AND t.project_id = m.project_id
        AND t.deleted_at IS NULL AND t.parent_id IS NULL AND t.status = 'done')";

fn milestone_title(title: &str) -> Result<String> {
    let title = title.trim();
    if title.is_empty() || title.chars().count() > MILESTONE_MAX {
        return invalid("title", Reason::Length);
    }
    Ok(title.to_string())
}

pub fn add_milestone(
    conn: &mut Connection,
    project_id: &str,
    title: &str,
    due_date: Option<&str>,
    actor: Actor,
    now: Timestamp,
) -> Result<(String, Change)> {
    let project_id = ids::parse("projectId", project_id)?;
    let title = milestone_title(title)?;
    if let Some(date) = due_date {
        parse_date("dueDate", date)?;
    }
    let tx = conn.transaction()?;
    let exists = tx
        .query_row("SELECT 1 FROM projects WHERE id = ?1 AND deleted_at IS NULL", [&project_id], |_| Ok(()))
        .optional()?;
    if exists.is_none() {
        return Err(Error::NotFound("project"));
    }
    let last: Option<String> = tx.query_row(
        "SELECT max(order_key) FROM milestones WHERE project_id = ?1",
        [&project_id],
        |row| row.get(0),
    )?;
    let id = new_id();
    let stamp = instant(now);
    tx.execute(
        "INSERT INTO milestones (id, project_id, title, due_date, order_key, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
        params![id, project_id, title, due_date, order::after(last.as_deref()), stamp],
    )?;
    record(&tx, now, actor, Verb::Created, Entity::Milestone, &id, &title)?;
    tx.commit()?;
    Ok((id, Change::one(Entity::Project, project_id)))
}

pub fn update_milestone(
    conn: &mut Connection,
    id: &str,
    title: &str,
    due_date: Option<&str>,
    actor: Actor,
    now: Timestamp,
) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let title = milestone_title(title)?;
    if let Some(date) = due_date {
        parse_date("dueDate", date)?;
    }
    let tx = conn.transaction()?;
    let project: Option<String> = tx
        .query_row("SELECT project_id FROM milestones WHERE id = ?1 AND deleted_at IS NULL", [&id], |row| row.get(0))
        .optional()?;
    let project = project.ok_or(Error::NotFound("milestone"))?;
    tx.execute(
        "UPDATE milestones SET title = ?2, due_date = ?3, updated_at = ?4 WHERE id = ?1",
        params![id, title, due_date, instant(now)],
    )?;
    record(&tx, now, actor, Verb::Updated, Entity::Milestone, &id, &title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Project, project))
}

/// Tick (or untick) a milestone that has no tasks. One with tasks is done when they are.
pub fn set_milestone_done(conn: &mut Connection, id: &str, done: bool, actor: Actor, now: Timestamp) -> Result<Option<Change>> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let row: Option<(String, String, bool, u32)> = tx
        .query_row(
            &format!(
                "SELECT m.project_id, m.title, m.done_at IS NOT NULL, {MILESTONE_TASKS} FROM milestones m
                 WHERE m.id = ?1 AND m.deleted_at IS NULL"
            ),
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let (project, title, was_done, tasks) = row.ok_or(Error::NotFound("milestone"))?;
    if tasks > 0 {
        return invalid("done", Reason::Derived);
    }
    if was_done == done {
        return Ok(None);
    }
    let stamp = instant(now);
    tx.execute(
        "UPDATE milestones SET done_at = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, if done { Some(stamp.clone()) } else { None }, stamp],
    )?;
    record(&tx, now, actor, if done { Verb::Completed } else { Verb::Reopened }, Entity::Milestone, &id, &title)?;
    tx.commit()?;
    Ok(Some(Change::one(Entity::Project, project)))
}

/// Remove a milestone. Its tasks stay in the project, without a milestone.
pub fn delete_milestone(conn: &mut Connection, id: &str, actor: Actor, now: Timestamp) -> Result<Change> {
    let id = ids::parse("id", id)?;
    let tx = conn.transaction()?;
    let row: Option<(String, String)> = tx
        .query_row(
            "SELECT project_id, title FROM milestones WHERE id = ?1 AND deleted_at IS NULL",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (project, title) = row.ok_or(Error::NotFound("milestone"))?;
    let stamp = instant(now);
    tx.execute("UPDATE milestones SET deleted_at = ?2, updated_at = ?2 WHERE id = ?1", params![id, stamp])?;
    tx.execute("UPDATE tasks SET milestone_id = NULL, updated_at = ?2 WHERE milestone_id = ?1", params![id, stamp])?;
    record(&tx, now, actor, Verb::Deleted, Entity::Milestone, &id, &title)?;
    tx.commit()?;
    Ok(Change::one(Entity::Project, project))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::tasks::{self, NewTask};

    fn now() -> Timestamp {
        "2026-10-23T03:00:00Z".parse().unwrap()
    }

    fn project(conn: &mut Connection, name: &str) -> String {
        create(conn, ProjectInput { name: name.into(), pigment: Pigment::Toska, ..Default::default() }, Actor::Owner, now())
            .unwrap()
            .0
    }

    fn task(conn: &mut Connection, project: &str, title: &str) -> String {
        tasks::create(conn, NewTask { title: title.into(), project_id: Some(project.into()), ..Default::default() }, Actor::Owner, now())
            .unwrap()
            .0
    }

    fn task_in(conn: &mut Connection, project: &str, milestone: &str, title: &str) -> String {
        let input = NewTask { title: title.into(), project_id: Some(project.into()), milestone_id: Some(milestone.into()), ..Default::default() };
        tasks::create(conn, input, Actor::Owner, now()).unwrap().0
    }

    fn milestone(detail: &ProjectDetail, title: &str) -> (bool, u32, u32) {
        let m = detail.milestones.iter().find(|m| m.title == title).unwrap();
        (m.done, m.tasks_done, m.tasks_total)
    }

    #[test]
    fn progress_always_counts_tasks() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn, "Laporan Lab Jaringan");
        let a = task(&mut conn, &id, "Topologi");
        task(&mut conn, &id, "Konfigurasi trunk");
        tasks::complete(&mut conn, &a, Actor::Owner, now()).unwrap();
        let summary = &list(&conn).unwrap()[0];
        assert_eq!((summary.progress.done, summary.progress.total, summary.progress.percent), (1, 2, 50));
        assert_eq!(summary.next_step.as_ref().unwrap().title, "Konfigurasi trunk");
        assert_eq!(summary.open_tasks, 1);

        // Milestones, ticked or not, do not change how far the project has come.
        let (m1, _) = add_milestone(&mut conn, &id, "Draf", None, Actor::Owner, now()).unwrap();
        add_milestone(&mut conn, &id, "Kumpul", Some("2026-10-30"), Actor::Owner, now()).unwrap();
        set_milestone_done(&mut conn, &m1, true, Actor::Owner, now()).unwrap();
        assert!(set_milestone_done(&mut conn, &m1, true, Actor::Owner, now()).unwrap().is_none());
        let detail = detail(&conn, &id, "2026-10-23").unwrap();
        assert_eq!((detail.summary.progress.done, detail.summary.progress.total), (1, 2));
        assert_eq!(milestone(&detail, "Draf"), (true, 0, 0), "a milestone without tasks is ticked by hand");
        assert_eq!(milestone(&detail, "Kumpul"), (false, 0, 0));
        // Open tasks first, then the finished one.
        let titles: Vec<&str> = detail.tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Konfigurasi trunk", "Topologi"]);
        assert!(detail.activity.iter().any(|item| item.summary == "Topologi"));
    }

    #[test]
    fn a_milestone_with_tasks_is_done_when_they_are() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn, "PSAJ");
        let (v2, _) = add_milestone(&mut conn, &id, "V2 dikirim", Some("2026-11-14"), Actor::Owner, now()).unwrap();
        // Ticked before it had tasks: the tick no longer counts once it has some.
        set_milestone_done(&mut conn, &v2, true, Actor::Owner, now()).unwrap();
        let a = task_in(&mut conn, &id, &v2, "Revisi Bab III");
        let b = task_in(&mut conn, &id, &v2, "Ekspor PDF");
        let cancelled = task_in(&mut conn, &id, &v2, "Cetak");
        conn.execute("UPDATE tasks SET status = 'cancelled' WHERE id = ?1", [&cancelled]).unwrap();
        task(&mut conn, &id, "Latihan pitching");

        let check = |conn: &Connection| milestone(&detail(conn, &id, "2026-11-10").unwrap(), "V2 dikirim");
        assert_eq!(check(&conn), (false, 0, 2), "cancelled tasks do not count");
        let card = detail(&conn, &id, "2026-11-10").unwrap().tasks.into_iter().find(|t| t.id == a).unwrap();
        assert_eq!(card.milestone_id.as_deref(), Some(v2.as_str()));
        assert!(matches!(
            set_milestone_done(&mut conn, &v2, false, Actor::Owner, now()),
            Err(Error::Invalid { field: "done", reason: Reason::Derived })
        ));

        tasks::complete(&mut conn, &a, Actor::Owner, now()).unwrap();
        assert_eq!(check(&conn), (false, 1, 2));
        tasks::complete(&mut conn, &b, Actor::Owner, now()).unwrap();
        assert_eq!(check(&conn), (true, 2, 2));
        let summary = detail(&conn, &id, "2026-11-10").unwrap().summary;
        assert_eq!((summary.progress.done, summary.progress.total), (2, 3), "the project still counts every task");
        tasks::reopen(&mut conn, &b, Actor::Owner, now()).unwrap();
        assert_eq!(check(&conn), (false, 1, 2));
    }

    #[test]
    fn a_task_takes_only_a_milestone_of_its_own_project() {
        let mut conn = open_in_memory().unwrap();
        let psaj = project(&mut conn, "PSAJ");
        let lab = project(&mut conn, "Lab");
        let (m, _) = add_milestone(&mut conn, &psaj, "V2 dikirim", None, Actor::Owner, now()).unwrap();
        let elsewhere = NewTask { title: "x".into(), project_id: Some(lab.clone()), milestone_id: Some(m.clone()), ..Default::default() };
        let nowhere = NewTask { title: "x".into(), milestone_id: Some(m.clone()), ..Default::default() };
        for input in [elsewhere, nowhere] {
            assert!(matches!(tasks::create(&mut conn, input, Actor::Owner, now()), Err(Error::Invalid { field: "milestoneId", .. })));
        }

        let t = task_in(&mut conn, &psaj, &m, "Revisi");
        let step = NewTask { title: "Langkah".into(), project_id: Some(psaj.clone()), milestone_id: Some(m.clone()), parent_id: Some(t.clone()), ..Default::default() };
        assert!(tasks::create(&mut conn, step, Actor::Owner, now()).is_err(), "a step counts with its task, not on its own");

        // Moving the task to another project needs its milestone let go in the same edit.
        let mut input = tasks::input_of(&conn, &t).unwrap();
        assert_eq!(input.milestone_id.as_deref(), Some(m.as_str()), "a partial edit keeps the milestone");
        input.project_id = Some(lab.clone());
        assert!(tasks::update(&mut conn, &t, input.clone(), Actor::Owner, now()).is_err());
        input.milestone_id = None;
        tasks::update(&mut conn, &t, input, Actor::Owner, now()).unwrap();
        assert_eq!(milestone(&detail(&conn, &psaj, "2026-11-10").unwrap(), "V2 dikirim"), (false, 0, 0));
    }

    #[test]
    fn removing_a_milestone_or_a_project_lets_its_tasks_go() {
        let mut conn = open_in_memory().unwrap();
        let id = project(&mut conn, "PSAJ");
        let (m, _) = add_milestone(&mut conn, &id, "V2 dikirim", None, Actor::Owner, now()).unwrap();
        let (m2, _) = add_milestone(&mut conn, &id, "Pitching", None, Actor::Owner, now()).unwrap();
        let a = task_in(&mut conn, &id, &m, "Revisi");
        let b = task_in(&mut conn, &id, &m2, "Latihan");

        delete_milestone(&mut conn, &m, Actor::Owner, now()).unwrap();
        let milestone_of = |conn: &Connection, task: &str| -> (Option<String>, Option<String>) {
            conn.query_row("SELECT project_id, milestone_id FROM tasks WHERE id = ?1", [task], |row| Ok((row.get(0)?, row.get(1)?))).unwrap()
        };
        assert_eq!(milestone_of(&conn, &a), (Some(id.clone()), None), "the task stays in the project");
        assert!(tasks::detail(&conn, &a, "2026-11-10").is_ok());

        delete(&mut conn, &id, Actor::Owner, now()).unwrap();
        assert_eq!(milestone_of(&conn, &b), (None, None));
    }

    #[test]
    fn input_is_checked() {
        let mut conn = open_in_memory().unwrap();
        let bad = [
            ProjectInput { name: " ".into(), ..Default::default() },
            ProjectInput { name: "x".into(), area_id: Some(new_id()), ..Default::default() },
            ProjectInput { name: "x".into(), target_date: Some("akhir bulan".into()), ..Default::default() },
        ];
        for input in bad {
            assert!(create(&mut conn, input, Actor::Owner, now()).is_err());
        }
        assert!(list(&conn).unwrap().is_empty());
    }

    #[test]
    fn status_and_removal() {
        let mut conn = open_in_memory().unwrap();
        let done = project(&mut conn, "Selesai");
        let active = project(&mut conn, "Aktif");
        let t = task(&mut conn, &done, "Tugas");
        set_status(&mut conn, &done, ProjectStatus::Done, Actor::Owner, now()).unwrap();
        let names: Vec<String> = list(&conn).unwrap().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Aktif", "Selesai"]);
        delete(&mut conn, &done, Actor::Owner, now()).unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
        let project_id: Option<String> = conn.query_row("SELECT project_id FROM tasks WHERE id = ?1", [&t], |row| row.get(0)).unwrap();
        assert_eq!(project_id, None);
        assert!(detail(&conn, &done, "2026-10-23").is_err());
        let _ = active;
    }
}
