//! A task as every list shows it, read the same way everywhere.

use rusqlite::Row;
use serde::Serialize;
use ts_rs::TS;

use crate::model::{Pigment, Source, TaskStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectRef {
    pub id: String,
    pub name: String,
    pub pigment: Pigment,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AreaRef {
    pub id: String,
    pub name: String,
    pub pigment: Pigment,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TaskCard {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub priority: u8,
    /// When to work on it (`YYYY-MM-DD`).
    pub do_date: Option<String>,
    /// When it is due, and optionally at what time (`HH:MM`).
    pub due_date: Option<String>,
    pub due_time: Option<String>,
    pub estimate_min: Option<u32>,
    /// The deadline has passed and the task is not done.
    pub overdue: bool,
    pub project: Option<ProjectRef>,
    pub area: Option<AreaRef>,
    pub parent_id: Option<String>,
    pub subtasks_done: u32,
    pub subtasks_total: u32,
    /// The task has a time block on the calendar.
    pub scheduled: bool,
    pub completed_at: Option<String>,
    pub source: Source,
}

/// Columns for [`task_card`]. `?1` must be today's local date (for `overdue`). Join as
/// `FROM tasks t` + [`TASK_JOINS`].
pub const TASK_COLUMNS: &str = "t.id, t.title, t.status, t.priority, t.do_date, t.due_date, t.due_time,
     t.estimate_min, t.source, p.id, p.name, p.pigment, a.id, a.name, a.pigment, t.parent_id,
     (SELECT count(*) FROM tasks s WHERE s.parent_id = t.id AND s.deleted_at IS NULL AND s.status = 'done'),
     (SELECT count(*) FROM tasks s WHERE s.parent_id = t.id AND s.deleted_at IS NULL AND s.status <> 'cancelled'),
     EXISTS (SELECT 1 FROM events e WHERE e.task_id = t.id AND e.deleted_at IS NULL),
     t.completed_at,
     (t.status NOT IN ('done', 'cancelled') AND t.due_date IS NOT NULL AND t.due_date < ?1)";

pub const TASK_JOINS: &str = "LEFT JOIN projects p ON p.id = t.project_id AND p.deleted_at IS NULL
     LEFT JOIN areas a ON a.id = coalesce(t.area_id, p.area_id) AND a.deleted_at IS NULL";

pub fn task_card(row: &Row<'_>) -> rusqlite::Result<TaskCard> {
    let project = match row.get::<_, Option<String>>(9)? {
        Some(id) => Some(ProjectRef {
            id,
            name: row.get(10)?,
            pigment: row.get(11)?,
        }),
        None => None,
    };
    let area = match row.get::<_, Option<String>>(12)? {
        Some(id) => Some(AreaRef {
            id,
            name: row.get(13)?,
            pigment: row.get(14)?,
        }),
        None => None,
    };
    Ok(TaskCard {
        id: row.get(0)?,
        title: row.get(1)?,
        status: row.get(2)?,
        priority: row.get(3)?,
        do_date: row.get(4)?,
        due_date: row.get(5)?,
        due_time: row.get(6)?,
        estimate_min: row.get(7)?,
        source: row.get(8)?,
        project,
        area,
        parent_id: row.get(15)?,
        subtasks_done: row.get(16)?,
        subtasks_total: row.get(17)?,
        scheduled: row.get(18)?,
        completed_at: row.get(19)?,
        overdue: row.get(20)?,
    })
}
