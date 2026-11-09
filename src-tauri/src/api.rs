//! The local API in the app, for ULTRON and for Claude Code (Studio): started with Hub while either
//! client is switched on in Pengaturan, moved when the port changes, stopped when both are off.
//! Each client has its own token and file; a client switched off is refused by the guard. The
//! routes and every defence live in `hub-api`; this gives them the database, the windows, the
//! reminder clock, the Vault, and the owner's ears for Claude's work (toasts). What Pantau and
//! Pengaturan say about Claude Code is stored by the core (`claude_sessions`).

use std::sync::{Arc, Mutex};

use hub_api::{token, ApiEvent, Events, Host, Server, StudioEvent};
use hub_core::api_clients;
use hub_core::model::Entity;
use hub_core::{settings, Change, Hub};
use serde::Serialize;
use tauri::AppHandle;

use crate::{commands, library, log, AppState};

/// `HUB_API_PORT` overrides the port for smokes, so a test Hub never takes the owner's.
const PORT_OVERRIDE: &str = "HUB_API_PORT";

/// What Pengaturan shows about the API.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStatus {
    pub enabled: bool,
    /// Listening now.
    pub running: bool,
    pub port: u16,
    /// Why it is not running when it should be (the port is taken).
    pub problem: Option<String>,
    /// When ULTRON last used it.
    pub last_used_at: Option<String>,
    /// Streams open now (ULTRON listening for reminders).
    pub listeners: usize,
    /// Claude Code's switch.
    pub claude_enabled: bool,
}

#[derive(Default)]
pub struct Api {
    server: Mutex<Option<Server>>,
    problem: Mutex<Option<String>>,
}

struct AppHost {
    app: AppHandle,
    state: Arc<AppState>,
}

impl Host for AppHost {
    fn hub(&self) -> &Hub {
        &self.state.hub
    }

    fn events(&self) -> &Events {
        &self.state.events
    }

    fn changed(&self, change: &Change) {
        if matches!(change.entity, Entity::Reminder | Entity::Settings | Entity::Focus) {
            self.state.scheduler.poke();
        }
        commands::announce(&self.app, change);
    }

    fn kept_from_removable(&self, file_id: &str) {
        let (app, state, id) = (self.app.clone(), self.state.clone(), file_id.to_string());
        std::thread::spawn(move || {
            if let Err(error) = library::copy_into_vault(&app, &state, &id) {
                log::error(state.hub.paths(), &format!("vault {id}: {}", error.message));
            }
        });
    }

    fn version(&self) -> String {
        self.app.package_info().version.to_string()
    }

    fn log(&self, message: &str) {
        log::error(self.state.hub.paths(), message);
    }

    fn studio_event(&self, event: StudioEvent) {
        let project = match &event {
            StudioEvent::Asked { project, .. } | StudioEvent::Reported { project, .. } => project.clone(),
        };
        let name = self.state.hub.read(|conn| hub_core::studio::get_project(conn, &project)).map(|p| p.name).unwrap_or_default();
        // The owner hears what waits for them, wherever Hub is (the tray included).
        let toast = match &event {
            StudioEvent::Asked { checkpoint, blocking: true, .. } => Some((format!("Claude terhambat di {checkpoint}"), format!("{name}: jawab di Pantau supaya Claude bisa lanjut."))),
            StudioEvent::Asked { checkpoint, .. } => Some((format!("Claude bertanya soal {checkpoint}"), format!("{name}: jawab di Pantau."))),
            StudioEvent::Reported { checkpoint, .. } => Some((format!("Laporan {checkpoint} siap ditinjau"), format!("{name}: setujui atau minta perubahan di Pantau."))),
        };
        if let Some((title, body)) = toast {
            crate::notify::show(crate::notify::plain_xml(&title, &body, &format!("studio/{project}")));
        }
        commands::announce(&self.app, &Change::one(Entity::StudioProject, project));
    }
}

fn port_from(stored: u16) -> u16 {
    std::env::var(PORT_OVERRIDE).ok().and_then(|text| text.parse().ok()).unwrap_or(stored)
}

/// Start, move, or stop the server to match the settings. Called on start and after the owner
/// changes the API settings.
pub fn apply(app: &AppHandle) {
    let state = crate::state(app);
    let stored = match state.hub.read(settings::load) {
        Ok(stored) => stored,
        Err(error) => {
            log::error(state.hub.paths(), &format!("api settings: {error}"));
            return;
        }
    };
    let port = port_from(stored.api_port);
    let wanted = stored.api_enabled || stored.claude_enabled;
    // Tokens for the clients that are on (a client switched on later gets its token then).
    let clients = [(stored.api_enabled, token::ULTRON), (stored.claude_enabled, token::CLAUDE)];
    for (on, client) in clients {
        if on && let Err(error) = token::ensure(&state.hub, port, client) {
            log::error(state.hub.paths(), &format!("api token {}: {error}", client.name));
            set_problem(&state, "token");
            return;
        }
    }
    if let Ok(mut server) = state.api.server.lock() {
        match server.as_ref() {
            Some(running) if wanted && running.port == port => return,
            _ => {}
        }
        // Dropping the old server closes its port and ends its event streams.
        *server = None;
    }
    if let Ok(mut problem) = state.api.problem.lock() {
        *problem = None;
    }
    if !wanted {
        return;
    }
    let host: Arc<dyn Host> = Arc::new(AppHost { app: app.clone(), state: state.clone() });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        match hub_api::start(host, port).await {
            Ok(server) => {
                if let Ok(mut slot) = state.api.server.lock() {
                    *slot = Some(server);
                }
            }
            Err(error) => {
                log::error(state.hub.paths(), &format!("api: port {port}: {error}"));
                set_problem(&state, "port");
            }
        }
        commands::announce(&app, &Change::all(Entity::Settings));
    });
}

fn set_problem(state: &AppState, problem: &str) {
    if let Ok(mut slot) = state.api.problem.lock() {
        *slot = Some(problem.into());
    }
}

pub fn status(state: &AppState) -> ApiStatus {
    let stored = state.hub.read(settings::load).unwrap_or_default();
    let running = state.api.server.lock().ok().and_then(|server| server.as_ref().map(|s| s.port));
    let last_used_at = state.hub.read(|conn| api_clients::by_name(conn, token::CLIENT)).ok().flatten().and_then(|c| c.last_used_at);
    ApiStatus {
        enabled: stored.api_enabled,
        running: running.is_some(),
        port: running.unwrap_or_else(|| port_from(stored.api_port)),
        problem: state.api.problem.lock().ok().and_then(|problem| problem.clone()),
        last_used_at,
        listeners: state.events.listeners(),
        claude_enabled: stored.claude_enabled,
    }
}

/// A new token for ULTRON: written to `api.json`, the old one refused from now on, and open
/// streams (opened with the old one) ended.
pub fn rotate(app: &AppHandle) -> hub_core::Result<()> {
    let state = crate::state(app);
    let stored = state.hub.read(settings::load)?;
    token::rotate(&state.hub, port_from(stored.api_port), token::ULTRON)?;
    state.events.publish(ApiEvent::TokenRotated);
    commands::announce(app, &Change::all(Entity::Settings));
    Ok(())
}
