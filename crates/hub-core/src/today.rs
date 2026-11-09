//! Hari Ini: everything the dashboard shows for one local day, in one read.
//!
//! Times come back as minutes from local midnight, because the Day Arc and the schedule think in
//! the owner's day, not in UTC. What is happening *now* is left to the UI, which owns the clock.

use jiff::tz::TimeZone;
use jiff::Timestamp;
use serde::Serialize;
use ts_rs::TS;

use crate::activity::{self, ActivityItem};
use crate::calendar::{self, CalendarItem, DayLoad, DayReminder, Waking};
use crate::cards::TaskCard;
use crate::error::Result;
use crate::files::{self, FileCard, FileQuery};
use crate::learning::{self, DayMinutes, LearningCard, Timer};
use crate::reminders::{self, ReminderItem};
use crate::tasks::{self, TaskView};
use crate::time::{date_text, Day};

const RECENT_LIMIT: u32 = 5;
const ALSO_LIMIT: usize = 8;
const FILES_LIMIT: u32 = 4;

/// Belajar on Hari Ini: the week against the goal, and what to pick up again.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LearningGlance {
    pub goal_days: u8,
    pub active_days: u8,
    pub minutes_week: u32,
    pub days: Vec<DayMinutes>,
    pub current: Option<LearningCard>,
    pub timer: Option<Timer>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TodayView {
    /// The local date, `YYYY-MM-DD`.
    pub date: String,
    /// Events, routine occurrences, and time blocks, by start.
    pub items: Vec<CalendarItem>,
    pub reminders: Vec<DayReminder>,
    pub load: DayLoad,
    /// Fokus, in slot order. Done tasks stay, struck through.
    pub focus: Vec<TaskCard>,
    /// When no focus was chosen: up to three Hub would choose.
    pub suggestions: Vec<TaskCard>,
    /// Other tasks for today (do date or deadline today or earlier, and today's finished ones),
    /// in the owner's order.
    pub also: Vec<TaskCard>,
    pub recent: Vec<ActivityItem>,
    /// Reminders that went off and wait for the owner.
    pub alerts: Vec<ReminderItem>,
    /// Closed with Tutup Hari.
    pub closed: bool,
    pub learning: LearningGlance,
    /// The files added or attached most recently.
    pub files: Vec<FileCard>,
}

pub fn today(conn: &rusqlite::Connection, now: Timestamp, tz: &TimeZone, waking: Waking) -> Result<TodayView> {
    let day = Day::containing(now, tz)?;
    let date = date_text(day.date);
    let calendar = calendar::day(conn, day.date, tz, waking)?;
    let focus = tasks::focus_of(conn, &date)?;
    let also = tasks::list(conn, TaskView::Today, &day)?
        .into_iter()
        .filter(|task| !focus.iter().any(|f| f.id == task.id))
        .take(ALSO_LIMIT)
        .collect();
    let suggestions = if focus.is_empty() { tasks::suggest_focus(conn, &date)? } else { Vec::new() };
    let overview = learning::overview(conn, now, tz)?;
    let learning = LearningGlance {
        goal_days: overview.goal_days,
        active_days: overview.active_days,
        minutes_week: overview.minutes_week,
        days: overview.days,
        current: overview.current,
        timer: overview.timer,
    };
    let files = files::list(conn, &FileQuery { limit: Some(FILES_LIMIT), ..Default::default() })?;
    Ok(TodayView {
        items: calendar.items,
        reminders: calendar.reminders,
        load: calendar.load,
        closed: calendar.closed,
        focus,
        suggestions,
        also,
        recent: activity::recent(conn, RECENT_LIMIT, false)?,
        alerts: reminders::alerts(conn)?,
        date,
        learning,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::ItemKind;
    use crate::db::open_in_memory;
    use crate::events::{create_timed, NewEvent};
    use crate::model::{Actor, Pigment, TaskStatus};
    use crate::projects::{self, ProjectInput};
    use crate::tasks::TaskInput;

    fn jakarta() -> TimeZone {
        TimeZone::get("Asia/Jakarta").unwrap()
    }

    fn at(text: &str) -> Timestamp {
        text.parse().unwrap()
    }

    const WAKING: Waking = Waking { wake: 300, sleep: 1350 };

    #[test]
    fn an_empty_day() {
        let conn = open_in_memory().unwrap();
        let view = today(&conn, at("2026-10-22T05:00:00Z"), &jakarta(), WAKING).unwrap();
        assert_eq!(view.date, "2026-10-22");
        assert!(view.items.is_empty() && view.focus.is_empty() && view.also.is_empty() && view.suggestions.is_empty());
        assert!(!view.closed);
    }

    #[test]
    fn events_are_placed_in_the_local_day() {
        let mut conn = open_in_memory().unwrap();
        let now = at("2026-10-22T03:00:00Z"); // 10:00 WIB
        let event = |title: &str, start: &str, end: &str| NewEvent {
            title: title.into(),
            location: String::new(),
            start: at(start),
            end: at(end),
            pigment: Pigment::Toska,
            project_id: None,
        };
        create_timed(&mut conn, event("Matematika", "2026-10-22T00:00:00Z", "2026-10-22T01:30:00Z"), Actor::Owner, now).unwrap();
        create_timed(&mut conn, event("Begadang", "2026-10-21T16:00:00Z", "2026-10-21T18:00:00Z"), Actor::Owner, now).unwrap();
        create_timed(&mut conn, event("Besok", "2026-10-23T00:00:00Z", "2026-10-23T01:00:00Z"), Actor::Owner, now).unwrap();
        crate::reminders::create(&mut conn, "Kumpulkan laporan", at("2026-10-22T14:00:00Z"), Actor::Owner, now).unwrap();

        let view = today(&conn, now, &jakarta(), WAKING).unwrap();
        let placed: Vec<(&str, i32, i32, ItemKind)> =
            view.items.iter().map(|e| (e.title.as_str(), e.start_minute, e.end_minute, e.kind)).collect();
        assert_eq!(placed, vec![("Begadang", 0, 60, ItemKind::Event), ("Matematika", 420, 510, ItemKind::Event)]);
        assert_eq!(view.reminders.len(), 1);
        assert_eq!(view.reminders[0].minute, 21 * 60);
    }

    #[test]
    fn focus_keeps_done_tasks_and_also_lists_the_rest_in_order() {
        let mut conn = open_in_memory().unwrap();
        let now = at("2026-10-22T03:00:00Z");
        let (project, _) =
            projects::create(&mut conn, ProjectInput { name: "Laporan Lab Jaringan".into(), pigment: Pigment::Toska, ..Default::default() }, Actor::Owner, now)
                .unwrap();
        let mut make = |title: &str, do_date: Option<&str>, due: Option<&str>| {
            let input = TaskInput {
                title: title.into(),
                do_date: do_date.map(Into::into),
                due_date: due.map(Into::into),
                project_id: Some(project.clone()),
                ..Default::default()
            };
            tasks::create(&mut conn, input, Actor::Owner, now).unwrap().0
        };
        let focus_a = make("Fokus A", None, None);
        let focus_b = make("Fokus B", None, None);
        let urgent = make("Hari ini", Some("2026-10-22"), None);
        let overdue = make("Lewat", None, Some("2026-10-20"));
        let _later = make("Nanti", Some("2026-10-30"), None);
        tasks::set_focus(&mut conn, "2026-10-22", &[&focus_a, &focus_b], Actor::Owner, now).unwrap();
        tasks::complete(&mut conn, &focus_b, Actor::Owner, now).unwrap();

        let view = today(&conn, now, &jakarta(), WAKING).unwrap();
        let focus: Vec<(&str, TaskStatus)> = view.focus.iter().map(|t| (t.title.as_str(), t.status)).collect();
        assert_eq!(focus, vec![("Fokus A", TaskStatus::Todo), ("Fokus B", TaskStatus::Done)]);
        let also: Vec<&str> = view.also.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(also, vec![urgent.as_str(), overdue.as_str()], "the owner's order");
        assert!(view.also[1].overdue);
        assert!(view.suggestions.is_empty(), "focus was chosen");

        // Checked today: still there, in the same place, no longer overdue.
        tasks::complete(&mut conn, &overdue, Actor::Owner, now).unwrap();
        let view = today(&conn, now, &jakarta(), WAKING).unwrap();
        let also: Vec<(&str, TaskStatus, bool)> = view.also.iter().map(|t| (t.id.as_str(), t.status, t.overdue)).collect();
        assert_eq!(also, vec![(urgent.as_str(), TaskStatus::Todo, false), (overdue.as_str(), TaskStatus::Done, false)]);
        // Checked yesterday: gone.
        let view = today(&conn, at("2026-10-23T03:00:00Z"), &jakarta(), WAKING).unwrap();
        assert!(view.also.iter().all(|t| t.id != overdue));
        assert!(!view.suggestions.is_empty(), "no focus chosen for the 23rd");
    }
}
