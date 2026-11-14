//! Hub desktop app: the windows, the tray, the reminder clock, and the commands the webviews may
//! call.
//!
//! Every rule about data lives in `hub-core`; this crate opens the Hub, relays the closed set of
//! commands listed in build.rs, fires reminders, and tells every window when data changed. Hub
//! starts with Windows into the tray (`--tray`), builds a window when the owner opens it, and hides
//! it on close: quitting is in the tray menu. Ctrl+Alt+H opens Catat Cepat from anywhere, and
//! Ctrl+Alt+L opens Lentera's bar (`lentera.rs`). The local API for ULTRON (`api.rs`, `hub-api`)
//! runs beside the windows on 127.0.0.1.

mod api;
mod belajar;
mod claude_code;
mod commands;
mod focus;
mod lentera;
mod lentera_ai;
mod library;
mod log;
mod maintenance;
mod notify;
mod protocol;
mod registry;
mod roadmap;
mod scheduler;
mod shell;
mod stack;
mod studio;
mod thumbs;
mod tray;
mod window;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use hub_core::model::Actor;
use hub_core::{areas, learning, reminders, settings, Hub, Paths};
use tauri::{AppHandle, Manager, RunEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, ShortcutState};

use crate::protocol::{Link, Page};
use crate::scheduler::Scheduler;

/// Shared by the commands, the tray, and the background threads.
pub struct AppState {
    pub hub: Hub,
    /// Started from Windows' autostart: no window until the owner asks for one.
    pub started_hidden: bool,
    pub scheduler: Scheduler,
    pub hotkey_ok: AtomicBool,
    /// Ctrl+Alt+L could be registered for Lentera.
    pub lentera_hotkey_ok: AtomicBool,
    /// Every change and fired reminder, for the API's event stream (ULTRON).
    pub events: hub_api::Events,
    pub api: api::Api,
    /// Hub Studio: the Gemini client and where its key is kept.
    pub studio: studio::Studio,
}

/// The argument Windows' autostart entry passes.
const TRAY_ARG: &str = "--tray";
/// The global shortcut for Catat Cepat (owner's choice, CONTEXT.md).
const CAPTURE_HOTKEY: &str = "ctrl+alt+h";
pub const CAPTURE_HOTKEY_LABEL: &str = "Ctrl+Alt+H";
/// The global shortcut for Lentera's bar (owner's choice, 2026-11-10, CONTEXT.md).
const LENTERA_HOTKEY: &str = "ctrl+alt+l";
pub const LENTERA_HOTKEY_LABEL: &str = "Ctrl+Alt+L";

/// `HUB_DATA_DIR` when set (tests and smokes), else `%LOCALAPPDATA%\Hub`.
fn data_root() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HUB_DATA_DIR").filter(|dir| !dir.is_empty()) {
        return Some(PathBuf::from(dir));
    }
    std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Hub"))
}

pub(crate) fn uses_override() -> bool {
    std::env::var_os("HUB_DATA_DIR").is_some_and(|dir| !dir.is_empty())
}

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    let link = args.iter().find_map(|arg| protocol::parse(arg));
    // A link from a notification only acts; it does not open a window by itself.
    let started_hidden = args.iter().any(|arg| arg == TRAY_ARG) || matches!(link, Some(Link::Snooze(_) | Link::Done(_)));
    let Some(root) = data_root() else {
        window::fatal("Hub tidak menemukan folder data Windows (LOCALAPPDATA).");
        return;
    };
    let hub = match Hub::open(Paths::under(&root)) {
        Ok(hub) => hub,
        Err(hub_core::Error::NewerDatabase { .. }) => {
            window::fatal("Data Hub ditulis oleh versi Hub yang lebih baru. Perbarui Hub dulu; datamu tidak diubah.");
            return;
        }
        Err(error) => {
            log::error(&Paths::under(&root), &format!("open failed: {error}"));
            window::fatal(&format!("Hub tidak bisa membuka datanya di {}.\n\n{error}", root.display()));
            return;
        }
    };
    let state = Arc::new(AppState {
        hub,
        started_hidden,
        scheduler: Scheduler::default(),
        hotkey_ok: AtomicBool::new(false),
        lentera_hotkey_ok: AtomicBool::new(false),
        events: hub_api::Events::default(),
        api: api::Api::default(),
        studio: studio::Studio::new(uses_override().then_some(root.as_path())),
    });

    let mut builder = tauri::Builder::default();
    // One Hub per owner. A test Hub (`HUB_DATA_DIR`, its own data folder) stays out of it, so a
    // smoke runs beside the owner's Hub instead of handing itself over to it and quitting.
    if !uses_override() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            // A link from a notification button, or Hub opened again. A second start with
            // `--tray` (ULTRON starting Hub quietly, or autostart) shows nothing.
            match args.iter().find_map(|arg| protocol::parse(arg)) {
                Some(link) => follow(app, link),
                None if args.iter().any(|arg| arg == TRAY_ARG) => {}
                None => window::show(app),
            }
        }));
    }
    let app = builder
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![TRAY_ARG])))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if shortcut.matches(Modifiers::CONTROL | Modifiers::ALT, Code::KeyL) {
                        lentera::toggle(app);
                    } else {
                        window::toggle_capture(app);
                    }
                })
                .build(),
        )
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            studio::gemini_status,
            studio::gemini_save_key,
            studio::gemini_delete_key,
            studio::gemini_check,
            studio::gemini_models,
            studio::list_studio_projects,
            studio::start_studio_project,
            studio::get_studio_project,
            studio::rename_studio_project,
            studio::delete_studio_project,
            studio::advance_studio_stage,
            studio::studio_messages,
            studio::studio_ledger,
            studio::add_studio_ledger,
            studio::edit_studio_ledger,
            studio::send_studio_message,
            studio::retry_studio_reply,
            studio::stop_studio_reply,
            studio::studio_prd,
            studio::get_prd_version,
            studio::generate_prd,
            studio::update_prd_section,
            studio::revise_prd_section,
            studio::accept_prd_revision,
            studio::save_prd_version,
            studio::approve_prd,
            studio::apply_stack_to_prd,
            studio::export_prd,
            stack::studio_stack,
            stack::save_studio_stack,
            stack::approve_studio_stack,
            stack::recommend_studio_stack,
            roadmap::studio_roadmap,
            roadmap::generate_roadmap,
            roadmap::update_roadmap_checkpoint,
            roadmap::add_roadmap_checkpoint,
            roadmap::remove_roadmap_checkpoint,
            roadmap::move_roadmap_checkpoint,
            roadmap::edit_roadmap,
            roadmap::discard_roadmap_draft,
            roadmap::append_roadmap_checkpoint,
            roadmap::pick_repo_folder,
            roadmap::check_repo_folder,
            roadmap::approve_roadmap,
            roadmap::set_repo_path,
            roadmap::set_claude_access,
            roadmap::studio_requests,
            roadmap::checkpoint_reports,
            roadmap::make_checkpoint_ready,
            roadmap::start_checkpoint,
            roadmap::block_checkpoint,
            roadmap::resume_checkpoint,
            roadmap::answer_studio_request,
            roadmap::submit_checkpoint_report,
            roadmap::check_checkpoint_report,
            roadmap::approve_checkpoint,
            roadmap::request_checkpoint_changes,
            roadmap::set_studio_task_status,
            roadmap::implementation_prompt,
            roadmap::standalone_prompt,
            roadmap::studio_link,
            claude_code::claude_code_status,
            claude_code::install_claude_code,
            claude_code::remove_claude_code,
            claude_code::test_claude_bridge,
            claude_code::rotate_claude_token,
            claude_code::claude_signals,
            commands::update_settings,
            commands::get_api_status,
            commands::rotate_api_token,
            commands::export_data,
            commands::get_info,
            commands::get_shell,
            commands::window_ready,
            commands::set_caption,
            commands::hide_capture,
            lentera::open_lentera,
            lentera::set_lentera_shape,
            lentera::hide_lentera,
            lentera::return_lentera_focus,
            lentera::take_lentera_open,
            lentera::focus_lentera,
            lentera_ai::ask_lentera,
            lentera_ai::stop_lentera,
            lentera_ai::clean_lentera_message,
            lentera_ai::save_lentera_proposal,
            focus::get_focus,
            focus::start_focus,
            focus::pause_focus,
            focus::resume_focus,
            focus::next_focus,
            focus::stop_focus,
            focus::tuck_focus,
            commands::take_start_path,
            commands::get_today,
            commands::list_tasks,
            commands::task_counts,
            commands::get_task,
            commands::create_task,
            commands::update_task,
            commands::complete_task,
            commands::reopen_task,
            commands::delete_task,
            commands::restore_task,
            commands::reorder_task,
            commands::set_task_do_date,
            commands::set_focus,
            commands::list_projects,
            commands::get_project,
            commands::create_project,
            commands::update_project,
            commands::set_project_status,
            commands::delete_project,
            commands::add_milestone,
            commands::update_milestone,
            commands::set_milestone_done,
            commands::delete_milestone,
            commands::list_areas,
            commands::create_area,
            commands::update_area,
            commands::delete_area,
            commands::get_calendar,
            commands::get_event,
            commands::create_event,
            commands::update_event,
            commands::reschedule_event,
            commands::delete_event,
            commands::restore_event,
            commands::list_routines,
            commands::create_routine,
            commands::update_routine,
            commands::delete_routine,
            commands::skip_occurrence,
            commands::move_occurrence,
            commands::restore_occurrence,
            commands::list_reminders,
            commands::create_reminder,
            commands::update_reminder,
            commands::snooze_reminder,
            commands::done_reminder,
            commands::cancel_reminder,
            commands::read_capture,
            commands::save_capture,
            commands::get_review,
            commands::close_day,
            library::keep_drop,
            library::pick_files,
            library::relocate_file,
            library::open_file,
            library::reveal_file,
            library::check_file,
            library::file_thumbnail,
            library::copy_to_vault,
            library::remove_vault_copy,
            library::list_files,
            library::file_counts,
            library::get_file,
            library::update_file,
            library::delete_file,
            library::restore_file,
            library::attach_file,
            library::detach_file,
            library::set_file_collections,
            library::list_collections,
            library::create_collection,
            library::update_collection,
            library::delete_collection,
            belajar::list_tracks,
            belajar::create_track,
            belajar::update_track,
            belajar::delete_track,
            belajar::restore_track,
            belajar::list_learning,
            belajar::get_learning,
            belajar::create_learning,
            belajar::update_learning,
            belajar::set_learning_status,
            belajar::delete_learning,
            belajar::restore_learning,
            belajar::start_timer,
            belajar::stop_timer,
            belajar::discard_timer,
            belajar::add_session,
            belajar::delete_session,
            belajar::list_ctf,
            belajar::create_ctf,
            belajar::update_ctf,
            belajar::delete_ctf,
            belajar::get_learning_overview,
            belajar::get_trail,
            belajar::get_week_insight,
            belajar::search,
            belajar::open_link,
        ])
        .on_window_event(window::on_event)
        .setup(move |app| {
            let handle = app.handle().clone();
            first_run(&handle, &state);
            studio::recover(&state);
            tray::build(&handle)?;
            let hotkey_ok = handle.global_shortcut().register(CAPTURE_HOTKEY).is_ok();
            if !hotkey_ok {
                log::error(state.hub.paths(), "Ctrl+Alt+H could not be registered (another application holds it)");
            }
            state.hotkey_ok.store(hotkey_ok, Ordering::SeqCst);
            let lentera_ok = handle.global_shortcut().register(LENTERA_HOTKEY).is_ok();
            if !lentera_ok {
                log::error(state.hub.paths(), "Ctrl+Alt+L could not be registered (another application holds it)");
            }
            state.lentera_hotkey_ok.store(lentera_ok, Ordering::SeqCst);
            // Notifications under Hub's own name, and the hubapp:// link of their buttons. Not for
            // test folders: a smoke run must not repoint the owner's link at a test build.
            if !uses_override() {
                std::thread::spawn(|| {
                    notify::register();
                    if !protocol::register() {
                        eprintln!("[hub] hubapp:// could not be registered");
                    }
                });
            }
            match link.clone() {
                Some(link) => follow(&handle, link),
                None if !state.started_hidden => window::show(&handle),
                None => {}
            }
            maintenance::start(handle.clone(), state.clone());
            scheduler::start(handle.clone(), state.clone());
            api::apply(&handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Hub failed to build");

    app.run(|_app, event| {
        // Hiding the last window must not end Hub: it lives in the tray. Only the tray's
        // "Keluar" exits (with an explicit code).
        if let RunEvent::ExitRequested { api, code: None, .. } = event {
            api.prevent_exit();
        }
    });
}

/// Act on a hubapp:// link.
fn follow(app: &AppHandle, link: Link) {
    let state = state(app);
    let now = jiff::Timestamp::now();
    let result = match &link {
        Link::Snooze(id) => state.hub.write(|conn| reminders::snooze(conn, id, 10, Actor::Owner, now)).map(|c| c.is_some()),
        Link::Done(id) => state.hub.write(|conn| reminders::done(conn, id, Actor::Owner, now)).map(|c| c.is_some()),
        Link::Open(Page::Today) => {
            window::show_at(app, "/");
            Ok(false)
        }
        Link::Open(Page::Review) => {
            window::show_at(app, "/?review=1");
            Ok(false)
        }
        Link::Open(Page::Studio(id)) => {
            window::show_at(app, &format!("/studio/{id}"));
            Ok(false)
        }
    };
    match result {
        Ok(true) => {
            state.scheduler.poke();
            commands::announce(app, &hub_core::Change::all(hub_core::model::Entity::Reminder));
        }
        Ok(false) => {}
        Err(error) => log::error(state.hub.paths(), &format!("link {link:?}: {error}")),
    }
}


/// Once per Hub: the four starting areas and the five learning tracks (owner's choices), and autostart on the first run of a
/// release build with the real data folder; after that only the owner's switch decides.
fn first_run(app: &AppHandle, state: &AppState) {
    let now = jiff::Timestamp::now();
    if let Err(error) = state.hub.write(|conn| areas::ensure_defaults(conn, now)) {
        log::error(state.hub.paths(), &format!("default areas: {error}"));
    }
    if let Err(error) = state.hub.write(|conn| learning::ensure_default_tracks(conn, now)) {
        log::error(state.hub.paths(), &format!("default tracks: {error}"));
    }
    if cfg!(debug_assertions) || uses_override() {
        return;
    }
    if !matches!(state.hub.read(|conn| settings::is_stored(conn, "autostart")), Ok(false)) {
        return;
    }
    let enabled = app.autolaunch().enable().is_ok();
    if let Err(error) = state.hub.read(|conn| settings::store_initial(conn, "autostart", serde_json::json!(enabled), now)) {
        log::error(state.hub.paths(), &format!("autostart setting: {error}"));
    }
}

/// The owner's view of autostart: what Windows will really do, when that can be read.
pub(crate) fn autostart_enabled(app: &AppHandle, stored: bool) -> bool {
    app.autolaunch().is_enabled().unwrap_or(stored)
}

pub(crate) fn state(app: &AppHandle) -> Arc<AppState> {
    app.state::<Arc<AppState>>().inner().clone()
}
