//! The Library's commands, and files dropped from Explorer.
//!
//! A drop is handled here, not in the page: Windows hands the paths to Rust, Rust keeps them under
//! a drop number and tells the page only where the pointer is, how many files, and their names.
//! The page answers with the drop number and the pool it landed on (`keep_drop`). So no path ever
//! comes from the page; every other command takes the id of a file Hub already knows.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use hub_core::files::{self, Added, Collection, CollectionInput, FileCard, FileCounts, FileEdit, FileQuery, LinkTarget};
use hub_core::identity::{self, System};
use hub_core::{vault, ErrorView};
use jiff::Timestamp;
use serde::Serialize;
use tauri::{AppHandle, DragDropEvent, Emitter, Manager, Window};

use crate::commands::{announce, internal, read, write, write_many, Hub, Result, OWNER};
use crate::{log, shell, thumbs, window, AppState};

/// Tells the main window's page about a drag of files over it.
pub const DRAG_EVENT: &str = "hub://drag";
/// At most this many paths are kept from one drop.
const DROP_MAX: usize = 200;

struct Pending {
    id: u32,
    paths: Vec<PathBuf>,
}

fn pending() -> &'static Mutex<Option<Pending>> {
    static PENDING: OnceLock<Mutex<Option<Pending>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}

fn next_drop() -> u32 {
    static NEXT: AtomicU32 = AtomicU32::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// What the page is told during a drag. Positions are logical pixels from the page's top left.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Drag {
    phase: &'static str,
    x: f64,
    y: f64,
    drop_id: Option<u32>,
    count: usize,
    /// The first few names, for the drop sheet.
    names: Vec<String>,
}

fn names(paths: &[PathBuf]) -> Vec<String> {
    paths
        .iter()
        .take(3)
        .map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
        .collect()
}

/// Windows' drag events for the main window.
pub fn on_drag(window: &Window, event: &DragDropEvent) {
    if window.label() != window::MAIN {
        return;
    }
    let scale = window.scale_factor().unwrap_or(1.0);
    let logical = |position: &tauri::PhysicalPosition<f64>| (position.x / scale, position.y / scale);
    let drag = match event {
        DragDropEvent::Enter { paths, position } => {
            let (x, y) = logical(position);
            Drag { phase: "enter", x, y, drop_id: None, count: paths.len(), names: names(paths) }
        }
        DragDropEvent::Over { position } => {
            let (x, y) = logical(position);
            Drag { phase: "over", x, y, drop_id: None, count: 0, names: Vec::new() }
        }
        DragDropEvent::Drop { paths, position } => {
            let (x, y) = logical(position);
            let id = next_drop();
            let kept: Vec<PathBuf> = paths.iter().take(DROP_MAX).cloned().collect();
            let count = kept.len();
            let shown = names(&kept);
            if let Ok(mut slot) = pending().lock() {
                *slot = Some(Pending { id, paths: kept });
            }
            Drag { phase: "drop", x, y, drop_id: Some(id), count, names: shown }
        }
        DragDropEvent::Leave => Drag { phase: "leave", x: 0.0, y: 0.0, drop_id: None, count: 0, names: Vec::new() },
        _ => return,
    };
    let _ = window.emit_to(window::MAIN, DRAG_EVENT, drag);
}

/// Keep paths in the Library (attached to `target` when given), then copy those from a removable
/// drive into the Vault in the background.
fn keep(app: &AppHandle, state: &AppState, paths: Vec<PathBuf>, target: Option<LinkTarget>) -> Result<Vec<Added>> {
    let mut added = Vec::new();
    for path in paths {
        let probe = match identity::probe(&path) {
            Ok(probe) => probe,
            Err(error) => {
                log::error(state.hub.paths(), &format!("drop {}: {error}", path.display()));
                continue;
            }
        };
        let one = write_many(app, state, |conn, ctx| files::add(conn, &probe, target.as_ref(), OWNER, ctx.now))?;
        added.push(one);
    }
    let copies: Vec<String> = added.iter().filter(|a| a.wants_copy).map(|a| a.id.clone()).collect();
    if !copies.is_empty() {
        let (app, state) = (app.clone(), app.state::<Arc<AppState>>().inner().clone());
        std::thread::spawn(move || {
            for id in copies {
                if let Err(error) = copy_into_vault(&app, &state, &id) {
                    log::error(state.hub.paths(), &format!("vault {id}: {}", error.message));
                }
            }
        });
    }
    Ok(added)
}

/// Copy without holding the database (a large file from a flash disk takes a while), then record.
pub(crate) fn copy_into_vault(app: &AppHandle, state: &AppState, id: &str) -> Result<()> {
    let (source, name) = read(state, |conn, _| files::source_path(conn, id))?;
    let copy = vault::copy_in(state.hub.paths(), &source, &name, Timestamp::now())?;
    write(app, state, |conn, ctx| Ok(((), Some(files::set_vault(conn, id, &copy, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn keep_drop(app: AppHandle, state: Hub<'_>, drop_id: u32, target: Option<LinkTarget>) -> Result<Vec<Added>> {
    let taken = pending().lock().ok().and_then(|mut slot| match slot.take() {
        Some(found) if found.id == drop_id => Some(found.paths),
        other => {
            *slot = other;
            None
        }
    });
    let Some(paths) = taken else {
        return Err(ErrorView::from(hub_core::Error::NotFound("drop")));
    };
    keep(&app, &state, paths, target)
}

pub(crate) fn main_hwnd(app: &AppHandle) -> Option<isize> {
    let window = app.get_webview_window(window::MAIN)?;
    window.hwnd().ok().map(|hwnd| hwnd.0 as isize)
}

/// "Tambah file": Windows' Open dialog, then the same as a drop.
#[tauri::command(async)]
pub fn pick_files(app: AppHandle, state: Hub<'_>, target: Option<LinkTarget>) -> Result<Vec<Added>> {
    let owner = main_hwnd(&app);
    let paths = shell::on_com_thread(move || shell::pick_files(owner, true, "Tambah file ke Hub")).unwrap_or_default();
    keep(&app, &state, paths.into_iter().take(DROP_MAX).collect(), target)
}

/// "Cari lokasinya": the owner points at where a missing file is now.
#[tauri::command(async)]
pub fn relocate_file(app: AppHandle, state: Hub<'_>, id: String) -> Result<bool> {
    let owner = main_hwnd(&app);
    let picked = shell::on_com_thread(move || shell::pick_files(owner, false, "Di mana file ini sekarang?")).unwrap_or_default();
    let Some(path) = picked.into_iter().next() else { return Ok(false) };
    let probe = identity::probe(&path).map_err(|error| internal(error.to_string()))?;
    write(&app, &state, |conn, ctx| Ok((true, Some(files::relocate(conn, &id, &probe, OWNER, ctx.now)?))))
}

/// Open with Windows' default app; a program is shown in Explorer instead. A file that is gone
/// is checked again (it may have moved), and the page told.
#[tauri::command(async)]
pub fn open_file(app: AppHandle, state: Hub<'_>, id: String) -> Result<&'static str> {
    let found = read(&state, |conn, _| files::openable(conn, state.hub.paths(), &id));
    let openable = match found {
        Ok(openable) => openable,
        Err(error) => {
            recheck(&app, &state, &id);
            return Err(error);
        }
    };
    let path = openable.path.clone();
    let program = openable.program;
    let done = shell::on_com_thread(move || if program { shell::reveal(&path) } else { shell::open(&path) }).unwrap_or(false);
    if !done {
        return Err(internal("Windows could not open the file"));
    }
    Ok(match (program, openable.from_vault) {
        (true, _) => "revealed",
        (false, true) => "vault",
        (false, false) => "opened",
    })
}

#[tauri::command(async)]
pub fn reveal_file(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    let openable = match read(&state, |conn, _| files::openable(conn, state.hub.paths(), &id)) {
        Ok(openable) => openable,
        Err(error) => {
            recheck(&app, &state, &id);
            return Err(error);
        }
    };
    let path = openable.path;
    if shell::on_com_thread(move || shell::reveal(&path)).unwrap_or(false) {
        Ok(())
    } else {
        Err(internal("Explorer could not show the file"))
    }
}

fn recheck(app: &AppHandle, state: &AppState, id: &str) {
    let checked = state.hub.write(|conn| files::check(conn, id, &System, Timestamp::now()));
    if let Ok(Some(change)) = checked {
        announce(app, &change);
    }
}

#[tauri::command(async)]
pub fn check_file(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), files::check(conn, &id, &System, ctx.now)?)))
}

#[tauri::command(async)]
pub fn file_thumbnail(state: Hub<'_>, id: String) -> Result<Option<thumbs::Thumb>> {
    let Ok(openable) = read(&state, |conn, _| files::openable(conn, state.hub.paths(), &id)) else {
        return Ok(None);
    };
    let cache = state.hub.paths().cache.clone();
    let path = openable.path;
    let icon_only = openable.program;
    Ok(shell::on_com_thread(move || thumbs::thumb(&cache, &id, &path, icon_only)).flatten())
}

#[tauri::command(async)]
pub fn copy_to_vault(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    copy_into_vault(&app, &state, &id)
}

#[tauri::command(async)]
pub fn remove_vault_copy(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(files::remove_vault(conn, state.hub.paths(), &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn list_files(state: Hub<'_>, query: FileQuery) -> Result<Vec<FileCard>> {
    read(&state, |conn, _| files::list(conn, &query))
}

#[tauri::command(async)]
pub fn file_counts(state: Hub<'_>) -> Result<FileCounts> {
    read(&state, |conn, _| files::counts(conn))
}

#[tauri::command(async)]
pub fn get_file(state: Hub<'_>, id: String) -> Result<FileCard> {
    read(&state, |conn, _| files::get(conn, &id))
}

#[tauri::command(async)]
pub fn update_file(app: AppHandle, state: Hub<'_>, id: String, edit: FileEdit) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(files::update(conn, &id, edit, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn delete_file(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(files::delete(conn, &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn restore_file(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(files::restore(conn, &id, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn attach_file(app: AppHandle, state: Hub<'_>, id: String, target: LinkTarget) -> Result<()> {
    write_many(&app, &state, |conn, ctx| Ok(((), files::attach(conn, &id, &target, OWNER, ctx.now)?)))
}

#[tauri::command(async)]
pub fn detach_file(app: AppHandle, state: Hub<'_>, id: String, target: LinkTarget) -> Result<()> {
    write_many(&app, &state, |conn, ctx| Ok(((), files::detach(conn, &id, &target, OWNER, ctx.now)?)))
}

#[tauri::command(async)]
pub fn set_file_collections(app: AppHandle, state: Hub<'_>, id: String, collection_ids: Vec<String>) -> Result<()> {
    write_many(&app, &state, |conn, ctx| Ok(((), files::set_collections(conn, &id, &collection_ids, OWNER, ctx.now)?)))
}

#[tauri::command(async)]
pub fn list_collections(state: Hub<'_>) -> Result<Vec<Collection>> {
    read(&state, |conn, _| files::collections(conn))
}

#[tauri::command(async)]
pub fn create_collection(app: AppHandle, state: Hub<'_>, input: CollectionInput) -> Result<String> {
    write(&app, &state, |conn, ctx| {
        let (id, change) = files::create_collection(conn, input, OWNER, ctx.now)?;
        Ok((id, Some(change)))
    })
}

#[tauri::command(async)]
pub fn update_collection(app: AppHandle, state: Hub<'_>, id: String, input: CollectionInput) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(files::update_collection(conn, &id, input, OWNER, ctx.now)?))))
}

#[tauri::command(async)]
pub fn delete_collection(app: AppHandle, state: Hub<'_>, id: String) -> Result<()> {
    write(&app, &state, |conn, ctx| Ok(((), Some(files::delete_collection(conn, &id, OWNER, ctx.now)?))))
}

/// The slow pass of maintenance.rs: a few files at a time, the ones seen longest ago first.
pub fn check_some(app: &AppHandle, state: &AppState) {
    let checked = state.hub.write(|conn| files::check_some(conn, &System, 100, Timestamp::now()));
    match checked {
        Ok(changes) => {
            for change in changes {
                announce(app, &change);
            }
        }
        Err(error) => log::error(state.hub.paths(), &format!("file check: {error}")),
    }
}

