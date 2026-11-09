//! Studio's routes for Claude Code through the real router (docs/studio.md, testing flows 8 and
//! 11): each client sees only its own routes, a switched-off client is refused, a session is bound
//! to one project and its repository folder, and Claude can do only what the owner allowed.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use hub_api::{token, ApiEvent, Events, Host, StudioEvent, BRIDGE_HEADER, SESSION_HEADER};
use hub_core::model::{Actor, Entity};
use hub_core::roadmap::{self, ApproveRoadmap, HubLink, PlanCheckpoint, PlanTask, RequiredTest, RoadmapPlan, TestKind};
use hub_core::settings::{self, SettingsPatch};
use hub_core::studio::{self, ClaudeAccess, StudioProjectInput};
use hub_core::{prd, Change, Hub, Paths};
use jiff::Timestamp;
use serde_json::{json, Value};
use tokio::sync::watch;
use tower::ServiceExt;

const PORT: u16 = 47731;

struct TestHost {
    hub: Hub,
    events: Events,
    changes: Mutex<Vec<Change>>,
    studio: Mutex<Vec<StudioEvent>>,
}

impl Host for TestHost {
    fn hub(&self) -> &Hub {
        &self.hub
    }
    fn events(&self) -> &Events {
        &self.events
    }
    fn changed(&self, change: &Change) {
        self.changes.lock().unwrap().push(change.clone());
        self.events.publish(ApiEvent::from(change));
    }
    fn version(&self) -> String {
        "0.1.0-test".into()
    }
    fn log(&self, message: &str) {
        eprintln!("[hub-test] {message}");
    }
    fn studio_event(&self, event: StudioEvent) {
        self.studio.lock().unwrap().push(event);
    }
}

fn now() -> Timestamp {
    Timestamp::now()
}

fn sample_prd() -> prd::Prd {
    let feature = |id: &str, priority: &str| {
        json!({
            "id": id, "name": id, "description": "d", "rationale": "", "priority": priority, "dependsOn": [],
            "story": { "asA": "siswa", "want": "absen", "soThat": "cepat" },
            "acceptance": [{ "id": format!("AC-{}-1", &id[5..]), "given": "g", "when": "w", "then": "t" }], "tests": ["t"]
        })
    };
    serde_json::from_value(json!({
        "identity": { "name": "Absensi", "summary": "Absensi dengan kartu.", "owner": "" },
        "background": { "problem": "p", "targetUsers": ["Siswa"] },
        "goals": { "primary": "g", "metrics": [{ "metric": "m", "target": "t" }] },
        "experience": { "mainFlows": [{ "name": "Absen", "steps": ["Tempel"] }] },
        "features": [feature("FEAT-001", "must"), feature("FEAT-002", "should")],
        "security": [{ "id": "SEC-01", "area": "data", "requirement": "Hanya guru yang melihat rekap", "verify": "Uji akses" }],
        "delivery": { "mvp": ["FEAT-001", "FEAT-002"] }
    }))
    .unwrap()
}

fn checkpoint(title: &str, features: &[&str], deps: &[&str]) -> PlanCheckpoint {
    PlanCheckpoint {
        title: title.into(),
        goal: format!("Tujuan {title}"),
        features: features.iter().map(|f| f.to_string()).collect(),
        tasks: vec![PlanTask { title: format!("Buat {title}"), ..Default::default() }],
        depends_on: deps.iter().map(|d| d.to_string()).collect(),
        acceptance: vec!["Bisa didemonstrasikan".into()],
        required_tests: vec![RequiredTest { name: "Unit test".into(), kind: TestKind::Unit, how: "cargo test".into() }],
        ..Default::default()
    }
}

/// A Studio project with its roadmap approved (`count` checkpoints) and `access` given, its
/// repository at `repo`. Returns its id and checkpoint ids.
fn project(hub: &Hub, repo: &std::path::Path, access: ClaudeAccess, count: usize) -> (String, Vec<String>) {
    hub.write(|conn| {
        let (id, _) = studio::create_project(conn, StudioProjectInput { name: "Absensi".into(), idea: String::new() }, Actor::Owner, now())?;
        prd::save_generated(conn, &id, &sample_prd(), now())?;
        prd::approve(conn, &id, now())?;
        let version = prd::view(conn, &id)?.approved.unwrap().id;
        let mut checkpoints = vec![
            PlanCheckpoint { code: "CP-01".into(), ..checkpoint("Fondasi", &["FEAT-001"], &[]) },
            PlanCheckpoint { code: "CP-02".into(), ..checkpoint("Rekap", &["FEAT-002"], &["CP-01"]) },
        ];
        for i in 2..count {
            checkpoints.push(PlanCheckpoint { code: format!("CP-{:02}", i + 1), ..checkpoint("Lain", &[], &[]) });
        }
        roadmap::save_generated(conn, &id, &version, &RoadmapPlan { checkpoints }, now())?;
        let input = ApproveRoadmap { link: HubLink::New, repo_path: repo.to_string_lossy().into_owned(), claude_access: access };
        roadmap::approve(conn, &id, &input, now())?;
        let ids = roadmap::view(conn, &id)?.approved.unwrap().checkpoints.into_iter().map(|c| c.id).collect();
        Ok((id, ids))
    })
    .unwrap()
}

struct Setup {
    host: Arc<TestHost>,
    ultron: String,
    claude: String,
    router: Router,
    _stop: watch::Sender<bool>,
    dir: tempfile::TempDir,
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let hub = Hub::open(Paths::under(dir.path().join("hub"))).unwrap();
    token::ensure(&hub, PORT, token::ULTRON).unwrap();
    token::ensure(&hub, PORT, token::CLAUDE).unwrap();
    hub.write(|conn| settings::update(conn, SettingsPatch { claude_enabled: Some(true), ..Default::default() }, Actor::Owner, now())).unwrap();
    let ultron = token::current(hub.paths(), token::ULTRON).unwrap().token;
    let claude = token::current(hub.paths(), token::CLAUDE).unwrap().token;
    let host = Arc::new(TestHost { hub, events: Events::default(), changes: Mutex::default(), studio: Mutex::default() });
    let (stop, stopping) = watch::channel(false);
    let router = hub_api::router(host.clone(), PORT, stopping);
    Setup { host, ultron, claude, router, _stop: stop, dir }
}

impl Setup {
    fn repo(&self, name: &str) -> std::path::PathBuf {
        let path = self.dir.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    async fn send(&self, token: &str, session: Option<&str>, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.send_with(token, session.map(|s| (SESSION_HEADER, s)), method, path, body).await
    }

    async fn send_with(&self, token: &str, named: Option<(&str, &str)>, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        let mut builder = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, format!("127.0.0.1:{PORT}"))
            .header(header::AUTHORIZATION, format!("Bearer {token}"));
        if let Some((name, value)) = named {
            builder = builder.header(name, value);
        }
        let request = match body {
            Some(body) => builder.header(header::CONTENT_TYPE, "application/json").body(Body::from(body.to_string())).unwrap(),
            None => builder.body(Body::empty()).unwrap(),
        };
        let mut request = request;
        request.extensions_mut().insert(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))));
        let response = self.router.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
    }

    async fn claude(&self, session: Option<&str>, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.send(&self.claude, session, method, path, body).await
    }

    /// A call of the bridge's own, naming its bridge session.
    async fn bridge(&self, id: Option<&str>, method: Method, path: &str, body: Option<Value>) -> (StatusCode, Value) {
        self.send_with(&self.claude, id.map(|id| (BRIDGE_HEADER, id)), method, path, body).await
    }

    async fn connect(&self, project: &str, folder: &std::path::Path) -> String {
        let (status, body) = self.claude(None, Method::POST, "/v1/studio/sessions", Some(json!({ "projectId": project, "folder": folder.to_string_lossy() }))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        body["session"].as_str().unwrap().to_string()
    }
}

fn reason(body: &Value) -> &str {
    body["error"]["reason"].as_str().or_else(|| body["error"]["field"].as_str()).unwrap_or("")
}

#[tokio::test]
async fn each_client_sees_only_its_own_routes() {
    let s = setup();
    let (status, body) = s.claude(None, Method::GET, "/v1/today", None).await;
    assert_eq!((status, reason(&body)), (StatusCode::FORBIDDEN, "scope"));
    let (status, _) = s.claude(None, Method::POST, "/v1/tasks", Some(json!({ "title": "x" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = s.send(&s.ultron, None, Method::GET, "/v1/studio/projects", None).await;
    assert_eq!((status, reason(&body)), (StatusCode::FORBIDDEN, "scope"));
    let (status, body) = s.claude(None, Method::GET, "/v1/studio/status", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!((body["client"].as_str(), body["projects"].as_u64()), (Some("Claude Code"), Some(0)));
    // No token at all.
    let request = Request::builder().uri("/v1/studio/status").header(header::HOST, format!("127.0.0.1:{PORT}")).body(Body::empty()).unwrap();
    assert_eq!(s.router.clone().oneshot(request).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_client_switched_off_is_refused_and_the_other_works_on() {
    let s = setup();
    s.host.hub.write(|conn| settings::update(conn, SettingsPatch { claude_enabled: Some(false), ..Default::default() }, Actor::Owner, now())).unwrap();
    let (status, body) = s.claude(None, Method::GET, "/v1/studio/status", None).await;
    assert_eq!((status, reason(&body)), (StatusCode::FORBIDDEN, "client"));
    assert_eq!(s.send(&s.ultron, None, Method::GET, "/v1/today", None).await.0, StatusCode::OK);
    s.host.hub.write(|conn| settings::update(conn, SettingsPatch { api_enabled: Some(false), claude_enabled: Some(true), ..Default::default() }, Actor::Owner, now())).unwrap();
    assert_eq!(s.send(&s.ultron, None, Method::GET, "/v1/today", None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(s.claude(None, Method::GET, "/v1/studio/status", None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn a_session_is_bound_to_its_project_and_folder() {
    let s = setup();
    let (repo_a, repo_b) = (s.repo("a"), s.repo("b"));
    let (a, _) = project(&s.host.hub, &repo_a, ClaudeAccess::Report, 2);
    let (b, _) = project(&s.host.hub, &repo_b, ClaudeAccess::Report, 3);
    let hidden_repo = s.repo("c");
    let (hidden, _) = project(&s.host.hub, &hidden_repo, ClaudeAccess::NoAccess, 2);

    let (_, listed) = s.claude(None, Method::GET, "/v1/studio/projects", None).await;
    let ids: Vec<&str> = listed.as_array().unwrap().iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&a.as_str()) && ids.contains(&b.as_str()));

    // The folder must be the project's repository (or inside it).
    let (status, body) = s.claude(None, Method::POST, "/v1/studio/sessions", Some(json!({ "projectId": a, "folder": repo_b.to_string_lossy() }))).await;
    assert_eq!((status, body["error"]["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("folder")));
    let (status, _) = s.claude(None, Method::POST, "/v1/studio/sessions", Some(json!({ "projectId": hidden, "folder": hidden_repo.to_string_lossy() }))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "a project without access is not there for Claude");
    std::fs::create_dir_all(repo_a.join("src")).unwrap();
    let session = s.connect(&a, &repo_a.join("src")).await;

    // Every later call names the session; its project is the only one it reaches.
    let (status, frame) = s.claude(Some(&session), Method::GET, "/v1/studio/frame", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(frame["project"]["id"].as_str(), Some(a.as_str()));
    assert_eq!(s.claude(Some(&session), Method::GET, "/v1/studio/checkpoints/CP-02", None).await.0, StatusCode::OK);
    assert_eq!(s.claude(Some(&session), Method::GET, "/v1/studio/checkpoints/CP-03", None).await.0, StatusCode::NOT_FOUND, "B's CP-03 is not A's");
    let (status, body) = s.claude(None, Method::GET, "/v1/studio/frame", None).await;
    assert_eq!((status, body["error"]["kind"].as_str()), (StatusCode::FORBIDDEN, Some("session")));
    let (status, _) = s.claude(Some(&"0".repeat(32)), Method::GET, "/v1/studio/frame", None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    // ULTRON cannot use Claude's session.
    assert_eq!(s.send(&s.ultron, Some(&session), Method::GET, "/v1/studio/frame", None).await.0, StatusCode::FORBIDDEN);

    let (status, outline) = s.claude(Some(&session), Method::GET, "/v1/studio/prd", None).await;
    assert_eq!((status, outline["features"].as_array().map(Vec::len)), (StatusCode::OK, Some(2)));
    let (_, feature) = s.claude(Some(&session), Method::GET, "/v1/studio/prd?feature=FEAT-002", None).await;
    assert_eq!(feature["feature"]["id"].as_str(), Some("FEAT-002"));
    assert_eq!(s.claude(Some(&session), Method::GET, "/v1/studio/prd?other=1", None).await.0, StatusCode::BAD_REQUEST);

    // Access taken away: the next call is refused, the session no help.
    s.host.hub.write(|conn| roadmap::set_claude_access(conn, &a, ClaudeAccess::NoAccess, now())).unwrap();
    assert_eq!(s.claude(Some(&session), Method::GET, "/v1/studio/frame", None).await.0, StatusCode::NOT_FOUND);

    // Ended by the bridge.
    let other = s.connect(&b, &repo_b).await;
    assert_eq!(s.claude(Some(&other), Method::DELETE, "/v1/studio/sessions", None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(s.claude(Some(&other), Method::GET, "/v1/studio/frame", None).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_new_token_ends_the_sessions_made_with_the_old_one() {
    let mut s = setup();
    let repo = s.repo("a");
    let (a, _) = project(&s.host.hub, &repo, ClaudeAccess::Read, 2);
    let session = s.connect(&a, &repo).await;
    token::rotate(&s.host.hub, PORT, token::CLAUDE).unwrap();
    assert_eq!(s.claude(Some(&session), Method::GET, "/v1/studio/frame", None).await.0, StatusCode::UNAUTHORIZED);
    s.claude = token::current(s.host.hub.paths(), token::CLAUDE).unwrap().token;
    let (status, body) = s.claude(Some(&session), Method::GET, "/v1/studio/frame", None).await;
    assert_eq!((status, body["error"]["kind"].as_str()), (StatusCode::FORBIDDEN, Some("session")));
    let fresh = s.connect(&a, &repo).await;
    assert_eq!(s.claude(Some(&fresh), Method::GET, "/v1/studio/frame", None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn claude_works_the_active_checkpoint_and_only_the_owner_approves() {
    let s = setup();
    let repo = s.repo("a");
    let (a, cps) = project(&s.host.hub, &repo, ClaudeAccess::Report, 2);
    let session = s.connect(&a, &repo).await;
    let call = |method: Method, path: &'static str, body: Option<Value>| s.claude(Some(&session), method, path, body);

    let (status, body) = call(Method::POST, "/v1/studio/checkpoints/CP-01/start", None).await;
    assert_eq!((status, body["error"]["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("status")), "only the owner makes it Ready");
    s.host.hub.write(|conn| roadmap::make_ready(conn, &cps[0], Actor::Owner, now())).unwrap();
    assert_eq!(call(Method::POST, "/v1/studio/checkpoints/CP-02/start", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(call(Method::POST, "/v1/studio/checkpoints/CP-01/start", None).await.0, StatusCode::OK);
    assert_eq!(call(Method::POST, "/v1/studio/tasks/CP-01.T01", Some(json!({ "status": "done", "note": "selesai" }))).await.0, StatusCode::OK);
    assert_eq!(call(Method::POST, "/v1/studio/tasks/CP-01.T01", Some(json!({ "status": "finished" }))).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // A blocker stops the work until the owner answers.
    let (status, asked) = call(Method::POST, "/v1/studio/questions", Some(json!({ "checkpoint": "CP-01", "text": "Butuh pembaca kartu", "blocking": true }))).await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, body) = call(Method::POST, "/v1/studio/checkpoints/CP-01/start", None).await;
    assert_eq!((status, body["error"]["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("blocker")));
    let request = asked["id"].as_str().unwrap().to_string();
    s.host.hub.write(|conn| roadmap::answer_request(conn, &request, "Pinjam dulu", now())).unwrap();
    assert_eq!(call(Method::POST, "/v1/studio/checkpoints/CP-01/start", None).await.0, StatusCode::OK);

    // A report that breaks a rule comes back with each problem; nothing is kept.
    let mut report = json!({
        "summary": "Fondasi selesai.",
        "changes": [{ "path": "C:\\Windows\\system.ini", "what": "x" }],
        "tests": [{ "name": "Unit test", "command": "cargo test", "result": "passed", "exitCode": 0, "durationMs": 1200, "outputTail": "ok" }]
    });
    let (status, body) = s.claude(Some(&session), Method::POST, "/v1/studio/checkpoints/CP-01/report", Some(report.clone())).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"]["problems"][0]["at"].as_str(), Some("changes[0].path"));
    report["changes"][0]["path"] = json!("src/main.rs");
    assert_eq!(s.claude(Some(&session), Method::POST, "/v1/studio/checkpoints/CP-01/report", Some(report)).await.0, StatusCode::CREATED);
    let (_, view) = call(Method::GET, "/v1/studio/checkpoints/CP-01", None).await;
    assert_eq!(view["checkpoint"]["status"].as_str(), Some("in_review"));
    assert_eq!(view["reports"][0]["madeBy"].as_str(), Some("claude"));

    // There is no way for Claude to approve: no such route, and nothing else completes it.
    assert_eq!(call(Method::POST, "/v1/studio/checkpoints/CP-01/approve", None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(call(Method::POST, "/v1/studio/checkpoints/CP-01/start", None).await.0, StatusCode::UNPROCESSABLE_ENTITY);

    // The app heard the question and the report (toasts).
    let heard = s.host.studio.lock().unwrap().clone();
    assert!(heard.contains(&StudioEvent::Asked { project: a.clone(), checkpoint: "CP-01".into(), blocking: true }));
    assert!(heard.contains(&StudioEvent::Reported { project: a.clone(), checkpoint: "CP-01".into() }));
    assert_eq!(heard.iter().filter(|e| matches!(e, StudioEvent::Reported { .. })).count(), 1, "the refused report was not one");

    // Every change was told to the app (Pantau updates at once) and is Claude's in Jejak.
    assert!(s.host.changes.lock().unwrap().iter().any(|c| c.entity == Entity::StudioRoadmap));
    let rows = s.host.hub.read(|conn| hub_core::activity::recent(conn, 100, true)).unwrap();
    assert!(rows.iter().any(|r| r.actor == Actor::Claude && r.entity == Entity::StudioReport));
}

#[tokio::test]
async fn read_access_reads_and_reports_nothing() {
    let s = setup();
    let repo = s.repo("a");
    let (a, cps) = project(&s.host.hub, &repo, ClaudeAccess::Read, 2);
    s.host.hub.write(|conn| roadmap::make_ready(conn, &cps[0], Actor::Owner, now())).unwrap();
    let session = s.connect(&a, &repo).await;
    assert_eq!(s.claude(Some(&session), Method::GET, "/v1/studio/checkpoints/CP-01", None).await.0, StatusCode::OK);
    let (status, body) = s.claude(Some(&session), Method::POST, "/v1/studio/checkpoints/CP-01/start", None).await;
    assert_eq!((status, reason(&body)), (StatusCode::FORBIDDEN, "access"));
    let (status, _) = s.claude(Some(&session), Method::POST, "/v1/studio/questions", Some(json!({ "checkpoint": "CP-01", "text": "?" }))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn pages_and_oversized_bodies_are_refused() {
    let s = setup();
    let repo = s.repo("a");
    let (a, _) = project(&s.host.hub, &repo, ClaudeAccess::Report, 2);
    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/v1/studio/sessions")
        .header(header::HOST, format!("127.0.0.1:{PORT}"))
        .header(header::AUTHORIZATION, format!("Bearer {}", s.claude))
        .header(header::ORIGIN, "https://evil.example")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "projectId": a, "folder": repo.to_string_lossy() }).to_string()))
        .unwrap();
    request.extensions_mut().insert(MockConnectInfo(SocketAddr::from(([127, 0, 0, 1], 50000))));
    assert_eq!(s.router.clone().oneshot(request).await.unwrap().status(), StatusCode::FORBIDDEN);

    let session = s.connect(&a, &repo).await;
    let huge = json!({ "summary": "x".repeat(90 * 1024), "tests": [] });
    let (status, _) = s.claude(Some(&session), Method::POST, "/v1/studio/checkpoints/CP-01/report", Some(huge)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let unknown = json!({ "projectId": a, "folder": repo.to_string_lossy(), "admin": true });
    assert_eq!(s.claude(None, Method::POST, "/v1/studio/sessions", Some(unknown)).await.0, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn the_bridge_reports_its_session_and_calls_and_changes_no_project_data() {
    let s = setup();
    let repo = s.repo("a");
    let (a, _) = project(&s.host.hub, &repo, ClaudeAccess::Report, 2);
    let jejak = || s.host.hub.read(|conn| hub_core::activity::recent(conn, 1000, true)).unwrap().len();
    let before = jejak();
    let hello = json!({ "folder": repo.join("src").to_string_lossy(), "client": "claude-code", "clientVersion": "2.1.317", "bridgeVersion": "0.1.0" });

    // Only Claude Code's client says hello.
    let (status, body) = s.send(&s.ultron, None, Method::POST, "/v1/studio/bridge", Some(hello.clone())).await;
    assert_eq!((status, reason(&body)), (StatusCode::FORBIDDEN, "scope"));
    let (status, opened) = s.bridge(None, Method::POST, "/v1/studio/bridge", Some(hello)).await;
    assert_eq!((status, opened["beatSeconds"].as_u64()), (StatusCode::CREATED, Some(60)), "{opened}");
    let id = opened["id"].as_str().unwrap().to_string();
    let signals = || s.host.hub.read(|conn| hub_core::claude_sessions::signals(conn, Some(&a), now())).unwrap();
    assert_eq!(signals().open, 1, "the folder is inside A's repository");

    // A beat, with a call and without one.
    let call = json!({ "call": { "tool": "hub_connect", "ok": false, "reason": "access", "projectId": a } });
    assert_eq!(s.bridge(Some(&id), Method::POST, "/v1/studio/bridge/beat", Some(call)).await.0, StatusCode::NO_CONTENT);
    let activity = signals().activity.unwrap();
    assert_eq!((activity.tool.as_str(), activity.ok, activity.reason.as_deref()), ("hub_connect", false, Some("access")));
    assert_eq!(s.bridge(Some(&id), Method::POST, "/v1/studio/bridge/beat", Some(json!({}))).await.0, StatusCode::NO_CONTENT);
    // Only the bridge's tools, no other fields, and a session named.
    let (status, body) = s.bridge(Some(&id), Method::POST, "/v1/studio/bridge/beat", Some(json!({ "call": { "tool": "delete_everything", "ok": true } }))).await;
    assert_eq!((status, body["error"]["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("body")));
    assert_eq!(s.bridge(Some(&id), Method::POST, "/v1/studio/bridge/beat", Some(json!({ "admin": true }))).await.0, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, body) = s.bridge(None, Method::POST, "/v1/studio/bridge/beat", Some(json!({}))).await;
    assert_eq!((status, body["error"]["field"].as_str()), (StatusCode::UNPROCESSABLE_ENTITY, Some("session")));

    // The goodbye closes it; a beat after it is told to say hello again.
    assert_eq!(s.bridge(Some(&id), Method::DELETE, "/v1/studio/bridge", None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(signals().session.map(|s| s.state.as_str()), Some("closed"));
    let (status, body) = s.bridge(Some(&id), Method::POST, "/v1/studio/bridge/beat", Some(json!({}))).await;
    assert_eq!((status, body["error"]["field"].as_str()), (StatusCode::NOT_FOUND, Some("claudeSession")));
    assert_eq!(s.bridge(Some(&id), Method::DELETE, "/v1/studio/bridge", None).await.0, StatusCode::NO_CONTENT, "a second goodbye is fine");

    // The windows heard it; Jejak and the project's data did not change; no toast.
    assert!(s.host.changes.lock().unwrap().iter().any(|c| c.entity == Entity::ClaudeSession));
    assert!(s.host.changes.lock().unwrap().iter().all(|c| c.entity == Entity::ClaudeSession));
    assert_eq!(jejak(), before);
    assert!(s.host.studio.lock().unwrap().is_empty());
}
