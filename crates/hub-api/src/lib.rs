//! Hub's local API for ULTRON (docs/architecture.md, "ULTRON integration").
//!
//! HTTP and JSON on 127.0.0.1, calling the same `hub-core` services as the UI. Loopback alone is
//! not a boundary (a page in the owner's browser can reach it, and through DNS rebinding read the
//! answers), so every request passes the checks in [`guard`], in this order:
//!
//! 1. a request a browser made (an `Origin` or `Sec-Fetch-Site` header) is refused; there is no CORS;
//! 2. `Host` must be exactly `127.0.0.1:<port>` or `localhost:<port>` (stops DNS rebinding);
//! 3. a bearer token is always required, even from loopback; only its hash is in the database;
//! 4. a rate limit per client;
//! 5. each route asks for its scope (`read`, `write:tasks`, ...).
//!
//! Bodies are small (64 KB) and strict (unknown fields refused). Every write is made by the actor
//! `ultron`, so Jejak and the UI show ULTRON's crimson mark. `/v1/events` streams changes and fired
//! reminders (server-sent events), so ULTRON can say a reminder aloud.
//!
//! The app (`src-tauri`) implements [`Host`]: the database, telling its windows what changed, and
//! the Vault copy for files from a removable drive. Tests use their own host.

mod dto;
mod error;
pub mod events;
pub mod guard;
mod routes;
mod studio;
pub use studio::{StudioEvent, BRIDGE_HEADER, SESSION_HEADER};
pub mod token;

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::Router;
use hub_core::{Change, Hub};
use tokio::net::TcpListener;
use tokio::sync::watch;

pub use events::{ApiEvent, Events};

/// Version of the API's routes and shapes (`/v1/...`).
pub const API_VERSION: u32 = 1;

/// What the API needs from the app around it.
pub trait Host: Send + Sync + 'static {
    fn hub(&self) -> &Hub;
    /// The stream `/v1/events` serves. The app publishes every change to it, its own included.
    fn events(&self) -> &Events;
    /// A write went through: tell the windows, wake the reminder clock when needed, and publish
    /// it to [`Host::events`].
    fn changed(&self, change: &Change);
    /// A file kept from a removable drive: copy it into the Vault (in the background).
    fn kept_from_removable(&self, _file_id: &str) {}
    /// Hub's version, for `/v1/health`.
    fn version(&self) -> String;
    /// Something went wrong that the owner may need in `hub.log`. Never a token or the owner's text.
    fn log(&self, message: &str);
    /// Claude's work the owner may want to hear about at once (a toast): a question, a report.
    fn studio_event(&self, _event: StudioEvent) {}
}

/// Shared by every request.
pub(crate) struct ApiState {
    pub host: Arc<dyn Host>,
    /// The port in the `Host` header a request must carry.
    pub port: u16,
    pub limiter: guard::Limiter,
    /// Flips to true when the server stops, so long-lived event streams end with it.
    pub stopping: watch::Receiver<bool>,
    /// Claude Code's sessions, each bound to a project (docs/studio.md, "Access").
    pub sessions: studio::Sessions,
}

/// The routes with every check in front of them. `port` is the one the server listens on.
pub fn router(host: Arc<dyn Host>, port: u16, stopping: watch::Receiver<bool>) -> Router {
    let state = Arc::new(ApiState { host, port, limiter: guard::Limiter::default(), stopping, sessions: studio::Sessions::default() });
    routes::routes(state)
}

/// A running server; dropping it (or [`Server::stop`]) closes the port and ends event streams.
pub struct Server {
    pub port: u16,
    stop: watch::Sender<bool>,
}

impl Server {
    pub fn stop(&self) {
        let _ = self.stop.send(true);
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Bind `127.0.0.1:port` and serve on the current tokio runtime. Fails when the port is taken.
pub async fn start(host: Arc<dyn Host>, port: u16) -> std::io::Result<Server> {
    let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port))).await?;
    let port = listener.local_addr()?.port();
    let (stop, stopping) = watch::channel(false);
    let app = router(host.clone(), port, stopping.clone());
    let mut signal = stopping;
    tokio::spawn(async move {
        let served = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
            .with_graceful_shutdown(async move {
                let _ = signal.wait_for(|stopped| *stopped).await;
            })
            .await;
        if let Err(error) = served {
            host.log(&format!("api: {error}"));
        }
    });
    Ok(Server { port, stop })
}
