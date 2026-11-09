//! Hub's local API as the bridge reaches it: the address and token from `claude-code.json` in
//! Hub's data folder (read again after a 401), the session bound to a project (made again when Hub
//! says it ended), and Hub started quietly when it is not running.
//!
//! The bridge also reports itself (R5): a hello, a beat every minute (Hub says how often) and after
//! every tool call with its outcome, and a goodbye. These never start Hub: a Claude Code session
//! that does not use Hub must not open it. When Hub is not there, the next beat tries again.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::{Duration, Instant};

use reqwest::{Method, StatusCode};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

const FILE: &str = "claude-code.json";
pub const SESSION_HEADER: &str = "hub-session";
pub const BRIDGE_HEADER: &str = "hub-bridge";
/// How long the goodbye may take: Claude Code is closing.
const GOODBYE_WAIT: Duration = Duration::from_secs(2);
/// Until Hub says otherwise.
const BEAT: Duration = Duration::from_secs(60);
/// How long a request to Hub may take.
const TIMEOUT: Duration = Duration::from_secs(20);
/// How long the bridge waits for Hub it started (docs/studio.md: 10 s).
const START_WAIT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Deserialize)]
struct ApiFile {
    url: String,
    token: String,
}

/// A refusal or failure, as the tool returns it to Claude: a kind, the field when there is one,
/// a message, and what to do about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub kind: String,
    pub field: Option<String>,
    pub reason: Option<String>,
    pub message: String,
    pub problems: Option<Value>,
}

/// Boxed, so a `Result` carrying one stays small.
pub type Fail = Box<Failure>;

impl Failure {
    pub fn new(kind: &str, message: impl Into<String>) -> Fail {
        Box::new(Failure { kind: kind.into(), field: None, reason: None, message: message.into(), problems: None })
    }

    pub fn to_json(&self) -> Value {
        let mut out = json!({ "kind": self.kind, "message": self.message });
        if let Some(field) = &self.field {
            out["field"] = json!(field);
        }
        if let Some(reason) = &self.reason {
            out["reason"] = json!(reason);
        }
        if let Some(problems) = &self.problems {
            out["problems"] = problems.clone();
        }
        out
    }
}

/// The messages Claude passes on to the owner name Hub's own Indonesian labels.
const NOT_ALLOWED: &str =
    "Claude Code is not switched on in Hub. Ask the owner to turn it on in Hub: Pengaturan > Integrasi AI > Claude Code.";
const NOT_RUNNING: &str =
    "Hub is not running, or its local API is off. Ask the owner to open Hub and check Pengaturan > Integrasi AI > Claude Code.";

#[derive(Debug, Clone)]
struct Bound {
    session: String,
    project: String,
}

/// The bridge's own session in Hub (hello, beat, goodbye).
#[derive(Debug, Default)]
struct Presence {
    id: Option<String>,
    client: String,
    client_version: String,
    beat: Option<Duration>,
}

pub struct Hub {
    data_dir: PathBuf,
    folder: PathBuf,
    /// `hub.exe` to start when Hub is not running; none in tests.
    hub_exe: Option<PathBuf>,
    http: reqwest::Client,
    file: Mutex<Option<ApiFile>>,
    bound: Mutex<Option<Bound>>,
    started: Mutex<bool>,
    /// Held for a whole hello or beat, so two reports never make two sessions.
    presence: Mutex<Presence>,
    /// Reports of tool calls still on their way, so the goodbye does not leave one behind.
    pending: std::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>,
}

impl Hub {
    pub fn new(data_dir: PathBuf, folder: PathBuf, hub_exe: Option<PathBuf>) -> Hub {
        let http = reqwest::Client::builder().timeout(TIMEOUT).no_proxy().build().unwrap_or_default();
        Hub { data_dir, folder, hub_exe, http, file: Mutex::default(), bound: Mutex::default(), started: Mutex::new(false), presence: Mutex::default(), pending: std::sync::Mutex::default() }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    async fn read_file(&self) -> Result<ApiFile, Fail> {
        let text = tokio::fs::read_to_string(self.data_dir.join(FILE)).await.map_err(|_| Failure::new("not_allowed", NOT_ALLOWED))?;
        let file: ApiFile = serde_json::from_str(&text).map_err(|_| Failure::new("not_allowed", NOT_ALLOWED))?;
        // Only ever Hub on this computer: the token never goes anywhere else.
        if !(file.url.starts_with("http://127.0.0.1:") || file.url.starts_with("http://localhost:")) {
            return Err(Failure::new("not_allowed", NOT_ALLOWED));
        }
        *self.file.lock().await = Some(file.clone());
        Ok(file)
    }

    async fn file(&self) -> Result<ApiFile, Fail> {
        if let Some(file) = self.file.lock().await.clone() {
            return Ok(file);
        }
        self.read_file().await
    }

    /// Start Hub into the tray once, then wait for its API (up to 10 s).
    async fn start_hub(&self) -> bool {
        let mut started = self.started.lock().await;
        if *started {
            return false;
        }
        *started = true;
        let Some(exe) = self.hub_exe.as_ref().filter(|exe| exe.is_file()) else { return false };
        if std::process::Command::new(exe).arg("--tray").spawn().is_err() {
            return false;
        }
        let until = Instant::now() + START_WAIT;
        while Instant::now() < until {
            tokio::time::sleep(Duration::from_millis(400)).await;
            if let Ok(file) = self.read_file().await
                && self.http.get(format!("{}/v1/studio/status", file.url)).bearer_auth(&file.token).send().await.is_ok()
            {
                return true;
            }
        }
        false
    }

    async fn send(&self, file: &ApiFile, method: &Method, path: &str, body: Option<&Value>, session: Option<&str>) -> Result<reqwest::Response, reqwest::Error> {
        let mut request = self.http.request(method.clone(), format!("{}{path}", file.url)).bearer_auth(&file.token);
        if let Some(session) = session {
            request = request.header(SESSION_HEADER, session);
        }
        if let Some(body) = body {
            request = request.json(body);
        }
        request.send().await
    }

    /// One call to Hub, with the recoveries: Hub not running (start it), a 401 (read the token
    /// again), a 403 `session` (connect again to the same project). Boxed: connecting again is
    /// itself a call.
    pub fn call<'a>(&'a self, method: Method, path: &'a str, body: Option<Value>, bound: bool) -> Pin<Box<dyn Future<Output = Result<Value, Fail>> + Send + 'a>> {
        Box::pin(self.call_once(method, path, body, bound))
    }

    async fn call_once(&self, method: Method, path: &str, body: Option<Value>, bound: bool) -> Result<Value, Fail> {
        let mut file = self.file().await?;
        let mut reread = false;
        let mut started = false;
        let mut reconnected = false;
        loop {
            let session = if bound { Some(self.session().await?) } else { None };
            let response = match self.send(&file, &method, path, body.as_ref(), session.as_ref().map(|b| b.session.as_str())).await {
                Ok(response) => response,
                Err(error) if error.is_connect() && !started => {
                    started = true;
                    if self.start_hub().await {
                        file = self.file().await?;
                        continue;
                    }
                    return Err(Failure::new("hub_not_running", NOT_RUNNING));
                }
                Err(error) if error.is_connect() => return Err(Failure::new("hub_not_running", NOT_RUNNING)),
                Err(_) => return Err(Failure::new("hub_unreachable", "Hub did not answer in time. Try again.")),
            };
            let status = response.status();
            let value: Value = response.json().await.unwrap_or(Value::Null);
            if status.is_success() {
                return Ok(value);
            }
            let error = &value["error"];
            let kind = error["kind"].as_str().unwrap_or("error");
            if status == StatusCode::UNAUTHORIZED && !reread {
                reread = true;
                file = self.read_file().await?;
                continue;
            }
            if status == StatusCode::FORBIDDEN && kind == "session" && bound && !reconnected {
                reconnected = true;
                let project = self.bound.lock().await.as_ref().map(|b| b.project.clone());
                *self.bound.lock().await = None;
                if let Some(project) = project {
                    self.connect(&project).await?;
                    continue;
                }
            }
            if status == StatusCode::FORBIDDEN && error["reason"].as_str() == Some("client") {
                return Err(Failure::new("not_allowed", NOT_ALLOWED));
            }
            return Err(Box::new(Failure {
                kind: kind.into(),
                field: error["field"].as_str().map(String::from),
                reason: error["reason"].as_str().map(String::from),
                message: error["message"].as_str().unwrap_or("Hub refused the request").into(),
                problems: error.get("problems").cloned(),
            }));
        }
    }

    /// Bind to `project` from this folder; returns the frame.
    pub async fn connect(&self, project: &str) -> Result<Value, Fail> {
        let body = json!({ "projectId": project, "folder": self.folder.to_string_lossy() });
        let answer = self.call(Method::POST, "/v1/studio/sessions", Some(body), false).await?;
        let session = answer["session"].as_str().unwrap_or_default().to_string();
        *self.bound.lock().await = Some(Bound { session, project: project.into() });
        Ok(answer["frame"].clone())
    }

    /// The project whose repository folder holds this folder, among those Claude may see.
    pub async fn project_here(&self) -> Result<Option<String>, Fail> {
        let projects = self.call(Method::GET, "/v1/studio/projects", None, false).await?;
        let here = comparable(&self.folder.to_string_lossy());
        let found = projects.as_array().into_iter().flatten().find(|p| {
            let repo = comparable(p["repoPath"].as_str().unwrap_or(""));
            !repo.is_empty() && (here == repo || here.starts_with(&format!("{repo}\\")))
        });
        Ok(found.and_then(|p| p["id"].as_str()).map(String::from))
    }

    /// The session, connecting first (to the project of this folder) when there is none.
    async fn session(&self) -> Result<Bound, Fail> {
        if let Some(bound) = self.bound.lock().await.clone() {
            return Ok(bound);
        }
        let project = self.project_here().await?.ok_or_else(|| {
            Failure::new(
                "not_connected",
                "This folder is not the repository folder of a Studio project Claude may use. Call hub_list_projects, then hub_connect from the project's folder.",
            )
        })?;
        self.connect(&project).await?;
        self.bound.lock().await.clone().ok_or_else(|| Failure::new("not_connected", "Call hub_connect first."))
    }

    pub async fn connected_project(&self) -> Option<String> {
        self.bound.lock().await.as_ref().map(|b| b.project.clone())
    }

    /// One call of the bridge's own: never starts Hub, reads the token again once after a 401.
    /// `None` when Hub could not be reached or refused in a way a retry will not fix.
    async fn quiet(&self, method: Method, path: &str, body: Option<&Value>, bridge: Option<&str>) -> Option<(StatusCode, Value)> {
        let mut file = self.file().await.ok()?;
        for attempt in 0..2 {
            let mut request = self.http.request(method.clone(), format!("{}{path}", file.url)).bearer_auth(&file.token);
            if let Some(bridge) = bridge {
                request = request.header(BRIDGE_HEADER, bridge);
            }
            if let Some(body) = body {
                request = request.json(body);
            }
            let response = request.send().await.ok()?;
            let status = response.status();
            if status == StatusCode::UNAUTHORIZED && attempt == 0 {
                file = self.read_file().await.ok()?;
                continue;
            }
            return Some((status, response.json().await.unwrap_or(Value::Null)));
        }
        None
    }

    /// Who started the bridge, as the MCP client named itself.
    pub async fn set_client(&self, name: &str, version: &str) {
        let mut presence = self.presence.lock().await;
        presence.client = name.chars().take(60).collect();
        presence.client_version = version.chars().take(40).collect();
    }

    async fn hello(&self, presence: &mut Presence) -> bool {
        let body = json!({
            "folder": self.folder.to_string_lossy(),
            "client": presence.client,
            "clientVersion": presence.client_version,
            "bridgeVersion": env!("CARGO_PKG_VERSION"),
        });
        match self.quiet(Method::POST, "/v1/studio/bridge", Some(&body), None).await {
            Some((status, answer)) if status == StatusCode::CREATED => {
                presence.id = answer["id"].as_str().map(String::from);
                presence.beat = answer["beatSeconds"].as_u64().map(|s| Duration::from_secs(s.clamp(15, 600)));
                presence.id.is_some()
            }
            _ => false,
        }
    }

    /// Tell Hub the bridge is still there, with the tool call just made if there is one. Says
    /// hello first when there is no session yet, or again when Hub no longer knows it.
    pub async fn beat(&self, call: Option<Value>) {
        let mut presence = self.presence.lock().await;
        let body = json!({ "call": call });
        for _ in 0..2 {
            if presence.id.is_none() && !self.hello(&mut presence).await {
                return;
            }
            let id = presence.id.clone();
            match self.quiet(Method::POST, "/v1/studio/bridge/beat", Some(&body), id.as_deref()).await {
                Some((StatusCode::NOT_FOUND, _)) => presence.id = None,
                _ => return,
            }
        }
    }

    /// How long until the next beat.
    pub async fn beat_every(&self) -> Duration {
        self.presence.lock().await.beat.unwrap_or(BEAT)
    }

    /// Report a tool call in the background: Claude never waits for it.
    pub fn report(self: &std::sync::Arc<Self>, call: Value) {
        let hub = self.clone();
        let handle = tokio::spawn(async move { hub.beat(Some(call)).await });
        if let Ok(mut pending) = self.pending.lock() {
            pending.retain(|h| !h.is_finished());
            pending.push(handle);
        }
    }

    /// Claude Code closed the bridge's input: let the last reports arrive, then say goodbye. All of
    /// it within [`GOODBYE_WAIT`]: Claude Code is closing.
    pub async fn goodbye(&self) {
        let pending: Vec<_> = self.pending.lock().map(|mut p| p.drain(..).collect()).unwrap_or_default();
        let _ = tokio::time::timeout(GOODBYE_WAIT, async {
            for handle in pending {
                let _ = handle.await;
            }
            let id = self.presence.lock().await.id.take();
            if let Some(id) = id {
                let _ = self.quiet(Method::DELETE, "/v1/studio/bridge", None, Some(&id)).await;
            }
        })
        .await;
    }
}

/// A path as compared with a repository folder (the same normalisation Hub uses: case, slashes,
/// `\\?\`, the trailing separator). Hub checks again; this only picks the candidate.
fn comparable(path: &str) -> String {
    let path = path.trim();
    let path = path.strip_prefix(r"\\?\").unwrap_or(path).replace('/', "\\");
    path.trim_end_matches('\\').to_lowercase()
}
