//! The commands the windows may call (build.rs lists them, capabilities/ allows them per window).
//! Arguments arrive as typed values; anything else is refused by serde before it gets here. Data
//! commands run off the main thread (`async`), so a slow disk never freezes a window. Every rule
//! lives in hub-core: these only build the owner's "now" (`Ctx`), call, and announce the change.

use std::sync::Arc;

use hub_core::areas::{self, Area, AreaInput};
use hub_core::calendar::{self, CalendarDay};
use hub_core::capture::{self, CaptureKind, Reading, Saved};
use hub_core::cards::{ProjectRef, TaskCard};
use hub_core::context::Ctx;
use hub_core::dayclose::{self, CloseInput, DayReview};
use hub_core::events::{self, EventInput, Slot};
use hub_core::info::Info;
use hub_core::model::{Actor, Change, Entity, ProjectStatus};
use hub_core::projects::{self, ProjectDetail, ProjectInput, ProjectSummary};
use hub_core::reminders::{self, ReminderInput, ReminderItem};
use hub_core::routines::{self, Routine, RoutineInput};
use hub_core::settings::{self, Settings, SettingsPatch};
use hub_core::tasks::{self, TaskCounts, TaskDetail, TaskInput, TaskView};
use hub_core::today::TodayView;
use hub_core::{info, time, today, ErrorView};
use jiff::Timestamp;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State, WebviewWindow};
use tauri_plugin_autostart::ManagerExt;

use crate::{log, window, AppState};

pub(crate) type Result<T> = std::result::Result<T, ErrorView>;
pub(crate) type Hub<'a> = State<'a, Arc<AppState>>;
pub(crate) type Done<T> = hub_core::Result<(T, Option<Change>)>;

/// Tells every window what changed, so each refetches only that.
pub const CHANGED_EVENT: &str = "hub://changed";

pub fn announce(app: &AppHandle, change: &Change) {
    // Claude Code's sessions are Hub's own business: ULTRON's stream carries the owner's data.
    if change.entity != Entity::ClaudeSession {
        crate::state(app).events.publish(hub_api::ApiEvent::from(change));
    }
    if let Err(error) = app.emit(CHANGED_EVENT, change) {
        eprintln!("[hub] change not announced: {error}");
    }
}

pub(crate) fn internal(message: impl Into<String>) -> ErrorView {
    ErrorView { kind: hub_core::ErrorKind::Internal, field: None, reason: None, message: message.into() }
}

pub(crate) fn read<T>(state: &AppState, f: impl FnOnce(&Connection, &Ctx) -> hub_core::Result<T>) -> Result<T> {
    Ok(state.hub.read(|conn| {
        let ctx = Ctx::load(conn, Timestamp::now())?;
        f(conn, &ctx)
    })?)
}

pub(crate) fn write<T>(app: &AppHandle, state: &AppState, f: impl FnOnce(&mut Connection, &Ctx) -> Done<T>) -> Result<T> {
    let (value, change) = state.hub.write(|conn| {
        let ctx = Ctx::load(conn, Timestamp::now())?;
        f(conn, &ctx)
    })?;
    if let Some(change) = change {
        if matches!(change.entity, Entity::Reminder | Entity::Settings | Entity::Focus) {
            state.scheduler.poke();
        }
        announce(app, &change);
    }
    Ok(value)
}

/// For writes that touch several kinds of thing (a file and the project it joined).
pub(crate) fn write_many<T>(
    app: &AppHandle,
    state: &AppState,
    f: impl FnOnce(&mut Connection, &Ctx) -> hub_core::Result<(T, Vec<Change>)>,
) -> Result<T> {
    let (value, changes) = state.hub.write(|conn| {
        let ctx = Ctx::load(conn, Timestamp::now())?;
        f(conn, &ctx)
    })?;
    for change in &changes {
        announce(app, change);
    }
    Ok(value)
}

pub(crate) const OWNER: Actor = Actor::Owner;

// ---------- settings and the app ----------

#[tauri::command(async)]
pub fn get_settings(app: AppHandle, state: Hub<'_>) -> Result<Settings> {
    let mut settings = read(&state, |conn, _| settings::load(conn))?;
    settings.autostart = crate::autostart_enabled(&app, settings.autostart);
    Ok(settings)
}

#[tauri::command(async)]
pub fn update_settings(app: AppHandle, state: Hub<'_>, patch: SettingsPatch) -> Result<Settings> {
    if let Some(enable) = patch.autostart {
        let launcher = app.autolaunch();
        let applied = if enable { launcher.enable() } else { launcher.disable() };
        if let Err(error) = applied {
            log::error(state.hub.paths(), &format!("autostart {enable}: {error}"));
            return Err(internal(format!("autostart: {error}")));
        }
    }
    let moves_api = patch.api_enabled.is_some() || patch.api_port.is_some() || patch.claude_enabled.is_some();
    let mut settings = write(&app, &state, |conn, ctx| settings::update(conn, patch, OWNER, ctx.now))?;
    settings.autostart = crate::autostart_enabled(&app, settings.autostart);
    if moves_api {
        crate::api::apply(&app);
    }
    Ok(settings)
}

/// "Ekspor semua data": the owner picks where, Hub writes everything there as JSON. The path written,
/// or None when the owner cancelled.
#[tauri::command(async)]
pub fn export_data(app: AppHandle, state: Hub<'_>) -> Result<Option<String>> {
    let owner = crate::library::main_hwnd(&app);
    let today = read(&state, |_, ctx| Ok(ctx.date.clone()))?;
    let name = format!("Hub-ekspor-{today}.json");
    let Some(path) = crate::shell::on_com_thread(move || crate::shell::save_file(owner, "Ekspor semua data Hub", &name, "json")).flatten() else {
        return Ok(None);
    };
    let version = app.package_info().version.to_string();
    state.hub.read(|conn| hub_core::export::write(conn, &path, &version, Timestamp::now()))?;
    Ok(Some(path.display().to_string()))
}

/// The local API for ULTRON, as Pengaturan shows it.
#[tauri::command(async)]
pub fn get_api_status(state: Hub<'_>) -> crate::api::ApiStatus {
    crate::api::status(&state)
}

/// A new token for ULTRON (the old one stops working at once).
#[tauri::command(async)]
pub fn rotate_api_token(app: AppHandle, state: Hub<'_>) -> Result<crate::api::ApiStatus> {
    crate::api::rotate(&app)?;
    Ok(crate::api::status(&state))
}

#[tauri::command(async)]
pub fn get_info(app: AppHandle, state: Hub<'_>) -> Result<Info> {
    let version = app.package_info().version.to_string();
    Ok(info::info(&state.hub, &version)?)
}

/// What the windows need to know about the app itself.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shell {
    /// Ctrl+Alt+H could be registered (another application may hold it).
    hotkey_ok: bool,
    hotkey: &'static str,
}

#[tauri::command]
pub fn get_shell(state: Hub<'_>) -> Shell {
    Shell { hotkey_ok: state.hotkey_ok.load(std::sync::atomic::Ordering::SeqCst), hotkey: crate::CAPTURE_HOTKEY_LABEL }
}

/// The page has drawn its first frame in the right light: show the window now, so it never
/// flashes white at night.
#[tauri::command]
pub fn window_ready(window: WebviewWindow) {
    window::reveal(&window);
}

#[derive(Deserialize)]
pub struct Rgb {
    r: u8,
    g: u8,
    b: u8,
}

#[derive(Deserialize)]
pub struct Caption {
    background: Rgb,
    text: Rgb,
}

/// Paint the Windows title bar in the page's paper and ink, so the window reads as one surface.
#[tauri::command]
pub fn set_caption(window: WebviewWindow, caption: Caption) {
    let background = (caption.background.r, caption.background.g, caption.background.b);
    let text = (caption.text.r, caption.text.g, caption.text.b);
    window::paint_caption(&window, background, text);
}

/// The page a freshly built main window should open on (from a notification), once.
#[tauri::command]
pub fn take_start_path() -> Option<String> {
    window::take_pending_path()
}

/// The capture window is done (saved or dismissed).
#[tauri::command]
pub fn hide_capture(window: WebviewWindow) {
    if window.label() == window::CAPTURE {
        window::hide(&window);
    }
}

// ---------- today ----------

#[tauri::command(async)]
pub fn get_today(state: Hub<'_>) -> Result<TodayView> {
    read(&state, |conn, ctx| today::today(conn, ctx.now, &ctx.tz, ctx.waking))
}

// ---------- tasks ----------

#[tauri::command(async)]
pub fn list_tasks(state: Hub<'_>, view: TaskView) -> Result<Vec<TaskCard>> {
    read(&state, |conn, ctx| tasks::list(conn, view, &ctx.day))
}

#[tauri::command(async)]
pub fn task_counts(state: Hub<'_>) -> Result<TaskCounts> {
    read(&state, |conn, ctx| tasks::counts(conn, &ctx.date))
}

#[tauri::command(async)]
pub fn get_task(state: Hub<'_>, id: String) -> Result<TaskDetail> {
    read(&state, |conn, ctx| tasks::detail(conn, &id, &ctx.date))
}

#[tauri::command(async)]
pub fn create_task(app: AppHandle, state: Hub<'_>, input: TaskInput) -> Result<String> {
    write(&app, &state, |conn, ctx| tasks::create(conn, input, OWNER, ctx.now).map(|(id, c)| (id, Some(c))))
}

#[tauri::command(async)]
pub fn update_task(app: AppHandle, state: Hub<'_>, id: String, input: TaskInput) -> Result<()> {
    write(&app, &state, |conn, ctx| tasks::update(conn, &id, input, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn complete_task(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| tasks::complete(conn, &id, OWNER, ctx.now).map(|c| ((), c)))
}

#[tauri::command(async)]
pub fn reopen_task(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| tasks::reopen(conn, &id, OWNER, ctx.now).map(|c| ((), c)))
}

#[tauri::command(async)]
pub fn delete_task(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| tasks::delete(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn restore_task(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| tasks::restore(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn reorder_task(app: AppHandle, state: Hub<'_>, id: String, before: Option<String>, after: Option<String>) -> Result<()> {
    write(&app, &state, |conn, ctx| {
        tasks::reorder(conn, &id, before.as_deref(), after.as_deref(), ctx.now).map(|c| ((), Some(c)))
    })
}

#[tauri::command(async)]
pub fn set_task_do_date(app: AppHandle, state: Hub<'_>, id: String, date: Option<String>) -> Result<()> {
    write(&app, &state, |conn, ctx| tasks::set_do_date(conn, &id, date.as_deref(), OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn set_focus(app: AppHandle, state: Hub<'_>, date: String, task_ids: Vec<String>) -> Result<()> {
    write(&app, &state, |conn, ctx| {
        let ids: Vec<&str> = task_ids.iter().map(String::as_str).collect();
        tasks::set_focus(conn, &date, &ids, OWNER, ctx.now).map(|c| ((), Some(c)))
    })
}

// ---------- projects and areas ----------

#[tauri::command(async)]
pub fn list_projects(state: Hub<'_>) -> Result<Vec<ProjectSummary>> {
    read(&state, |conn, _| projects::list(conn))
}

#[tauri::command(async)]
pub fn get_project(state: Hub<'_>, id: String) -> Result<ProjectDetail> {
    read(&state, |conn, ctx| projects::detail(conn, &id, &ctx.date))
}

#[tauri::command(async)]
pub fn create_project(app: AppHandle, state: Hub<'_>, input: ProjectInput) -> Result<String> {
    write(&app, &state, |conn, ctx| projects::create(conn, input, OWNER, ctx.now).map(|(id, c)| (id, Some(c))))
}

#[tauri::command(async)]
pub fn update_project(app: AppHandle, state: Hub<'_>, id: String, input: ProjectInput) -> Result<()> {
    write(&app, &state, |conn, ctx| projects::update(conn, &id, input, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn set_project_status(app: AppHandle, state: Hub<'_>, id: String, status: ProjectStatus) -> Result<()> {
    write(&app, &state, |conn, ctx| projects::set_status(conn, &id, status, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn delete_project(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| projects::delete(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn add_milestone(app: AppHandle, state: Hub<'_>, project_id: String, title: String, due_date: Option<String>) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        projects::add_milestone(conn, &project_id, &title, due_date.as_deref(), OWNER, ctx.now).map(|(id, c)| (id, Some(c)))
    })
}

#[tauri::command(async)]
pub fn update_milestone(app: AppHandle, state: Hub<'_>, id: String, title: String, due_date: Option<String>) -> Result<()> {
    write(&app, &state, |conn, ctx| {
        projects::update_milestone(conn, &id, &title, due_date.as_deref(), OWNER, ctx.now).map(|c| ((), Some(c)))
    })
}

#[tauri::command(async)]
pub fn set_milestone_done(app: AppHandle, state: Hub<'_>, id: String, done: bool) -> Result<()> {
    write(&app, &state, |conn, ctx| projects::set_milestone_done(conn, &id, done, OWNER, ctx.now).map(|c| ((), c)))
}

#[tauri::command(async)]
pub fn delete_milestone(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| projects::delete_milestone(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn list_areas(state: Hub<'_>) -> Result<Vec<Area>> {
    read(&state, |conn, _| areas::list(conn))
}

#[tauri::command(async)]
pub fn create_area(app: AppHandle, state: Hub<'_>, input: AreaInput) -> Result<String> {
    write(&app, &state, |conn, ctx| areas::create(conn, input, OWNER, ctx.now).map(|(id, c)| (id, Some(c))))
}

#[tauri::command(async)]
pub fn update_area(app: AppHandle, state: Hub<'_>, id: String, input: AreaInput) -> Result<()> {
    write(&app, &state, |conn, ctx| areas::update(conn, &id, input, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn delete_area(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| areas::delete(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

// ---------- calendar: events and routines ----------

const CALENDAR_DAYS_MAX: u32 = 42;

#[tauri::command(async)]
pub fn get_calendar(state: Hub<'_>, from: String, days: u32) -> Result<Vec<CalendarDay>> {
    read(&state, |conn, ctx| {
        let first = time::parse_date("from", &from)?;
        calendar::range(conn, first, days.clamp(1, CALENDAR_DAYS_MAX), &ctx.tz, ctx.waking)
    })
}

#[tauri::command(async)]
pub fn get_event(state: Hub<'_>, id: String) -> Result<EventInput> {
    read(&state, |conn, ctx| events::get(conn, &id, &ctx.tz))
}

#[tauri::command(async)]
pub fn create_event(app: AppHandle, state: Hub<'_>, input: EventInput) -> Result<String> {
    write(&app, &state, |conn, ctx| events::create(conn, input, &ctx.tz, OWNER, ctx.now).map(|(id, c)| (id, Some(c))))
}

#[tauri::command(async)]
pub fn update_event(app: AppHandle, state: Hub<'_>, id: String, input: EventInput) -> Result<()> {
    write(&app, &state, |conn, ctx| events::update(conn, &id, input, &ctx.tz, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn reschedule_event(app: AppHandle, state: Hub<'_>, id: String, slot: Slot) -> Result<()> {
    write(&app, &state, |conn, ctx| events::reschedule(conn, &id, &slot, &ctx.tz, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn delete_event(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| events::delete(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn restore_event(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| events::restore(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn list_routines(state: Hub<'_>) -> Result<Vec<Routine>> {
    read(&state, |conn, _| routines::list(conn))
}

#[tauri::command(async)]
pub fn create_routine(app: AppHandle, state: Hub<'_>, input: RoutineInput) -> Result<String> {
    write(&app, &state, |conn, ctx| routines::create(conn, input, OWNER, ctx.now).map(|(id, c)| (id, Some(c))))
}

#[tauri::command(async)]
pub fn update_routine(app: AppHandle, state: Hub<'_>, id: String, input: RoutineInput) -> Result<()> {
    write(&app, &state, |conn, ctx| routines::update(conn, &id, input, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn delete_routine(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| routines::delete(conn, &id, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn skip_occurrence(app: AppHandle, state: Hub<'_>, routine_id: String, date: String) -> Result<()> {
    write(&app, &state, |conn, ctx| routines::skip_occurrence(conn, &routine_id, &date, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn move_occurrence(app: AppHandle, state: Hub<'_>, routine_id: String, date: String, slot: Slot) -> Result<()> {
    write(&app, &state, |conn, ctx| {
        routines::move_occurrence(conn, &routine_id, &date, &slot, &ctx.tz, OWNER, ctx.now).map(|c| ((), Some(c)))
    })
}

#[tauri::command(async)]
pub fn restore_occurrence(app: AppHandle, state: Hub<'_>, routine_id: String, date: String) -> Result<()> {
    write(&app, &state, |conn, _| routines::restore_occurrence(conn, &routine_id, &date).map(|c| ((), Some(c))))
}

// ---------- reminders ----------

#[tauri::command(async)]
pub fn list_reminders(state: Hub<'_>) -> Result<Vec<ReminderItem>> {
    read(&state, |conn, ctx| reminders::list(conn, ctx.now))
}

#[tauri::command(async)]
pub fn create_reminder(app: AppHandle, state: Hub<'_>, input: ReminderInput) -> Result<String> {
    write(&app, &state, |conn, ctx| reminders::create_at(conn, input, &ctx.tz, OWNER, ctx.now).map(|(id, c)| (id, Some(c))))
}

#[tauri::command(async)]
pub fn update_reminder(app: AppHandle, state: Hub<'_>, id: String, input: ReminderInput) -> Result<()> {
    write(&app, &state, |conn, ctx| reminders::update(conn, &id, input, &ctx.tz, OWNER, ctx.now).map(|c| ((), Some(c))))
}

#[tauri::command(async)]
pub fn snooze_reminder(app: AppHandle, state: Hub<'_>, id: String, minutes: u32) -> Result<()> {
    write(&app, &state, |conn, ctx| reminders::snooze(conn, &id, minutes, OWNER, ctx.now).map(|c| ((), c)))
}

#[tauri::command(async)]
pub fn done_reminder(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| reminders::done(conn, &id, OWNER, ctx.now).map(|c| ((), c)))
}

#[tauri::command(async)]
pub fn cancel_reminder(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| reminders::cancel(conn, &id, OWNER, ctx.now).map(|c| ((), c)))
}

// ---------- quick capture ----------

fn active_projects(conn: &Connection) -> hub_core::Result<Vec<ProjectRef>> {
    let mut statement = conn.prepare(
        "SELECT id, name, pigment FROM projects WHERE deleted_at IS NULL AND status IN ('active', 'paused') ORDER BY order_key",
    )?;
    let rows = statement.query_map([], |row| Ok(ProjectRef { id: row.get(0)?, name: row.get(1)?, pigment: row.get(2)? }))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// What a line would become, for the preview under the input.
#[tauri::command(async)]
pub fn read_capture(state: Hub<'_>, text: String, kind: CaptureKind) -> Result<Reading> {
    if text.chars().count() > 500 {
        return Err(ErrorView::from(hub_core::Error::Invalid { field: "text", reason: hub_core::Reason::Length }));
    }
    read(&state, |conn, ctx| {
        let projects = active_projects(conn)?;
        let context = capture::Context { today: ctx.day.date, now_minute: ctx.minute(), projects: &projects };
        Ok(capture::read(&text, kind, &context))
    })
}

#[tauri::command(async)]
pub fn save_capture(app: AppHandle, state: Hub<'_>, text: String, kind: CaptureKind) -> Result<Saved> {
    if text.chars().count() > 500 {
        return Err(ErrorView::from(hub_core::Error::Invalid { field: "text", reason: hub_core::Reason::Length }));
    }
    write(&app, &state, |conn, ctx| capture::save(conn, &text, kind, ctx.now, &ctx.tz, OWNER).map(|(saved, c)| (saved, Some(c))))
}

// ---------- Tutup Hari ----------

#[tauri::command(async)]
pub fn get_review(state: Hub<'_>, date: Option<String>) -> Result<DayReview> {
    read(&state, |conn, ctx| {
        let date = date.unwrap_or_else(|| ctx.date.clone());
        dayclose::review(conn, &date, &ctx.tz, ctx.waking, ctx.now)
    })
}

#[tauri::command(async)]
pub fn close_day(app: AppHandle, state: Hub<'_>, input: CloseInput) -> Result<()> {
    write(&app, &state, |conn, ctx| dayclose::close(conn, input, OWNER, ctx.now).map(|c| ((), Some(c))))
}
