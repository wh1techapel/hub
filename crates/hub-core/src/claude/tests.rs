use super::*;
use crate::db::open_in_memory;
use crate::prd::tests::sample as sample_prd;
use crate::roadmap::{ApproveRoadmap, HubLink, PlanCheckpoint, PlanTask, RequiredTest, RoadmapPlan, TestKind, TestResult};
use crate::studio::{create_project, StudioProjectInput};

fn now() -> Timestamp {
    "2026-11-06T07:00:00Z".parse().unwrap()
}

fn cp(title: &str, features: &[&str], deps: &[&str]) -> PlanCheckpoint {
    PlanCheckpoint {
        title: title.into(),
        goal: format!("Tujuan {title}"),
        features: features.iter().map(|f| f.to_string()).collect(),
        tasks: vec![PlanTask { title: format!("Buat {title}"), ..Default::default() }, PlanTask { title: format!("Uji {title}"), ..Default::default() }],
        depends_on: deps.iter().map(|d| d.to_string()).collect(),
        acceptance: vec!["Bisa didemonstrasikan".into()],
        required_tests: vec![RequiredTest { name: "Unit test".into(), kind: TestKind::Unit, how: "cargo test".into() }],
        ..Default::default()
    }
}

/// A project with its PRD and roadmap approved, `access` given, the repository in `dir`. Returns
/// the project id and its checkpoint ids.
fn project(conn: &mut Connection, dir: &tempfile::TempDir, access: ClaudeAccess) -> (String, Vec<String>) {
    let (id, _) = create_project(conn, StudioProjectInput { name: "Absensi".into(), idea: String::new() }, Actor::Owner, now()).unwrap();
    prd::save_generated(conn, &id, &sample_prd(), now()).unwrap();
    prd::approve(conn, &id, now()).unwrap();
    let version = prd::view(conn, &id).unwrap().approved.unwrap().id;
    let plan = RoadmapPlan {
        checkpoints: vec![
            PlanCheckpoint { code: "CP-01".into(), ..cp("Fondasi", &["FEAT-001"], &[]) },
            PlanCheckpoint { code: "CP-02".into(), ..cp("Izin", &["FEAT-002", "FEAT-003"], &["CP-01"]) },
        ],
    };
    roadmap::save_generated(conn, &id, &version, &plan, now()).unwrap();
    let input = ApproveRoadmap { link: HubLink::New, repo_path: dir.path().to_string_lossy().into_owned(), claude_access: access };
    roadmap::approve(conn, &id, &input, now()).unwrap();
    let ids = roadmap::view(conn, &id).unwrap().approved.unwrap().checkpoints.into_iter().map(|c| c.id).collect();
    (id, ids)
}

fn refused<T: std::fmt::Debug>(result: Result<T>, field: &str) {
    match result {
        Err(Error::Invalid { field: f, .. }) if f == field => {}
        other => panic!("expected refusal on {field}, got {other:?}"),
    }
}

fn not_found<T: std::fmt::Debug>(result: Result<T>, what: &str) {
    match result {
        Err(Error::NotFound(w)) if w == what => {}
        other => panic!("expected {what} not found, got {other:?}"),
    }
}

#[test]
fn only_projects_given_to_claude_are_visible() {
    let mut conn = open_in_memory().unwrap();
    let (a, b, c) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let (shown, _) = project(&mut conn, &a, ClaudeAccess::Report);
    let (read_only, _) = project(&mut conn, &b, ClaudeAccess::Read);
    let (hidden, _) = project(&mut conn, &c, ClaudeAccess::NoAccess);
    // A project still being planned (no approved roadmap) is not visible either.
    create_project(&mut conn, StudioProjectInput { name: "Ide".into(), idea: String::new() }, Actor::Owner, now()).unwrap();
    let visible: Vec<String> = projects(&conn).unwrap().into_iter().map(|p| p.id).collect();
    assert_eq!(visible.len(), 2);
    assert!(visible.contains(&shown) && visible.contains(&read_only) && !visible.contains(&hidden));
    not_found(require(&conn, &hidden, Need::Read), "studioProject");
    refused(require(&conn, &read_only, Need::Report), "access");
    let p = require(&conn, &shown, Need::Report).unwrap();
    assert_eq!((p.prd_version, p.active_code), (Some(1), None));
}

#[test]
fn connecting_needs_the_repository_folder_or_one_inside_it() {
    let mut conn = open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (id, _) = project(&mut conn, &dir, ClaudeAccess::Read);
    let repo = dir.path().to_string_lossy().into_owned();
    let frame = connect(&conn, &id, &repo).unwrap();
    assert_eq!(frame.checkpoints.iter().map(|c| c.code.as_str()).collect::<Vec<_>>(), ["CP-01", "CP-02"]);
    connect(&conn, &id, &format!("{repo}\\src\\app")).unwrap();
    connect(&conn, &id, &repo.to_uppercase()).unwrap();
    connect(&conn, &id, &format!("{}/", repo.replace('\\', "/"))).unwrap();
    connect(&conn, &id, &format!(r"\\?\{repo}")).unwrap();
    refused(connect(&conn, &id, &format!("{repo}-other")), "folder");
    refused(connect(&conn, &id, &format!("{repo}\\..")), "folder");
    refused(connect(&conn, &id, "C:\\Windows"), "folder");
    refused(connect(&conn, &id, "relative"), "folder");

    // Access taken away: Claude is stopped at its next call.
    roadmap::set_claude_access(&mut conn, &id, ClaudeAccess::NoAccess, now()).unwrap();
    not_found(connect(&conn, &id, &repo), "studioProject");
}

#[test]
fn the_prd_is_served_in_parts() {
    let mut conn = open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (id, _) = project(&mut conn, &dir, ClaudeAccess::Read);
    let outline = prd(&conn, &id, &[], None).unwrap();
    assert_eq!(outline["version"], serde_json::json!(1));
    assert_eq!(outline["features"].as_array().unwrap().len(), 3);
    assert!(outline.get("background").is_none(), "the outline leaves the sections out");
    let goals = prd(&conn, &id, &["goals".into()], None).unwrap();
    assert!(goals["sections"]["goals"]["primary"].is_string());
    let feature = prd(&conn, &id, &[], Some("FEAT-002")).unwrap();
    assert_eq!(feature["feature"]["id"], serde_json::json!("FEAT-002"));
    not_found(prd(&conn, &id, &[], Some("FEAT-009")), "feature");
    refused(prd(&conn, &id, &["secrets".into()], None), "sections");
}

#[test]
fn claude_works_on_the_active_checkpoint_only_and_never_approves() {
    let mut conn = open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (id, cps) = project(&mut conn, &dir, ClaudeAccess::Report);
    // Not Ready yet: only the owner makes it Ready.
    refused(start(&mut conn, &id, "CP-01", now()), "status");
    roadmap::make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    refused(start(&mut conn, &id, "CP-02", now()), "status");
    start(&mut conn, &id, "CP-01", now()).unwrap();
    assert_eq!(roadmap::checkpoint(&conn, &cps[0]).unwrap().status, CheckpointStatus::InProgress);

    update_task(&mut conn, &id, "CP-01.T01", StudioTaskStatus::Done, Some("selesai"), now()).unwrap();
    refused(update_task(&mut conn, &id, "CP-02.T01", StudioTaskStatus::Doing, None, now()), "status");
    not_found(update_task(&mut conn, &id, "CP-09.T01", StudioTaskStatus::Doing, None, now()), "task");
    not_found(checkpoint(&conn, &id, "CP-07"), "checkpoint");

    // A question waits for the owner; a blocker stops the work until it is answered.
    let (question, _) = ask(&mut conn, &id, &Ask { checkpoint: "CP-01".into(), text: "SQLite atau Postgres?".into(), options: vec!["SQLite".into(), "Postgres".into()], blocking: false }, now()).unwrap();
    let (blocker, _) = ask(&mut conn, &id, &Ask { checkpoint: "CP-01".into(), text: "Butuh pembaca kartu".into(), options: vec![], blocking: true }, now()).unwrap();
    assert_eq!(roadmap::checkpoint(&conn, &cps[0]).unwrap().status, CheckpointStatus::Blocked);
    let open = roadmap::requests(&conn, &id, true).unwrap();
    assert!(open.iter().any(|r| r.id == question && r.options == ["SQLite", "Postgres"] && r.made_by == roadmap::Maker::Claude));
    refused(start(&mut conn, &id, "CP-01", now()), "blocker");
    roadmap::answer_request(&mut conn, &blocker, "Pinjam dari lab sebelah", now()).unwrap();
    start(&mut conn, &id, "CP-01", now()).unwrap();
    let frame = frame(&conn, &id).unwrap();
    assert!(frame.owner_notes.iter().any(|n| n.answer.as_deref() == Some("Pinjam dari lab sebelah")));

    // A report that breaks a rule comes back with its problems; a good one puts it In Review.
    let mut report = ReportInput {
        summary: "Selesai".into(),
        changes: vec![roadmap::ReportChange { path: "../luar.txt".into(), what: String::new() }],
        commit: String::new(),
        tests: vec![roadmap::ReportTest { name: "Unit test".into(), command: "cargo test".into(), result: TestResult::Passed, exit_code: Some(0), duration_ms: Some(900), output_tail: "ok".into() }],
        evidence: vec![],
        not_verified: vec![],
        open_issues: vec![],
        next: String::new(),
    };
    let problems = submit(&mut conn, &id, "CP-01", &report, now()).unwrap().unwrap_err();
    assert!(problems.iter().any(|p| p.at == "changes[0].path"));
    assert!(roadmap::reports(&conn, &cps[0]).unwrap().is_empty(), "nothing saved");
    report.changes[0].path = "src/main.rs".into();
    submit(&mut conn, &id, "CP-01", &report, now()).unwrap().unwrap();
    let view = checkpoint(&conn, &id, "CP-01").unwrap();
    assert_eq!(view.checkpoint.status, CheckpointStatus::InReview);
    assert_eq!(view.reports[0].made_by, roadmap::Maker::Claude);
    refused(start(&mut conn, &id, "CP-01", now()), "status");

    // Every change Claude made is Claude's in Jejak.
    let rows = crate::activity::recent(&conn, 100, true).unwrap();
    assert!(rows.iter().filter(|r| r.actor == Actor::Claude).count() >= 6);
}

#[test]
fn read_access_reads_but_does_not_report() {
    let mut conn = open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (id, cps) = project(&mut conn, &dir, ClaudeAccess::Read);
    roadmap::make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    checkpoint(&conn, &id, "CP-01").unwrap();
    refused(start(&mut conn, &id, "CP-01", now()), "access");
    refused(update_task(&mut conn, &id, "CP-01.T01", StudioTaskStatus::Doing, None, now()), "access");
    refused(ask(&mut conn, &id, &Ask { checkpoint: "CP-01".into(), text: "?".into(), options: vec![], blocking: false }, now()), "access");
}

#[test]
fn questions_have_limits() {
    let mut conn = open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (id, _) = project(&mut conn, &dir, ClaudeAccess::Report);
    let ask_with = |options: Vec<String>, text: &str| Ask { checkpoint: "CP-01".into(), text: text.into(), options, blocking: false };
    refused(ask(&mut conn, &id, &ask_with(vec![], " "), now()), "text");
    refused(ask(&mut conn, &id, &ask_with((0..5).map(|i| i.to_string()).collect(), "?"), now()), "options");
    refused(ask(&mut conn, &id, &ask_with(vec!["x".repeat(201)], "?"), now()), "options");
    // A blocker needs the work to be going.
    refused(ask(&mut conn, &id, &Ask { blocking: true, ..ask_with(vec![], "Terhambat") }, now()), "status");
}

#[test]
fn jejak_has_a_studio_group_with_claudes_work() {
    let mut conn = open_in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let (id, cps) = project(&mut conn, &dir, ClaudeAccess::Report);
    roadmap::make_ready(&mut conn, &cps[0], Actor::Owner, now()).unwrap();
    start(&mut conn, &id, "CP-01", now()).unwrap();
    let query = crate::insights::TrailQuery { group: Some(crate::insights::TrailGroup::Studio), actor: Some(Actor::Claude), ..Default::default() };
    let page = crate::insights::trail(&conn, &query).unwrap();
    assert!(!page.items.is_empty());
    assert!(page.items.iter().all(|i| i.actor == Actor::Claude && i.entity.as_str().starts_with("studio_")));
    let projects = crate::insights::TrailQuery { group: Some(crate::insights::TrailGroup::Projects), ..Default::default() };
    // The Hub Proyek made by approval is in Proyek's group, not Studio's.
    assert!(crate::insights::trail(&conn, &projects).unwrap().items.iter().all(|i| !i.entity.as_str().starts_with("studio_")));
}
