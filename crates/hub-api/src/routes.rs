//! The routes. Each one asks for its scope, builds the owner's "now" (`Ctx`), calls the core off
//! the async threads, and tells the host what changed. These are ULTRON's (only ULTRON has their
//! scopes), so writes are made by the actor `ultron`; a write that carries an `importRef` is
//! recorded as an import and made at most once. Studio's routes for Claude Code are in `studio`.

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{DefaultBodyLimit, Path as Id, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{middleware, Extension, Json, Router};
use futures_util::stream::{self, Stream};
use hub_core::api_clients::{ApiClient, Scope};
use hub_core::context::Ctx;
use hub_core::files::{self, FileCard, FileEdit, FileQuery, LinkTarget};
use hub_core::journal::{self, JournalEntry, NewEntry};
use hub_core::model::{Actor, Entity, LinkKind, Source};
use hub_core::search::{self, SearchKind};
use hub_core::tasks::{self, TaskInput, TaskView};
use hub_core::time::{parse_date, parse_instant};
use hub_core::{calendar, db, identity, imports, learning, projects, reminders, today, Change};
use jiff::Timestamp;
use rusqlite::Connection;
use serde::Serialize;
use tokio::sync::broadcast::error::RecvError;

use crate::dto::{self, Body, Params};
use crate::error::{from_core, ApiError};
use crate::events::ApiEvent;
use crate::guard::{self, need};
use crate::{ApiState, Host};

type Shared = Arc<ApiState>;
type Answer<T> = Result<Json<T>, ApiError>;

/// Bodies are small: a task, a reminder, a note, a path.
const BODY_MAX: usize = 64 * 1024;
const ACTOR: Actor = Actor::Ultron;
/// The longest schedule one request may ask for.
const SCHEDULE_DAYS_MAX: i64 = 31;
const KEEP_ALIVE: Duration = Duration::from_secs(15);

pub fn routes(state: Shared) -> Router {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/today", get(today_view))
        .route("/v1/schedule", get(schedule))
        .route("/v1/tasks", get(list_tasks).post(create_task))
        .route("/v1/tasks/{id}", get(get_task).patch(patch_task).delete(delete_task))
        .route("/v1/tasks/{id}/complete", post(complete_task))
        .route("/v1/tasks/{id}/reopen", post(reopen_task))
        .route("/v1/reminders", get(list_reminders).post(create_reminder))
        .route("/v1/reminders/{id}", get(get_reminder).patch(patch_reminder))
        .route("/v1/reminders/{id}/snooze", post(snooze_reminder))
        .route("/v1/reminders/{id}/done", post(done_reminder))
        .route("/v1/reminders/{id}/cancel", post(cancel_reminder))
        .route("/v1/projects", get(list_projects))
        .route("/v1/projects/{id}", get(get_project))
        .route("/v1/files", get(list_files).post(add_file))
        .route("/v1/files/search", get(search_files))
        .route("/v1/files/{id}", get(get_file))
        .route("/v1/files/{id}/attach", post(attach_file))
        .route("/v1/journal", get(list_journal).post(create_journal))
        .route("/v1/journal/{id}", get(get_journal).patch(patch_journal).delete(delete_journal))
        .route("/v1/learning/summary", get(learning_summary))
        .route("/v1/search", get(search_all))
        .route("/v1/events", get(events))
        .merge(crate::studio::routes())
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(BODY_MAX))
        // Last added runs first: every request, the fallback included, passes the guard.
        .layer(middleware::from_fn_with_state(state.clone(), guard::check))
        .with_state(state)
}

// ---------- plumbing ----------

/// Run core work on the blocking pool; core errors become API errors (internal ones logged).
async fn blocking<T: Send + 'static>(
    state: &Shared,
    work: impl FnOnce(&dyn Host) -> hub_core::Result<T> + Send + 'static,
) -> Result<T, ApiError> {
    let host = state.host.clone();
    match tokio::task::spawn_blocking(move || work(host.as_ref())).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            let refusal = from_core(&error);
            if refusal.kind == "internal" {
                state.host.log(&format!("api: {error}"));
            }
            Err(refusal)
        }
        Err(_) => Err(ApiError::internal()),
    }
}

fn read<T>(host: &dyn Host, work: impl FnOnce(&Connection, &Ctx) -> hub_core::Result<T>) -> hub_core::Result<T> {
    host.hub().read(|conn| {
        let ctx = Ctx::load(conn, Timestamp::now())?;
        work(conn, &ctx)
    })
}

/// A write, then the changes it made told to the host (outside the database lock).
fn write<T>(host: &dyn Host, work: impl FnOnce(&mut Connection, &Ctx) -> hub_core::Result<(T, Vec<Change>)>) -> hub_core::Result<T> {
    let (value, changes) = host.hub().write(|conn| {
        let ctx = Ctx::load(conn, Timestamp::now())?;
        work(conn, &ctx)
    })?;
    for change in &changes {
        host.changed(change);
    }
    Ok(value)
}

fn source_for(import_ref: &Option<String>) -> Source {
    if import_ref.is_some() { Source::Import } else { Source::Ultron }
}

/// The id an import made earlier, so a second run returns it instead of a copy.
fn imported(conn: &Connection, import_ref: &Option<String>, entity: Entity) -> hub_core::Result<Option<String>> {
    match import_ref {
        Some(reference) => imports::find(conn, reference, entity),
        None => Ok(None),
    }
}

fn remember(conn: &Connection, import_ref: &Option<String>, entity: Entity, id: &str, now: Timestamp) -> hub_core::Result<()> {
    match import_ref {
        Some(reference) => imports::remember(conn, reference, entity, id, now),
        None => Ok(()),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Created<T: Serialize> {
    id: String,
    /// An import already made earlier: nothing new was written.
    existed: bool,
    item: T,
}

fn created<T: Serialize>(value: Created<T>) -> Response {
    let status = if value.existed { StatusCode::OK } else { StatusCode::CREATED };
    (status, Json(value)).into_response()
}

#[derive(Serialize)]
struct Done {
    /// False when there was nothing to change (already done, already handled).
    changed: bool,
}

async fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "not_found", "route", "no such route")
}

// ---------- the day ----------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Health {
    name: &'static str,
    version: String,
    api_version: u32,
    schema_version: i64,
    today: String,
    time_zone: String,
    client: String,
}

async fn health(State(state): State<Shared>, Extension(client): Extension<ApiClient>) -> Answer<Health> {
    need(&client, Scope::Read)?;
    let version = state.host.version();
    let health = blocking(&state, move |host| {
        read(host, |conn, ctx| {
            Ok(Health {
                name: "Hub",
                version,
                api_version: crate::API_VERSION,
                schema_version: db::schema_version(conn)?,
                today: ctx.date.clone(),
                time_zone: ctx.tz.iana_name().unwrap_or("UTC").to_string(),
                client: client.name,
            })
        })
    })
    .await?;
    Ok(Json(health))
}

async fn today_view(State(state): State<Shared>, Extension(client): Extension<ApiClient>) -> Answer<today::TodayView> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, |host| read(host, |conn, ctx| today::today(conn, ctx.now, &ctx.tz, ctx.waking))).await?))
}

async fn schedule(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Params(range): Params<dto::Range>,
) -> Answer<Vec<calendar::CalendarDay>> {
    need(&client, Scope::Read)?;
    let days = blocking(&state, move |host| {
        read(host, |conn, ctx| {
            let from = match &range.from {
                Some(text) => parse_date("from", text)?,
                None => ctx.day.date,
            };
            let to = match &range.to {
                Some(text) => parse_date("to", text)?,
                None => from,
            };
            let count = from.until(to)?.get_days() as i64 + 1;
            if !(1..=SCHEDULE_DAYS_MAX).contains(&count) {
                return Err(hub_core::Error::Invalid { field: "to", reason: hub_core::Reason::Order });
            }
            calendar::range(conn, from, count as u32, &ctx.tz, ctx.waking)
        })
    })
    .await?;
    Ok(Json(days))
}

async fn learning_summary(State(state): State<Shared>, Extension(client): Extension<ApiClient>) -> Answer<learning::LearningOverview> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, |host| read(host, |conn, ctx| learning::overview(conn, ctx.now, &ctx.tz))).await?))
}

async fn search_all(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Params(query): Params<dto::Search>,
) -> Answer<Vec<search::SearchHit>> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, move |host| read(host, |conn, _| search::search(conn, &query.q, query.limit.unwrap_or(20)))).await?))
}

// ---------- tasks ----------

async fn list_tasks(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Params(list): Params<dto::TaskList>,
) -> Answer<Vec<hub_core::cards::TaskCard>> {
    need(&client, Scope::Read)?;
    let view = list.view.unwrap_or(TaskView::Anytime);
    Ok(Json(blocking(&state, move |host| read(host, |conn, ctx| tasks::list(conn, view, &ctx.day))).await?))
}

async fn get_task(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<tasks::TaskDetail> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, move |host| read(host, |conn, ctx| tasks::detail(conn, &id, &ctx.date))).await?))
}

async fn create_task(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Body(body): Body<dto::NewTask>) -> Result<Response, ApiError> {
    need(&client, Scope::WriteTasks)?;
    let made = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            if let Some(id) = imported(conn, &body.import_ref, Entity::Task)? {
                let card = tasks::detail(conn, &id, &ctx.date)?.card;
                return Ok((Created { id, existed: true, item: card }, Vec::new()));
            }
            let input = TaskInput {
                title: body.title,
                notes: body.notes,
                priority: body.priority,
                do_date: body.do_date,
                due_date: body.due_date,
                due_time: body.due_time,
                estimate_min: body.estimate_min,
                project_id: body.project_id,
                area_id: body.area_id,
                milestone_id: None,
                parent_id: None,
            };
            let (id, change) = tasks::create_as(conn, input, ACTOR, source_for(&body.import_ref), ctx.now)?;
            remember(conn, &body.import_ref, Entity::Task, &id, ctx.now)?;
            let card = tasks::detail(conn, &id, &ctx.date)?.card;
            Ok((Created { id, existed: false, item: card }, vec![change]))
        })
    })
    .await?;
    Ok(created(made))
}

async fn patch_task(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Id(id): Id<String>,
    Body(patch): Body<dto::TaskPatch>,
) -> Answer<hub_core::cards::TaskCard> {
    need(&client, Scope::WriteTasks)?;
    let card = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let mut input = tasks::input_of(conn, &id)?;
            if let Some(title) = patch.title {
                input.title = title;
            }
            if let Some(notes) = patch.notes {
                input.notes = notes;
            }
            if let Some(priority) = patch.priority {
                input.priority = priority;
            }
            if let Some(date) = patch.do_date {
                input.do_date = date;
            }
            if let Some(date) = patch.due_date {
                // A deadline taken away takes its time with it.
                if date.is_none() && patch.due_time.is_none() {
                    input.due_time = None;
                }
                input.due_date = date;
            }
            if let Some(time) = patch.due_time {
                input.due_time = time;
            }
            if let Some(estimate) = patch.estimate_min {
                input.estimate_min = estimate;
            }
            if let Some(project) = patch.project_id {
                // A milestone belongs to its project: a task moved elsewhere lets it go.
                if project != input.project_id {
                    input.milestone_id = None;
                }
                input.project_id = project;
            }
            if let Some(area) = patch.area_id {
                input.area_id = area;
            }
            let change = tasks::update(conn, &id, input, ACTOR, ctx.now)?;
            Ok((tasks::detail(conn, &id, &ctx.date)?.card, vec![change]))
        })
    })
    .await?;
    Ok(Json(card))
}

async fn complete_task(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<Done> {
    need(&client, Scope::WriteTasks)?;
    let changed = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let change = tasks::complete(conn, &id, ACTOR, ctx.now)?;
            Ok((change.is_some(), change.into_iter().collect()))
        })
    })
    .await?;
    Ok(Json(Done { changed }))
}

async fn reopen_task(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<Done> {
    need(&client, Scope::WriteTasks)?;
    let changed = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let change = tasks::reopen(conn, &id, ACTOR, ctx.now)?;
            Ok((change.is_some(), change.into_iter().collect()))
        })
    })
    .await?;
    Ok(Json(Done { changed }))
}

/// Soft, like every removal in Hub: the task goes to the trash and can be restored in Hub.
async fn delete_task(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<Done> {
    need(&client, Scope::WriteTasks)?;
    blocking(&state, move |host| write(host, |conn, ctx| Ok(((), vec![tasks::delete(conn, &id, ACTOR, ctx.now)?])))).await?;
    Ok(Json(Done { changed: true }))
}

// ---------- reminders ----------

async fn list_reminders(State(state): State<Shared>, Extension(client): Extension<ApiClient>) -> Answer<Vec<reminders::ReminderItem>> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, |host| read(host, |conn, ctx| reminders::list(conn, ctx.now))).await?))
}

async fn get_reminder(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<reminders::ReminderItem> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, move |host| read(host, |conn, _| reminders::get(conn, &id))).await?))
}

async fn create_reminder(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Body(body): Body<dto::NewReminder>,
) -> Result<Response, ApiError> {
    need(&client, Scope::WriteReminders)?;
    let made = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            if let Some(id) = imported(conn, &body.import_ref, Entity::Reminder)? {
                return Ok((Created { item: reminders::get(conn, &id)?, id, existed: true }, Vec::new()));
            }
            let due = parse_instant("dueAt", &body.due_at)?;
            let source = source_for(&body.import_ref);
            let (id, change) = reminders::create_as(conn, &body.text, due, body.task_id.as_deref(), ACTOR, source, ctx.now)?;
            remember(conn, &body.import_ref, Entity::Reminder, &id, ctx.now)?;
            Ok((Created { item: reminders::get(conn, &id)?, id, existed: false }, vec![change]))
        })
    })
    .await?;
    Ok(created(made))
}

async fn patch_reminder(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Id(id): Id<String>,
    Body(patch): Body<dto::ReminderPatch>,
) -> Answer<reminders::ReminderItem> {
    need(&client, Scope::WriteReminders)?;
    let item = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let due = patch.due_at.as_deref().map(|text| parse_instant("dueAt", text)).transpose()?;
            let change = reminders::change(conn, &id, patch.text.as_deref(), due, ACTOR, ctx.now)?;
            Ok((reminders::get(conn, &id)?, vec![change]))
        })
    })
    .await?;
    Ok(Json(item))
}

async fn snooze_reminder(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Id(id): Id<String>,
    Body(body): Body<dto::Snooze>,
) -> Answer<reminders::ReminderItem> {
    need(&client, Scope::WriteReminders)?;
    let item = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let change = reminders::snooze(conn, &id, body.minutes, ACTOR, ctx.now)?;
            Ok((reminders::get(conn, &id)?, change.into_iter().collect()))
        })
    })
    .await?;
    Ok(Json(item))
}

async fn done_reminder(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<Done> {
    need(&client, Scope::WriteReminders)?;
    let changed = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let change = reminders::done(conn, &id, ACTOR, ctx.now)?;
            Ok((change.is_some(), change.into_iter().collect()))
        })
    })
    .await?;
    Ok(Json(Done { changed }))
}

async fn cancel_reminder(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<Done> {
    need(&client, Scope::WriteReminders)?;
    let changed = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let change = reminders::cancel(conn, &id, ACTOR, ctx.now)?;
            Ok((change.is_some(), change.into_iter().collect()))
        })
    })
    .await?;
    Ok(Json(Done { changed }))
}

// ---------- projects ----------

async fn list_projects(State(state): State<Shared>, Extension(client): Extension<ApiClient>) -> Answer<Vec<projects::ProjectSummary>> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, |host| read(host, |conn, _| projects::list(conn))).await?))
}

async fn get_project(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<projects::ProjectDetail> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, move |host| read(host, |conn, ctx| projects::detail(conn, &id, &ctx.date))).await?))
}

// ---------- files ----------

/// A file found for a question ("the file for my networking project"), and why: what it is
/// attached to that matched, or nothing when its own name, note, or Koleksi did.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileHit {
    file: FileCard,
    via: Option<Via>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Via {
    kind: SearchKind,
    id: String,
    title: String,
}

const FILE_HITS_MAX: u32 = 50;

/// Files whose names match, then files attached to matching projects, tasks, learning items,
/// events, notes, and Koleksi. Each is looked at on disk first (moved, missing).
fn find_files(conn: &mut Connection, query: &str, limit: u32, now: Timestamp) -> hub_core::Result<(Vec<FileHit>, Vec<Change>)> {
    let limit = limit.clamp(1, FILE_HITS_MAX) as usize;
    let mut found: Vec<(String, Option<Via>)> = Vec::new();
    let add = |id: String, via: Option<Via>, found: &mut Vec<(String, Option<Via>)>| {
        if found.len() < limit && !found.iter().any(|(seen, _)| *seen == id) {
            found.push((id, via));
        }
    };
    let hits = search::search(conn, query, 30)?;
    // A file named for the question is the most direct answer: those first.
    for hit in hits.iter().filter(|hit| hit.kind == SearchKind::File) {
        add(hit.id.clone(), None, &mut found);
    }
    for hit in hits.iter().filter(|hit| hit.kind != SearchKind::File) {
        let via = Some(Via { kind: hit.kind, id: hit.id.clone(), title: hit.title.clone() });
        let attached = |kind: LinkKind| FileQuery { target: Some(LinkTarget { kind, id: hit.id.clone() }), limit: Some(20), ..Default::default() };
        let listed = match hit.kind {
            SearchKind::File => continue,
            SearchKind::Project => FileQuery { project_id: Some(hit.id.clone()), limit: Some(20), ..Default::default() },
            SearchKind::Collection => FileQuery { collection_id: Some(hit.id.clone()), limit: Some(20), ..Default::default() },
            SearchKind::Task => attached(LinkKind::Task),
            SearchKind::LearningItem => attached(LinkKind::LearningItem),
            SearchKind::Event => attached(LinkKind::Event),
            SearchKind::Journal => attached(LinkKind::Journal),
        };
        for card in files::list(conn, &listed)? {
            add(card.id, via.clone(), &mut found);
        }
    }
    let mut changes = Vec::new();
    let mut hits = Vec::new();
    for (id, via) in found {
        changes.extend(files::check(conn, &id, &identity::System, now)?);
        hits.push(FileHit { file: files::get(conn, &id)?, via });
    }
    Ok((hits, changes))
}

async fn search_files(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Params(query): Params<dto::Search>,
) -> Answer<Vec<FileHit>> {
    need(&client, Scope::Read)?;
    let hits = blocking(&state, move |host| write(host, |conn, ctx| find_files(conn, &query.q, query.limit.unwrap_or(10), ctx.now))).await?;
    Ok(Json(hits))
}

async fn list_files(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Params(list): Params<dto::FileList>,
) -> Answer<Vec<FileCard>> {
    need(&client, Scope::Read)?;
    let target = match (list.kind, list.id) {
        (Some(kind), Some(id)) => Some(LinkTarget { kind, id }),
        (None, None) => None,
        _ => return Err(ApiError::invalid("id", "order")),
    };
    let query = FileQuery { target, project_id: list.project_id, limit: Some(list.limit.unwrap_or(50).clamp(1, 200)), ..Default::default() };
    Ok(Json(blocking(&state, move |host| read(host, |conn, _| files::list(conn, &query))).await?))
}

/// One file, looked at on disk first, so a moved file comes back with its new path.
async fn get_file(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<FileCard> {
    need(&client, Scope::Read)?;
    let card = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let change = files::check(conn, &id, &identity::System, ctx.now)?;
            Ok((files::get(conn, &id)?, change.into_iter().collect()))
        })
    })
    .await?;
    Ok(Json(card))
}

/// A local absolute path. Network shares and device paths are refused: looking at one would make
/// Windows reach out (and offer the owner's credentials) on ULTRON's say-so.
fn local_path(text: &str) -> Result<PathBuf, ApiError> {
    let path = Path::new(text);
    let remote = text.starts_with("\\\\") || text.starts_with("//");
    if text.is_empty() || text.len() > 32_000 || text.contains('\0') || remote || !path.is_absolute() {
        return Err(ApiError::invalid("path", "format"));
    }
    Ok(path.to_path_buf())
}

async fn add_file(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Body(body): Body<dto::NewFile>) -> Result<Response, ApiError> {
    need(&client, Scope::WriteFiles)?;
    let path = local_path(&body.path)?;
    let probe = {
        let path = path.clone();
        tokio::task::spawn_blocking(move || identity::probe(&path)).await.map_err(|_| ApiError::internal())?
    };
    let probe = probe.map_err(|_| ApiError::invalid("path", "unknown"))?;
    let (made, wants_copy) = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            if let Some(id) = imported(conn, &body.import_ref, Entity::File)? {
                return Ok(((Created { item: files::get(conn, &id)?, id, existed: true }, false), Vec::new()));
            }
            let source = source_for(&body.import_ref);
            let (added, mut changes) = files::add_as(conn, &probe, body.attach.as_ref(), ACTOR, source, ctx.now)?;
            if let Some(note) = body.note.filter(|note| !note.trim().is_empty()) {
                let name = files::get(conn, &added.id)?.name;
                changes.push(files::update(conn, &added.id, FileEdit { name, note }, ACTOR, ctx.now)?);
            }
            remember(conn, &body.import_ref, Entity::File, &added.id, ctx.now)?;
            let item = files::get(conn, &added.id)?;
            Ok(((Created { id: added.id, existed: false, item }, added.wants_copy), changes))
        })
    })
    .await?;
    if wants_copy {
        state.host.kept_from_removable(&made.id);
    }
    Ok(created(made))
}

async fn attach_file(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Id(id): Id<String>,
    Body(target): Body<LinkTarget>,
) -> Answer<FileCard> {
    need(&client, Scope::WriteFiles)?;
    let card = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let changes = files::attach(conn, &id, &target, ACTOR, ctx.now)?;
            Ok((files::get(conn, &id)?, changes))
        })
    })
    .await?;
    Ok(Json(card))
}

// ---------- journal (Catatan) ----------

/// A note with the files it keeps.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JournalItem {
    #[serde(flatten)]
    entry: JournalEntry,
    files: Vec<FileCard>,
}

fn with_files(conn: &Connection, entry: JournalEntry) -> hub_core::Result<JournalItem> {
    let query = FileQuery { target: Some(LinkTarget { kind: LinkKind::Journal, id: entry.id.clone() }), limit: Some(50), ..Default::default() };
    Ok(JournalItem { files: files::list(conn, &query)?, entry })
}

const JOURNAL_LIMIT: u32 = 50;

async fn list_journal(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Params(list): Params<dto::JournalList>,
) -> Answer<Vec<JournalItem>> {
    need(&client, Scope::Read)?;
    let items = blocking(&state, move |host| {
        read(host, |conn, _| {
            let limit = list.limit.unwrap_or(JOURNAL_LIMIT).clamp(1, 200);
            let mut entries = if let Some(date) = &list.date {
                journal::on(conn, date)?
            } else if let Some(task) = &list.task_id {
                journal::for_task(conn, task)?
            } else if let Some(query) = &list.q {
                let mut found = Vec::new();
                for hit in search::search(conn, query, 50)?.into_iter().filter(|hit| hit.kind == SearchKind::Journal) {
                    found.push(journal::get(conn, &hit.id)?);
                }
                found
            } else {
                journal::recent(conn, list.kind, limit)?
            };
            if let Some(kind) = list.kind {
                entries.retain(|entry| entry.kind == kind);
            }
            entries.truncate(limit as usize);
            entries.into_iter().map(|entry| with_files(conn, entry)).collect()
        })
    })
    .await?;
    Ok(Json(items))
}

async fn get_journal(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<JournalItem> {
    need(&client, Scope::Read)?;
    Ok(Json(blocking(&state, move |host| read(host, |conn, _| with_files(conn, journal::get(conn, &id)?))).await?))
}

async fn create_journal(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Body(body): Body<dto::NewJournal>,
) -> Result<Response, ApiError> {
    need(&client, Scope::WriteJournal)?;
    let made = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            if let Some(id) = imported(conn, &body.import_ref, Entity::Journal)? {
                return Ok((Created { item: with_files(conn, journal::get(conn, &id)?)?, id, existed: true }, Vec::new()));
            }
            let entry = NewEntry { date: body.date.unwrap_or_else(|| ctx.date.clone()), text: body.text, kind: body.kind, task_id: body.task_id };
            let (id, change) = journal::create_with(conn, &entry, ACTOR, source_for(&body.import_ref), ctx.now)?;
            remember(conn, &body.import_ref, Entity::Journal, &id, ctx.now)?;
            Ok((Created { item: with_files(conn, journal::get(conn, &id)?)?, id, existed: false }, vec![change]))
        })
    })
    .await?;
    Ok(created(made))
}

async fn patch_journal(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Id(id): Id<String>,
    Body(patch): Body<dto::JournalPatch>,
) -> Answer<JournalItem> {
    need(&client, Scope::WriteJournal)?;
    let item = blocking(&state, move |host| {
        write(host, |conn, ctx| {
            let mut changes = Vec::new();
            if let Some(task) = patch.task_id {
                changes.push(journal::link_task(conn, &id, task.as_deref(), ACTOR, ctx.now)?);
            }
            Ok((with_files(conn, journal::get(conn, &id)?)?, changes))
        })
    })
    .await?;
    Ok(Json(item))
}

/// Soft, like every removal in Hub.
async fn delete_journal(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Id(id): Id<String>) -> Answer<Done> {
    need(&client, Scope::WriteJournal)?;
    blocking(&state, move |host| write(host, |conn, ctx| Ok(((), vec![journal::delete(conn, &id, ACTOR, ctx.now)?])))).await?;
    Ok(Json(Done { changed: true }))
}

// ---------- events ----------

/// Server-sent events: `hello` first, then `changed` and `reminder` as they happen; `resync`
/// when this reader fell behind (read everything again). The stream ends when Hub stops the API
/// or the token is replaced.
async fn events(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    need(&client, Scope::Read)?;
    let receiver = state.host.events().subscribe();
    let stopping = state.stopping.clone();
    let hello = serde_json::json!({ "apiVersion": crate::API_VERSION, "version": state.host.version() }).to_string();
    let stream = stream::unfold((receiver, stopping, Some(hello)), |(mut receiver, mut stopping, hello)| async move {
        if let Some(hello) = hello {
            return Some((Ok(Event::default().event("hello").data(hello)), (receiver, stopping, None)));
        }
        if *stopping.borrow() {
            return None;
        }
        let event = tokio::select! {
            _ = stopping.wait_for(|stopped| *stopped) => return None,
            received = receiver.recv() => match received {
                Ok(ApiEvent::TokenRotated) | Err(RecvError::Closed) => return None,
                Ok(event) => Event::default().event(event.name()).data(serde_json::to_string(&event).unwrap_or_default()),
                Err(RecvError::Lagged(_)) => Event::default().event("resync").data("{}"),
            },
        };
        Some((Ok(event), (receiver, stopping, None)))
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(KEEP_ALIVE)))
}
