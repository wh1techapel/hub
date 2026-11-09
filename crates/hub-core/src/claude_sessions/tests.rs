use super::*;
use crate::db::open_in_memory;
use crate::model::Actor;
use crate::studio::{create_project, StudioProjectInput};

fn now() -> Timestamp {
    "2026-11-08T07:00:00Z".parse().unwrap()
}

fn later(seconds: i64) -> Timestamp {
    now() + SignedDuration::from_secs(seconds)
}

/// A Studio project whose repository folder is `repo`.
fn project(conn: &mut Connection, name: &str, repo: &str) -> String {
    let (id, _) = create_project(conn, StudioProjectInput { name: name.into(), idea: String::new() }, Actor::Owner, now()).unwrap();
    conn.execute("UPDATE studio_projects SET repo_path = ?2 WHERE id = ?1", params![id, repo]).unwrap();
    id
}

fn hello(folder: &str) -> Hello {
    Hello { folder: folder.into(), client: "claude-code".into(), client_version: "2.1.317".into(), bridge_version: "0.1.0".into() }
}

fn call(tool: ClaudeTool, ok: bool, reason: Option<&str>, project: Option<&str>) -> ToolCall {
    ToolCall { tool, ok, reason: reason.map(String::from), project_id: project.map(String::from) }
}

fn refused<T: std::fmt::Debug>(result: Result<T>, field: &str) {
    match result {
        Err(Error::Invalid { field: f, .. }) if f == field => {}
        other => panic!("expected refusal on {field}, got {other:?}"),
    }
}

#[test]
fn nothing_is_known_before_the_bridge_says_hello() {
    let mut conn = open_in_memory().unwrap();
    let absensi = project(&mut conn, "Absensi", r"D:\repo\absensi");
    assert_eq!(signals(&conn, None, now()).unwrap(), ClaudeSignals::default());
    assert_eq!(signals(&conn, Some(&absensi), now()).unwrap(), ClaudeSignals::default());
}

#[test]
fn a_session_belongs_to_the_project_whose_folder_holds_it() {
    let mut conn = open_in_memory().unwrap();
    let absensi = project(&mut conn, "Absensi", r"D:\repo\absensi");
    let kasir = project(&mut conn, "Kasir", r"D:\repo\kasir");
    let (opened, change) = open(&mut conn, &hello(r"d:/REPO/absensi/src/"), now()).unwrap();
    assert_eq!(opened.beat_seconds, BEAT_SECONDS);
    assert_eq!(change, Change::one(Entity::ClaudeSession, opened.id.clone()));

    let here = signals(&conn, Some(&absensi), now()).unwrap();
    let session = here.session.unwrap();
    assert_eq!((session.state, here.open, session.client.as_str(), session.client_version.as_str()), (SessionState::Open, 1, "claude-code", "2.1.317"));
    assert_eq!(session.folder, "d:/REPO/absensi/src", "kept as Claude Code named it, without the trailing separator");
    assert_eq!(here.activity, None, "a hello is not a call");
    assert_eq!(signals(&conn, Some(&kasir), now()).unwrap(), ClaudeSignals::default());
    // A session in no project's folder still counts for Claude Code as a whole.
    open(&mut conn, &hello(r"C:\Users\kamu"), now()).unwrap();
    assert_eq!(signals(&conn, None, now()).unwrap().open, 2);
    assert_eq!(signals(&conn, Some(&absensi), now()).unwrap().open, 1);
}

#[test]
fn the_innermost_project_folder_wins_and_deleted_projects_do_not_count() {
    let mut conn = open_in_memory().unwrap();
    let outer = project(&mut conn, "Monorepo", r"D:\repo");
    let inner = project(&mut conn, "Absensi", r"D:\repo\absensi");
    open(&mut conn, &hello(r"D:\repo\absensi\web"), now()).unwrap();
    assert_eq!(signals(&conn, Some(&inner), now()).unwrap().open, 1);
    assert_eq!(signals(&conn, Some(&outer), now()).unwrap().open, 0);
    crate::studio::delete_project(&mut conn, &inner, Actor::Owner, now()).unwrap();
    open(&mut conn, &hello(r"D:\repo\absensi\web"), now()).unwrap();
    assert_eq!(signals(&conn, Some(&outer), now()).unwrap().open, 1, "the deleted project's folder no longer claims it");
}

#[test]
fn every_call_is_recorded_with_its_outcome_and_project() {
    let mut conn = open_in_memory().unwrap();
    let absensi = project(&mut conn, "Absensi", r"D:\repo\absensi");
    let kasir = project(&mut conn, "Kasir", r"D:\repo\kasir");
    let (opened, _) = open(&mut conn, &hello(r"D:\repo\absensi"), now()).unwrap();

    let change = beat(&mut conn, &opened.id, Some(&call(ClaudeTool::GetCheckpoint, true, Some("ignored when it worked"), None)), later(5)).unwrap();
    assert_eq!(change, Some(Change::one(Entity::ClaudeSession, opened.id.clone())));
    let here = signals(&conn, Some(&absensi), later(5)).unwrap();
    assert_eq!(here.activity, Some(ActivitySignal { tool: ClaudeTool::GetCheckpoint, at: "2026-11-08T07:00:05.000Z".into(), ok: true, reason: None }));
    assert_eq!(here.session.unwrap().calls, 1);

    // A connection refused for another project shows on that project, not on this folder's.
    beat(&mut conn, &opened.id, Some(&call(ClaudeTool::Connect, false, Some("folder"), Some(&kasir))), later(9)).unwrap();
    let there = signals(&conn, Some(&kasir), later(9)).unwrap();
    assert_eq!(there.activity.as_ref().map(|a| (a.tool, a.ok, a.reason.as_deref())), Some((ClaudeTool::Connect, false, Some("folder"))));
    assert_eq!(there.session, None, "Claude Code is not open in Kasir's folder");
    assert_eq!(signals(&conn, None, later(9)).unwrap().activity.map(|a| a.tool), Some(ClaudeTool::Connect));

    // A project Hub does not know (or not an id at all) falls back to the session's project.
    for named in ["0192e0c8-0000-7000-8000-000000000000", "absensi"] {
        beat(&mut conn, &opened.id, Some(&call(ClaudeTool::Connect, false, Some("studioProject"), Some(named))), later(12)).unwrap();
        assert_eq!(signals(&conn, Some(&absensi), later(12)).unwrap().activity.map(|a| a.reason), Some(Some("studioProject".into())));
    }
    assert_eq!(signals(&conn, Some(&absensi), later(12)).unwrap().session.unwrap().calls, 4);
}

#[test]
fn a_plain_beat_keeps_a_session_open_without_announcing_it() {
    let mut conn = open_in_memory().unwrap();
    let (opened, _) = open(&mut conn, &hello(r"D:\repo"), now()).unwrap();
    assert_eq!(beat(&mut conn, &opened.id, None, later(60)).unwrap(), None);
    let seen = signals(&conn, None, later(200)).unwrap().session.unwrap();
    assert_eq!((seen.state, seen.seen_at.as_str()), (SessionState::Open, "2026-11-08T07:01:00Z"));
    // Not heard from for longer than OPEN_SECONDS: quiet. Heard from again: open, and announced.
    let quiet = signals(&conn, None, later(60 + OPEN_SECONDS + 1)).unwrap();
    assert_eq!((quiet.session.unwrap().state, quiet.open), (SessionState::Quiet, 0));
    assert!(beat(&mut conn, &opened.id, None, later(400)).unwrap().is_some());
    assert_eq!(signals(&conn, None, later(400)).unwrap().session.unwrap().state, SessionState::Open);
}

#[test]
fn a_goodbye_closes_the_session_once() {
    let mut conn = open_in_memory().unwrap();
    let (opened, _) = open(&mut conn, &hello(r"D:\repo"), now()).unwrap();
    assert!(close(&mut conn, &opened.id, later(30)).unwrap().is_some());
    assert_eq!(close(&mut conn, &opened.id, later(31)).unwrap(), None, "said twice: nothing more");
    let closed = signals(&conn, None, later(40)).unwrap();
    assert_eq!((closed.open, closed.session.as_ref().map(|s| (s.state, s.ended_at.clone()))), (0, Some((SessionState::Closed, Some("2026-11-08T07:00:30Z".into())))));
    // The bridge reporting after its goodbye (or for a session Hub never knew) says hello again.
    assert!(matches!(beat(&mut conn, &opened.id, None, later(50)), Err(Error::NotFound("claudeSession"))));
    assert!(matches!(beat(&mut conn, &ids::new_id(), None, later(50)), Err(Error::NotFound("claudeSession"))));
    assert_eq!(close(&mut conn, &ids::new_id(), later(50)).unwrap(), None);
    refused(beat(&mut conn, "not-an-id", None, later(50)), "session");
}

#[test]
fn an_open_session_is_shown_before_one_that_ended_later() {
    let mut conn = open_in_memory().unwrap();
    let (first, _) = open(&mut conn, &hello(r"D:\repo\a"), now()).unwrap();
    let (second, _) = open(&mut conn, &hello(r"D:\repo\b"), later(10)).unwrap();
    close(&mut conn, &second.id, later(20)).unwrap();
    beat(&mut conn, &first.id, None, later(15)).unwrap();
    let shown = signals(&conn, None, later(25)).unwrap();
    assert_eq!((shown.open, shown.session.unwrap().folder.as_str()), (1, r"D:\repo\a"));
}

#[test]
fn what_the_bridge_sends_is_checked() {
    let mut conn = open_in_memory().unwrap();
    refused(open(&mut conn, &hello("relative\\folder"), now()), "folder");
    refused(open(&mut conn, &hello(""), now()), "folder");
    refused(open(&mut conn, &Hello { client: "x".repeat(61), ..hello(r"D:\repo") }, now()), "client");
    refused(open(&mut conn, &Hello { client: "claude\u{7}".into(), ..hello(r"D:\repo") }, now()), "client");
    refused(open(&mut conn, &Hello { client_version: "9".repeat(41), ..hello(r"D:\repo") }, now()), "clientVersion");
    let (opened, _) = open(&mut conn, &hello(r"D:\repo"), now()).unwrap();
    refused(beat(&mut conn, &opened.id, Some(&call(ClaudeTool::Status, false, Some("no spaces please"), None)), now()), "reason");
    refused(beat(&mut conn, &opened.id, Some(&call(ClaudeTool::Status, false, Some(&"r".repeat(41)), None)), now()), "reason");
    assert_eq!(signals(&conn, None, now()).unwrap().activity, None, "a refused report records nothing");
    // Only the bridge's own tools, and nothing else in the call.
    assert!(serde_json::from_str::<ToolCall>(r#"{"tool":"hub_status","ok":true}"#).is_ok());
    assert!(serde_json::from_str::<ToolCall>(r#"{"tool":"rm_rf","ok":true}"#).is_err());
    assert!(serde_json::from_str::<ToolCall>(r#"{"tool":"hub_status","ok":true,"extra":1}"#).is_err());
}

#[test]
fn old_sessions_are_let_go() {
    let mut conn = open_in_memory().unwrap();
    let (old, _) = open(&mut conn, &hello(r"D:\repo"), now()).unwrap();
    close(&mut conn, &old.id, now()).unwrap();
    let month = SignedDuration::from_hours(24 * KEEP_DAYS);
    let count = |conn: &Connection| -> i64 { conn.query_row("SELECT count(*) FROM studio_claude_sessions", [], |row| row.get(0)).unwrap() };
    open(&mut conn, &hello(r"D:\repo"), now() + month).unwrap();
    assert_eq!(count(&conn), 2, "exactly 30 days is still kept");
    open(&mut conn, &hello(r"D:\repo"), now() + month + SignedDuration::from_secs(1)).unwrap();
    assert_eq!(count(&conn), 2, "older than 30 days is gone");
    for second in 0..(KEEP_MAX + 5) {
        open(&mut conn, &hello(r"D:\repo"), now() + month + SignedDuration::from_secs(10 + second)).unwrap();
    }
    assert_eq!(count(&conn), KEEP_MAX);
}

/// Found by the native smoke: two sessions calling about one project in the same second.
#[test]
fn the_last_call_is_the_last_even_within_one_second() {
    let mut conn = open_in_memory().unwrap();
    let absensi = project(&mut conn, "Absensi", "D:/repo/absensi");
    let (working, _) = open(&mut conn, &hello("D:/repo/absensi"), now()).unwrap();
    let (elsewhere, _) = open(&mut conn, &hello("D:/lain"), now()).unwrap();
    let at = |ms: i64| now() + SignedDuration::from_millis(ms);
    beat(&mut conn, &elsewhere.id, Some(&call(ClaudeTool::Connect, false, Some("folder"), Some(&absensi))), at(200)).unwrap();
    beat(&mut conn, &working.id, Some(&call(ClaudeTool::GetCheckpoint, true, None, None)), at(700)).unwrap();
    let last = signals(&conn, Some(&absensi), at(900)).unwrap().activity.unwrap();
    assert_eq!((last.tool, last.at.as_str()), (ClaudeTool::GetCheckpoint, "2026-11-08T07:00:00.700Z"), "not the earlier session's refusal");
}
