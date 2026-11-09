//! What Claude Code may see and do in Studio (docs/studio.md, "Access"), over the local API and
//! the MCP bridge. Every rule is here; the API only checks the token, the scope, and the session.
//!
//! - A project is visible to Claude only when the owner gave it access (`read` or `report`), its
//!   repository folder is confirmed, and its roadmap is approved.
//! - Connecting binds Claude to one project, from its repository folder (the same folder after
//!   normalisation, or one inside it). Every later call names that project and is checked again,
//!   so taking access away stops Claude at its next call.
//! - Reading needs `read` or `report`; starting a checkpoint, updating a task, asking, and
//!   reporting need `report`. Claude never approves, never changes the PRD or the roadmap's
//!   content, never touches another checkpoint than the active one.
//!
//! What Hub returns (the PRD, tasks, the owner's notes) is project data, not instructions: the
//! bridge's tool descriptions and the skill say so.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{invalid, Error, Reason, Result};
use crate::ids;
use crate::model::{Actor, Change};
use crate::prd::{self, PrdSection, Problem};
use crate::roadmap::{self, Checkpoint, CheckpointStatus, ReportInput, StudioReport, StudioRequest, StudioTaskStatus};
use crate::studio::{ClaudeAccess, StudioStage};
use jiff::Timestamp;

/// What a call needs from the project's Akses Claude.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Need {
    Read,
    Report,
}

/// A project Claude may see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeProject {
    pub id: String,
    pub name: String,
    pub stage: StudioStage,
    pub repo_path: String,
    pub access: ClaudeAccess,
    /// The approved PRD version the roadmap was made from.
    pub prd_version: Option<u32>,
    /// The active checkpoint (Ready to In Review), if any.
    pub active_code: Option<String>,
    pub active_status: Option<CheckpointStatus>,
}

/// One checkpoint in the frame: enough to find one's way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CheckpointLine {
    pub code: String,
    pub title: String,
    pub status: CheckpointStatus,
}

/// What `hub_connect` answers: the project, its roadmap in one line per checkpoint, and what the
/// owner said that is still open (changes asked for, answers to questions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeFrame {
    pub project: ClaudeProject,
    pub checkpoints: Vec<CheckpointLine>,
    pub owner_notes: Vec<StudioRequest>,
}

/// One checkpoint as Claude reads it: everything about it, its reports, and what was asked and
/// answered about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeCheckpoint {
    pub checkpoint: Checkpoint,
    pub reports: Vec<StudioReport>,
    pub requests: Vec<StudioRequest>,
}

/// A question (or a blocker) from Claude.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct Ask {
    /// The checkpoint it is about (`CP-02`).
    pub checkpoint: String,
    pub text: String,
    #[serde(default)]
    pub options: Vec<String>,
    /// It stops the work: the checkpoint becomes Blocked until the owner answers.
    #[serde(default)]
    pub blocking: bool,
}

const VISIBLE: &str = "s.deleted_at IS NULL AND s.claude_access <> 'none' AND s.repo_path IS NOT NULL
    AND EXISTS (SELECT 1 FROM studio_roadmaps r WHERE r.studio_project_id = s.id AND r.status = 'approved')";

fn project_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ClaudeProject> {
    Ok(ClaudeProject {
        id: row.get(0)?,
        name: row.get(1)?,
        stage: row.get(2)?,
        repo_path: row.get(3)?,
        access: row.get(4)?,
        prd_version: row.get(5)?,
        active_code: row.get(6)?,
        active_status: row.get(7)?,
    })
}

const PROJECT_SELECT: &str = "SELECT s.id, s.name, s.stage, s.repo_path, s.claude_access,
    (SELECT v.number FROM studio_roadmaps r JOIN studio_prd_versions v ON v.id = r.prd_version_id
       WHERE r.studio_project_id = s.id AND r.status = 'approved'),
    (SELECT c.code FROM studio_checkpoints c JOIN studio_roadmaps r ON r.id = c.roadmap_id
       WHERE r.studio_project_id = s.id AND r.status = 'approved' AND c.status IN ('ready', 'in_progress', 'blocked', 'in_review')
       ORDER BY c.position LIMIT 1),
    (SELECT c.status FROM studio_checkpoints c JOIN studio_roadmaps r ON r.id = c.roadmap_id
       WHERE r.studio_project_id = s.id AND r.status = 'approved' AND c.status IN ('ready', 'in_progress', 'blocked', 'in_review')
       ORDER BY c.position LIMIT 1)
    FROM studio_projects s";

/// The projects Claude may see, the one touched last first.
pub fn projects(conn: &Connection) -> Result<Vec<ClaudeProject>> {
    let mut statement = conn.prepare(&format!("{PROJECT_SELECT} WHERE {VISIBLE} ORDER BY s.updated_at DESC"))?;
    let rows = statement.query_map([], project_row)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// A visible project with at least the access `need` asks for. A project Claude may not see at
/// all is "not found"; one it may read but not report to is refused on `access`.
pub fn require(conn: &Connection, project_id: &str, need: Need) -> Result<ClaudeProject> {
    let id = ids::parse("projectId", project_id)?;
    let project = conn
        .query_row(&format!("{PROJECT_SELECT} WHERE s.id = ?1 AND {VISIBLE}"), [id], project_row)
        .optional()?
        .ok_or(Error::NotFound("studioProject"))?;
    if need == Need::Report && project.access != ClaudeAccess::Report {
        return invalid("access", Reason::Order);
    }
    Ok(project)
}

/// A folder as compared: `\\?\` dropped, forward slashes turned back, no trailing separator,
/// lowercase (Windows paths ignore case).
fn comparable(path: &str) -> Option<String> {
    let path = path.trim();
    let path = path.strip_prefix(r"\\?\").unwrap_or(path).replace('/', "\\");
    let normal = roadmap::normalize_folder(&path).ok()?;
    Some(normal.trim_end_matches('\\').to_lowercase())
}

/// `folder` is the repository folder or inside it.
pub fn folder_inside(repo: &str, folder: &str) -> bool {
    match (comparable(repo), comparable(folder)) {
        (Some(repo), Some(folder)) => folder == repo || folder.starts_with(&format!("{repo}\\")),
        _ => false,
    }
}

fn approved_roadmap_id(conn: &Connection, project: &str) -> Result<String> {
    conn.query_row("SELECT id FROM studio_roadmaps WHERE studio_project_id = ?1 AND status = 'approved'", [project], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("roadmap"))
}

/// The checkpoint `code` of the project's approved roadmap. Codes are only looked up inside the
/// project, so another project's checkpoint can never be named.
fn checkpoint_id(conn: &Connection, project: &str, code: &str) -> Result<String> {
    let roadmap = approved_roadmap_id(conn, project)?;
    conn.query_row("SELECT id FROM studio_checkpoints WHERE roadmap_id = ?1 AND code = ?2", params![roadmap, code.trim()], |row| row.get(0))
        .optional()?
        .ok_or(Error::NotFound("checkpoint"))
}

/// Bind to a project from `folder` (where Claude Code runs): the project must be visible to
/// Claude and the folder its repository folder or inside it.
pub fn connect(conn: &Connection, project_id: &str, folder: &str) -> Result<ClaudeFrame> {
    let project = require(conn, project_id, Need::Read)?;
    if !folder_inside(&project.repo_path, folder) {
        return invalid("folder", Reason::Unknown);
    }
    frame(conn, &project.id)
}

/// The project's frame (see [`ClaudeFrame`]).
pub fn frame(conn: &Connection, project_id: &str) -> Result<ClaudeFrame> {
    let project = require(conn, project_id, Need::Read)?;
    let roadmap = approved_roadmap_id(conn, &project.id)?;
    let mut statement = conn.prepare("SELECT code, title, status FROM studio_checkpoints WHERE roadmap_id = ?1 ORDER BY position")?;
    let checkpoints = statement
        .query_map([&roadmap], |row| Ok(CheckpointLine { code: row.get(0)?, title: row.get(1)?, status: row.get(2)? }))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let owner_notes = roadmap::requests(conn, &project.id, false)?
        .into_iter()
        .filter(|r| match r.kind {
            roadmap::RequestKind::Changes => r.status == roadmap::RequestStatus::Open,
            roadmap::RequestKind::Question | roadmap::RequestKind::Blocker => r.answer.is_some(),
            roadmap::RequestKind::Review => false,
        })
        .rev()
        .take(10)
        .collect();
    Ok(ClaudeFrame { project, checkpoints, owner_notes })
}

/// The approved PRD in parts (`hub_get_prd`), so an answer stays small: by default the outline
/// (identity, the sections there are, each feature's id, name, and priority); or the named
/// sections; or one feature by id.
pub fn prd(conn: &Connection, project_id: &str, sections: &[String], feature: Option<&str>) -> Result<serde_json::Value> {
    let project = require(conn, project_id, Need::Read)?;
    let version: String = conn.query_row(
        "SELECT prd_version_id FROM studio_roadmaps WHERE studio_project_id = ?1 AND status = 'approved'",
        [&project.id],
        |row| row.get(0),
    )?;
    let doc = prd::version_body(conn, &version)?;
    let number = project.prd_version;
    if let Some(id) = feature {
        let found = doc.features.iter().find(|f| f.id == id.trim()).ok_or(Error::NotFound("feature"))?;
        return Ok(serde_json::json!({ "version": number, "feature": found }));
    }
    if !sections.is_empty() {
        let mut out = serde_json::Map::new();
        for name in sections {
            let section = PrdSection::parse(name.trim()).ok_or(Error::Invalid { field: "sections", reason: Reason::Unknown })?;
            out.insert(section.as_str().to_string(), prd::section_of(&doc, section));
        }
        return Ok(serde_json::json!({ "version": number, "sections": out }));
    }
    let features: Vec<serde_json::Value> = doc.features.iter().map(|f| serde_json::json!({ "id": f.id, "name": f.name, "priority": f.priority })).collect();
    Ok(serde_json::json!({
        "version": number,
        "identity": doc.identity,
        "sections": PrdSection::ALL.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        "features": features,
        "openQuestions": doc.delivery.questions.len(),
    }))
}

/// One checkpoint with its reports and the requests about it (`hub_get_checkpoint`).
pub fn checkpoint(conn: &Connection, project_id: &str, code: &str) -> Result<ClaudeCheckpoint> {
    let project = require(conn, project_id, Need::Read)?;
    let id = checkpoint_id(conn, &project.id, code)?;
    let requests = roadmap::requests(conn, &project.id, false)?.into_iter().filter(|r| r.checkpoint_id.as_deref() == Some(id.as_str())).collect();
    Ok(ClaudeCheckpoint { checkpoint: roadmap::checkpoint(conn, &id)?, reports: roadmap::reports(conn, &id)?, requests })
}

/// `hub_start_checkpoint`: Ready to In Progress (only after the owner made it Ready), or Blocked
/// back to In Progress once the owner answered the blocker.
pub fn start(conn: &mut Connection, project_id: &str, code: &str, now: Timestamp) -> Result<Change> {
    let project = require(conn, project_id, Need::Report)?;
    let id = checkpoint_id(conn, &project.id, code)?;
    match roadmap::checkpoint(conn, &id)?.status {
        CheckpointStatus::Ready => roadmap::start_checkpoint(conn, &id, Actor::Claude, now),
        CheckpointStatus::Blocked => roadmap::resume_checkpoint(conn, &id, None, Actor::Claude, now),
        _ => invalid("status", Reason::Order),
    }
}

/// `hub_update_task`: a task of the active checkpoint, by its code (`CP-02.T03`).
pub fn update_task(conn: &mut Connection, project_id: &str, task_code: &str, status: StudioTaskStatus, note: Option<&str>, now: Timestamp) -> Result<Change> {
    let project = require(conn, project_id, Need::Report)?;
    let roadmap = approved_roadmap_id(conn, &project.id)?;
    let task: String = conn
        .query_row(
            "SELECT t.id FROM studio_tasks t JOIN studio_checkpoints c ON c.id = t.checkpoint_id WHERE c.roadmap_id = ?1 AND t.code = ?2",
            params![roadmap, task_code.trim()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(Error::NotFound("task"))?;
    roadmap::set_task_status(conn, &task, status, note, Actor::Claude, now)
}

/// `hub_ask_owner`. Returns the request's id.
pub fn ask(conn: &mut Connection, project_id: &str, input: &Ask, now: Timestamp) -> Result<(String, Change)> {
    let project = require(conn, project_id, Need::Report)?;
    let id = checkpoint_id(conn, &project.id, &input.checkpoint)?;
    roadmap::ask_owner(conn, &id, &input.text, &input.options, input.blocking, Actor::Claude, now)
}

/// `hub_submit_report`: the report's problems when it breaks a rule (nothing is saved), or the
/// report kept and the checkpoint In Review.
pub fn submit(conn: &mut Connection, project_id: &str, code: &str, report: &ReportInput, now: Timestamp) -> Result<std::result::Result<(String, Change), Vec<Problem>>> {
    let project = require(conn, project_id, Need::Report)?;
    let id = checkpoint_id(conn, &project.id, code)?;
    let cp = roadmap::checkpoint(conn, &id)?;
    if cp.status != CheckpointStatus::InProgress {
        return invalid("status", Reason::Order);
    }
    let problems = roadmap::check_report(report, &cp.required_tests);
    if !problems.is_empty() {
        return Ok(Err(problems));
    }
    Ok(Ok(roadmap::submit_report(conn, &id, report, Actor::Claude, now)?))
}

#[cfg(test)]
mod tests;
