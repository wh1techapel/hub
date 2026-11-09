//! Commands for Belajar, Jejak, and search. The rules live in hub-core (learning, insights,
//! search); these build the owner's "now", call, and announce.

use hub_core::files::{self, FileCard, FileQuery, LinkTarget};
use hub_core::insights::{self, TrailPage, TrailQuery, WeekInsight};
use hub_core::learning::{
    self, CtfEvent, CtfInput, LearningCard, LearningInput, LearningOverview, LearningQuery, Session, SessionInput, Track, TrackInput,
};
use hub_core::model::{LearningStatus, LinkKind};
use hub_core::search::{self, SearchHit};
use serde::Serialize;
use tauri::AppHandle;

use crate::commands::{internal, read, write, write_many, Hub, Result, OWNER};
use crate::shell;

#[tauri::command(async)]
pub fn list_tracks(state: Hub<'_>) -> Result<Vec<Track>> {
    read(&state, |conn, _| learning::tracks(conn))
}

#[tauri::command(async)]
pub fn create_track(app: AppHandle, state: Hub<'_>, input: TrackInput) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        let (id, change) = learning::create_track(conn, input, OWNER, ctx.now)?;
        Ok((id, Some(change)))
    })
}

#[tauri::command(async)]
pub fn update_track(app: AppHandle, state: Hub<'_>, id: String, input: TrackInput) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::update_track(conn, &id, input, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn delete_track(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::delete_track(conn, &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn restore_track(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::restore_track(conn, &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn list_learning(state: Hub<'_>, query: LearningQuery) -> Result<Vec<LearningCard>> {
    read(&state, |conn, _| learning::items(conn, &query))
}

/// One learning item with its sessions and attached files.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningDetail {
    card: LearningCard,
    sessions: Vec<Session>,
    files: Vec<FileCard>,
}

#[tauri::command(async)]
pub fn get_learning(state: Hub<'_>, id: String) -> Result<LearningDetail> {
    read(&state, |conn, _| {
        let card = learning::item(conn, &id)?;
        let sessions = learning::sessions(conn, &card.id)?;
        let target = LinkTarget { kind: LinkKind::LearningItem, id: card.id.clone() };
        let files = files::list(conn, &FileQuery { target: Some(target), limit: Some(100), ..Default::default() })?;
        Ok(LearningDetail { card, sessions, files })
    })
}

#[tauri::command(async)]
pub fn create_learning(app: AppHandle, state: Hub<'_>, input: LearningInput) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        let (id, change) = learning::create_item(conn, input, OWNER, ctx.now)?;
        Ok((id, Some(change)))
    })
}

#[tauri::command(async)]
pub fn update_learning(app: AppHandle, state: Hub<'_>, id: String, input: LearningInput) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::update_item(conn, &id, input, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn set_learning_status(app: AppHandle, state: Hub<'_>, id: String, status: LearningStatus) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::set_status(conn, &id, status, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn delete_learning(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::delete_item(conn, &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn restore_learning(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::restore_item(conn, &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn start_timer(app: AppHandle, state: Hub<'_>, item_id: String) -> Result<()> {
    write_many(&app, &state, |conn, ctx| Ok(((), learning::start_timer(conn, &item_id, OWNER, ctx.now)?)))
}

/// Stop the timer; the minutes it wrote, or none when nothing ran.
#[tauri::command(async)]
pub fn stop_timer(app: AppHandle, state: Hub<'_>) -> Result<Option<u32>> {
    write_many(&app, &state, |conn, ctx| {
        Ok(match learning::stop_timer(conn, OWNER, ctx.now)? {
            Some((minutes, changes)) => (Some(minutes), changes),
            None => (None, Vec::new()),
        })
    })
}

#[tauri::command(async)]
pub fn discard_timer(app: AppHandle, state: Hub<'_>) -> Result<()> {
    write(&app, &state, |conn, _| Ok(((), Some(learning::discard_timer(conn)?))))
}

#[tauri::command(async)]
pub fn add_session(app: AppHandle, state: Hub<'_>, input: SessionInput) -> Result<String> {
    write_many(&app, &state, |conn, ctx| learning::add_session(conn, input, &ctx.tz, OWNER, ctx.now))
}

#[tauri::command(async)]
pub fn delete_session(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write_many(&app, &state, |conn, ctx| Ok(((), learning::delete_session(conn, &id, OWNER, ctx.now)?)))
}

#[tauri::command(async)]
pub fn list_ctf(state: Hub<'_>) -> Result<Vec<CtfEvent>> {
    read(&state, |conn, _| learning::ctf_events(conn))
}

#[tauri::command(async)]
pub fn create_ctf(app: AppHandle, state: Hub<'_>, input: CtfInput) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        let (id, change) = learning::create_ctf(conn, input, OWNER, ctx.now)?;
        Ok((id, Some(change)))
    })
}

#[tauri::command(async)]
pub fn update_ctf(app: AppHandle, state: Hub<'_>, id: String, input: CtfInput) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::update_ctf(conn, &id, input, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn delete_ctf(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(learning::delete_ctf(conn, &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn get_learning_overview(state: Hub<'_>) -> Result<LearningOverview> {
    read(&state, |conn, ctx| learning::overview(conn, ctx.now, &ctx.tz))
}

#[tauri::command(async)]
pub fn get_trail(state: Hub<'_>, query: TrailQuery) -> Result<TrailPage> {
    read(&state, |conn, _| insights::trail(conn, &query))
}

#[tauri::command(async)]
pub fn get_week_insight(state: Hub<'_>) -> Result<WeekInsight> {
    read(&state, |conn, ctx| insights::week(conn, ctx.now, &ctx.tz))
}

/// Ctrl+K: up to 20 hits; the query is cut to 100 characters in hub-core.
#[tauri::command(async)]
pub fn search(state: Hub<'_>, query: String) -> Result<Vec<SearchHit>> {
    if query.chars().count() > 200 {
        return Ok(Vec::new());
    }
    read(&state, |conn, _| search::search(conn, &query, 20))
}

/// A track's or a learning item's web page, opened in the browser. By id: the page never hands
/// Hub an address to open.
#[tauri::command(async)]
pub fn open_link(state: Hub<'_>, kind: String, id: String) -> Result<()> {
    let url = read(&state, |conn, _| match kind.as_str() {
        "track" => Ok(learning::tracks(conn)?.into_iter().find(|t| t.id == id).and_then(|t| t.url)),
        "learning_item" => Ok(learning::item(conn, &id)?.url),
        _ => Err(hub_core::Error::NotFound("link")),
    })?;
    let Some(url) = url else { return Err(internal("no link")) };
    if shell::on_com_thread(move || shell::open_url(&url)).unwrap_or(false) {
        Ok(())
    } else {
        Err(internal("the browser could not be opened"))
    }
}
