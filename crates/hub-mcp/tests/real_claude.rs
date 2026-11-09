//! The one run of the real Claude Code (owner's decision, CONTEXT.md S5): `claude -p` in a test
//! repository, with only the test bridge as its MCP server (`--strict-mcp-config`, so nothing in
//! the owner's own Claude Code configuration is used or changed), against Hub's real API on a free
//! port. It uses the owner's Claude usage, so it runs only when asked:
//!
//!     cargo test -p hub-mcp --test real_claude -- --ignored --nocapture

mod common;

use std::time::Duration;

use common::*;
use hub_core::roadmap::{self, CheckpointStatus, Maker};
use serde_json::{json, Value};

/// The `claude` command, found the way Hub finds it (src-tauri `claude_code::find_claude`): a real
/// `claude.exe` on PATH or in `%USERPROFILE%\.local\bin` first, an npm `claude.cmd` shim last.
fn claude() -> Option<std::path::PathBuf> {
    let on_path: Vec<std::path::PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    let native = std::env::var_os("USERPROFILE").map(|h| std::path::PathBuf::from(h).join(".local").join("bin").join("claude.exe"));
    on_path.iter().map(|d| d.join("claude.exe")).chain(native).chain(on_path.iter().map(|d| d.join("claude.cmd"))).find(|p| p.is_file())
}

const TOOLS: &str = "mcp__hub__hub_status,mcp__hub__hub_list_projects,mcp__hub__hub_connect,mcp__hub__hub_get_prd,mcp__hub__hub_get_checkpoint,mcp__hub__hub_start_checkpoint,mcp__hub__hub_update_task,mcp__hub__hub_ask_owner,mcp__hub__hub_submit_report,Bash(cargo test:*)";

/// The steps of the skill /hub-connect, on one line (a .cmd argument cannot hold a line break).
const PROMPT: &str = "You are working on a Hub Studio project through the hub MCP server. Call hub_status, then hub_connect with no project id (this folder is the repository). Read the active checkpoint with hub_get_checkpoint and the PRD outline with hub_get_prd. Start the checkpoint with hub_start_checkpoint. This is an empty test repository: do not create or change any files. Mark each task of the checkpoint done with hub_update_task and a short note saying it was a test run. Run each required test command once and report its real result: if it fails or cannot run, report failed or not_run and explain in not_verified. Then send the report with hub_submit_report; if Hub returns problems, fix them and send again. Then stop and say in one sentence what you reported.";

#[tokio::test(flavor = "multi_thread")]
#[ignore = "runs the real Claude Code with the owner's usage; run with --ignored"]
async fn the_real_claude_code_works_a_checkpoint_through_the_bridge() {
    let claude = claude().expect("the claude command is on PATH");
    let w = world().await;
    let config = w._dir.path().join("mcp.json");
    let server = json!({ "mcpServers": { "hub": { "command": env!("CARGO_BIN_EXE_hub-mcp"), "args": ["--data-dir", w.data, "--no-start"] } } });
    std::fs::write(&config, server.to_string()).unwrap();

    let run = tokio::process::Command::new(&claude)
        .args(["-p", PROMPT, "--mcp-config"])
        .arg(&config)
        .args(["--strict-mcp-config", "--allowedTools", TOOLS, "--output-format", "json", "--max-turns", "30"])
        .current_dir(&w.repo)
        .stdin(std::process::Stdio::null())
        .output();
    let output = tokio::time::timeout(Duration::from_secs(600), run).await.expect("Claude Code finished in 10 minutes").expect("claude ran");
    let text = String::from_utf8_lossy(&output.stdout);
    println!("claude exit: {:?}", output.status.code());
    let answer: Value = serde_json::from_str(text.trim()).unwrap_or(json!({ "raw": text }));
    println!(
        "result: {}\nturns: {}  cost (USD): {}  duration: {} ms",
        answer["result"].as_str().unwrap_or(""),
        answer["num_turns"],
        answer["total_cost_usd"],
        answer["duration_ms"]
    );
    if !output.stderr.is_empty() {
        println!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    let checkpoint = w.host.hub.read(|conn| roadmap::checkpoint(conn, &w.checkpoints[0])).unwrap();
    let reports = w.host.hub.read(|conn| roadmap::reports(conn, &w.checkpoints[0])).unwrap();
    println!("checkpoint: {:?}; tasks: {:?}", checkpoint.status, checkpoint.tasks.iter().map(|t| (t.code.as_str(), t.status, t.changed_by)).collect::<Vec<_>>());
    if let Some(report) = reports.first() {
        println!("report: {}", serde_json::to_string_pretty(&report.body).unwrap());
    }
    assert_eq!(checkpoint.status, CheckpointStatus::InReview, "Claude Code started CP-01 and sent its report");
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].made_by, Maker::Claude);
    assert!(checkpoint.tasks.iter().filter(|t| t.parent_id.is_none()).all(|t| t.changed_by == Maker::Claude), "Claude updated its tasks");
    // Hub cannot know what Claude ran; it holds Claude to reporting it. A test not passed must be
    // explained.
    let test = &reports[0].body.tests[0];
    assert_eq!(test.name, "Unit test");
    if test.result != roadmap::TestResult::Passed {
        assert!(!reports[0].body.not_verified.is_empty());
    }
}
