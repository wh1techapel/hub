//! Claude Code's sessions as the bridge reports them (docs/studio.md, "Claude's status", R5).
//!
//! Hub shows three signals about Claude Code, each from real data: the bridge (installed,
//! registered, the skill: the app checks those), the **session**, and the **activity**. This module
//! keeps the last two:
//!
//! - The bridge says hello ([`open`]) once Claude Code has it open, reports every
//!   [`BEAT_SECONDS`] ([`beat`]), and says goodbye ([`close`]) when Claude Code closes its input.
//!   A session is open while Hub heard from it in the last [`OPEN_SECONDS`]; one that ended
//!   without a goodbye is "quiet" (the bridge was stopped, or could not reach Hub).
//! - Each tool call is reported with its outcome ([`ToolCall`]); a session keeps its last one.
//! - Hub decides which Studio project a session belongs to, from the folder Claude Code runs in
//!   (the same rule as connecting: the repository folder or inside it).
//!
//! Nothing here is the owner's data: sessions are not in Jejak or the export, and Hub keeps 30
//! days of them, at most 100. Claude's changes to the data are recorded where they happen.

use jiff::{SignedDuration, Timestamp};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::claude::folder_inside;
use crate::error::{invalid, Error, Reason, Result};
use crate::ids;
use crate::model::{text_enum, Change, Entity};
use crate::roadmap::normalize_folder;
use crate::time::instant;

/// How often the bridge reports while Claude Code has it open. Hub tells the bridge in [`Opened`].
pub const BEAT_SECONDS: u32 = 60;
/// A session is open while Hub heard from it within this long: two beats and a half.
pub const OPEN_SECONDS: i64 = 150;
const KEEP_DAYS: i64 = 30;
const KEEP_MAX: i64 = 100;

text_enum!(
    /// The bridge's tools (crates/hub-mcp).
    ClaudeTool {
        Status => "hub_status",
        ListProjects => "hub_list_projects",
        Connect => "hub_connect",
        GetPrd => "hub_get_prd",
        GetCheckpoint => "hub_get_checkpoint",
        StartCheckpoint => "hub_start_checkpoint",
        UpdateTask => "hub_update_task",
        AskOwner => "hub_ask_owner",
        SubmitReport => "hub_submit_report",
    }
);

text_enum!(
    /// Open (heard from lately), closed (said goodbye), or quiet (gone without a word).
    SessionState {
        Open => "open",
        Closed => "closed",
        Quiet => "quiet",
    }
);

/// The bridge's hello.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Hello {
    /// The folder Claude Code runs in.
    pub folder: String,
    /// The MCP client as it named itself (`claude-code`), and its version.
    #[serde(default)]
    pub client: String,
    #[serde(default)]
    pub client_version: String,
    #[serde(default)]
    pub bridge_version: String,
}

/// What Hub answers a hello with: the session, and how often to report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    pub id: String,
    pub beat_seconds: u32,
}

/// One tool call, as the bridge reports it after the call.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCall {
    pub tool: ClaudeTool,
    pub ok: bool,
    /// Why it failed: the refusal's field or kind (`folder`, `access`, `hub_not_running`).
    #[serde(default)]
    pub reason: Option<String>,
    /// The project it was about: the connected one, or the one `hub_connect` asked for. An id Hub
    /// does not know is ignored; the session's own project is used then.
    #[serde(default)]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SessionSignal {
    pub state: SessionState,
    pub folder: String,
    pub client: String,
    pub client_version: String,
    pub started_at: String,
    pub seen_at: String,
    pub ended_at: Option<String>,
    /// Tool calls in this session.
    pub calls: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ActivitySignal {
    pub tool: ClaudeTool,
    pub at: String,
    pub ok: bool,
    pub reason: Option<String>,
}

/// Claude Code's session and activity, for one project or for all of them. `None` means Hub has no
/// evidence yet ("Status belum diketahui").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeSignals {
    /// The open session heard from last, otherwise the one that ended last.
    pub session: Option<SessionSignal>,
    /// Sessions open now.
    pub open: u32,
    /// The last tool call.
    pub activity: Option<ActivitySignal>,
}

fn short(field: &'static str, text: &str, max: usize) -> Result<String> {
    let text = text.trim();
    if text.chars().count() > max {
        return invalid(field, Reason::Length);
    }
    if text.chars().any(char::is_control) {
        return invalid(field, Reason::Format);
    }
    Ok(text.to_string())
}

/// The Studio project whose repository folder holds `folder` (the innermost one when they nest).
fn project_of(conn: &Connection, folder: &str) -> Result<Option<String>> {
    let mut statement = conn.prepare("SELECT id, repo_path FROM studio_projects WHERE deleted_at IS NULL AND repo_path IS NOT NULL")?;
    let rows = statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
    let mut best: Option<(String, usize)> = None;
    for row in rows {
        let (id, repo) = row?;
        if folder_inside(&repo, folder) && best.as_ref().is_none_or(|(_, length)| repo.len() > *length) {
            best = Some((id, repo.len()));
        }
    }
    Ok(best.map(|(id, _)| id))
}

/// A call's time to the millisecond: two sessions can call within the same second, and the last
/// call must still be the last. Same text order as Hub's other instants.
fn call_instant(ts: Timestamp) -> String {
    ts.strftime("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn cutoff(now: Timestamp) -> String {
    instant(now - SignedDuration::from_secs(OPEN_SECONDS))
}

/// A new session from the bridge's hello. Old sessions are let go here.
pub fn open(conn: &mut Connection, hello: &Hello, now: Timestamp) -> Result<(Opened, Change)> {
    let folder = normalize_folder(&hello.folder).or_else(|_| invalid("folder", Reason::Format))?;
    let client = short("client", &hello.client, 60)?;
    let client_version = short("clientVersion", &hello.client_version, 40)?;
    let bridge_version = short("bridgeVersion", &hello.bridge_version, 40)?;
    let id = ids::new_id();
    let stamp = instant(now);
    let tx = conn.transaction()?;
    let project = project_of(&tx, &folder)?;
    tx.execute(
        "INSERT INTO studio_claude_sessions (id, folder, studio_project_id, client, client_version, bridge_version, started_at, seen_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![id, folder, project, client, client_version, bridge_version, stamp],
    )?;
    let old = instant(now - SignedDuration::from_hours(24 * KEEP_DAYS));
    tx.execute("DELETE FROM studio_claude_sessions WHERE coalesce(ended_at, seen_at) < ?1", [old])?;
    tx.execute(
        "DELETE FROM studio_claude_sessions WHERE id NOT IN (SELECT id FROM studio_claude_sessions ORDER BY seen_at DESC, id DESC LIMIT ?1)",
        [KEEP_MAX],
    )?;
    tx.commit()?;
    Ok((Opened { id: id.clone(), beat_seconds: BEAT_SECONDS }, Change::one(Entity::ClaudeSession, id)))
}

/// The bridge is still there, and maybe just made a call. A session Hub does not know, or one that
/// said goodbye, is not found: the bridge says hello again. Returns a change when what the signals
/// show changed (a call, or a quiet session heard from again).
pub fn beat(conn: &mut Connection, id: &str, call: Option<&ToolCall>, now: Timestamp) -> Result<Option<Change>> {
    let id = ids::parse("session", id)?;
    let tx = conn.transaction()?;
    let found: Option<(String, Option<String>)> = tx
        .query_row("SELECT seen_at, studio_project_id FROM studio_claude_sessions WHERE id = ?1 AND ended_at IS NULL", [&id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .optional()?;
    let Some((seen_at, own_project)) = found else { return Err(Error::NotFound("claudeSession")) };
    let stamp = instant(now);
    let was_quiet = seen_at < cutoff(now);
    tx.execute("UPDATE studio_claude_sessions SET seen_at = max(seen_at, ?2) WHERE id = ?1", params![id, stamp])?;
    if let Some(call) = call {
        let reason = match (call.ok, call.reason.as_deref().map(str::trim).filter(|r| !r.is_empty())) {
            (true, _) | (false, None) => None,
            (false, Some(reason)) => {
                if reason.len() > 40 || !reason.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                    return invalid("reason", Reason::Format);
                }
                Some(reason.to_string())
            }
        };
        let named = match call.project_id.as_deref().and_then(|p| ids::parse("projectId", p).ok()) {
            Some(project) => tx.query_row("SELECT id FROM studio_projects WHERE id = ?1 AND deleted_at IS NULL", [&project], |row| row.get(0)).optional()?,
            None => None,
        };
        let project: Option<String> = named.or(own_project);
        tx.execute(
            "UPDATE studio_claude_sessions SET tool = ?2, tool_at = ?3, tool_ok = ?4, tool_reason = ?5, tool_project_id = ?6, tool_calls = tool_calls + 1
             WHERE id = ?1",
            params![id, call.tool, call_instant(now), call.ok, reason, project],
        )?;
    }
    tx.commit()?;
    Ok((call.is_some() || was_quiet).then(|| Change::one(Entity::ClaudeSession, id)))
}

/// The bridge's goodbye. Saying it twice, or for a session Hub does not know, changes nothing.
pub fn close(conn: &mut Connection, id: &str, now: Timestamp) -> Result<Option<Change>> {
    let id = ids::parse("session", id)?;
    let stamp = instant(now);
    let ended = conn.execute("UPDATE studio_claude_sessions SET ended_at = ?2, seen_at = max(seen_at, ?2) WHERE id = ?1 AND ended_at IS NULL", params![id, stamp])?;
    Ok((ended > 0).then(|| Change::one(Entity::ClaudeSession, id)))
}

/// The session and activity signals, for the project (by its sessions and the calls about it) or,
/// with no project, for every session.
pub fn signals(conn: &Connection, project: Option<&str>, now: Timestamp) -> Result<ClaudeSignals> {
    let project = project.map(|p| ids::parse("projectId", p)).transpose()?;
    let since = cutoff(now);
    let mine = "(?1 IS NULL OR studio_project_id = ?1)";
    let open: u32 = conn.query_row(
        &format!("SELECT count(*) FROM studio_claude_sessions WHERE {mine} AND ended_at IS NULL AND seen_at >= ?2"),
        params![project, since],
        |row| row.get(0),
    )?;
    let session = conn
        .query_row(
            &format!(
                "SELECT folder, client, client_version, started_at, seen_at, ended_at, tool_calls FROM studio_claude_sessions WHERE {mine}
                 ORDER BY (ended_at IS NULL AND seen_at >= ?2) DESC, coalesce(ended_at, seen_at) DESC, id DESC LIMIT 1"
            ),
            params![project, since],
            |row| {
                let seen_at: String = row.get(4)?;
                let ended_at: Option<String> = row.get(5)?;
                let state = match &ended_at {
                    Some(_) => SessionState::Closed,
                    None if seen_at >= since => SessionState::Open,
                    None => SessionState::Quiet,
                };
                Ok(SessionSignal {
                    state,
                    folder: row.get(0)?,
                    client: row.get(1)?,
                    client_version: row.get(2)?,
                    started_at: row.get(3)?,
                    seen_at,
                    ended_at,
                    calls: row.get(6)?,
                })
            },
        )
        .optional()?;
    let calls = if project.is_some() { "tool_project_id = ?1" } else { "?1 IS NULL" };
    let activity = conn
        .query_row(
            &format!("SELECT tool, tool_at, tool_ok, tool_reason FROM studio_claude_sessions WHERE {calls} AND tool IS NOT NULL ORDER BY tool_at DESC, id DESC LIMIT 1"),
            params![project],
            |row| Ok(ActivitySignal { tool: row.get(0)?, at: row.get(1)?, ok: row.get(2)?, reason: row.get(3)? }),
        )
        .optional()?;
    Ok(ClaudeSignals { session, open, activity })
}

#[cfg(test)]
mod tests;
