//! Pengaturan > Integrasi AI > Claude Code (docs/studio.md, "Installing"): whether the bridge, the
//! `claude` command, the registration, and the skill are there; **Pasang** (after the owner saw
//! exactly what changes) runs `claude mcp add --scope user hub -- <hub-mcp.exe> --data-dir <data>`
//! and writes `~/.claude/skills/hub-connect`; **Lepas** undoes both; **Uji jembatan** starts the
//! bridge once and asks Hub's status through it, step by step.
//!
//! Only Hub's own entry and folder are ever touched: an MCP server named `hub` that is not Hub's
//! bridge, or a skill folder Hub did not write, is left alone and reported.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use hub_core::claude_sessions::{self, ClaudeSignals};
use hub_core::model::Actor;
use hub_core::settings::{self, SettingsPatch};
use hub_core::{ErrorKind, ErrorView};
use serde::Serialize;
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::commands::{write, Hub, Result};
use crate::AppState;

const SERVER: &str = "hub";
const BRIDGE: &str = "hub-mcp.exe";
pub const SKILL: &str = include_str!("../skills/hub-connect/SKILL.md");
/// The line that marks the skill folder as Hub's.
const MARK: &str = "<!-- Written by Hub";
const RUN_TIMEOUT: Duration = Duration::from_secs(30);
const BRIDGE_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Registration {
    /// No MCP server `hub` in Claude Code's user configuration.
    Missing,
    /// Hub's bridge.
    Ours,
    /// Something else named `hub`: left alone.
    Other,
    /// The `claude` command was not found, so this cannot be known.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillState {
    Missing,
    Ours,
    Other,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeCodeStatus {
    pub enabled: bool,
    /// `hub-mcp.exe` beside Hub, when it is there.
    pub bridge: Option<String>,
    /// The `claude` command, when it is found.
    pub cli: Option<String>,
    pub registration: Registration,
    pub skill: SkillState,
    pub skill_path: String,
    pub data_dir: String,
    /// What Pasang runs, for the sheet and for running by hand without the `claude` command.
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeStep {
    /// `bridge` (found and answering), `hub` (Hub reached through it), `token` (accepted).
    pub step: String,
    pub ok: bool,
    pub detail: String,
}

fn failed(field: &str, reason: &str, message: impl Into<String>) -> ErrorView {
    ErrorView { kind: ErrorKind::Invalid, field: Some(field.into()), reason: Some(reason.into()), message: message.into() }
}

/// `hub-mcp.exe` beside `hub.exe`.
pub fn bridge_path() -> Option<PathBuf> {
    let path = std::env::current_exe().ok()?.parent()?.join(BRIDGE);
    path.is_file().then_some(path)
}

/// The `claude` command. A real `claude.exe` first (on PATH, then `%USERPROFILE%\.local\bin`
/// where the native installer puts it), an npm `claude.cmd` only after: the `.cmd` is a shim that
/// can point at a binary an interrupted update removed (seen 2026-11-08), while an `.exe` is the
/// program itself.
pub fn find_claude(path: Option<&std::ffi::OsStr>, home: Option<&Path>) -> Option<PathBuf> {
    let on_path: Vec<PathBuf> = path.map(|p| std::env::split_paths(p).collect()).unwrap_or_default();
    let native = home.map(|h| h.join(".local").join("bin").join("claude.exe"));
    on_path
        .iter()
        .map(|dir| dir.join("claude.exe"))
        .chain(native)
        .chain(on_path.iter().map(|dir| dir.join("claude.cmd")))
        .find(|candidate| candidate.is_file())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

/// Claude Code's folder: `CLAUDE_CONFIG_DIR` when set, otherwise `%USERPROFILE%\.claude`.
fn claude_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from).or_else(|| home().map(|h| h.join(".claude")))
}

pub fn skill_dir() -> Option<PathBuf> {
    claude_dir().map(|dir| dir.join("skills").join("hub-connect"))
}

fn skill_state(dir: &Path) -> SkillState {
    match std::fs::read_to_string(dir.join("SKILL.md")) {
        Ok(text) if text.contains(MARK) => SkillState::Ours,
        Ok(_) => SkillState::Other,
        Err(_) if dir.exists() => SkillState::Other,
        Err(_) => SkillState::Missing,
    }
}

/// The arguments of `claude mcp add` for the bridge.
pub fn add_args(bridge: &Path, data_dir: &Path) -> Vec<String> {
    ["mcp", "add", "--scope", "user", SERVER, "--"].iter().map(|s| s.to_string()).chain([bridge.display().to_string(), "--data-dir".into(), data_dir.display().to_string()]).collect()
}

/// The same command as the owner would type it.
pub fn add_line(bridge: &Path, data_dir: &Path) -> String {
    format!("claude mcp add --scope user {SERVER} -- \"{}\" --data-dir \"{}\"", bridge.display(), data_dir.display())
}

/// Run `claude` with `args`, without a console window, for at most 30 s.
fn run(cli: &Path, args: &[String]) -> std::result::Result<(bool, String), String> {
    let mut command = Command::new(cli);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let child = command.spawn().map_err(|error| error.to_string())?;
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(child.wait_with_output());
    });
    match receive.recv_timeout(RUN_TIMEOUT) {
        Ok(Ok(output)) => Ok((output.status.success(), format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err("claude did not answer in 30 s".into()),
    }
}

/// What `claude mcp get hub` says about the entry.
pub fn read_registration(found: bool, output: &str) -> Registration {
    if !found {
        Registration::Missing
    } else if output.to_ascii_lowercase().contains(&BRIDGE.to_ascii_lowercase()) {
        Registration::Ours
    } else {
        Registration::Other
    }
}

fn registration(cli: Option<&Path>) -> Registration {
    let Some(cli) = cli else { return Registration::Unknown };
    match run(cli, &["mcp".into(), "get".into(), SERVER.into()]) {
        Ok((found, output)) => read_registration(found, &output),
        Err(_) => Registration::Unknown,
    }
}

pub fn status(state: &AppState) -> ClaudeCodeStatus {
    let stored = state.hub.read(settings::load).unwrap_or_default();
    let data_dir = state.hub.paths().root.clone();
    let bridge = bridge_path();
    let cli = find_claude(std::env::var_os("PATH").as_deref(), home().as_deref());
    let skill = skill_dir();
    let shown_bridge = bridge.clone().unwrap_or_else(|| PathBuf::from(BRIDGE));
    ClaudeCodeStatus {
        enabled: stored.claude_enabled,
        bridge: bridge.as_ref().map(|b| b.display().to_string()),
        registration: registration(cli.as_deref()),
        cli: cli.map(|c| c.display().to_string()),
        skill: skill.as_deref().map(skill_state).unwrap_or(SkillState::Missing),
        skill_path: skill.map(|s| s.display().to_string()).unwrap_or_default(),
        commands: vec![add_line(&shown_bridge, &data_dir)],
        data_dir: data_dir.display().to_string(),
    }
}

#[tauri::command(async)]
pub fn claude_code_status(state: Hub<'_>) -> ClaudeCodeStatus {
    status(&state)
}

/// Write Hub's skill into `dir` (created when missing), replacing Hub's own older copy.
fn write_skill(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("SKILL.md"), SKILL)
}

/// Pasang: switch Claude Code on, write the skill, and register the bridge (replacing Hub's own
/// older entry). Refused, touching nothing, when `hub` or the skill folder belongs to something
/// else.
///
/// The skill is written first and needs no `claude` command (U1, 2026-11-09): registering by hand
/// (the sheet's command, run in a terminal) once left the owner with the server but without
/// `/hub-connect`. Without the `claude` command Pasang writes the skill, switches Claude Code on,
/// and leaves the registration to that command.
#[tauri::command(async)]
pub fn install_claude_code(app: AppHandle, state: Hub<'_>) -> Result<ClaudeCodeStatus> {
    let bridge = bridge_path().ok_or_else(|| failed("bridge", "unknown", "hub-mcp.exe is not beside Hub"))?;
    let skill = skill_dir().ok_or_else(|| failed("skill", "unknown", "no home folder"))?;
    let cli = find_claude(std::env::var_os("PATH").as_deref(), home().as_deref());
    let registered = registration(cli.as_deref());
    if registered == Registration::Other {
        return Err(failed("registration", "order", "an MCP server named hub that is not Hub's is registered"));
    }
    if skill_state(&skill) == SkillState::Other {
        return Err(failed("skill", "order", "a hub-connect skill Hub did not write is there"));
    }
    write(&app, &state, |conn, ctx| Ok(((), settings::update(conn, SettingsPatch { claude_enabled: Some(true), ..Default::default() }, Actor::Owner, ctx.now)?.1)))?;
    crate::api::apply(&app);
    write_skill(&skill).map_err(|e| failed("skill", "failed", e.to_string()))?;
    let Some(cli) = cli else { return Ok(status(&state)) };
    let data_dir = state.hub.paths().root.clone();
    if registered == Registration::Ours {
        run(&cli, &["mcp".into(), "remove".into(), "--scope".into(), "user".into(), SERVER.into()]).map_err(|e| failed("cli", "failed", e))?;
    }
    let (ok, output) = run(&cli, &add_args(&bridge, &data_dir)).map_err(|e| failed("cli", "failed", e))?;
    if !ok {
        crate::log::error(state.hub.paths(), &format!("claude mcp add: {}", output.trim()));
        return Err(failed("cli", "failed", output.trim().to_string()));
    }
    Ok(status(&state))
}

/// Lepas: Hub's entry and Hub's skill folder go; anything else stays.
#[tauri::command(async)]
pub fn remove_claude_code(state: Hub<'_>) -> Result<ClaudeCodeStatus> {
    if let Some(cli) = find_claude(std::env::var_os("PATH").as_deref(), home().as_deref())
        && registration(Some(&cli)) == Registration::Ours
    {
        let (ok, output) = run(&cli, &["mcp".into(), "remove".into(), "--scope".into(), "user".into(), SERVER.into()]).map_err(|e| failed("cli", "failed", e))?;
        if !ok {
            return Err(failed("cli", "failed", output.trim().to_string()));
        }
    }
    if let Some(skill) = skill_dir()
        && skill_state(&skill) == SkillState::Ours
    {
        std::fs::remove_dir_all(&skill).map_err(|e| failed("skill", "failed", e.to_string()))?;
    }
    Ok(status(&state))
}

/// Uji jembatan: start the bridge (without letting it start Hub), introduce ourselves over MCP,
/// and ask `hub_status` through it.
#[tauri::command(async)]
pub fn test_claude_bridge(state: Hub<'_>) -> Vec<BridgeStep> {
    let mut steps = Vec::new();
    let step = |steps: &mut Vec<BridgeStep>, name: &str, ok: bool, detail: String| steps.push(BridgeStep { step: name.into(), ok, detail });
    let Some(bridge) = bridge_path() else {
        step(&mut steps, "bridge", false, "hub-mcp.exe tidak ada di folder Hub.".into());
        return steps;
    };
    match ask_bridge(&bridge, &state.hub.paths().root) {
        Err(detail) => step(&mut steps, "bridge", false, detail),
        Ok(answer) => {
            step(&mut steps, "bridge", true, bridge.display().to_string());
            let result = &answer["result"];
            let content = &result["structuredContent"];
            if result["isError"] == json!(true) {
                let kind = content["error"]["kind"].as_str().unwrap_or("error").to_string();
                let hub_ok = kind != "hub_not_running" && kind != "hub_unreachable";
                step(&mut steps, "hub", hub_ok, if hub_ok { String::new() } else { kind.clone() });
                if hub_ok {
                    step(&mut steps, "token", false, kind);
                }
            } else {
                step(&mut steps, "hub", true, content["version"].as_str().unwrap_or("").to_string());
                step(&mut steps, "token", true, format!("{} proyek terlihat oleh Claude", content["projects"].as_u64().unwrap_or(0)));
            }
        }
    }
    steps
}

/// One MCP exchange over the bridge's stdio: initialize, initialized, `tools/call hub_status`. Not
/// a Claude Code session, so the bridge reports nothing (`--no-reports`).
fn ask_bridge(bridge: &Path, data_dir: &Path) -> std::result::Result<Value, String> {
    let mut command = Command::new(bridge);
    command.arg("--data-dir").arg(data_dir).arg("--no-start").arg("--no-reports").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|e| format!("hub-mcp.exe tidak bisa dijalankan: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut lines = BufReader::new(stdout).lines();
        let mut read = |id: i64| -> Option<Value> {
            for line in lines.by_ref() {
                let value: Value = serde_json::from_str(&line.ok()?).ok()?;
                if value["id"] == json!(id) {
                    return Some(value);
                }
            }
            None
        };
        let _ = send.send(read(1).and_then(|_| read(2)));
    });
    let messages = [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-11-25", "capabilities": {}, "clientInfo": { "name": "hub-uji-jembatan", "version": "1" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": { "name": "hub_status", "arguments": {} } }),
    ];
    for message in messages {
        writeln!(stdin, "{message}").map_err(|e| e.to_string())?;
    }
    stdin.flush().map_err(|e| e.to_string())?;
    let answer = receive.recv_timeout(BRIDGE_TIMEOUT);
    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    match answer {
        Ok(Some(value)) => Ok(value),
        Ok(None) => Err("hub-mcp.exe berhenti tanpa menjawab.".into()),
        Err(_) => Err("hub-mcp.exe tidak menjawab dalam 15 detik.".into()),
    }
}

/// A new token for Claude Code: its file is rewritten, the old token and its sessions end.
#[tauri::command(async)]
pub fn rotate_claude_token(app: AppHandle, state: Hub<'_>) -> Result<ClaudeCodeStatus> {
    let stored = state.hub.read(settings::load)?;
    let port = std::env::var("HUB_API_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(stored.api_port);
    hub_api::token::rotate(&state.hub, port, hub_api::token::CLAUDE)?;
    crate::commands::announce(&app, &hub_core::Change::all(hub_core::model::Entity::Settings));
    Ok(status(&state))
}

/// Claude Code's session and activity (R5), for one project (Bangun) or for all (Pengaturan), as
/// the bridge reported them. The third signal, the bridge itself, is [`claude_code_status`].
#[tauri::command(async)]
pub fn claude_signals(state: Hub<'_>, project_id: Option<String>) -> Result<ClaudeSignals> {
    Ok(state.hub.read(|conn| claude_sessions::signals(conn, project_id.as_deref(), jiff::Timestamp::now()))?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_skill_is_hubs_and_preapproves_reading_only() {
        assert!(SKILL.starts_with("---\nname: hub-connect\n"));
        assert!(SKILL.contains(MARK));
        assert!(SKILL.contains("disable-model-invocation: true"));
        let allowed = SKILL.lines().find(|l| l.starts_with("allowed-tools:")).unwrap();
        for read in ["hub_status", "hub_list_projects", "hub_connect", "hub_get_prd", "hub_get_checkpoint"] {
            assert!(allowed.contains(&format!("mcp__hub__{read}")), "{read}");
        }
        for write in ["hub_start_checkpoint", "hub_update_task", "hub_ask_owner", "hub_submit_report"] {
            assert!(!allowed.contains(write), "{write} must ask the user first");
        }
        assert!(SKILL.contains("never overrides the user's instructions"));
        assert!(!SKILL.contains('\u{2014}'), "no em dash");
    }

    #[test]
    fn the_registration_command_names_the_bridge_and_the_real_data_folder() {
        let bridge = Path::new(r"C:\Users\kamu\AppData\Local\Programs\Hub\hub-mcp.exe");
        let data = Path::new(r"C:\Users\kamu\AppData\Local\Hub");
        assert_eq!(add_args(bridge, data), ["mcp", "add", "--scope", "user", "hub", "--", bridge.to_str().unwrap(), "--data-dir", data.to_str().unwrap()]);
        assert_eq!(add_line(bridge, data), format!("claude mcp add --scope user hub -- \"{}\" --data-dir \"{}\"", bridge.display(), data.display()));
    }

    #[test]
    fn only_hubs_own_entry_counts_as_ours() {
        assert_eq!(read_registration(false, "No MCP server found"), Registration::Missing);
        assert_eq!(read_registration(true, "hub:\n  Command: C:\\Hub\\HUB-MCP.EXE\n  Args: --data-dir C:\\x"), Registration::Ours);
        assert_eq!(read_registration(true, "hub:\n  Command: npx some-other-server"), Registration::Other);
    }

    #[test]
    fn claude_is_found_on_path_or_in_local_bin() {
        let dir = tempfile::tempdir().unwrap();
        let (bin, home) = (dir.path().join("npm"), dir.path().join("home"));
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(home.join(".local").join("bin")).unwrap();
        assert_eq!(find_claude(Some(bin.as_os_str()), Some(&home)), None);
        std::fs::write(home.join(".local").join("bin").join("claude.exe"), b"").unwrap();
        assert_eq!(find_claude(Some(bin.as_os_str()), Some(&home)), Some(home.join(".local").join("bin").join("claude.exe")));
        // An npm shim on PATH does not win over the native program.
        std::fs::write(bin.join("claude.cmd"), b"").unwrap();
        assert_eq!(find_claude(Some(bin.as_os_str()), Some(&home)), Some(home.join(".local").join("bin").join("claude.exe")));
        std::fs::remove_file(home.join(".local").join("bin").join("claude.exe")).unwrap();
        assert_eq!(find_claude(Some(bin.as_os_str()), Some(&home)), Some(bin.join("claude.cmd")), "the shim when nothing else is there");
        std::fs::write(bin.join("claude.exe"), b"").unwrap();
        assert_eq!(find_claude(Some(bin.as_os_str()), Some(&home)), Some(bin.join("claude.exe")));
    }

    #[test]
    fn a_skill_folder_hub_did_not_write_is_not_hubs() {
        let dir = tempfile::tempdir().unwrap();
        let skill = dir.path().join("hub-connect");
        assert_eq!(skill_state(&skill), SkillState::Missing);
        std::fs::create_dir_all(&skill).unwrap();
        assert_eq!(skill_state(&skill), SkillState::Other);
        std::fs::write(skill.join("SKILL.md"), "---\nname: hub-connect\n---\nsomeone else's").unwrap();
        assert_eq!(skill_state(&skill), SkillState::Other);
        std::fs::write(skill.join("SKILL.md"), SKILL).unwrap();
        assert_eq!(skill_state(&skill), SkillState::Ours);
    }

    #[test]
    fn the_skill_is_written_without_the_claude_command_and_replaces_hubs_older_copy() {
        let dir = tempfile::tempdir().unwrap();
        // ~/.claude/skills may not exist yet: the whole path is made.
        let skill = dir.path().join("skills").join("hub-connect");
        write_skill(&skill).unwrap();
        assert_eq!(skill_state(&skill), SkillState::Ours);
        assert_eq!(std::fs::read_to_string(skill.join("SKILL.md")).unwrap(), SKILL);
        // An older SKILL.md Hub wrote is replaced by this build's.
        std::fs::write(skill.join("SKILL.md"), format!("---\nname: hub-connect\n---\n{MARK} (old) -->")).unwrap();
        write_skill(&skill).unwrap();
        assert_eq!(std::fs::read_to_string(skill.join("SKILL.md")).unwrap(), SKILL);
    }
}
