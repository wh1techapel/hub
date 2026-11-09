//! Studio's routes for Claude Code (docs/studio.md, "MCP"), behind the same guard as ULTRON's.
//! Only a client with `studio:read` (and, to change anything, `studio:report`) gets past
//! [`need`]; ULTRON has neither, and Claude Code has none of ULTRON's scopes.
//!
//! A connection is a session: `POST /v1/studio/sessions` checks the project and the folder Claude
//! Code runs in (hub-core `claude::connect`) and answers with a handle (128 random bits). Every
//! later call carries it in the `Hub-Session` header; it is bound to the client, the token it was
//! made with (a rotated token ends it), and the project, and ends after 12 hours unused. Sessions
//! live in memory: after Hub restarts the bridge connects again. Every rule is in hub-core; the
//! project's access is checked again on every call.
//!
//! Apart from that, the bridge reports itself (R5, hub-core `claude_sessions`): a hello once Claude
//! Code has it open (`POST /v1/studio/bridge`), a beat every minute and after every tool call with
//! the call's outcome (`POST /v1/studio/bridge/beat`), and a goodbye when Claude Code closes its
//! input (`DELETE /v1/studio/bridge`), each naming its bridge session in the `Hub-Bridge` header.
//! That is what Pantau and Pengaturan say about Claude Code; it changes no project data.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, Path as Id, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use hub_core::api_clients::{ApiClient, Scope};
use hub_core::claude::{self, Ask, ClaudeCheckpoint, ClaudeFrame, ClaudeProject};
use hub_core::claude_sessions::{self, Hello, Opened, ToolCall};
use hub_core::roadmap::{ReportInput, StudioTaskStatus};
use hub_core::Change;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::dto::{Body, Params};
use crate::error::{from_core, ApiError};
use crate::guard::{need, Presented};
use crate::{ApiState, Host};

type Shared = Arc<ApiState>;
type Answer<T> = Result<Json<T>, ApiError>;

/// A report is at most 64 KB (hub-core checks it); its JSON around it fits in this.
const BODY_MAX: usize = 80 * 1024;
const IDLE: Duration = Duration::from_secs(12 * 60 * 60);
pub const SESSION_HEADER: &str = "hub-session";
/// The bridge's own session (hello, beat, goodbye), not bound to a project.
pub const BRIDGE_HEADER: &str = "hub-bridge";

/// What the app hears about Claude's work (see [`Host::studio_event`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StudioEvent {
    Asked { project: String, checkpoint: String, blocking: bool },
    Reported { project: String, checkpoint: String },
}
const SESSION_CHARS: usize = 32;

struct Session {
    client: String,
    token_hash: String,
    project: String,
    last: Instant,
}

/// The open sessions, by handle.
#[derive(Default)]
pub struct Sessions {
    open: Mutex<HashMap<String, Session>>,
}

impl Sessions {
    fn add(&self, client: &ApiClient, token_hash: &str, project: &str) -> Result<String, ApiError> {
        let mut bytes = [0u8; SESSION_CHARS / 2];
        getrandom::fill(&mut bytes).map_err(|_| ApiError::internal())?;
        let handle: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let mut open = self.open.lock().map_err(|_| ApiError::internal())?;
        open.retain(|_, s| s.last.elapsed() < IDLE);
        open.insert(handle.clone(), Session { client: client.id.clone(), token_hash: token_hash.into(), project: project.into(), last: Instant::now() });
        Ok(handle)
    }

    /// The project of the session the request names, if it is this client's, made with this
    /// token, and not idle too long.
    fn project(&self, headers: &HeaderMap, client: &ApiClient, token_hash: &str) -> Result<String, ApiError> {
        let handle = headers.get(SESSION_HEADER).and_then(|v| v.to_str().ok()).map(str::trim).unwrap_or("");
        let gone = || ApiError::new(StatusCode::FORBIDDEN, "session", "session", "connect again with POST /v1/studio/sessions (hub_connect)");
        if handle.len() != SESSION_CHARS || !handle.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(gone());
        }
        let mut open = self.open.lock().map_err(|_| ApiError::internal())?;
        let Some(session) = open.get_mut(handle) else { return Err(gone()) };
        if session.client != client.id || session.token_hash != token_hash || session.last.elapsed() >= IDLE {
            open.remove(handle);
            return Err(gone());
        }
        session.last = Instant::now();
        Ok(session.project.clone())
    }

    fn end(&self, headers: &HeaderMap) {
        let handle = headers.get(SESSION_HEADER).and_then(|v| v.to_str().ok()).unwrap_or("");
        if let Ok(mut open) = self.open.lock() {
            open.remove(handle.trim());
        }
    }
}

pub(crate) fn routes() -> Router<Shared> {
    Router::new()
        .route("/v1/studio/status", get(status))
        .route("/v1/studio/projects", get(projects))
        .route("/v1/studio/sessions", post(connect).delete(disconnect))
        .route("/v1/studio/bridge", post(hello).delete(goodbye))
        .route("/v1/studio/bridge/beat", post(beat))
        .route("/v1/studio/frame", get(frame))
        .route("/v1/studio/prd", get(prd))
        .route("/v1/studio/checkpoints/{code}", get(checkpoint))
        .route("/v1/studio/checkpoints/{code}/start", post(start))
        .route("/v1/studio/checkpoints/{code}/report", post(report))
        .route("/v1/studio/tasks/{code}", post(update_task))
        .route("/v1/studio/questions", post(ask))
        .layer(DefaultBodyLimit::max(BODY_MAX))
}

/// A core refusal: access the project does not grant is "forbidden", the rest as everywhere.
fn refusal(error: &hub_core::Error) -> ApiError {
    match error {
        hub_core::Error::Invalid { field: "access", .. } => ApiError::forbidden("access", "the owner has not given Claude this access to the project"),
        _ => from_core(error),
    }
}

async fn blocking<T: Send + 'static>(state: &Shared, work: impl FnOnce(&dyn Host) -> hub_core::Result<T> + Send + 'static) -> Result<T, ApiError> {
    let host = state.host.clone();
    match tokio::task::spawn_blocking(move || work(host.as_ref())).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => {
            let answer = refusal(&error);
            if answer.kind == "internal" {
                state.host.log(&format!("api studio: {error}"));
            }
            Err(answer)
        }
        Err(_) => Err(ApiError::internal()),
    }
}

/// A write by Claude, then its change told to the host.
fn write<T>(host: &dyn Host, work: impl FnOnce(&mut rusqlite::Connection, Timestamp) -> hub_core::Result<(T, Option<Change>)>) -> hub_core::Result<T> {
    let (value, change) = host.hub().write(|conn| work(conn, Timestamp::now()))?;
    if let Some(change) = change {
        host.changed(&change);
    }
    Ok(value)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Status {
    name: &'static str,
    version: String,
    api_version: u32,
    client: String,
    /// Projects this client may see.
    projects: usize,
}

async fn status(State(state): State<Shared>, Extension(client): Extension<ApiClient>) -> Answer<Status> {
    need(&client, Scope::StudioRead)?;
    let count = blocking(&state, |host| host.hub().read(|conn| Ok(claude::projects(conn)?.len()))).await?;
    Ok(Json(Status { name: "Hub", version: state.host.version(), api_version: crate::API_VERSION, client: client.name, projects: count }))
}

async fn projects(State(state): State<Shared>, Extension(client): Extension<ApiClient>) -> Answer<Vec<ClaudeProject>> {
    need(&client, Scope::StudioRead)?;
    Ok(Json(blocking(&state, |host| host.hub().read(claude::projects)).await?))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Connect {
    project_id: String,
    /// The folder Claude Code runs in.
    folder: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Connected {
    session: String,
    frame: ClaudeFrame,
}

async fn connect(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Extension(Presented(hash)): Extension<Presented>,
    Body(body): Body<Connect>,
) -> Answer<Connected> {
    need(&client, Scope::StudioRead)?;
    let frame = blocking(&state, move |host| host.hub().read(|conn| claude::connect(conn, &body.project_id, &body.folder))).await?;
    let session = state.sessions.add(&client, &hash, &frame.project.id)?;
    Ok(Json(Connected { session, frame }))
}

async fn disconnect(State(state): State<Shared>, Extension(client): Extension<ApiClient>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    need(&client, Scope::StudioRead)?;
    state.sessions.end(&headers);
    Ok(StatusCode::NO_CONTENT)
}

fn bridge_id(headers: &HeaderMap) -> String {
    headers.get(BRIDGE_HEADER).and_then(|v| v.to_str().ok()).map(str::trim).unwrap_or("").to_string()
}

/// The bridge's hello: a new session, and how often to beat.
async fn hello(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Body(body): Body<Hello>) -> Result<Response, ApiError> {
    need(&client, Scope::StudioRead)?;
    let opened: Opened = blocking(&state, move |host| {
        write(host, |conn, now| {
            let (opened, change) = claude_sessions::open(conn, &body, now)?;
            Ok((opened, Some(change)))
        })
    })
    .await?;
    Ok((StatusCode::CREATED, Json(opened)).into_response())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Beat {
    /// The tool call just made, if this beat reports one.
    #[serde(default)]
    call: Option<ToolCall>,
}

/// Still there (and maybe a call made). A session Hub does not know is 404: say hello again.
async fn beat(State(state): State<Shared>, Extension(client): Extension<ApiClient>, headers: HeaderMap, Body(body): Body<Beat>) -> Result<StatusCode, ApiError> {
    need(&client, Scope::StudioRead)?;
    let id = bridge_id(&headers);
    blocking(&state, move |host| write(host, |conn, now| Ok(((), claude_sessions::beat(conn, &id, body.call.as_ref(), now)?)))).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn goodbye(State(state): State<Shared>, Extension(client): Extension<ApiClient>, headers: HeaderMap) -> Result<StatusCode, ApiError> {
    need(&client, Scope::StudioRead)?;
    let id = bridge_id(&headers);
    blocking(&state, move |host| write(host, |conn, now| Ok(((), claude_sessions::close(conn, &id, now)?)))).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The session's project, for a client with `scope`.
fn bound(state: &Shared, client: &ApiClient, hash: &str, headers: &HeaderMap, scope: Scope) -> Result<String, ApiError> {
    need(client, scope)?;
    state.sessions.project(headers, client, hash)
}

async fn frame(State(state): State<Shared>, Extension(client): Extension<ApiClient>, Extension(Presented(hash)): Extension<Presented>, headers: HeaderMap) -> Answer<ClaudeFrame> {
    let project = bound(&state, &client, &hash, &headers, Scope::StudioRead)?;
    Ok(Json(blocking(&state, move |host| host.hub().read(|conn| claude::frame(conn, &project))).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrdQuery {
    /// Section names, comma separated (`features,nfr`).
    sections: Option<String>,
    /// One feature by id (`FEAT-002`).
    feature: Option<String>,
}

async fn prd(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Extension(Presented(hash)): Extension<Presented>,
    headers: HeaderMap,
    Params(query): Params<PrdQuery>,
) -> Answer<serde_json::Value> {
    let project = bound(&state, &client, &hash, &headers, Scope::StudioRead)?;
    let sections: Vec<String> = query.sections.unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    if sections.len() > hub_core::prd::PrdSection::ALL.len() {
        return Err(ApiError::invalid("sections", "too_many"));
    }
    Ok(Json(blocking(&state, move |host| host.hub().read(|conn| claude::prd(conn, &project, &sections, query.feature.as_deref()))).await?))
}

async fn checkpoint(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Extension(Presented(hash)): Extension<Presented>,
    headers: HeaderMap,
    Id(code): Id<String>,
) -> Answer<ClaudeCheckpoint> {
    let project = bound(&state, &client, &hash, &headers, Scope::StudioRead)?;
    Ok(Json(blocking(&state, move |host| host.hub().read(|conn| claude::checkpoint(conn, &project, &code))).await?))
}

#[derive(Serialize)]
struct Done {
    changed: bool,
}

async fn start(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Extension(Presented(hash)): Extension<Presented>,
    headers: HeaderMap,
    Id(code): Id<String>,
) -> Answer<Done> {
    let project = bound(&state, &client, &hash, &headers, Scope::StudioReport)?;
    blocking(&state, move |host| write(host, |conn, now| Ok(((), Some(claude::start(conn, &project, &code, now)?))))).await?;
    Ok(Json(Done { changed: true }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TaskUpdate {
    status: StudioTaskStatus,
    note: Option<String>,
}

async fn update_task(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Extension(Presented(hash)): Extension<Presented>,
    headers: HeaderMap,
    Id(code): Id<String>,
    Body(body): Body<TaskUpdate>,
) -> Answer<Done> {
    let project = bound(&state, &client, &hash, &headers, Scope::StudioReport)?;
    blocking(&state, move |host| {
        write(host, |conn, now| Ok(((), Some(claude::update_task(conn, &project, &code, body.status, body.note.as_deref(), now)?))))
    })
    .await?;
    Ok(Json(Done { changed: true }))
}

#[derive(Serialize)]
struct Made {
    id: String,
}

async fn ask(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Extension(Presented(hash)): Extension<Presented>,
    headers: HeaderMap,
    Body(body): Body<Ask>,
) -> Result<Response, ApiError> {
    let project = bound(&state, &client, &hash, &headers, Scope::StudioReport)?;
    let event = StudioEvent::Asked { project: project.clone(), checkpoint: body.checkpoint.trim().to_uppercase(), blocking: body.blocking };
    let id = blocking(&state, move |host| {
        write(host, |conn, now| {
            let (id, change) = claude::ask(conn, &project, &body, now)?;
            Ok((id, Some(change)))
        })
    })
    .await?;
    state.host.studio_event(event);
    Ok((StatusCode::CREATED, Json(Made { id })).into_response())
}

async fn report(
    State(state): State<Shared>,
    Extension(client): Extension<ApiClient>,
    Extension(Presented(hash)): Extension<Presented>,
    headers: HeaderMap,
    Id(code): Id<String>,
    Body(body): Body<ReportInput>,
) -> Result<Response, ApiError> {
    let project = bound(&state, &client, &hash, &headers, Scope::StudioReport)?;
    let event = StudioEvent::Reported { project: project.clone(), checkpoint: code.trim().to_uppercase() };
    let outcome = blocking(&state, move |host| {
        write(host, |conn, now| match claude::submit(conn, &project, &code, &body, now)? {
            Ok((id, change)) => Ok((Ok(id), Some(change))),
            Err(problems) => Ok((Err(problems), None)),
        })
    })
    .await?;
    match outcome {
        Ok(id) => {
            state.host.studio_event(event);
            Ok((StatusCode::CREATED, Json(Made { id })).into_response())
        }
        // The report's problems, each with where and what, so Claude can fix them and send again.
        Err(problems) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": { "kind": "invalid", "field": "report", "reason": "format", "message": "the report breaks rules", "problems": problems } })),
        )
            .into_response()),
    }
}
