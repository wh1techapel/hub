//! What the bridge's tests share: Hub's real API on a free port with one project given to Claude,
//! and the real `hub-mcp` binary started over stdio the way Claude Code starts it.
#![allow(dead_code)]

use std::path::Path;
use std::sync::{Arc, Mutex};

use hub_api::{token, ApiEvent, Events, Host};
use hub_core::model::Actor;
use hub_core::roadmap::{self, ApproveRoadmap, HubLink, PlanCheckpoint, PlanTask, RequiredTest, RoadmapPlan, TestKind};
use hub_core::settings::{self, SettingsPatch};
use hub_core::studio::{self, ClaudeAccess, StudioProjectInput};
use hub_core::{prd, Change, Hub, Paths};
use jiff::Timestamp;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::RunningService;
use rmcp::transport::{ConfigureCommandExt, TokioChildProcess};
use rmcp::{RoleClient, ServiceExt};
use serde_json::{json, Value};

pub struct TestHost {
    pub hub: Hub,
    pub events: Events,
    pub changes: Mutex<Vec<Change>>,
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
}

pub fn now() -> Timestamp {
    Timestamp::now()
}

pub fn sample_prd() -> prd::Prd {
    let feature = |id: &str, priority: &str| {
        json!({
            "id": id, "name": format!("Fitur {id}"), "description": "d", "rationale": "", "priority": priority, "dependsOn": [],
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

pub fn plan() -> RoadmapPlan {
    let cp = |code: &str, title: &str, features: &[&str], deps: &[&str]| PlanCheckpoint {
        code: code.into(),
        title: title.into(),
        goal: format!("Tujuan {title}"),
        features: features.iter().map(|f| f.to_string()).collect(),
        tasks: vec![PlanTask { title: format!("Buat {title}"), ..Default::default() }, PlanTask { title: format!("Uji {title}"), ..Default::default() }],
        depends_on: deps.iter().map(|d| d.to_string()).collect(),
        acceptance: vec!["Bisa didemonstrasikan".into()],
        required_tests: vec![RequiredTest { name: "Unit test".into(), kind: TestKind::Unit, how: "cargo test".into() }],
        ..Default::default()
    };
    RoadmapPlan { checkpoints: vec![cp("CP-01", "Fondasi", &["FEAT-001"], &[]), cp("CP-02", "Rekap", &["FEAT-002"], &["CP-01"])] }
}

pub struct World {
    pub host: Arc<TestHost>,
    pub _server: hub_api::Server,
    pub data: std::path::PathBuf,
    pub repo: std::path::PathBuf,
    pub project: String,
    pub checkpoints: Vec<String>,
    pub _dir: tempfile::TempDir,
}

/// Hub's API on a free port, Claude Code switched on with its token file, and one project given to
/// Claude (report access) with CP-01 made Ready by the owner.
pub async fn world() -> World {
    let dir = tempfile::tempdir().unwrap();
    let hub = Hub::open(Paths::under(dir.path().join("hub"))).unwrap();
    let data = hub.paths().root.clone();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    hub.write(|conn| settings::update(conn, SettingsPatch { claude_enabled: Some(true), ..Default::default() }, Actor::Owner, now())).unwrap();
    let (project, checkpoints) = hub
        .write(|conn| {
            let (id, _) = studio::create_project(conn, StudioProjectInput { name: "Absensi".into(), idea: String::new() }, Actor::Owner, now())?;
            prd::save_generated(conn, &id, &sample_prd(), now())?;
            prd::approve(conn, &id, now())?;
            let version = prd::view(conn, &id)?.approved.unwrap().id;
            roadmap::save_generated(conn, &id, &version, &plan(), now())?;
            let input = ApproveRoadmap { link: HubLink::New, repo_path: repo.to_string_lossy().into_owned(), claude_access: ClaudeAccess::Report };
            roadmap::approve(conn, &id, &input, now())?;
            let ids: Vec<String> = roadmap::view(conn, &id)?.approved.unwrap().checkpoints.into_iter().map(|c| c.id).collect();
            roadmap::make_ready(conn, &ids[0], Actor::Owner, now())?;
            Ok((id, ids))
        })
        .unwrap();
    let host = Arc::new(TestHost { hub, events: Events::default(), changes: Mutex::default() });
    let server = hub_api::start(host.clone(), 0).await.unwrap();
    token::ensure(&host.hub, server.port, token::CLAUDE).unwrap();
    World { host, _server: server, data, repo, project, checkpoints, _dir: dir }
}

/// The real bridge, started the way Claude Code starts it, in `folder`, with no
/// `CLAUDE_PROJECT_DIR` (a test run from inside Claude Code must not inherit one).
pub async fn bridge(data: &Path, folder: &Path) -> RunningService<RoleClient, ()> {
    bridge_with_project(data, folder, None).await
}

/// The bridge started in `folder` with `CLAUDE_PROJECT_DIR` set to `project` (as Claude Code sets it).
pub async fn bridge_with_project(data: &Path, folder: &Path, project: Option<&Path>) -> RunningService<RoleClient, ()> {
    bridge_with(data, folder, project, &[]).await
}

/// The bridge with more arguments (`--no-reports`).
pub async fn bridge_with(data: &Path, folder: &Path, project: Option<&Path>, extra: &[&str]) -> RunningService<RoleClient, ()> {
    let command = tokio::process::Command::new(env!("CARGO_BIN_EXE_hub-mcp")).configure(|cmd| {
        cmd.arg("--data-dir").arg(data).arg("--no-start").args(extra).current_dir(folder).env_remove("CLAUDE_PROJECT_DIR");
        if let Some(project) = project {
            cmd.env("CLAUDE_PROJECT_DIR", project);
        }
    });
    ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap()
}

/// What Hub shows about Claude Code for `project` (or for all), as Pantau and Pengaturan read it.
pub fn signals(w: &World, project: Option<&str>) -> hub_core::claude_sessions::ClaudeSignals {
    w.host.hub.read(|conn| hub_core::claude_sessions::signals(conn, project, now())).unwrap()
}

/// Wait (up to 10 s) until `done` holds: the bridge reports in the background.
pub async fn until(w: &World, project: Option<&str>, what: &str, done: impl Fn(&hub_core::claude_sessions::ClaudeSignals) -> bool) -> hub_core::claude_sessions::ClaudeSignals {
    for _ in 0..100 {
        let now = signals(w, project);
        if done(&now) {
            return now;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("{what}: {:?}", signals(w, project));
}

pub async fn call(client: &RunningService<RoleClient, ()>, tool: &'static str, args: Value) -> CallToolResult {
    let params = CallToolRequestParams::new(tool).with_arguments(args.as_object().cloned().unwrap_or_default());
    client.call_tool(params).await.unwrap()
}

pub fn ok(result: &CallToolResult) -> &Value {
    assert_ne!(result.is_error, Some(true), "{:?}", result.structured_content);
    result.structured_content.as_ref().expect("structured content")
}

pub fn failed(result: &CallToolResult) -> &Value {
    assert_eq!(result.is_error, Some(true), "{:?}", result.structured_content);
    &result.structured_content.as_ref().expect("structured content")["error"]
}

