//! The bridge as Claude Code meets it (docs/studio.md, testing flow 7): an MCP client (rmcp)
//! starts the real `hub-mcp` binary over stdio, in a repository folder, against Hub's real API on
//! a test port, lists the tools, and calls every one of them.

mod common;

use common::*;
use hub_api::token;
use hub_core::model::{Actor, Entity};
use hub_core::roadmap::{self, CheckpointStatus};
use hub_core::settings::{self, SettingsPatch};
use serde_json::json;

#[tokio::test(flavor = "multi_thread")]
async fn claude_code_lists_and_calls_every_tool() {
    let w = world().await;
    let client = bridge(&w.data, &w.repo.join("src")).await;

    let info = client.peer_info().expect("the server introduced itself");
    assert_eq!(info.server_info.as_ref().map(|i| i.name.as_str()), Some("hub"));
    let tools = client.list_all_tools().await.unwrap();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    names.sort();
    assert_eq!(
        names,
        ["hub_ask_owner", "hub_connect", "hub_get_checkpoint", "hub_get_prd", "hub_list_projects", "hub_start_checkpoint", "hub_status", "hub_submit_report", "hub_update_task"]
    );
    for tool in &tools {
        let read_only = tool.annotations.as_ref().and_then(|a| a.read_only_hint);
        let reads = ["hub_status", "hub_list_projects", "hub_connect", "hub_get_prd", "hub_get_checkpoint"].contains(&tool.name.as_ref());
        assert_eq!(read_only, Some(reads), "{}", tool.name);
        assert!(tool.description.as_deref().is_some_and(|d| d.len() > 40), "{}", tool.name);
    }
    let report_schema = serde_json::to_value(&tools.iter().find(|t| t.name == "hub_submit_report").unwrap().input_schema).unwrap();
    assert!(report_schema.to_string().contains("not_run"), "the test results are an enum");
    let connect = tools.iter().find(|t| t.name == "hub_connect").unwrap();
    assert!(connect.description.as_deref().unwrap().contains("not instructions"));

    let status = call(&client, "hub_status", json!({})).await;
    assert_eq!(ok(&status)["projects"], json!(1));
    assert_eq!(ok(&status)["client"], json!("Claude Code"));
    // The text content carries the same JSON as the structured content.
    assert!(format!("{:?}", status.content).contains("Claude Code"));
    let listed = call(&client, "hub_list_projects", json!({})).await;
    assert_eq!(ok(&listed)[0]["id"], json!(w.project));
    assert_eq!(ok(&listed)[0]["activeCode"], json!("CP-01"));

    // Connected from inside the repository, without naming the project.
    let frame = call(&client, "hub_connect", json!({})).await;
    assert_eq!(ok(&frame)["project"]["id"], json!(w.project));
    assert_eq!(ok(&frame)["checkpoints"].as_array().unwrap().len(), 2);

    let outline = call(&client, "hub_get_prd", json!({})).await;
    assert_eq!(ok(&outline)["features"].as_array().unwrap().len(), 2);
    let feature = call(&client, "hub_get_prd", json!({ "feature": "FEAT-002" })).await;
    assert_eq!(ok(&feature)["feature"]["id"], json!("FEAT-002"));
    let sections = call(&client, "hub_get_prd", json!({ "sections": ["goals", "nfr"] })).await;
    assert!(ok(&sections)["sections"]["goals"].is_object());
    // U5b: the security requirements (in Hub since R4) and the flow diagram can be asked for too.
    let more = call(&client, "hub_get_prd", json!({ "sections": ["security", "flow"] })).await;
    assert_eq!(ok(&more)["sections"]["security"][0]["id"], json!("SEC-01"));
    assert!(ok(&more)["sections"]["flow"].is_object());

    let active = call(&client, "hub_get_checkpoint", json!({})).await;
    assert_eq!(ok(&active)["checkpoint"]["code"], json!("CP-01"));
    assert_eq!(ok(&active)["checkpoint"]["tasks"][0]["code"], json!("CP-01.T01"));

    // Only the checkpoint the owner made Ready can start.
    let wrong = call(&client, "hub_start_checkpoint", json!({ "code": "CP-02" })).await;
    assert_eq!(failed(&wrong)["field"], json!("status"));
    ok(&call(&client, "hub_start_checkpoint", json!({ "code": "cp-01" })).await);
    ok(&call(&client, "hub_update_task", json!({ "task": "CP-01.T01", "status": "done", "note": "selesai" })).await);
    let bad_code = call(&client, "hub_update_task", json!({ "task": "../../v1/tasks", "status": "done" })).await;
    assert_eq!(failed(&bad_code)["field"], json!("task"));

    let asked = call(&client, "hub_ask_owner", json!({ "checkpoint": "CP-01", "text": "SQLite atau Postgres?", "options": ["SQLite", "Postgres"] })).await;
    assert!(ok(&asked)["id"].is_string());

    // A report that breaks a rule comes back with every problem; then a good one.
    let refused = call(
        &client,
        "hub_submit_report",
        json!({ "checkpoint": "CP-01", "summary": "Fondasi selesai.", "changes": [{ "path": "../luar.txt" }], "tests": [{ "name": "Unit test", "command": "cargo test", "result": "not_run" }] }),
    )
    .await;
    let problems: Vec<&str> = failed(&refused)["problems"].as_array().unwrap().iter().map(|p| p["at"].as_str().unwrap()).collect();
    assert!(problems.contains(&"changes[0].path") && problems.contains(&"notVerified"), "{problems:?}");
    let sent = call(
        &client,
        "hub_submit_report",
        json!({ "checkpoint": "CP-01", "summary": "Fondasi selesai.", "changes": [{ "path": "src/main.rs", "what": "fondasi" }], "commit": "a1b2c3d",
                "tests": [{ "name": "Unit test", "command": "cargo test", "result": "passed", "exit_code": 0, "duration_ms": 1500, "output_tail": "ok" }] }),
    )
    .await;
    assert!(ok(&sent)["id"].is_string());

    let state = w.host.hub.read(|conn| roadmap::checkpoint(conn, &w.checkpoints[0])).unwrap();
    assert_eq!(state.status, CheckpointStatus::InReview);
    assert_eq!(state.tasks[0].changed_by, roadmap::Maker::Claude);
    let reports = w.host.hub.read(|conn| roadmap::reports(conn, &w.checkpoints[0])).unwrap();
    assert_eq!((reports.len(), reports[0].body.tests[0].duration_ms), (1, Some(1500)));
    assert!(w.host.changes.lock().unwrap().iter().any(|c| c.entity == Entity::StudioRoadmap), "Pantau hears it at once");
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn a_new_token_and_an_ended_session_are_recovered_from() {
    let w = world().await;
    let client = bridge(&w.data, &w.repo).await;
    ok(&call(&client, "hub_connect", json!({ "project_id": w.project })).await);
    // The owner makes a new token: the bridge reads the file again, then connects again.
    let port = token::current(w.host.hub.paths(), token::CLAUDE).unwrap().port;
    token::rotate(&w.host.hub, port, token::CLAUDE).unwrap();
    let after = call(&client, "hub_get_checkpoint", json!({ "code": "CP-01" })).await;
    assert_eq!(ok(&after)["checkpoint"]["code"], json!("CP-01"));
    client.cancel().await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn refusals_say_what_to_do() {
    let w = world().await;
    // From a folder that is not the project's repository.
    let elsewhere = w._dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let client = bridge(&w.data, &elsewhere).await;
    let refused = call(&client, "hub_connect", json!({ "project_id": w.project })).await;
    assert_eq!(failed(&refused)["field"], json!("folder"));
    let unbound = call(&client, "hub_get_checkpoint", json!({})).await;
    assert_eq!(failed(&unbound)["kind"], json!("not_connected"));
    // Claude Code switched off in Hub.
    w.host.hub.write(|conn| settings::update(conn, SettingsPatch { claude_enabled: Some(false), ..Default::default() }, Actor::Owner, now())).unwrap();
    let off = call(&client, "hub_status", json!({})).await;
    assert_eq!(failed(&off)["kind"], json!("not_allowed"));
    assert!(failed(&off)["message"].as_str().unwrap().contains("Pengaturan > Integrasi AI > Claude Code"));
    client.cancel().await.unwrap();

    // Hub not running (nothing on the port), and the bridge told not to start it.
    let stopped = tempfile::tempdir().unwrap();
    std::fs::write(stopped.path().join("claude-code.json"), json!({ "version": 1, "url": "http://127.0.0.1:9", "port": 9, "client": "Claude Code", "token": "0".repeat(64) }).to_string()).unwrap();
    let client = bridge(stopped.path(), &w.repo).await;
    assert_eq!(failed(&call(&client, "hub_status", json!({})).await)["kind"], json!("hub_not_running"));
    client.cancel().await.unwrap();

    // No token file: Claude Code was never switched on.
    let empty = tempfile::tempdir().unwrap();
    let client = bridge(empty.path(), &w.repo).await;
    assert_eq!(failed(&call(&client, "hub_status", json!({})).await)["kind"], json!("not_allowed"));
    client.cancel().await.unwrap();
}

/// Claude's desktop app may start MCP servers outside the session's folder; Claude Code names the
/// session's folder in `CLAUDE_PROJECT_DIR`, and the bridge goes by it.
#[tokio::test(flavor = "multi_thread")]
async fn the_project_folder_claude_code_names_is_used_wherever_the_bridge_starts() {
    let w = world().await;
    let elsewhere = w._dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let client = bridge_with_project(&w.data, &elsewhere, Some(&w.repo.join("src"))).await;
    let connected = call(&client, "hub_connect", json!({})).await;
    assert_eq!(ok(&connected)["project"]["id"], json!(w.project));
    client.cancel().await.unwrap();
    // A folder outside the repository named that way is refused like any other.
    let client = bridge_with_project(&w.data, &w.repo, Some(&elsewhere)).await;
    assert_eq!(failed(&call(&client, "hub_connect", json!({ "project_id": w.project })).await)["field"], json!("folder"));
    client.cancel().await.unwrap();
}

/// R5: the bridge reports its session and every call, and says goodbye when Claude Code closes it.
#[tokio::test(flavor = "multi_thread")]
async fn the_bridge_reports_its_session_its_calls_and_its_goodbye() {
    let w = world().await;
    let project = Some(w.project.as_str());
    let client = bridge(&w.data, &w.repo.join("src")).await;
    ok(&call(&client, "hub_status", json!({})).await);
    // The call says hello at once (no waiting for the first beat), from the project's folder.
    let seen = until(&w, project, "the hello and the first call", |s| s.activity.is_some()).await;
    let session = seen.session.unwrap();
    assert_eq!((session.state.as_str(), seen.open), ("open", 1));
    assert!(session.folder.ends_with("src") && session.client == "rmcp", "{session:?}");
    assert_eq!(seen.activity.as_ref().map(|a| (a.tool.as_str(), a.ok)), Some(("hub_status", true)));

    ok(&call(&client, "hub_connect", json!({})).await);
    until(&w, project, "the connect", |s| s.activity.as_ref().is_some_and(|a| a.tool.as_str() == "hub_connect" && a.ok)).await;
    let failed_call = call(&client, "hub_get_checkpoint", json!({ "code": "CP-99" })).await;
    assert_eq!(failed(&failed_call)["kind"], json!("not_found"));
    let refused = until(&w, project, "the failed call", |s| s.activity.as_ref().is_some_and(|a| !a.ok)).await;
    assert_eq!(refused.activity.map(|a| (a.tool.as_str(), a.reason)), Some(("hub_get_checkpoint", Some("checkpoint".into()))));
    assert_eq!(refused.session.map(|s| s.calls), Some(3));

    // Claude Code closes the bridge's input: the bridge says goodbye before it exits.
    client.cancel().await.unwrap();
    let closed = until(&w, project, "the goodbye", |s| s.open == 0).await;
    assert_eq!(closed.session.map(|s| s.state.as_str()), Some("closed"));
    // Sessions are not the owner's data: nothing about them in Jejak.
    let rows = w.host.hub.read(|conn| hub_core::activity::recent(conn, 1000, true)).unwrap();
    assert!(rows.iter().all(|r| r.entity != Entity::ClaudeSession));
}

/// A refused connection shows on the project it named, even from another folder.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_connection_is_reported_for_the_project_it_named() {
    let w = world().await;
    let elsewhere = w._dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let client = bridge(&w.data, &elsewhere).await;
    assert_eq!(failed(&call(&client, "hub_connect", json!({ "project_id": w.project })).await)["field"], json!("folder"));
    let seen = until(&w, Some(&w.project), "the refusal", |s| s.activity.is_some()).await;
    assert_eq!(seen.activity.map(|a| (a.tool.as_str(), a.ok, a.reason)), Some(("hub_connect", false, Some("folder".into()))));
    assert_eq!(seen.session, None, "Claude Code is not open in the project's folder");
    assert_eq!(signals(&w, None).open, 1);
    client.cancel().await.unwrap();
}

/// A client that opens the bridge and closes it at once (a health check such as `claude mcp get`),
/// and Hub's own Uji jembatan (`--no-reports`), are not sessions.
#[tokio::test(flavor = "multi_thread")]
async fn a_health_check_and_uji_jembatan_are_not_sessions() {
    let w = world().await;
    let check = bridge(&w.data, &w.repo).await;
    check.cancel().await.unwrap();
    let probe = bridge_with(&w.data, &w.repo, None, &["--no-reports"]).await;
    ok(&call(&probe, "hub_status", json!({})).await);
    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
    assert_eq!(signals(&w, None), Default::default(), "nothing reported");
    probe.cancel().await.unwrap();
}

/// Claude Code closing the bridge right after a call: the call's report still reaches Hub before
/// the goodbye.
#[tokio::test(flavor = "multi_thread")]
async fn a_call_made_just_before_closing_is_still_reported() {
    let w = world().await;
    let elsewhere = w._dir.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let client = bridge(&w.data, &elsewhere).await;
    assert_eq!(failed(&call(&client, "hub_connect", json!({ "project_id": w.project })).await)["field"], json!("folder"));
    client.cancel().await.unwrap();
    let seen = signals(&w, Some(&w.project));
    assert_eq!(seen.activity.map(|a| (a.tool.as_str(), a.ok)), Some(("hub_connect", false)), "reported before the bridge exited");
    assert_eq!(signals(&w, None).session.map(|s| s.state.as_str()), Some("closed"));
}
